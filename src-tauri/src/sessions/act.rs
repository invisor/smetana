//! The four things the Sessions tab does *to* a session, rather than with it:
//! open its transcript, open the directory it ran in, show that transcript in
//! the file manager, and delete it.
//!
//! Everything else in this module is a read, and the header of `mod.rs` says so
//! at length — nothing there is an error, because a missing folder and a
//! corrupt line both mean "fewer rows". **These two are the opposite of that**,
//! and the difference is the whole reason they are a file of their own: each is
//! a verb somebody pressed, each can fail, and each failure is worth a sentence
//! on screen. So both answer with a `Result` carrying words a person can read,
//! where `read.rs` answers with a shorter list and no complaint.
//!
//! **Why the delete is ours and not `plugin-fs`'s, and why the two opens are
//! ours and not `plugin-opener`'s.** Both plugins expose the verb straight to the
//! webview and are scoped in `capabilities/default.json`; granting either would
//! have to be wide enough for a path under `$HOME/.claude/projects` *and* for
//! the arbitrary working directory a session ran in, which between them is
//! every path on the machine — and `opener`'s own scope check refuses
//! `open_path` outright unless some path entry allows it, so there is no narrow
//! grant to make. `src-tauri/src/updates.rs` records the same refusal for the
//! updater plugin and for the same reason: a permission is published to every
//! window in the app, so the narrow thing to publish is a command of ours that
//! names its own rule. [`is_transcript`] is that rule for the three verbs about
//! a transcript, it is pure, and it is pinned by tests in `model.rs`; the
//! fourth, [`open_directory`], carries the only other one there is.
//!
//! **The reveal is ours for a different reason**, and it is the one that cost
//! something: the plugin's own `reveal_item_in_dir` needs no scope and this app
//! already grants it, so `stores/app.js` could and did call it. What it cannot
//! do is *say* anything — it answers a boolean shared with the file tree, whose
//! one failure sentence is about a browser having no file manager. In the built
//! app the commonest failure is a transcript that has gone, and that sentence
//! was a lie. [`reveal_transcript`] carries the whole argument.
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

/// What a verb says about a path that is not there any more.
///
/// This is the ordinary failure rather than an exotic one: the list is read
/// when the tab is opened and not watched (see `mod.rs`), so a transcript
/// deleted from another window — or a worktree removed after the session that
/// ran in it — is a row still on screen with nothing behind it.
///
/// The noun comes from the caller because only the caller knows which of the
/// two it was asked about, and the difference is the whole of what makes the
/// sentence worth reading: "the transcript is no longer on disk" sends somebody
/// to a different place from "the working directory is no longer on disk". That
/// is also why the two openers below are two functions rather than one over a
/// path of either shape.
fn gone(what: &str) -> String {
    format!("The {what} is no longer on disk.")
}

/// The refusal, with the path in it.
///
/// The path is quoted back because this failure means the front end asked about
/// something the back end does not recognise, and the only way anybody debugs
/// that is by seeing which path it was.
fn refusal(path: &Path, what: &str) -> String {
    format!("That is not {what}, so nothing was done with it: {}", path.display())
}

/// `stat`, with the two failures told apart.
///
/// A path that is not there is the case every verb here is written around and
/// gets [`gone`]. **Anything else is not that**: a folder whose permissions
/// were changed, a volume that went away mid-read, a name too long for the
/// filesystem — all of them would otherwise be reported as "no longer on disk",
/// which is a claim about the person's history rather than about the machine,
/// and would send them looking for a file that is sitting exactly where they
/// left it.
fn stat(path: &Path, what: &str) -> Result<std::fs::Metadata, String> {
    std::fs::metadata(path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => gone(what),
        _ => format!("The {what} could not be read: {err}"),
    })
}

/// The one sentence for a desktop that would not do as it was asked.
fn refused_by_system(verb: &str, err: impl std::fmt::Display) -> String {
    format!("The system would not {verb} it: {err}")
}

/// A session's transcript, handed to whatever the desktop has registered for
/// it.
///
/// Guarded by [`is_transcript`] and by the file being a file: a `.jsonl` outside
/// the projects root is somebody else's, and a *directory* named `x.jsonl`
/// would otherwise be opened by the branch below this one's rules rather than
/// by its own.
pub fn open_transcript(path: &Path, root: &Path) -> Result<(), String> {
    let meta = stat(path, "transcript")?;
    if !meta.is_file() || !is_transcript(path, root) {
        return Err(refusal(path, "a Claude Code transcript"));
    }
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|err| refused_by_system("open", err))
}

