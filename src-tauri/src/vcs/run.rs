//! The only file in the tree that runs git.
//!
//! Almost no test module: this is the disk and the process table, the same
//! standing `files/fs.rs` and `tracker/bd.rs` have, and what it produces is fed
//! to `model.rs`, which is pure and carries the tests. The one exception is
//! `bounded` at the bottom — it is a rule about a child process rather than an
//! argument list handed to git, and the two things it has to get right (a pipe
//! that is drained, a child that is killed *and* reaped) are invisible in every
//! outcome a person ever sees.
//!
//! **Two spawns, and the second one goes to a remote.** Everything above
//! `git_network` is local — a status, a merge, a commit — and deliberately has
//! no deadline at all: a hook that hangs was started by somebody who is
//! standing over it. A fetch against an unreachable host is not that. Nobody
//! may be watching it, git has no terminal here to ask a password on, and the
//! panel would sit dead with nothing on screen saying why. So `git_network`
//! adds what that difference calls for: nothing may be asked of a person who is
//! not there, there is a deadline with a kill behind it, and the pipes are
//! handled by hand rather than by `Command::output` — which is `bounded`, and
//! is the one thing here with tests under it.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

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

/// How long a call to a remote may run before it is killed.
///
/// The number is a judgement rather than a measurement: a clone-sized fetch on
/// a slow line can pass it, and the alternative — no ceiling at all — is the
/// panel dead with nothing on screen. A person who hits it presses the button
/// again.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(60);

/// The same call as `git`, for the three commands that reach a remote.
///
/// Two things are different, and both exist because nobody is standing over a
/// background fetch the way somebody stands over a commit hook.
///
/// **git may not ask anybody anything.** There is no terminal on this process,
/// so a prompt for a username, a password or an SSH passphrase is at best a
/// dialog from some other program and at worst a wait with no end.
/// `GIT_TERMINAL_PROMPT=0` refuses the first, `SSH_ASKPASS_REQUIRE=never` the
/// second, and `BatchMode=yes` makes ssh fail instead of asking.
/// `StrictHostKeyChecking` is deliberately left alone: what a machine trusts is
/// not this app's to change.
///
/// **There is a deadline with a kill behind it**, which the rest of this module
/// deliberately does without. A hook that hangs was started by somebody who is
/// watching; a fetch against an unreachable host was started by a window
/// getting focus.
///
/// **Nothing comes back but a refusal.** Standard output goes to `/dev/null`,
/// which `bounded` records as a correctness matter rather than as tidiness, so
/// there is no answer left to hand back and this signature says so.
pub fn git_network(repo: &Path, args: &[&str]) -> Result<(), VcsError> {
    let out = spawn_network(repo, args)?;
    if !out.status.success() {
        return Err(refusal(&out));
    }
    Ok(())
}

/// `git_network`, with the non-zero exit handed back instead of raised — the
/// networked half of `git_attempt`, and for the same one caller: a pull that
/// conflicted has to be told from a pull git refused, and only the tree can say
/// which.
pub fn git_network_attempt(repo: &Path, args: &[&str]) -> Result<Attempt, VcsError> {
    let out = spawn_network(repo, args)?;
    Ok(if out.status.success() { Attempt::Done } else { Attempt::Refused(refusal(&out)) })
}

/// The environment the three remote calls run in. What waiting for one looks
/// like is `bounded` below, which is where everything difficult about this is.
///
/// The `Output` that comes back carries **no stdout at all**, by construction
/// rather than by accident — see `bounded`. Nothing here has ever read it, and
/// `git_network` returning `()` is what keeps that visible from the outside.
fn spawn_network(repo: &Path, args: &[&str]) -> Result<Output, VcsError> {
    let mut command = Command::new(GIT);
    command
        .args(args)
        .current_dir(repo)
        .env("GIT_OPTIONAL_LOCKS", "0")
        // Nothing may be asked of a person who is not there.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }
    bounded(command, NETWORK_TIMEOUT)
}

