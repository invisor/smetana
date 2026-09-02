//! The disk: where the settings file lives, how it is read and how it is written.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::model::{parse, Outcome, Settings};

/// A counter for temp files. Together with the pid it gives a name no other
/// write has — neither in this process nor in a neighbouring one.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Why the file did not read. Diagnostics for the log, not for the interface.
#[derive(Debug)]
pub enum Problem {
    Broken,
    TooNew,
    /// The file is there but could not be read: not enough permissions, it is
    /// a directory, the disk failed. It differs from a corrupted file in that
    /// there is nothing to copy — `fs::copy` of that same file would fail for
    /// the same reason.
    Unreadable,
}

/// Reads the settings. A missing file is the first run, not an error. A broken
/// or too-new file is not thrown away: it may have been somebody's work, so it
/// goes to `.bak` and the app starts from defaults. A file that exists but does
/// not read (no permissions, a directory in its place and so on) is not a first
/// run: no copy can be taken, so we simply report the problem.
pub fn load(path: &Path) -> (Settings, Option<Problem>) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return (Settings::default(), None);
        }
        Err(err) => {
            log::warn!("settings: could not read {}: {err}", path.display());
            return (Settings::default(), Some(Problem::Unreadable));
        }
    };
    match parse(&text) {
        Outcome::Ok(settings) => (settings, None),
        Outcome::Broken => {
            back_up(path);
            (Settings::default(), Some(Problem::Broken))
        }
        Outcome::TooNew => {
            back_up(path);
            (Settings::default(), Some(Problem::TooNew))
        }
    }
}

/// The configured agent id, and nothing else out of the file.
///
/// The answer is always one of `agents::IDS`: `parse` validates the field on the
/// way in, so an id nobody ships comes back as the default rather than reaching
/// `agents::pick` as an unknown name. A missing or unreadable file answers with
/// the default too — the first run has no file, and neither of those is a reason
/// to refuse to start an agent.
///
/// One value rather than the resolved view `settings_load` hands the front end:
/// the callers here want the file's own answer to a single question, and none of
/// them has a project to resolve against.
pub fn agent(path: &Path) -> String {
    load(path).0.agent
}

/// The configured languages, and nothing else out of the file. The same shape
/// as `agent` above and answering under the same guarantees: every one of them
/// is always an id `agents::LANGUAGES` knows, because `parse` validates them on
/// the way in, and a missing or unreadable file answers with the default set.
pub fn languages(path: &Path) -> crate::agents::Languages {
    let settings = load(path).0;
    crate::agents::Languages {
        agent: settings.agent_language,
        task: settings.task_language,
        commit: settings.commit_language,
        report: settings.report_language,
    }
}

/// The standing instruction off the file, and nothing else out of it. The shape
/// of `agent` above, one field over, and answering on the same terms: a missing
/// or unreadable file answers with the empty string, which says nothing at all —
/// today's behaviour to the letter, and the right answer on a first run when
/// there is no file yet.
pub fn agent_prompt(path: &Path) -> String {
    load(path).0.agent_prompt
}

/// How compressed an agent's answers should be in one project, and nothing
/// else out of the file. The shape of `agent` above, with the one difference
/// that makes this function worth having: **the project's `inherit` is resolved
/// here**, so the rule "a project's own level beats the global one" exists in a
/// single place and no caller has to know there are two fields.
///
/// The key is the project's absolute path exactly as `resolve` stores it — the
/// same key `run_target_branch` takes, and for the reason written there.
///
/// A project with no entry, an entry that says `inherit`, a missing file and an
/// unreadable one all come to the global level, and a file that names none of
/// that comes to `off`: the shipped answer, which is today's behaviour to the
/// letter, since nothing in this app says a word about caveman to any agent
/// until somebody chooses a level.
///
/// What reads it is `settings::caveman_level`, and through it the one place
/// every session in the app is built. The resolution below stays here rather
/// than in that caller, which is the one thing this function exists for: two
/// answers to "which level applies" is how the two fields start to disagree.
pub fn caveman_level(path: &Path, project: &str) -> String {
    let settings = load(path).0;
    let own = settings.projects.get(project).map(|state| state.caveman.as_str());
    match own {
        Some(level) if level != super::model::CAVEMAN_INHERIT => level.to_owned(),
        _ => settings.caveman.level,
    }
}

