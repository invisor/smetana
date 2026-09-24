//! A Claude Code transcript, as the journal of a driven session.
//!
//! An interactive `claude --resume` paints the conversation so far before it
//! takes a word from anybody, and the panel has to do the same: under
//! `--input-format stream-json` the harness replays nothing at all, so a
//! resumed session that started from an empty journal would be a person looking
//! at a blank panel over a conversation they are in the middle of.
//!
//! **The transcript is read through the driver rather than beside it.** An
//! `assistant` record in a `.jsonl` file and an `assistant` line of the
//! stream-json output have the same shape, so
//! [`crate::agents::claude_driver::one_event`] answers both, and the history a
//! person scrolls back through is made of the very same events the live half
//! produces. A second reading of that format would be a second vocabulary to
//! keep in step with `EventKind`, and the drift would be silent: rows that
//! simply stop appearing.
//!
//! What this adds to it is the half the live stream does not carry. A person's
//! own turn reaches the worker as a `session_send` and is journalled there, so
//! the driver never has to recognise one; in a transcript it is an ordinary
//! `user` record, and telling it from a hook's output, a skill's body or a
//! subagent's prompt is [`crate::sessions::model::human_text`]'s rule — the one
//! the Sessions tab already titles its rows by.
//!
//! **Anything else is skipped in silence**, which is `EventKind`'s own standing
//! rule one layer up: `summary`, `file-history-snapshot`, `ai-title`, a record
//! type Claude Code added last week, a line that is not JSON at all. A missing
//! row costs a person nothing the transcript itself does not still hold, while
//! a wall of raw protocol costs them the panel.
//!
//! **There is no marker between the history and the live conversation and none
//! is wanted.** The first live `TurnStart` is the boundary: the harness emits
//! `{"type":"system","subtype":"init"}` as it comes up, the driver turns that
//! into one, and everything above it is the past.

use std::collections::VecDeque;
use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::driver;
use super::journal::BUDGET;
use super::model::EventKind;
use crate::agents::claude_driver::one_event;
use crate::sessions::model::{human_text, Record};

/// One event of history, and the moment the transcript says it happened.
///
/// A pair rather than an `Event`, because `seq` is the journal's to mint: these
/// are appended through the same `Journal::append` the live half uses, so the
/// numbering is one counter and a window catching up cannot meet two events
/// wearing one number.
pub type Past = (EventKind, String);

/// What one record of a transcript becomes.
///
/// `at` is the fallback for a record with no `timestamp` of its own — an older
/// transcript, or a record type that never carried one. It is the caller's
/// clock rather than an empty string, so that every event in a journal has a
/// time on it.
pub fn events_of(line: &str, at: &str) -> Vec<Past> {
    let Ok(value) = serde_json::from_str::<Value>(line) else { return Vec::new() };
    events_of_value(&value, at)
}

fn events_of_value(value: &Value, at: &str) -> Vec<Past> {
    // Every field is optional, so this fails only for a line that is not an
    // object at all — an array, a bare number — and such a line has no record
    // in it for either half below to read.
    let record = Record::deserialize(value).unwrap_or_default();
    // A subagent's turn, dropped whole and before anything else looks at it.
    // `human_text` refuses one already; the assistant half would not, and a
    // panel replaying every subagent's text inline would be a conversation
    // nobody had.
    if record.is_sidechain == Some(true) {
        return Vec::new();
    }
    let at = record.timestamp.clone().unwrap_or_else(|| at.to_owned());
    let mut kinds: Vec<EventKind> = Vec::new();
    // The person's own words, which the live stream never carries: the worker
    // journals a turn when it sends one, so the driver has never had to know
    // what one looks like coming back.
    //
    // `attachments` is empty rather than reconstructed. A picture reaches this
    // harness as a path inside the prose (`ImageDelivery::InPrompt`), so the
    // paths are already in `text`, and a transcript's `attachment` records are
    // the harness's own bookkeeping rather than a list of what somebody sent.
    if let Some(text) = human_text(&record) {
        kinds.push(EventKind::UserMessage { text, attachments: Vec::new() });
    }
    kinds.extend(one_event(value).into_iter().filter(is_past));
    kinds.into_iter().map(|kind| (kind, at.clone())).collect()
}