/// Run a child with a ceiling on how long it may take, and come back with what
/// it said on standard error.
///
/// **The pipes are the whole of this function, and they are why it is not the
/// loop in `agents::oneshot::ask`.** That loop polls `try_wait` and reads both
/// pipes only once the child is gone, and its own comment says exactly what
/// makes that safe: the output is bounded — one line asked for — so neither
/// pipe can fill. Nothing here is bounded. `git pull` writes a merge diffstat
/// of one line per changed file, and `git fetch --prune` a line per updated
/// ref; past the 64 KiB a pipe holds, git blocks in `write`, `try_wait` never
/// answers `Some`, and the deadline kills a git that had already written the
/// merge commit — under a sentence blaming a remote that answered perfectly.
///
/// So: standard output goes to `/dev/null`, since no caller has ever read it,
/// and standard error is **drained on a thread of its own** while the wait
/// happens. That thread is the precondition `oneshot`'s loop states and this
/// one cannot: with it, the child can write as much as it likes and the only
/// thing holding it up is its own work.
///
/// **The child is killed with its group, and then reaped.** `Child` has no
/// reaping `Drop`, so a kill with no `wait` behind it leaves a defunct process
/// for the lifetime of the app — and `autoFetch` is a sweep that repeats every
/// five minutes, so a blackholed route is a hundred zombies a day against
/// `kern.maxprocperuid`, after which nothing in this app can fork at all. The
/// signal goes to the group for the reason `runs/preflight.rs::terminate`
/// records: the process actually blocked on the socket is `ssh` or
/// `git-remote-https`, a child of git's, and killing git alone leaves it
/// holding the connection — and the stderr pipe.
///
/// The reader is joined on the way out of the ordinary path and deliberately
/// **not** on the timeout path: it ends by itself when the last writer is gone,
/// which is what killing the whole group guarantees, and a join there would be
/// this function waiting again on exactly the thing whose wait it just gave up
/// on.
fn bounded(mut command: Command, timeout: Duration) -> Result<Output, VcsError> {
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
    group_of_its_own(&mut command);

    // The only command that ever reaches this in the app is git, which is what
    // makes a missing binary `NoGit` here rather than a plain `Io`.
    let mut child = command.spawn().map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => VcsError::NoGit(GIT.to_string()),
        _ => VcsError::Io(err.to_string()),
    })?;

    let mut pipe = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut said = Vec::new();
        if let Some(pipe) = pipe.as_mut() {
            let _ = pipe.read_to_end(&mut said);
        }
        said
    });

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                terminate(&mut child);
                return Err(VcsError::Timeout(timeout.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(err) => {
                terminate(&mut child);
                return Err(VcsError::Io(err.to_string()));
            }
        }
    };

    // A thread that panicked leaves this call with no stderr rather than with
    // no answer: what it was carrying is a message to read, and the exit code
    // is the fact the caller branches on.
    let stderr = reader.join().unwrap_or_default();
    Ok(Output { status, stdout: Vec::new(), stderr })
}