/// The directory a session ran in, opened in the platform's file manager.
///
/// **Two conditions, and the second is the one worth reading.** It has to be a
/// directory, which is the obvious half. And it must have **no extension at
/// all**, which is what keeps this from being "run anything on the machine": on
/// macOS `open_path` goes to `/usr/bin/open`, and a directory with an extension
/// is how that platform spells an application — `.app`, `.bundle`, `.pkg`, all
/// of them folders, all of them *launched* rather than shown. A session's
/// working directory is a project, a worktree or a folder inside one and never
/// carries an extension, so nothing this command is for is lost by refusing
/// them.
///
/// Today the only caller hands over a `cwd` read out of a transcript, so no
/// path a person did not choose can reach here at all. That is what makes this
/// a hardening rather than a hole — and it is written down because the header
/// above promises that every verb in this file names its own rule, and until
/// this clause existed the directory branch had none.
///
/// Not asked to be under the projects root, and cannot be: this path is the
/// project the person opened the app on, which is the opposite end of the disk
/// from `~/.claude/projects`.
pub fn open_directory(path: &Path) -> Result<(), String> {
    let meta = stat(path, "working directory")?;
    if !meta.is_dir() || path.extension().is_some() {
        return Err(refusal(path, "a session's working directory"));
    }
    tauri_plugin_opener::open_path(path, None::<&str>)
        .map_err(|err| refused_by_system("open", err))
}

/// Show a session's transcript in the platform's own file manager.
///
/// **Ours rather than `revealInFileManager` in `stores/app.js`, and the reason
/// is a sentence on a screen.** That function is the file tree's, it answers
/// `false` and cannot say why, and its one message is about a browser having no
/// file manager — which is true there and a lie in the built app, where the
/// commonest way for this to fail is a transcript that has gone since the list
/// was read. `reveal_item_in_dir` canonicalises the path before it shows
/// anything, so a missing file is a refusal rather than a no-op, and it would
/// have reached a person as advice to go and install the desktop app they are
/// already running. So this verb is stated here, behind the same guard and with
/// the same words as the other three.
pub fn reveal_transcript(path: &Path, root: &Path) -> Result<(), String> {
    let meta = stat(path, "transcript")?;
    if !meta.is_file() || !is_transcript(path, root) {
        return Err(refusal(path, "a Claude Code transcript"));
    }
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|err| refused_by_system("show", err))
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
        return Err(refusal(path, "a Claude Code transcript"));
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

    /// The row was drawn from a list read a while ago and the file has since
    /// gone. Neither opener reaches the desktop for it, which is what keeps the
    /// same sentence honest across all four verbs.
    #[test]
    fn opening_a_transcript_that_is_not_there_says_which_thing_is_missing() {
        let root = root("open-gone");
        let path = root.join("-p/nothing.jsonl");

        assert_eq!(
            open_transcript(&path, &root),
            Err("The transcript is no longer on disk.".to_owned())
        );
        assert_eq!(
            reveal_transcript(&path, &root),
            Err("The transcript is no longer on disk.".to_owned())
        );
    }

    /// The other noun, and the whole reason the two openers are two functions:
    /// a person sent to look for a missing "file" learns less than one told
    /// which of the two things they pressed has gone.
    #[test]
    fn opening_a_directory_that_is_not_there_names_the_directory() {
        let root = root("cwd-gone");
        let path = root.join("-p/no-such-worktree");

        assert_eq!(
            open_directory(&path),
            Err("The working directory is no longer on disk.".to_owned())
        );
    }

    /// A file that exists, is not a directory and is not a transcript is
    /// refused before the system is asked to open it with whatever it has
    /// registered for that extension.
    #[test]
    fn opening_a_file_that_is_not_a_transcript_is_refused() {
        let root = root("open-wrong");
        let path = root.join("-p/script.sh");
        std::fs::write(&path, "echo hi\n").expect("a file");

        assert!(open_transcript(&path, &root).is_err());
        assert!(reveal_transcript(&path, &root).is_err());
    }

    /// **On macOS a directory with an extension is an application**, and
    /// `open_path` goes to `/usr/bin/open`, which launches one rather than
    /// showing it. A session's working directory never has an extension, so
    /// refusing them costs this command nothing and is what lets the header
    /// claim every verb here names its own rule.
    #[test]
    fn a_directory_with_an_extension_is_refused_rather_than_launched() {
        let root = root("bundle");
        let bundle = root.join("-p/Calculator.app");
        std::fs::create_dir_all(bundle.join("Contents/MacOS")).expect("a bundle");

        let answer = open_directory(&bundle);
        assert!(answer.is_err(), "a .app is launched rather than shown, so it is refused");
        assert!(answer.unwrap_err().contains("working directory"));
    }

    #[test]
    fn an_ordinary_working_directory_passes_the_guard() {
        let root = root("cwd-ok");
        let cwd = root.join("-p/worktree");
        std::fs::create_dir_all(&cwd).expect("a directory");

        // The desktop is not asked in a test binary, so the guard is what is
        // checked: it gets past every clause and reaches `open_path`, whose own
        // failure — there is no desktop here — is the only thing left.
        let answer = open_directory(&cwd);
        assert!(
            answer.is_ok() || !answer.clone().unwrap_err().contains("working directory"),
            "the guard must not be what refuses an ordinary folder: {answer:?}"
        );
    }

    /// A transcript is a file, and a *directory* that happens to be named
    /// `something.jsonl` is not one. Without the `is_file` clause it would be
    /// handed to the desktop by the transcript branch's rules while being the
    /// shape the other branch exists for.
    #[test]
    fn a_directory_named_like_a_transcript_is_not_one() {
        let root = root("dir-jsonl");
        let path = root.join("-p/pretend.jsonl");
        std::fs::create_dir_all(&path).expect("a directory");

        assert!(open_transcript(&path, &root).is_err());
        assert!(reveal_transcript(&path, &root).is_err());
    }
}
