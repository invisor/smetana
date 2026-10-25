//! Which conversation a Codex session opened, asked after the fact.
//!
//! Codex has no `--session-id`: it names the conversation itself and writes the
//! name into the first line of its rollout file,
//! `$CODEX_HOME/sessions/<yyyy>/<mm>/<dd>/rollout-<timestamp>-<uuid>.jsonl`. So
//! the id is discovered rather than assigned, which is what
//! `Profile::session_id_after_start` exists for.
//!
//! **Read-only, and that is a boundary rather than an implementation detail**:
//! nothing of this app is written into a person's own Codex directory, the same
//! line `SkillDelivery::Inline` already draws.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Where Codex keeps its sessions: `$CODEX_HOME/sessions`, and `~/.codex` when
/// that variable is unset, which is Codex's own default.
///
/// `HOME` is read straight rather than through a crate, the way
/// `agents::library`, `sessions::read` and `runs::browser` already read it — a
/// fifth answer to the same question is a fifth thing to keep in step.
pub fn sessions_root() -> Option<PathBuf> {
    root_from(std::env::var_os("CODEX_HOME"), std::env::var_os("HOME"))
}

/// The rule the function above is made of, with the environment handed in.
///
/// Split out so a test can state it without touching the process's own
/// environment: `set_var` in a test binary changes it for every other test
/// running beside this one, and the rule is what is worth pinning anyway.
///
/// An empty variable is treated as an unset one, the same filter
/// `tracker::access::home` applies and for its reason — an exported `HOME=`
/// would otherwise make `/.codex` the answer.
fn root_from(
    codex_home: Option<std::ffi::OsString>,
    home: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    let root = match codex_home.filter(|set| !set.is_empty()) {
        Some(set) => PathBuf::from(set),
        None => PathBuf::from(home.filter(|home| !home.is_empty())?).join(".codex"),
    };
    Some(root.join("sessions"))
}

/// The id of the newest session recorded under `root` that ran in `cwd`, was
/// written after `started_after`, and **was not already there** when `before`
/// was taken.
///
/// That third condition is the load-bearing one and it was learnt the hard way.
/// Only a *resume* runs in a worktree of its own: every other intent runs at the
/// project root (`terminal::service`'s `Create` arm), so two agent sessions in
/// one project always share a `cwd`. Picking by modification time alone then
/// hands a new session the id of any **older session still writing**, which is
/// an ordinary way to use the agents panel rather than an edge — and the damage
/// is not a wrong row but a lost one: `Request::SessionIdFound` would write the
/// new session's `cwd`, work and start time into `.smetana/agents.json` under
/// the older session's id, destroying its record, and closing the new one would
/// then delete it outright.
///
/// A file that did not exist before the spawn cannot belong to a session that
/// started before it. That is what `before` says, and it is a set of paths
/// rather than a timestamp because `meta.created()` is not answered on every
/// filesystem this ships on.
///
/// `cwd` is still asked, and it is what stops a Codex session running somewhere
/// else entirely from being claimed by this card. `started_after` is still asked
/// too, and it is deliberately kept beside `before` rather than replaced by it:
/// the two disagree only if the walk raced a write, and the cheaper filter costs
/// one `metadata` call that is made anyway.
///
/// What remains is a session **started in the same folder inside the few seconds
/// this is looking**, which is accepted. The alternative, `codex resume --last`,
/// is that same ambiguity as the design rather than as an edge.
///
/// Every way of failing answers `None`. A session with no id recorded is exactly
/// the session this harness had before, so nothing is lost that was not already
/// lost.
pub fn newest_session_id(
    root: &Path,
    cwd: &Path,
    started_after: SystemTime,
    before: &[PathBuf],
) -> Option<String> {
    let mut best: Option<(SystemTime, String)> = None;
    for path in rollouts(root) {
        if before.contains(&path) {
            continue;
        }
        let Ok(modified) = path.metadata().and_then(|meta| meta.modified()) else { continue };
        if modified < started_after {
            continue;
        }
        let Some((id, recorded)) = first_record(&path) else { continue };
        if Path::new(&recorded) != cwd {
            continue;
        }
        if best.as_ref().map_or(true, |(when, _)| modified > *when) {
            best = Some((modified, id));
        }
    }
    best.map(|(_, id)| id)
}

/// Every `.jsonl` under the year/month/day directories Codex writes. Walked
/// rather than matched against a pattern: the layout is somebody else's, and a
/// depth written into a glob is a guess about it.
///
/// Public because the same walk is what a caller takes a *snapshot* with, at the
/// instant of the spawn, to hand back to `newest_session_id` as `before`. One
/// function rather than two, so the snapshot and the search cannot come to
/// disagree about which files are even in the running.
pub fn rollouts(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                found.push(path);
            }
        }
    }
    found
}