/// The run gate's thresholds, and nothing else out of the file. The shape of
/// `agent` above, one section over, and read from the disk at every gate check
/// rather than once per run: that is the whole of what lets somebody watching a
/// paused run lower the gate and have that run go on.
///
/// A missing or unreadable file answers with the shipped thresholds, which is
/// today's behaviour — the safe direction for this field, since the other one
/// would be a run spending an allowance nobody meant it to.
pub fn subscription(path: &Path) -> crate::settings::model::SubscriptionSettings {
    load(path).0.subscription
}

/// Whether a run may remove a task's worktree once it is merged and closed, and
/// nothing else out of the file. The shape of `agent` above, one field over.
///
/// A missing or unreadable file answers `true`, the shipped state, which is what
/// the running-tasks skill has always done: this switch exists to *stop* the
/// removal, so a file nobody could read must not silently start keeping every
/// worktree on a person's disk.
pub fn git_remove_worktrees(path: &Path) -> bool {
    load(path).0.git.remove_worktrees
}

/// Whether the update timer may go to the release feed by itself, and nothing
/// else out of the file. The shape of `agent` above, one section over, and read
/// from the disk at every tick rather than once at start: that is the whole of
/// what makes the switch take effect without a restart.
///
/// A missing or unreadable file answers `true`, the shipped state, for
/// `git_remove_worktrees`' reason read the other way round: this switch exists
/// to *stop* the check, so a file nobody could read must not silently leave
/// somebody on an old build for ever.
pub fn updates_auto_check(path: &Path) -> bool {
    load(path).0.updates.auto_check
}

/// What this project's runs were last aimed at, and nothing else out of the
/// file. The shape of `agent` above, one map deeper.
///
/// The key is the project's absolute path exactly as `resolve` stores it, which
/// is the directory `project::nearest_tracked_ancestor` normalized — the same
/// one the tracker worker holds, so the two cannot come to name different
/// folders.
///
/// `None` covers every way of having no answer and they all mean the same
/// thing to the caller: no file, a file that would not read, a project nobody
/// has opened the run dialog for, and a run dialog left with no target branch
/// chosen. There is no default to fall back on here — a branch name guessed for
/// somebody's project would be a wrong answer rather than a missing one.
pub fn run_target_branch(path: &Path, project: &str) -> Option<String> {
    load(path).0.projects.get(project)?.run_settings.as_ref()?.target_branch.clone()
}

/// How big one dialog window was left, and nothing else out of the file. The
/// shape of `agent` above, one section over, and read from the disk at the
/// moment the window opens rather than once at start — the same reason
/// `updates_auto_check` is: it is what makes a size chosen a minute ago apply
/// to the next window without a restart.
///
/// `None` means nobody has ever dragged this kind of window, which is the
/// ordinary case and asks for the fitted height. A missing or unreadable file
/// answers `None` too, which is the same request.
pub fn dialog_size(path: &Path, kind: &str) -> Option<super::model::DialogSize> {
    load(path).0.dialogs.get(kind).copied()
}

/// Keeps how big one dialog window was left.
///
/// The file is re-read on the way in, exactly as `settings_save` re-reads it:
/// the front end writes the same file from the other side, and a write built on
/// a copy taken at startup would put back whatever it held then.
pub fn remember_dialog_size(
    path: &Path,
    kind: &str,
    size: super::model::DialogSize,
) -> Result<(), String> {
    let (mut settings, problem) = load(path);
    // The asymmetry `settings_save` explains, and for its reason: a broken or
    // too-new file has already gone to `.bak` and may be written over, while an
    // unreadable one must not be erased on the strength of a window size.
    if matches!(problem, Some(Problem::Unreadable)) {
        return Err(format!("{}: the existing file could not be read", path.display()));
    }
    settings.dialogs.insert(kind.to_string(), size);
    settings.validate();
    save(path, &settings)
}

/// The write is atomic: a neighbouring file first, then a rename. Otherwise a
/// break halfway through would leave half a JSON and the next launch would lose
/// everything. The content is flushed to disk before the rename — without that
/// a power loss could make the rename durable but not what is in the file. The
/// temp file's name is its own per call: two overlapping writes would share one
/// common name, and the first would rename what the second had not finished writing.
pub fn save(path: &Path, settings: &Settings) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    let text = serde_json::to_string_pretty(settings).map_err(|err| err.to_string())?;
    let temp = temp_path(path);
    if let Err(err) = write_all(&temp, &text) {
        // We clean up after ourselves: the name is unique, and there is nobody
        // to reuse a half-written file anyway.
        let _ = fs::remove_file(&temp);
        return Err(format!("{}: {err}", temp.display()));
    }
    fs::rename(&temp, path).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("{}: {err}", path.display())
    })
}