/// Which of the driver's answers may stand in a history, and the exclusion is
/// load-bearing rather than tidy.
///
/// A `TurnStart` with no `Result` after it is what `state_of` reads as a turn
/// in flight: a history ending on one would pin a freshly resumed session at
/// `running`, with the composer showing Stop and no way back — the very shape
/// `journal.js` records `starting` having had. A transcript carries no
/// `system`/`init` record today, so this filter is usually about nothing; the
/// day one appears it is the difference between a panel that works and a panel
/// that is wedged, with nothing failing anywhere to say so.
///
/// **`TextDelta` and `TextQuestion` are excluded for the same reason and belong to a fact that
/// has already been checked rather than merely hoped for.** Claude Code's own
/// `.jsonl` transcript — what `read_file` below streams — never contains a
/// `stream_event` record; `one_event` only ever produces one from a *live*
/// `content_block_delta` line, and this file's caller is the one place that
/// would otherwise hand a stray one to `journal.js` on a re-entry, leaving a
/// caret pinned to a reply nothing is still writing. Excluding it here costs
/// nothing on the ordinary path and does not depend on that fact staying
/// true if some later transcript format ever changes. The latter is a live
/// completion marker, not a historical request somebody can answer again.
fn is_past(kind: &EventKind) -> bool {
    !matches!(
        kind,
        EventKind::TurnStart { .. } | EventKind::TextDelta { .. } | EventKind::TextQuestion
    )
}

/// The conversation a session has already had, oldest first.
///
/// `cwd` and `id` are the pair Claude Code itself names a session by, and the
/// pair `sessions::read::transcript` finds the file from.
///
/// **Every failure is an empty history and a line in the log, never a
/// refusal.** A transcript that has been deleted, a home directory that cannot
/// be read, a file whose permissions changed — none of them says anything about
/// whether the session can be resumed, which is the harness's own business, and
/// a resume refused over a missing history would be the app withholding the
/// conversation because it could not draw the part that had already happened.
pub fn read(cwd: &Path, id: &str) -> Vec<Past> {
    let at = chrono::Utc::now().to_rfc3339();
    let Some(path) = crate::sessions::read::transcript(cwd, id) else {
        log::info!(
            "[session] no transcript for {id} under {}; the conversation opens with no history",
            cwd.display()
        );
        return Vec::new();
    };
    match read_file(&path, &at) {
        Ok(past) => past,
        Err(err) => {
            log::warn!("[session] {} could not be read: {err}", path.display());
            Vec::new()
        }
    }
}