/// The `session_id` and `cwd` off a rollout's **first line**, which is where
/// Codex writes its `session_meta` record. Only the first line is read: these
/// files reach tens of megabytes and everything after it is the conversation.
///
/// `session_id` and deliberately not `id`, which sits beside it in the same
/// payload and holds the same UUID at 0.146.0. The two have not always both
/// been there: measured across the rollouts on the machine this was written
/// against, an older CLI wrote a `session_meta` payload carrying `id` and no
/// `session_id` at all. Such a file is skipped, and **that is the right answer
/// rather than a gap to fill**: the session being looked for was written
/// moments ago by the installed CLI, so a file only an older one could have
/// produced is by construction somebody else's. Reading `id` as a fallback
/// would widen the search to conversations this app has no other reason to
/// believe anything about, which is the opposite of what every other filter
/// here is for.
fn first_record(path: &Path) -> Option<(String, String)> {
    let file = File::open(path).ok()?;
    let mut line = String::new();
    BufReader::new(file).read_line(&mut line).ok()?;
    let event: serde_json::Value = serde_json::from_str(&line).ok()?;
    if event.get("type").and_then(serde_json::Value::as_str) != Some("session_meta") {
        return None;
    }
    let payload = event.get("payload")?;
    let id = payload.get("session_id").and_then(serde_json::Value::as_str)?;
    let cwd = payload.get("cwd").and_then(serde_json::Value::as_str)?;
    Some((id.to_owned(), cwd.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread::sleep;
    use std::time::Duration;

    /// One rollout file laid out the way Codex lays them out:
    /// `<root>/2026/09/06/rollout-<timestamp>-<uuid>.jsonl`, whose first line is
    /// the `session_meta` record carrying the id and the directory.
    fn rollout(root: &Path, id: &str, cwd: &str) {
        let dir = root.join("2026").join("09").join("06");
        fs::create_dir_all(&dir).expect("setup");
        let meta = format!(
            r#"{{"ordinal":0,"type":"session_meta","payload":{{"session_id":"{id}","cwd":"{cwd}"}}}}"#
        );
        fs::write(
            dir.join(format!("rollout-2026-09-06T12-37-08-{id}.jsonl")),
            format!("{meta}\n{{\"type\":\"event_msg\"}}\n"),
        )
        .expect("setup");
    }

    /// Nothing was there when this session spawned. Written once because most of
    /// these tests are about the other two filters and would otherwise repeat an
    /// empty slice at every call.
    const NOTHING_BEFORE: &[PathBuf] = &[];

    #[test]
    fn the_newest_session_started_in_this_directory_is_the_one_claimed() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let since = SystemTime::now();
        rollout(root.path(), "older-0000", "/work/tree");
        sleep(Duration::from_millis(20));
        rollout(root.path(), "newer-1111", "/work/tree");
        assert_eq!(
            newest_session_id(root.path(), Path::new("/work/tree"), since, NOTHING_BEFORE)
                .as_deref(),
            Some("newer-1111")
        );
    }

    /// The reason `before` exists, and the case modification time alone gets
    /// wrong. Every intent but a resume runs at the project root, so two agent
    /// sessions in one project share a `cwd` as a matter of course; an older one
    /// still writing has the newer mtime, and used to be handed to whoever
    /// started second. What that cost was the older session's record in
    /// `.smetana/agents.json`, overwritten and then deleted.
    #[test]
    fn a_session_that_was_already_running_here_is_never_claimed_however_recently_it_wrote() {
        let root = tempfile::tempdir().expect("a temporary directory");
        rollout(root.path(), "still-writing", "/work/tree");
        let before = rollouts(root.path());
        assert_eq!(before.len(), 1, "the snapshot saw the session already there");

        let since = SystemTime::now();
        sleep(Duration::from_millis(20));
        // The older session writes again, so its mtime is now the newest of the
        // two and its `cwd` matches. Only the snapshot tells them apart.
        rollout(root.path(), "still-writing", "/work/tree");

        assert_eq!(newest_session_id(root.path(), Path::new("/work/tree"), since, &before), None);
    }

    /// And the session that really is this one is still found, in the same
    /// directory and with that older session still writing beside it.
    #[test]
    fn the_session_written_after_the_spawn_is_found_beside_one_that_was_already_there() {
        let root = tempfile::tempdir().expect("a temporary directory");
        rollout(root.path(), "still-writing", "/work/tree");
        let before = rollouts(root.path());

        let since = SystemTime::now();
        sleep(Duration::from_millis(20));
        rollout(root.path(), "ours-2222", "/work/tree");
        rollout(root.path(), "still-writing", "/work/tree");

        assert_eq!(
            newest_session_id(root.path(), Path::new("/work/tree"), since, &before).as_deref(),
            Some("ours-2222")
        );
    }

    #[test]
    fn a_session_running_somewhere_else_is_never_claimed() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let since = SystemTime::now();
        rollout(root.path(), "elsewhere-0", "/some/other/tree");
        assert_eq!(
            newest_session_id(root.path(), Path::new("/work/tree"), since, NOTHING_BEFORE),
            None
        );
    }

    #[test]
    fn a_session_older_than_the_spawn_is_somebody_elses_conversation() {
        let root = tempfile::tempdir().expect("a temporary directory");
        rollout(root.path(), "before-0000", "/work/tree");
        sleep(Duration::from_millis(20));
        let since = SystemTime::now();
        assert_eq!(
            newest_session_id(root.path(), Path::new("/work/tree"), since, NOTHING_BEFORE),
            None
        );
    }

    #[test]
    fn a_machine_with_no_such_directory_answers_nothing_rather_than_failing() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let missing = root.path().join("never-created");
        assert!(rollouts(&missing).is_empty(), "the snapshot is empty rather than a failure");
        assert_eq!(
            newest_session_id(
                &missing,
                Path::new("/work/tree"),
                SystemTime::UNIX_EPOCH,
                NOTHING_BEFORE
            ),
            None
        );
    }

    /// An older Codex wrote `id` and no `session_id`, which was found on the
    /// machine this was written against rather than imagined. The file is
    /// skipped, and the test exists so that reading `id` as a fallback is a
    /// decision somebody has to take deliberately: a rollout only an older CLI
    /// could have written is not the session this app just started.
    #[test]
    fn a_record_from_an_older_cli_that_names_no_session_id_is_passed_over() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = root.path().join("2026").join("05").join("18");
        fs::create_dir_all(&dir).expect("setup");
        fs::write(
            dir.join("rollout-2026-05-18T20-44-18-019e3c30.jsonl"),
            concat!(
                r#"{"type":"session_meta","payload":{"id":"019e3c30","cwd":"/work/tree"}}"#,
                "\n",
            ),
        )
        .expect("setup");
        assert_eq!(
            newest_session_id(
                root.path(),
                Path::new("/work/tree"),
                SystemTime::UNIX_EPOCH,
                NOTHING_BEFORE
            ),
            None
        );
    }

    #[test]
    fn a_file_whose_first_line_is_not_a_session_record_is_passed_over() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = root.path().join("2026").join("09").join("06");
        fs::create_dir_all(&dir).expect("setup");
        fs::write(dir.join("rollout-broken.jsonl"), "not json at all\n").expect("setup");
        assert_eq!(
            newest_session_id(
                root.path(),
                Path::new("/work/tree"),
                SystemTime::UNIX_EPOCH,
                NOTHING_BEFORE
            ),
            None
        );
    }

    /// Only the first line of a rollout is read, and nothing under the root is
    /// written to. Both are properties this app owes somebody else's directory,
    /// and neither is visible in the answer, so they are checked directly: the
    /// file is left with the modification time it had, and a body that is not
    /// JSON at all cannot be parsed by anything that never opened it.
    #[test]
    fn a_rollouts_body_is_never_read_and_the_directory_is_never_written_to() {
        let root = tempfile::tempdir().expect("a temporary directory");
        let dir = root.path().join("2026").join("09").join("06");
        fs::create_dir_all(&dir).expect("setup");
        let file = dir.join("rollout-2026-09-06T12-37-08-only-first.jsonl");
        fs::write(
            &file,
            concat!(
                r#"{"ordinal":0,"type":"session_meta","payload":{"session_id":"only-first","cwd":"/work/tree"}}"#,
                "\nthis body is not JSON and must never be parsed\n",
            ),
        )
        .expect("setup");
        let before = fs::metadata(&file).and_then(|meta| meta.modified()).expect("setup");
        assert_eq!(
            newest_session_id(
                root.path(),
                Path::new("/work/tree"),
                SystemTime::UNIX_EPOCH,
                NOTHING_BEFORE
            )
            .as_deref(),
            Some("only-first")
        );
        assert_eq!(
            fs::read_dir(&dir).expect("the day directory").count(),
            1,
            "nothing of this app is written into a Codex session directory"
        );
        assert_eq!(
            fs::metadata(&file).and_then(|meta| meta.modified()).expect("the rollout"),
            before,
            "the rollout is opened for reading and never touched"
        );
    }

    #[test]
    fn the_root_follows_codex_home_and_falls_back_to_the_home_directory() {
        use std::ffi::OsString;
        assert_eq!(
            root_from(Some(OsString::from("/opt/codex-home")), Some(OsString::from("/home/x"))),
            Some(PathBuf::from("/opt/codex-home/sessions")),
            "the variable Codex itself reads wins over the home directory"
        );
        assert_eq!(
            root_from(None, Some(OsString::from("/home/x"))),
            Some(PathBuf::from("/home/x/.codex/sessions")),
            "with nothing set, Codex's own default is under the home directory"
        );
        assert_eq!(
            root_from(Some(OsString::new()), Some(OsString::from("/home/x"))),
            Some(PathBuf::from("/home/x/.codex/sessions")),
            "an exported but empty variable is not a directory"
        );
        assert_eq!(
            root_from(None, None),
            None,
            "a machine with no home directory is asked nothing rather than /.codex"
        );
    }
}
