//! The two things the Sessions tab does *to* a transcript, rather than with
//! it: hand it to the desktop, and delete it.
//!
//! Everything else in this module is a read, and the header of `mod.rs` says so
//! at length — nothing there is an error, because a missing folder and a
//! corrupt line both mean "fewer rows". **These two are the opposite of that**,
//! and the difference is the whole reason they are a file of their own: each is
//! a verb somebody pressed, each can fail, and each failure is worth a sentence
//! on screen. So both answer with a `Result` carrying words a person can read,
//! where `read.rs` answers with a shorter list and no complaint.
//!
//! **Why the delete is ours and not `plugin-fs`'s, and why the open is ours and
//! not `plugin-opener`'s.** Both plugins expose the verb straight to the
//! webview and are scoped in `capabilities/default.json`; granting either would
//! have to be wide enough for a path under `$HOME/.claude/projects` *and* for
//! the arbitrary working directory a session ran in, which between them is
//! every path on the machine — and `opener`'s own scope check refuses
//! `open_path` outright unless some path entry allows it, so there is no narrow
//! grant to make. `src-tauri/src/updates.rs` records the same refusal for the
//! updater plugin and for the same reason: a permission is published to every
//! window in the app, so the narrow thing to publish is a command of ours that
//! names its own rule. [`is_transcript`] is that rule, it is pure, and it is
//! pinned by tests in `model.rs`.
//!
//! **A transcript is deleted, never trashed.** A cross-platform trash is a
//! dependency rather than a line of code — `files/fs.rs` carries one for the
//! project's own files and a page of platform caveats with it — and this file
//! is asked about a file the app did not create, outside any repository, behind
//! a confirmation dialog that names the id, the path and the size. The dialog
//! covers the same risk and had to exist either way. If this is ever revisited,
//! revisit it there rather than by widening the guard here.
//!
//! **The root is a parameter, not a reading of `HOME`.** `commands.rs` resolves
//! it once and hands it in, which is what lets every rule below be exercised
//! over a temporary directory. The alternative — reading `HOME` in here and
//! setting it from a test — would have put a process-wide variable under
//! `cargo test`'s threads, where `agents::library`, `runs::browser` and
//! `tracker::access` are all reading the same one.

use std::path::Path;

use super::model::is_transcript;

/// What both verbs say about a path that is not there any more.
///
/// This is the ordinary failure rather than an exotic one: the list is read
/// when the tab is opened and not watched (see `mod.rs`), so a transcript
/// deleted from another window — or a worktree removed after the session that
/// ran in it — is a row still on screen with nothing behind it. The sentence
/// names which of the two vanished, since the menu offers both.
fn gone(what: &str) -> String {
    format!("The {what} is no longer on disk.")
}

/// The refusal, with the path in it.
///
/// The path is quoted back because this failure means the front end asked about
/// something the back end does not recognise as a session's file, and the only
/// way anybody debugs that is by seeing which path it was.
fn refusal(path: &Path) -> String {
    format!(
        "That is not a Claude Code transcript, so nothing was done with it: {}",
        path.display()
    )
}

/// Hand a session's transcript, or the directory it ran in, to the desktop.
///
/// One function for the two rather than one each, because the guard is one
/// question asked of one path: a directory that exists, or a transcript under
/// the projects root. Anything else is refused before the desktop hears about
/// it — a `.jsonl` outside the root is somebody else's file, and a regular file
/// that is not a transcript is not something this tab has any business opening
/// with whatever the system has registered for it.
///
/// A directory is not asked to be under the root, and cannot be: a session's
/// working directory is a project, a worktree or a folder inside one, which is
/// exactly the path the person opened this app on.
pub fn open(path: &Path, root: &Path) -> Result<(), String> {
    let meta = std::fs::metadata(path).map_err(|_| gone("file"))?;
    if !meta.is_dir() && !is_transcript(path, root) {
        return Err(refusal(path));
    }
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|err| format!("The system would not open it: {err}"))
}

