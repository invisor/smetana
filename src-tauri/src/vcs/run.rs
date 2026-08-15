//! The only file in the tree that runs git.
//!
//! No test module: this is the disk and the process table, the same standing
//! `files/fs.rs` and `tracker/bd.rs` have. What it produces is fed to
//! `model.rs`, which is pure and carries the tests.

use std::path::Path;
use std::process::Command;

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
    let mut command = Command::new(GIT);
    command.args(args).current_dir(repo).env("GIT_OPTIONAL_LOCKS", "0");
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }
    let out = command.output().map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => VcsError::NoGit(GIT.to_string()),
        _ => VcsError::Io(err.to_string()),
    })?;
    if !out.status.success() {
        return Err(VcsError::Git {
            // `None` is a signal: git was killed rather than exiting. `-1` says
            // so without a second field nothing else would read.
            status: out.status.code().unwrap_or(-1),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        });
    }
    // Lossy, because a repository holding a path in some other encoding must
    // not take the panel down: one mangled row beats no list at all.
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}