/// A group of the child's own, so `terminate` has something to name that
/// reaches everything git started and nothing of ours.
#[cfg(unix)]
fn group_of_its_own(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

/// Windows has no process group to ask for, and `terminate` there kills the one
/// process it has.
#[cfg(not(unix))]
fn group_of_its_own(_command: &mut Command) {}

/// End the call and whatever it started, then reap it.
///
/// Naming the group by the child's pid is safe only because the child has not
/// been reaped yet — a reaped pid can be reused and would then name somebody
/// else's group — which is why the `wait` comes last. The same ordering, and
/// the same reason, as `runs/preflight.rs::terminate`.
#[cfg(unix)]
fn terminate(child: &mut Child) {
    unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
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

/// The two things `bounded` has to get right, neither of which shows up in any
/// outcome a person sees: a child that can write more than a pipe holds is
/// still waited on correctly, and a child that outstays its deadline is gone
/// afterwards rather than defunct.
///
/// Driven with `sh` rather than with git, deliberately. What is under test is
/// this module's own handling of a process, and a script can be made to flood a
/// pipe or to outlive a deadline on demand, where git can only be made to do
/// either by having a remote to fail against. Unix only, because the assertions
/// are about process groups and reaping, which is what the function has two
/// halves for.
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-run-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    fn script(command: &str) -> Command {
        let mut sh = Command::new("sh");
        sh.arg("-c").arg(command);
        sh
    }

    /// A zombie is still a process: `kill(pid, 0)` succeeds for one, and fails
    /// with `ESRCH` only once it has been reaped. Which is the whole point —
    /// nothing else this test could look at tells a killed child from a killed
    /// child that is still holding a slot in the process table.
    fn present(pid: i32) -> bool {
        unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
    }

    fn gone_within(pid: i32, patience: Duration) -> bool {
        let deadline = Instant::now() + patience;
        while Instant::now() < deadline {
            if !present(pid) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        !present(pid)
    }

    fn pid_from(path: &std::path::Path) -> i32 {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if let Ok(text) = fs::read_to_string(path) {
                if let Ok(pid) = text.trim().parse() {
                    return pid;
                }
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("the script never wrote its pid to {}", path.display());
    }

    /// The regression this function exists for. 200 KB is past what a pipe
    /// holds, so a child whose stderr nobody is reading blocks in `write` and
    /// never exits — and the symptom is not a wrong answer but a deadline
    /// firing on a call that had already finished its work.
    #[test]
    fn a_child_that_writes_more_than_a_pipe_holds_still_finishes() {
        let out = bounded(script("yes 0123456789 | head -c 200000 1>&2"), Duration::from_secs(20))
            .expect("the child finished inside its deadline");

        assert!(out.status.success(), "the child exited of its own accord");
        assert_eq!(out.stderr.len(), 200_000, "every byte it wrote came back");
    }

    /// Nothing reads standard output, so it is never even opened — and a
    /// child that floods it is therefore not the second half of the deadlock
    /// above.
    #[test]
    fn standard_output_is_discarded_rather_than_carried() {
        let out = bounded(script("yes 0123456789 | head -c 200000"), Duration::from_secs(20))
            .expect("the child finished inside its deadline");

        assert!(out.status.success());
        assert!(out.stdout.is_empty(), "nothing has ever read it");
    }

    /// `Child` has no reaping `Drop`, so a kill with no `wait` behind it leaves
    /// a defunct process for the lifetime of the app — and this is the call the
    /// five-minute sweep makes against a remote that never answers.
    #[test]
    fn a_child_that_outstays_its_deadline_is_killed_and_reaped() {
        let dir = scratch("deadline");
        let pidfile = dir.join("pid");
        let command = script(&format!("echo $$ > {}; sleep 30", pidfile.display()));

        let err = bounded(command, Duration::from_millis(400)).expect_err("the deadline fired");
        assert_eq!(err.kind(), "timeout", "this app stopped it, and git said nothing");

        let pid = pid_from(&pidfile);
        assert!(gone_within(pid, Duration::from_secs(5)), "the child was reaped, not left defunct");

        let _ = fs::remove_dir_all(&dir);
    }

    /// The process actually blocked on the socket is a child of git's — `ssh`,
    /// or `git-remote-https` — so a signal to git alone leaves it running with
    /// the connection and the stderr pipe still in its hands.
    #[test]
    fn the_kill_reaches_what_the_child_started() {
        let dir = scratch("group");
        let pidfile = dir.join("pid");
        let command = script(&format!("sleep 30 & echo $! > {}; wait", pidfile.display()));

        let err = bounded(command, Duration::from_millis(400)).expect_err("the deadline fired");
        assert_eq!(err.kind(), "timeout");

        let grandchild = pid_from(&pidfile);
        assert!(
            gone_within(grandchild, Duration::from_secs(5)),
            "what the child started died with it"
        );

        let _ = fs::remove_dir_all(&dir);
    }
}