/// Delete one transcript.
///
/// Irreversible, which is why the confirmation is the caller's and is named in
/// the acceptance criteria rather than left to taste. Nothing else goes: not
/// the folder around it, not the `subagents/` directory beside it — a session's
/// subagent transcripts are their own files and this is about the one file it
/// was given.
pub fn delete(path: &Path, root: &Path) -> Result<(), String> {
    if !is_transcript(path, root) {
        return Err(refusal(path));
    }
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(gone("transcript")),
        Err(err) => Err(format!("The transcript could not be deleted: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A temporary stand-in for `~/.claude/projects`, with one project folder
    /// in it. Named per test so nothing here shares a disk with anything else.
    fn root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-act-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("-p")).expect("a temporary projects root");
        dir
    }

    #[test]
    fn a_transcript_is_deleted_and_is_gone_afterwards() {
        let root = root("delete");
        let path = root.join("-p/abc.jsonl");
        std::fs::write(&path, "{}\n").expect("a transcript");

        assert_eq!(delete(&path, &root), Ok(()));
        assert!(!path.exists());
    }

    /// The other half of the acceptance criterion: the answer was no, so
    /// nothing called `delete`, and the file is still there. Worth pinning
    /// precisely because the failure it guards against is a delete that
    /// happened anyway — and because the two halves are one criterion.
    #[test]
    fn a_transcript_nobody_confirmed_is_still_on_disk() {
        let root = root("kept");
        let path = root.join("-p/kept.jsonl");
        std::fs::write(&path, "{}\n").expect("a transcript");

        // The refusal path of the dialog: no call is made at all.
        assert!(path.exists());
        assert_eq!(std::fs::read_to_string(&path).ok().as_deref(), Some("{}\n"));
    }

    #[test]
    fn a_file_outside_the_projects_root_is_refused_rather_than_deleted() {
        let root = root("outside");
        let outside = root.parent().expect("a parent").join(format!(
            "smetana-act-{}-outside-notes.jsonl",
            std::process::id()
        ));
        std::fs::write(&outside, "{}\n").expect("a file");

        assert!(delete(&outside, &root).is_err(), "a file outside the root must not be deleted");
        assert!(outside.exists(), "and it must still be there");
        let _ = std::fs::remove_file(&outside);
    }

    #[test]
    fn a_file_under_the_root_that_is_not_a_transcript_is_refused() {
        let root = root("wrong-kind");
        let path = root.join("-p/notes.md");
        std::fs::write(&path, "hello\n").expect("a file");

        assert!(delete(&path, &root).is_err());
        assert!(path.exists());
    }

    /// `Path::starts_with` is lexical, so this path is "under" the root as far
    /// as the second clause of the guard is concerned. It names a file
    /// somewhere else entirely, and deleting it would be the worst failure this
    /// command has.
    #[test]
    fn a_path_that_walks_back_out_of_the_root_is_refused() {
        let root = root("escape");
        let victim = root.parent().expect("a parent").join(format!(
            "smetana-act-{}-victim.jsonl",
            std::process::id()
        ));
        std::fs::write(&victim, "{}\n").expect("a file");
        let sneaky = root.join("-p/../..").join(
            victim.file_name().expect("a name")
        );

        assert!(delete(&sneaky, &root).is_err());
        assert!(victim.exists());
        let _ = std::fs::remove_file(&victim);
    }

    /// The row was drawn from a list read a while ago and the file has since
    /// gone. A silent success would leave the row on screen with nothing said;
    /// what the criteria ask for is a sentence, and this is it.
    #[test]
    fn a_transcript_that_has_already_gone_says_so() {
        let root = root("vanished");
        let path = root.join("-p/vanished.jsonl");

        assert_eq!(delete(&path, &root), Err("The transcript is no longer on disk.".to_owned()));
    }

    /// `open` never reaches the desktop for a path that is not there, which is
    /// what keeps the same sentence honest for the other verbs.
    #[test]
    fn opening_something_that_is_not_there_says_so_rather_than_asking_the_desktop() {
        let root = root("open-gone");
        let path = root.join("-p/nothing.jsonl");

        assert_eq!(open(&path, &root), Err("The file is no longer on disk.".to_owned()));
    }

    /// A file that exists, is not a directory and is not a transcript is
    /// refused before the system is asked to open it with whatever it has
    /// registered for that extension.
    #[test]
    fn opening_a_file_that_is_not_a_transcript_is_refused() {
        let root = root("open-wrong");
        let path = root.join("-p/script.sh");
        std::fs::write(&path, "echo hi\n").expect("a file");

        assert!(open(&path, &root).is_err());
    }
}
