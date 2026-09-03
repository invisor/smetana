//! The agent sessions this project had when the app last closed, so that the
//! sidebar can offer them back.
//!
//! **A session's process still does not survive a restart.** `RunEvent::Exit`
//! calls `service::shutdown`, which hangs every session's process group up, and
//! nothing here changes that. What survives is a *record*: enough to draw a row
//! that is explicitly not a live one, and enough to take the existing resume
//! road when somebody presses it.
//!
//! **Written at the spawn, not at the exit.** An exit can be unclean — a kill,
//! a panic, a machine that went down — and a registry written only on the way
//! out is exactly the defect `window.rs` was written for. A record is taken
//! away when the agent exits on its own, when a person removes the row, and
//! when the session is resumed (the resume writes its own).
//!
//! **No pid and no liveness check.** `shutdown` killed the processes, and an
//! unclean exit leaves an orphan today with or without this file, so nothing is
//! made worse by not writing one. Pid-plus-start-time liveness is
//! `runs::registry`'s, for a case this feature does not have.
//!
//! The file is `.smetana/agents.json` in the project folder, beside
//! `project.toml` and `runs.json` and outside the repository —
//! `runs::gitignore` is what keeps `.smetana/` out of it.

use std::path::Path;
use std::sync::atomic::{AtomicU32, Ordering};

use serde::{Deserialize, Serialize};

use crate::terminal::model::SessionWork;

/// The shape of the file. A file claiming anything else was written by another
/// app version and is not ours to reason about.
pub const VERSION: u32 = 1;

/// Where it lives, relative to the project's root.
pub const REGISTRY_PATH: &str = ".smetana/agents.json";

/// The counter beside the pid in a temporary file's name, copied from
/// `runs::recovery::write` along with its reason: two overlapping writes would
/// otherwise share one name, and the first would rename what the second had not
/// finished writing.
static TEMP_COUNTER: AtomicU32 = AtomicU32::new(0);

/// One agent session a person started, as it will be offered back after a
/// restart.
///
/// `cwd` and `project` differ for a worktree session, and the resume is spawned
/// in `cwd`: `claude --resume` resolves an id against the directory it is run
/// in, so the project root would be an agent reading a tree its own transcript
/// never mentions.
///
/// `work` is the same `SessionWork` the live row is captioned by, so a restored
/// row carries the caption it had rather than a second wording of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Restorable {
    /// The conversation id, which this app chose at the spawn — see
    /// `terminal::conversation`. A harness whose id this app could not choose
    /// never reaches this file at all.
    pub session_id: String,
    pub agent: String,
    pub cwd: String,
    pub project: String,
    pub work: SessionWork,
    pub started_at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub version: u32,
    pub sessions: Vec<Restorable>,
}

impl Default for Registry {
    fn default() -> Self {
        Self { version: VERSION, sessions: Vec::new() }
    }
}

/// One id is one row: a resume writes a record under the id it reopened, so an
/// append would leave the same conversation offered twice.
pub fn remember(held: &mut Registry, entry: Restorable) {
    held.sessions.retain(|kept| kept.session_id != entry.session_id);
    held.sessions.push(entry);
}

/// True when a record went. The answer is what saves a write on the common
/// case: a session with no record behind it — a run's batch, a shell, a harness
/// that could not be told an id — ends without touching the disk at all.
pub fn forget(held: &mut Registry, session_id: &str) -> bool {
    let before = held.sessions.len();
    held.sessions.retain(|kept| kept.session_id != session_id);
    held.sessions.len() != before
}

/// A missing file, an unreadable one, a malformed one and one written under
/// another version are the same answer: this project has nothing to offer back.
/// None of them is an error a person is told about — the cost of being wrong is
/// a sidebar that opens the way it opened before this existed — but each leaves
/// a line in the log, because a registry silently read as empty is a complaint
/// nobody could otherwise explain.
pub fn read(root: &Path) -> Registry {
    let path = root.join(REGISTRY_PATH);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Registry::default(),
        Err(err) => {
            log::warn!("[terminal] {} could not be read: {err}", path.display());
            return Registry::default();
        }
    };
    match serde_json::from_str::<Registry>(&text) {
        Ok(held) if held.version == VERSION => held,
        Ok(held) => {
            log::warn!(
                "[terminal] {} is version {}, not {VERSION}; nothing will be offered back",
                path.display(),
                held.version
            );
            Registry::default()
        }
        Err(err) => {
            log::warn!("[terminal] {} could not be parsed: {err}", path.display());
            Registry::default()
        }
    }
}

/// Atomic, the same way `runs::recovery::write` writes: a neighbour first,
/// flushed, then renamed. A break halfway through would otherwise leave half a
/// JSON where the next launch expects a registry.
///
/// Every failure is logged and stepped over. What is at stake is an offer to
/// resume, and refusing to start a session over one would cost the whole
/// feature the file exists for.
pub fn write(root: &Path, held: &Registry) {
    let path = root.join(REGISTRY_PATH);
    let Some(dir) = path.parent() else { return };
    if let Err(err) = std::fs::create_dir_all(dir) {
        log::warn!("[terminal] could not create {}: {err}", dir.display());
        return;
    }
    let text = match serde_json::to_string_pretty(held) {
        Ok(text) => text,
        Err(err) => {
            log::warn!("[terminal] could not render the session registry: {err}");
            return;
        }
    };
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!("agents.{}.{n}.tmp", std::process::id()));
    if let Err(err) = write_all(&temp, &text) {
        log::warn!("[terminal] could not write {}: {err}", temp.display());
        let _ = std::fs::remove_file(&temp);
        return;
    }
    if let Err(err) = std::fs::rename(&temp, &path) {
        log::warn!("[terminal] could not write {}: {err}", path.display());
        let _ = std::fs::remove_file(&temp);
    }
}

