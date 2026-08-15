//! The only file in the tree that runs git.
//!
//! No test module: this is the disk and the process table, the same standing
//! `files/fs.rs` and `tracker/bd.rs` have. What it produces is fed to
//! `model.rs`, which is pure and carries the tests.

use std::path::Path;
use std::process::{Command, Output};

use super::model::VcsError;

/// The program, written once. Named in `VcsError::NoGit` as well, so what the
/// panel says was looked for is what was actually looked for.
const GIT: &str = "git";

/// Every git invocation goes through here.
///
/// The environment is the login shell's `PATH` (`shell_env::path`), the same
/// answer `runs/preflight.rs` and `terminal/pty.rs` work from. This is not
/// tidiness: a bundled app on macOS inherits launchd's environment, which on a
/// stock machine holds nothing a person installed, and the bug is invisible in
/// `npm run tauri dev` because that binary is started from a terminal.
///
/// Four more decisions, each of which is a decision rather than a detail:
///
/// - `Command::current_dir` rather than `-C <repo>`, so a repository path
///   holding an odd character never has to survive being an argument.
/// - `GIT_OPTIONAL_LOCKS=0`, so reading a status never takes `index.lock` out
///   from under an agent working in the same tree. It suppresses only the locks
///   git takes on its own account — a refresh of the index it did not have to
///   do — so a write added here later still takes the locks its own work needs.
/// - `git` not being on `PATH` is `VcsError::NoGit` and never an empty answer:
///   the rule `runs/browser.rs` sets is that anything unobservable reads as
///   "no", loudly.
/// - A non-zero exit carries git's **own stderr, untouched**. The person
///   reading it knows git; a message rewritten here is a worse version of one
///   they can already act on.
pub fn git(repo: &Path, args: &[&str]) -> Result<String, VcsError> {
    let out = spawn(repo, args)?;
    if !out.status.success() {
        return Err(refusal(&out));
    }
    // Lossy, because a repository holding a path in some other encoding must
    // not take the panel down: one mangled row beats no list at all.
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The same call with **one** exit code read as an answer rather than as a
/// refusal.
///
/// git answers "there is nothing by that name" by exiting non-zero, and a
/// caller that asked a question rather than gave an order has to tell that
/// apart from a repository git could not read at all. The code is the caller's
/// to name, so this function never guesses which non-zero exit was a question:
/// everything else is still a refusal carrying git's own stderr.
pub fn git_maybe(repo: &Path, args: &[&str], absent: i32) -> Result<Option<String>, VcsError> {
    let out = spawn(repo, args)?;
    if out.status.code() == Some(absent) {
        return Ok(None);
    }
    if !out.status.success() {
        return Err(refusal(&out));
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// What a call came to, for a caller that decides from something **other than
/// the exit code** whether git refused.
pub enum Attempt {
    /// git exited zero.
    Done,
    /// git exited non-zero, and its refusal is built and ready to be returned
    /// — carried rather than raised, because the caller may find that this was
    /// not a refusal at all.
    Refused(VcsError),
}

/// The same call again, with the non-zero exit handed back instead of raised.
///
/// One caller and one reason: `git merge` and `git rebase` exit non-zero for a
/// tree they left conflicted exactly as they do for one they refused to touch,
/// and telling those apart means reading the tree afterwards rather than the
/// message. Everything else still goes through `git` above, where a non-zero
/// exit is a refusal and nothing else — a caller that has no second question to
/// ask must not have to remember to ask this one.
///
/// A spawn that failed is still an `Err`: no git on the machine is not an exit
/// code, and there is no tree to go and look at.
pub fn git_attempt(repo: &Path, args: &[&str]) -> Result<Attempt, VcsError> {
    let out = spawn(repo, args)?;
    Ok(if out.status.success() { Attempt::Done } else { Attempt::Refused(refusal(&out)) })
}

/// The same call, with the bytes exactly as git wrote them.
///
/// Everything else here takes the lossy conversion because everything else
/// reads git's own output, which is text by construction. The contents of a
/// blob are not: whether it is text at all is the question `looks_binary`
/// answers, and how many bytes it is is the question the ceiling answers —
/// both of them about the bytes on disk rather than about a string a
/// replacement character has already been substituted into.
pub fn git_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, VcsError> {
    let out = spawn(repo, args)?;
    if !out.status.success() {
        return Err(refusal(&out));
    }
    Ok(out.stdout)
}

/// The child itself: the environment, the working directory and the two ways
/// spawning can fail. Everything above differs only in what it makes of the
/// exit code.
fn spawn(repo: &Path, args: &[&str]) -> Result<Output, VcsError> {
    let mut command = Command::new(GIT);
    command.args(args).current_dir(repo).env("GIT_OPTIONAL_LOCKS", "0");
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }
    command.output().map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => VcsError::NoGit(GIT.to_string()),
        _ => VcsError::Io(err.to_string()),
    })
}

fn refusal(out: &Output) -> VcsError {
    VcsError::Git {
        // `None` is a signal: git was killed rather than exiting. `-1` says
        // so without a second field nothing else would read.
        status: out.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}
