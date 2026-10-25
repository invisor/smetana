//! The statistics file for Claude Code's auto-mode refusals.
//!
//! The detector in `agents/claude_driver.rs` matches one measured phrase, and a
//! phrase measured on a few transcripts will miss some refusals. The way to
//! tune it is to keep the raw material: every failed tool result that mentions
//! auto mode or the classifier is appended here, whole, with whether the exact
//! phrase matched. Never read by the app itself; it is for a person to open.
//!
//! A write failing is logged and dropped. A statistics file must never be the
//! reason a session breaks.

use std::io::Write;
use std::path::Path;

use crate::session::model::{Event, EventKind};

pub const FILE_NAME: &str = "auto-mode-blocks.jsonl";

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlockRecord {
    pub at: String,
    pub session: String,
    pub tool: String,
    pub detail: String,
    pub matched: bool,
    pub text: String,
}

/// Append one record as one JSON line, creating the file if need be.
pub fn append_block(dir: &Path, record: &BlockRecord) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let mut line = serde_json::to_vec(record).map_err(std::io::Error::other)?;
    line.push(b'\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join(FILE_NAME))?
        .write_all(&line)
}

/// The `(tool, detail)` of the `ToolUse` carrying `id`, or two empty strings
/// when that event is not in `events` (already trimmed off the journal's front).
pub fn tool_of<'a>(events: impl IntoIterator<Item = &'a EventKind>, id: &str) -> (String, String) {
    events
        .into_iter()
        .find_map(|kind| match kind {
            EventKind::ToolUse { id: use_id, name, detail } if use_id == id => {
                Some((name.clone(), detail.clone()))
            }
            _ => None,
        })
        .unwrap_or_default()
}

/// Record every sighting from one read of the stream. `earlier` is the
/// session's journal so far and `batch` the events of this very read, since a
/// call and its refusal can arrive together.
pub fn record_sightings(
    dir: &Path,
    session: &str,
    at: &str,
    earlier: &[Event],
    batch: &[EventKind],
    sightings: Vec<(String, String, bool)>,
) {
    for (id, text, matched) in sightings {
        let (tool, detail) =
            tool_of(earlier.iter().map(|event| &event.kind).chain(batch.iter()), &id);
        let record = BlockRecord {
            at: at.to_string(),
            session: session.to_string(),
            tool,
            detail,
            matched,
            text,
        };
        if let Err(error) = append_block(dir, &record) {
            log::warn!("the auto-mode statistics file could not be written: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(detail: &str) -> BlockRecord {
        BlockRecord {
            at: "2026-10-23T14:12:03Z".into(),
            session: "s1".into(),
            tool: "Bash".into(),
            detail: detail.into(),
            matched: true,
            text: "denied".into(),
        }
    }

    #[test]
    fn a_record_is_appended_as_one_json_line() {
        let dir = tempfile::tempdir().unwrap();
        append_block(dir.path(), &record("git push --force")).unwrap();
        append_block(dir.path(), &record("git push --force")).unwrap();
        let body = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert_eq!(body.lines().count(), 2);
        let back: BlockRecord = serde_json::from_str(body.lines().next().unwrap()).unwrap();
        assert_eq!(back, record("git push --force"));
        let raw: serde_json::Value = serde_json::from_str(body.lines().next().unwrap()).unwrap();
        for key in ["at", "session", "tool", "detail", "matched", "text"] {
            assert!(raw.get(key).is_some(), "missing {key}");
        }
    }

    #[test]
    fn a_tool_use_already_trimmed_away_yields_empty_names() {
        assert_eq!(tool_of([], "gone"), (String::new(), String::new()));
    }

    #[test]
    fn a_trimmed_tool_use_still_gets_its_record_with_empty_names() {
        let dir = tempfile::tempdir().unwrap();
        record_sightings(dir.path(), "s1", "now", &[], &[], vec![("gone".into(), "raw".into(), false)]);
        let body = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        let back: BlockRecord = serde_json::from_str(body.trim()).unwrap();
        assert_eq!((back.tool.as_str(), back.detail.as_str(), back.matched), ("", "", false));
        assert_eq!(back.text, "raw");
    }

    #[test]
    fn the_tool_use_is_found_in_the_same_batch() {
        let dir = tempfile::tempdir().unwrap();
        let batch = [EventKind::ToolUse { id: "t1".into(), name: "Bash".into(), detail: "ls".into() }];
        record_sightings(dir.path(), "s1", "now", &[], &batch, vec![("t1".into(), "raw".into(), true)]);
        let body = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        let back: BlockRecord = serde_json::from_str(body.trim()).unwrap();
        assert_eq!((back.tool.as_str(), back.detail.as_str()), ("Bash", "ls"));
    }
}