fn write_all(temp: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

/// Read, remember, write. The whole of what the worker does at a spawn.
pub fn record(root: &Path, entry: Restorable) {
    let mut held = read(root);
    remember(&mut held, entry);
    write(root, &held);
}

/// Read, forget, write — and write nothing when there was nothing to forget,
/// which is the ordinary case for every session this file has no record of.
pub fn drop_record(root: &Path, session_id: &str) {
    let mut held = read(root);
    if forget(&mut held, session_id) {
        write(root, &held);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicU32;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn record_for(id: &str) -> Restorable {
        Restorable {
            session_id: id.to_owned(),
            agent: "claude".to_owned(),
            cwd: "/p".to_owned(),
            project: "/p".to_owned(),
            work: SessionWork::Bare,
            started_at: "2026-09-04T10:00:00Z".to_owned(),
        }
    }

    /// A project folder of its own per test, with the pid and a counter in the
    /// name so that parallel runs do not collide — the same trick
    /// `runs::recovery`'s tests use.
    fn scratch(name: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir()
            .join(format!("smetana-restore-{name}-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".smetana")).expect("create the project folder");
        dir
    }

    /* ---- the rules ------------------------------------------------------- */

    #[test]
    fn a_remembered_session_is_in_the_registry() {
        let mut held = Registry::default();
        remember(&mut held, record_for("a"));
        assert_eq!(held.sessions.len(), 1);
        assert_eq!(held.sessions[0].session_id, "a");
    }

    #[test]
    fn remembering_one_id_twice_replaces_rather_than_duplicates() {
        let mut held = Registry::default();
        remember(&mut held, record_for("a"));
        let mut again = record_for("a");
        again.cwd = "/p/worktree".to_owned();
        remember(&mut held, again);
        assert_eq!(held.sessions.len(), 1, "one id is one row");
        assert_eq!(held.sessions[0].cwd, "/p/worktree");
    }

    #[test]
    fn forgetting_a_session_takes_it_out_and_says_it_did() {
        let mut held =
            Registry { version: VERSION, sessions: vec![record_for("a"), record_for("b")] };
        assert!(forget(&mut held, "a"));
        assert_eq!(held.sessions.len(), 1);
        assert_eq!(held.sessions[0].session_id, "b");
    }

    #[test]
    fn forgetting_a_session_nobody_recorded_changes_nothing() {
        let mut held = Registry { version: VERSION, sessions: vec![record_for("a")] };
        assert!(!forget(&mut held, "gone"));
        assert_eq!(held.sessions.len(), 1);
    }

    #[test]
    fn a_record_travels_as_camel_case() {
        // The front end reads `sessionId`, `startedAt` and `cwd` off these
        // records, so a rename on this side goes quiet on the other: the row
        // keeps being drawn and stops knowing what to resume.
        let json = serde_json::to_string(&record_for("a")).expect("serialise");
        assert!(json.contains("\"sessionId\":\"a\""), "{json}");
        assert!(json.contains("\"startedAt\""), "{json}");
    }

    #[test]
    fn the_work_a_row_is_captioned_by_survives_the_round_trip() {
        // `SessionWork` is serialised for the front end and deserialised only
        // here, so this is the one test that would notice the two halves of
        // that derive disagreeing.
        let mut entry = record_for("a");
        entry.work = SessionWork::EditTask { id: "smetana-42".to_owned() };
        let json = serde_json::to_string(&entry).expect("serialise");
        let back: Restorable = serde_json::from_str(&json).expect("deserialise");
        assert_eq!(back, entry);
    }

    /* ---- the file -------------------------------------------------------- */

    #[test]
    fn a_project_with_no_registry_has_no_restorable_sessions() {
        let root = scratch("empty");
        assert!(read(&root).sessions.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_recorded_session_is_read_back() {
        let root = scratch("round-trip");
        record(&root, record_for("a"));
        let held = read(&root);
        assert_eq!(held.sessions.len(), 1);
        assert_eq!(held.sessions[0].session_id, "a");
        assert_eq!(held.version, VERSION);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_record_survives_the_process_that_wrote_it() {
        // The whole point of the file: nothing is rewritten at the exit, so
        // what is on disk after a spawn is what the next launch reads.
        let root = scratch("survives");
        record(&root, record_for("a"));
        let text = std::fs::read_to_string(root.join(REGISTRY_PATH)).expect("the file is there");
        assert!(text.contains("\"sessionId\": \"a\""), "{text}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_dropped_record_is_gone_from_disk() {
        let root = scratch("drop");
        record(&root, record_for("a"));
        record(&root, record_for("b"));
        drop_record(&root, "a");
        let held = read(&root);
        assert_eq!(held.sessions.len(), 1);
        assert_eq!(held.sessions[0].session_id, "b");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_malformed_registry_reads_as_an_empty_one() {
        let root = scratch("malformed");
        std::fs::write(root.join(REGISTRY_PATH), "{ not json").expect("write the rubbish");
        assert!(read(&root).sessions.is_empty(), "a file nobody can parse says nothing");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_registry_from_another_version_is_not_ours_to_read() {
        let root = scratch("newer");
        std::fs::write(root.join(REGISTRY_PATH), r#"{"version":99,"sessions":[]}"#)
            .expect("write the file");
        assert!(read(&root).sessions.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_registry_in_a_project_folder_that_is_not_there_yet_is_still_written() {
        // A project whose `.smetana/` has never been created is the ordinary
        // case for the very first agent somebody starts in it.
        let root = scratch("no-folder");
        let _ = std::fs::remove_dir_all(root.join(".smetana"));
        record(&root, record_for("a"));
        assert_eq!(read(&root).sessions.len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }
}