/// One transcript file, streamed.
///
/// Two ceilings, and both are about a file this app did not write.
///
/// The line buffer never grows past [`driver::MAX_LINE`], **the live half's own
/// ceiling and deliberately not `sessions::read`'s**. That module caps a line at
/// 64 KB because it needs only the head of a record — its comment says `type`,
/// `cwd` and the start of a message all sit in the first few hundred bytes —
/// and this needs the whole of one. A line over the cap is dropped, and a
/// dropped `tool_result` is worse than a missing row: its `tool_use` decoded
/// perfectly well, so `journalRows` leaves the pair at `result: null` and
/// `ToolCall.vue` draws that as a call still running, for ever, with nothing in
/// the log. A `Read` of a large file is one such line and an ordinary turn. At
/// the driver's ceiling the two halves of one codec agree about what a line is
/// worth, which is the only answer that cannot make history and live disagree.
///
/// The history itself is capped at [`BUDGET`], the journal's own ceiling, by
/// dropping from the front as it fills: a year-old transcript is longer than a
/// journal may be, the tail is the half a person is coming back to, and feeding
/// the whole of it to `Journal::append` would have that function's trim walk the
/// events once per appended row.
fn read_file(path: &Path, at: &str) -> std::io::Result<Vec<Past>> {
    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut line: Vec<u8> = Vec::new();
    let mut past: VecDeque<Past> = VecDeque::new();
    while let Some(read) =
        crate::sessions::read::next_line(&mut reader, &mut line, driver::MAX_LINE)?
    {
        // Half an object parses as nothing, which is the answer this would
        // have reached anyway — the flag only saves the parse.
        if read.truncated {
            continue;
        }
        let text = String::from_utf8_lossy(&line);
        for event in events_of(&text, at) {
            past.push_back(event);
            if past.len() > BUDGET {
                past.pop_front();
            }
        }
    }
    Ok(past.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: &str = "2026-09-11T09:00:00Z";

    fn kinds(line: &str) -> Vec<EventKind> {
        events_of(line, AT).into_iter().map(|(kind, _)| kind).collect()
    }

    /// The fixture the acceptance criterion names: a human reply, a meta
    /// record, a sidechain record and a tool result, in one file's worth of
    /// lines.
    const TRANSCRIPT: &[&str] = &[
        r#"{"type":"user","timestamp":"2026-09-10T10:00:00Z","origin":{"kind":"human"},"message":{"role":"user","content":"Rename the worktree when the branch changes."}}"#,
        r#"{"type":"user","timestamp":"2026-09-10T10:00:01Z","isMeta":true,"message":{"role":"user","content":"Base directory for this skill: /x"}}"#,
        r#"{"type":"user","timestamp":"2026-09-10T10:00:02Z","isSidechain":true,"message":{"role":"user","content":"Look at the worktree module and report back."}}"#,
        r#"{"type":"assistant","timestamp":"2026-09-10T10:00:03Z","message":{"role":"assistant","content":[{"type":"text","text":"The collision is in `rename`."},{"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"src-tauri/src/vcs/worktree.rs"}}]}}"#,
        r#"{"type":"user","timestamp":"2026-09-10T10:00:04Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"one\ntwo"}]}}"#,
        r#"{"type":"ai-title","timestamp":"2026-09-10T10:00:05Z","aiTitle":"Worktree naming"}"#,
        r#"{"type":"file-history-snapshot","timestamp":"2026-09-10T10:00:06Z","snapshot":{}}"#,
        r#"{"type":"summary","summary":"a conversation about worktrees"}"#,
        "not json at all",
    ];

    fn replay() -> Vec<Past> {
        TRANSCRIPT.iter().flat_map(|line| events_of(line, AT)).collect()
    }

    #[test]
    fn a_transcript_replays_as_the_conversation_that_was_had() {
        let events: Vec<EventKind> = replay().into_iter().map(|(kind, _)| kind).collect();
        assert_eq!(
            events,
            vec![
                EventKind::UserMessage {
                    text: "Rename the worktree when the branch changes.".into(),
                    attachments: Vec::new()
                },
                EventKind::Text { text: "The collision is in `rename`.".into() },
                EventKind::ToolUse {
                    id: "t1".into(),
                    name: "Read".into(),
                    detail: "src-tauri/src/vcs/worktree.rs".into()
                },
                EventKind::ToolResult { id: "t1".into(), ok: true, summary: "2 lines".into() },
            ],
            "the meta record, the sidechain turn, the title, the snapshot, the summary and the \
             line that is not JSON all produce nothing"
        );
    }

    #[test]
    fn an_event_is_stamped_with_the_moment_the_record_carries() {
        let (_, at) = replay().into_iter().next().expect("the person's own words");
        assert_eq!(at, "2026-09-10T10:00:00Z");
    }

    #[test]
    fn a_record_with_no_timestamp_takes_the_clock_it_was_read_by() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hi"}]}}"#;
        assert_eq!(events_of(line, AT), vec![(EventKind::Text { text: "hi".into() }, AT.into())]);
    }

    /// Both halves of the skip, said one at a time, because the two records go
    /// out through two different rules — `human_text` refuses the meta one, and
    /// this module refuses the sidechain one before anything looks at it.
    #[test]
    fn injected_context_is_not_a_person_talking() {
        let line = r#"{"type":"user","isMeta":true,"message":{"content":"Base directory for this skill: /x"}}"#;
        assert!(kinds(line).is_empty());
    }

    #[test]
    fn a_subagents_turn_is_dropped_whole_and_not_only_its_prompt() {
        // Its assistant half would otherwise reach the panel through the
        // driver, which has no opinion about sidechains: a conversation with
        // another agent's working-out spliced into it.
        let said = r#"{"type":"assistant","isSidechain":true,"message":{"content":[{"type":"text","text":"reporting back"}]}}"#;
        assert!(kinds(said).is_empty());
    }

    #[test]
    fn a_turn_opened_in_the_past_is_never_left_open_in_the_journal() {
        // `state_of` folds a `TurnStart` with no `Result` after it into
        // `running`, which is a resumed panel whose composer shows Stop and
        // never comes back.
        let init = r#"{"type":"system","subtype":"init","session_id":"x"}"#;
        assert!(kinds(init).is_empty());
    }

    /// The acceptance criterion smetana-6we6 is explicit about: a re-entry
    /// never leaves a scrap of a partial reply behind. A `stream_event` line
    /// never actually reaches a `.jsonl` transcript (this file's own header
    /// carries the check that established that), but `is_past` refuses one on
    /// its own account too, so the guarantee does not stand on that fact
    /// alone.
    #[test]
    fn a_stray_partial_delta_in_a_transcript_is_never_replayed() {
        let line = r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"partial"}}}"#;
        assert!(kinds(line).is_empty());
    }

    #[test]
    fn a_live_text_question_marker_is_never_replayed() {
        assert!(!is_past(&EventKind::TextQuestion));
    }

    #[test]
    fn a_transcript_that_is_not_there_is_a_conversation_with_no_history() {
        let dir = std::env::temp_dir();
        assert!(read(&dir, "0f7a5f2e-0000-4000-8000-000000000000").is_empty());
    }

    /// A `Read` of a large file is one enormous `tool_result` line and an
    /// ordinary turn in this repository. Dropped, its `tool_use` still decodes,
    /// and `journalRows` leaves the pair at `result: null` — which
    /// `ToolCall.vue` draws as a call still in flight, for ever. The ceiling is
    /// the live codec's, so history and live agree about what a line is worth.
    #[test]
    fn a_large_tool_result_is_replayed_rather_than_dropped() {
        let dir = std::env::temp_dir()
            .join(format!("smetana-history-long-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        let path = dir.join("transcript.jsonl");
        // Well past `sessions::read::MAX_LINE`, which this module used to
        // borrow, and well inside `driver::MAX_LINE`, which it holds itself to.
        let payload = "x".repeat(300 * 1024);
        let lines = [
            TRANSCRIPT[3].to_owned(),
            format!(
                r#"{{"type":"user","timestamp":"2026-09-10T10:00:04Z","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"t1","content":"{payload}"}}]}}}}"#
            ),
        ];
        std::fs::write(&path, format!("{}\n", lines.join("\n"))).expect("write the fixture");

        let events: Vec<EventKind> =
            read_file(&path, AT).expect("read it back").into_iter().map(|(k, _)| k).collect();
        assert!(
            events.iter().any(|kind| matches!(kind, EventKind::ToolResult { id, .. } if id == "t1")),
            "the call must not be left drawn as one still running: {events:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_is_read_as_the_same_conversation_its_lines_are() {
        let dir = std::env::temp_dir()
            .join(format!("smetana-history-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        let path = dir.join("transcript.jsonl");
        std::fs::write(&path, format!("{}\n", TRANSCRIPT.join("\n"))).expect("write the fixture");
        assert_eq!(read_file(&path, AT).expect("read it back"), replay());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