/// `settings.<pid>.<n>.tmp` next to the target: a rename within a single
/// directory is the only thing the filesystem promises to do atomically.
fn temp_path(path: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = format!("settings.{}.{n}.tmp", std::process::id());
    match path.parent() {
        Some(dir) => dir.join(name),
        None => PathBuf::from(name),
    }
}

fn write_all(temp: &Path, text: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

fn back_up(path: &Path) {
    let backup = path.with_extension("json.bak");
    if let Err(err) = fs::copy(path, &backup) {
        log::warn!("settings: could not save a copy to {}: {err}", backup.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A directory of its own per test: cargo runs them in parallel in one process.
    fn temp_dir() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("smetana-settings-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the test's directory");
        dir
    }

    #[test]
    fn a_missing_file_is_the_first_run() {
        let dir = temp_dir();

        let (settings, problem) = load(&dir.join("settings.json"));

        assert_eq!(settings, Settings::default());
        assert!(problem.is_none(), "a missing file is not a problem");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_file_is_reported_without_a_backup() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        // A directory in the file's place is the portable way to get a read
        // error other than NotFound without chmod (which behaves differently
        // under root).
        fs::create_dir_all(&path).expect("setup");

        let (settings, problem) = load(&path);

        assert_eq!(settings, Settings::default());
        assert!(matches!(problem, Some(Problem::Unreadable)));
        assert!(!dir.join("settings.json.bak").exists(), "nothing to copy — there was no file");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_file_is_kept_as_a_backup() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        fs::write(&path, "{not json").expect("setup");

        let (settings, problem) = load(&path);

        assert_eq!(settings, Settings::default());
        assert!(matches!(problem, Some(Problem::Broken)));
        assert_eq!(
            fs::read_to_string(dir.join("settings.json.bak")).expect("a copy next to it"),
            "{not json"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_newer_file_is_kept_as_a_backup_too() {
        let dir = temp_dir();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"version":99,"appearance":{"theme":"light"}}"#).expect("setup");

        let (settings, problem) = load(&path);

        assert_eq!(settings, Settings::default());
        assert!(matches!(problem, Some(Problem::TooNew)));
        assert!(dir.join("settings.json.bak").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_configured_agent_is_read_off_the_disk() {
        // The path a run takes to find out which harness to start and whose
        // allowance to ask about (smetana-3fi). It is not the path
        // `settings_load` takes, so the round trip through `resolve` and
        // `merge` that `model.rs` pins says nothing about this one.
        let dir = temp_dir();
        let path = dir.join("settings.json");
        fs::write(&path, r#"{"version":1,"agent":"codex"}"#).expect("setup");

        assert_eq!(agent(&path), "codex");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_absent_or_unusable_agent_answers_with_the_default_rather_than_nothing() {
        // Three ways to have no answer, and all of them have to produce an id
        // `agents::resolve` knows: an unknown name would reach `pick` as a
        // request for a harness that does not exist, and the run would fall
        // back to the first installed one having asked nobody's allowance.
        let dir = temp_dir();
        let default_agent = Settings::default().agent;

        let missing = dir.join("settings.json");
        assert_eq!(agent(&missing), default_agent, "a missing file is the first run");

        let unknown = dir.join("unknown.json");
        fs::write(&unknown, r#"{"version":1,"agent":"cursor"}"#).expect("setup");
        assert_eq!(agent(&unknown), default_agent, "an id nobody ships loses the field");

        let broken = dir.join("broken.json");
        fs::write(&broken, "{not json").expect("setup");
        assert_eq!(agent(&broken), default_agent);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn whether_worktrees_are_removed_is_read_off_the_disk_and_defaults_to_removing() {
        // The path a run takes to find out what its prompt should say about
        // worktrees, and it is not the path `settings_load` takes, so the round
        // trip `model.rs` pins says nothing about this one.
        let dir = temp_dir();

        let stored = dir.join("stored.json");
        fs::write(&stored, r#"{"version":1,"git":{"removeWorktrees":false}}"#).expect("setup");
        assert!(!git_remove_worktrees(&stored));

        // Three ways to have no answer, and every one of them has to say
        // "remove": that is what the skill did before this switch existed, so a
        // file nobody can read must not quietly start filling a disk.
        let missing = dir.join("settings.json");
        assert!(git_remove_worktrees(&missing), "a missing file is the first run");

        let broken = dir.join("broken.json");
        fs::write(&broken, "{not json").expect("setup");
        assert!(git_remove_worktrees(&broken));

        let unreadable = dir.join("a-directory-in-its-place");
        fs::create_dir_all(&unreadable).expect("setup");
        assert!(git_remove_worktrees(&unreadable));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_caveman_level_resolves_the_project_against_the_global_one() {
        // The one read here that answers with two fields rather than one, and
        // the whole reason it is a function: the rule "a project's own level
        // beats the global one" lives here and nowhere else.
        let dir = temp_dir();

        let stored = dir.join("stored.json");
        fs::write(
            &stored,
            r#"{"version":1,"caveman":{"level":"lite"},
                "projects":{"/a":{"caveman":"ultra"},"/b":{"caveman":"inherit"}}}"#,
        )
        .expect("setup");
        assert_eq!(caveman_level(&stored, "/a"), "ultra", "a project's own level wins");
        assert_eq!(caveman_level(&stored, "/b"), "lite", "inherit is the global level");
        assert_eq!(caveman_level(&stored, "/never-opened"), "lite", "and so is no entry at all");

        // Three ways to have no answer, and every one of them has to say `off`:
        // that is what the app said about caveman before this setting existed,
        // and a file nobody can read must not start compressing anybody's
        // answers.
        let missing = dir.join("settings.json");
        assert_eq!(caveman_level(&missing, "/a"), "off", "a missing file is the first run");
        assert!(!missing.exists(), "and reading it does not write the default back");

        let without = dir.join("without.json");
        fs::write(&without, r#"{"version":1,"projects":{"/a":{"sideTab":"agents"}}}"#)
            .expect("setup");
        assert_eq!(caveman_level(&without, "/a"), "off", "a file written before the section");

        let broken = dir.join("broken.json");
        fs::write(&broken, "{not json").expect("setup");
        assert_eq!(caveman_level(&broken, "/a"), "off");

        let unreadable = dir.join("a-directory-in-its-place");
        fs::create_dir_all(&unreadable).expect("setup");
        assert_eq!(caveman_level(&unreadable, "/a"), "off");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn whether_updates_are_checked_for_is_read_off_the_disk_and_defaults_to_checking() {
        // The path the timer takes at every tick, and not the path
        // `settings_load` takes, so the round trip `model.rs` pins says nothing
        // about this one.
        let dir = temp_dir();

        let stored = dir.join("stored.json");
        fs::write(&stored, r#"{"version":1,"updates":{"autoCheck":false}}"#).expect("setup");
        assert!(!updates_auto_check(&stored));

        // Three ways to have no answer, and every one of them has to say
        // "check": the switch is there to stop the request, so a file nobody
        // can read must not quietly strand somebody on an old build.
        let missing = dir.join("settings.json");
        assert!(updates_auto_check(&missing), "a missing file is the first run");

        let broken = dir.join("broken.json");
        fs::write(&broken, "{not json").expect("setup");
        assert!(updates_auto_check(&broken));

        let unreadable = dir.join("a-directory-in-its-place");
        fs::create_dir_all(&unreadable).expect("setup");
        assert!(updates_auto_check(&unreadable));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_was_saved_is_what_is_read_back() {
        let dir = temp_dir();
        // The settings directory may not exist yet — the write creates it itself.
        let path = dir.join("nested").join("settings.json");
        let mut settings = Settings::default();
        settings.appearance.theme = "light".into();
        settings.layout.right_collapsed = true;

        save(&path, &settings).expect("write");
        let (read_back, problem) = load(&path);

        assert_eq!(read_back, settings);
        assert!(problem.is_none());
        // The temp file's name is its own per call now, so we look not for a
        // particular name but for none being left in the directory at all.
        let leftovers: Vec<_> = fs::read_dir(path.parent().expect("the directory"))
            .expect("walking the directory")
            .filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
            .filter(|name| name.to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "no temp file is left behind: {leftovers:?}");
        let _ = fs::remove_dir_all(&dir);
    }
}
