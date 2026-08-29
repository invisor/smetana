//! The only file in the tree that runs git.
//!
//! Almost no test module: this is the disk and the process table, the same
//! standing `files/fs.rs` and `tracker/bd.rs` have, and what it produces is fed
//! to `model.rs`, which is pure and carries the tests. The exceptions are
//! `bounded` and `terminate` at the bottom — they are rules about a child
//! process rather than an argument list handed to git, and the three things
//! they have to get right (a pipe that is drained, a child that is killed *and*
//! reaped, a git given the chance to take its own lock files back off the disk)
//! are invisible in every outcome a person ever sees.
//!
//! **Every call has a ceiling, and there are three of them**: `READ_CEILING`
//! for a local read, `WRITE_CEILING` for a local write, `NETWORK_TIMEOUT` for
//! the calls that reach a remote. Each carries the reason for its number beside
//! it, and every entry point below says which of the three it runs under, so a
//! call site is classified by the function it names rather than by what a
//! reader knows about git.
//!
//! **The local ceilings replace a decision this file used to record, and that
//! decision was not wrong.** It said a hook that hangs was started by somebody
//! who is standing over it, so stopping their build would be this app inventing
//! a policy for a repository it knows nothing about. What it leaves out is
//! where the wait actually lands. There is no worker in `vcs/` and no queue —
//! `mod.rs` records why, and it still holds — so a git that never returns is an
//! IPC call that never answers: the button stays inert, nothing on screen says
//! why, and the only way out is restarting the app. Somebody standing over a
//! hook can watch it in the terminal they started it from; nobody can press a
//! dead button. A call that comes back can be pressed again, and that is the
//! whole of why the ceiling wins the argument now that this module writes.
//!
//! **A call to a remote is still its own pair of functions and not a third
//! number handed to the local ones**, because the deadline is the smaller half
//! of what makes it different: nobody pressed it (a window came back into
//! focus) and there is nobody for git to ask, so `git_network` refuses every
//! prompt as well. That part is an environment rather than a ceiling.

use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::model::VcsError;

/// The program, written once. Named in `VcsError::NoGit` as well, so what the
/// panel says was looked for is what was actually looked for.
const GIT: &str = "git";

/// How long a local **read** may run before it is stopped.
///
/// The reads are `status`, `for-each-ref`, `rev-parse`, `cat-file` and `diff`,
/// every one of them against the local disk: measured on this repository at
/// 220 ms for a `git status` with a cold cache and 10 ms warm. Thirty seconds
/// is two orders of magnitude above that on purpose — the number is not a
/// budget for the work, it is the point past which the work is no longer
/// happening. What is meant to fit under it is a large tree on a slow or
/// networked filesystem; what is meant to hit it is a read sitting on a helper
/// somebody configured — a `textconv`, a clean filter — which is the way a read
/// hangs at all.
const READ_CEILING: Duration = Duration::from_secs(30);

/// How long a local **write** may run before it is stopped.
///
/// Ten times the read, and the difference is hooks: a write runs the
/// repository's own `pre-commit`, `commit-msg` or merge driver, which is
/// somebody else's program and may reasonably lint, test or compile. The number
/// is taken from the first repository this ships against — this one configures
/// `core.hooksPath = .beads/hooks`, and every hook there wraps `bd hooks run`
/// in `timeout ${BEADS_HOOK_TIMEOUT:-300}` and carries on when it expires. That
/// is a patience of 300 seconds *declared by the project itself*, and a ceiling
/// under it stops a commit the project expects to succeed.
///
/// **The two ways of being wrong are not symmetric, which is what sets the
/// number.** A ceiling too low is a hard failure with no way round it from
/// inside the app: the commit is killed, the person presses again, it is killed
/// again, and committing in that repository is simply impossible. A ceiling too
/// high costs one wait, on a hang that should not have happened, and it ends in
/// an error somebody can act on. A ceiling is here to turn "forever" into
/// "eventually" and not to enforce a budget on somebody else's hook, so it is
/// set by the longest legitimate wait rather than by the shortest annoying one.
///
/// The other half is that the work is off the runtime (`commands.rs`), so five
/// minutes of ceiling costs one inert button and nothing else in the window —
/// no tokio worker is held, and the file tree, the editor and the terminals
/// carry on. That is what makes the asymmetry cheap enough to act on.
const WRITE_CEILING: Duration = Duration::from_secs(300);

/// Every git **read** goes through here.
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
///   do — so `git_write` below still takes the locks its own work needs.
/// - `git` not being on `PATH` is `VcsError::NoGit` and never an empty answer:
///   the rule `runs/browser.rs` sets is that anything unobservable reads as
///   "no", loudly.
/// - A non-zero exit carries git's **own stderr, untouched**. The person
///   reading it knows git; a message rewritten here is a worse version of one
///   they can already act on.
pub fn git_read(repo: &Path, args: &[&str]) -> Result<String, VcsError> {
    let out = spawn(repo, args, READ_CEILING, Capture::Keep)?;
    if !out.status.success() {
        return Err(refusal(&out));
    }
    // Lossy, because a repository holding a path in some other encoding must
    // not take the panel down: one mangled row beats no list at all.
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Every git **write** goes through here: a checkout, a branch, an add, a
/// commit, an abort.
///
/// Everything about the call is `git_read`'s above except the ceiling, and the
/// split into two entry points is what that ceiling is for — a call site says
/// which of the two it is by the name it writes, so nobody has to know which
/// git subcommands take `index.lock` to read this module.
///
/// **Nothing comes back but a refusal.** Standard output is discarded, the way
/// `git_network` discards it and for the same reason: no caller has ever read
/// what a write prints, and a signature that says so is one fewer pipe to keep
/// drained.
pub fn git_write(repo: &Path, args: &[&str]) -> Result<(), VcsError> {
    let out = spawn(repo, args, WRITE_CEILING, Capture::Discard)?;
    if !out.status.success() {
        return Err(refusal(&out));
    }
    Ok(())
}

/// A read with **one** exit code taken as an answer rather than as a refusal.
///
/// git answers "there is nothing by that name" by exiting non-zero, and a
/// caller that asked a question rather than gave an order has to tell that
/// apart from a repository git could not read at all. The code is the caller's
/// to name, so this function never guesses which non-zero exit was a question:
/// everything else is still a refusal carrying git's own stderr.
///
/// A read by construction and not by convention — asking git what a name
/// resolves to writes nothing — so it runs under `READ_CEILING` with no way for
/// a caller to say otherwise.
pub fn git_maybe(repo: &Path, args: &[&str], absent: i32) -> Result<Option<String>, VcsError> {
    let out = spawn(repo, args, READ_CEILING, Capture::Keep)?;
    if out.status.code() == Some(absent) {
        return Ok(None);
    }
    if !out.status.success() {
        return Err(refusal(&out));
    }
    Ok(Some(String::from_utf8_lossy(&out.stdout).into_owned()))
}

/// `git_maybe`, with bytes written to the child's standard input.
///
/// One caller and one reason: `git check-ignore -z --stdin` is how the file
/// tree asks about a whole listing in a single process, and a listing is up to
/// `files::model::MAX_ENTRIES` names. The same question as arguments is a
/// command line that a directory of long names pushes past the 32 767
/// characters `CreateProcess` accepts, so on Windows the call would begin
/// failing on exactly the directories the question is most worth asking about.
///
/// Everything else is `git_maybe`'s, the exit code that counts as an answer
/// included. A read by construction — `check-ignore` writes nothing — so
/// `READ_CEILING`.
pub fn git_maybe_fed(
    repo: &Path,
    args: &[&str],
    absent: i32,
    input: &[u8],
) -> Result<Option<String>, VcsError> {
    let out = bounded(local(repo, args), READ_CEILING, Capture::Keep, Feed::Bytes(input))?;
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

/// A **write** with the non-zero exit handed back instead of raised.
///
/// Two callers and one reason between them: a non-zero exit that is not by
/// itself an answer. `git merge` and `git rebase` exit non-zero for a tree they
/// left conflicted exactly as they do for one they refused to touch, so telling
/// those apart means reading the tree afterwards rather than the message; and
/// `git branch -d` declines an unmerged branch and one held by another worktree
/// at the same code, so telling *those* apart means asking git a second
/// question. Everything else still goes through `git_write` above, where a
/// non-zero exit is a refusal and nothing else — a caller that has no second
/// question to ask must not have to remember to ask this one.
///
/// A write by construction, so `WRITE_CEILING`: two of the three operations it
/// is used for rewrite the working tree, and the third writes a ref, which is
/// still a call a repository's own hooks can sit in front of.
///
/// A spawn that failed is still an `Err`: no git on the machine is not an exit
/// code, and there is no tree to go and look at.
pub fn git_attempt(repo: &Path, args: &[&str]) -> Result<Attempt, VcsError> {
    let out = spawn(repo, args, WRITE_CEILING, Capture::Discard)?;
    Ok(if out.status.success() { Attempt::Done } else { Attempt::Refused(refusal(&out)) })
}

/// A read, with the bytes exactly as git wrote them.
///
/// Everything else here takes the lossy conversion because everything else
/// reads git's own output, which is text by construction. The contents of a
/// blob are not: whether it is text at all is the question `looks_binary`
/// answers, and how many bytes it is is the question the ceiling answers —
/// both of them about the bytes on disk rather than about a string a
/// replacement character has already been substituted into.
pub fn git_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, VcsError> {
    let out = spawn(repo, args, READ_CEILING, Capture::Keep)?;
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
/// again. Longer than a local read and shorter than a local write, and neither
/// comparison is arithmetic: this one waits on a network, that one on a hook.
const NETWORK_TIMEOUT: Duration = Duration::from_secs(60);

/// The same call as `git_read`, for the three commands that reach a remote.
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
/// **The deadline is its own number.** Every call in this module has one now,
/// and this one is neither of the local pair: a fetch of a large repository is
/// slow for a reason that has nothing to do with a hook or with the disk.
///
/// **Nothing comes back but a refusal.** Standard output is discarded, which
/// `bounded` records as a correctness matter rather than as tidiness, so there
/// is no answer left to hand back and this signature says so.
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

/// The environment the three remote calls run in — the local spawn's, plus the
/// four variables that make sure nothing is asked of a person who is not there.
fn spawn_network(repo: &Path, args: &[&str]) -> Result<Output, VcsError> {
    let mut command = local(repo, args);
    command
        // Nothing may be asked of a person who is not there.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("SSH_ASKPASS_REQUIRE", "never")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes");
    bounded(command, NETWORK_TIMEOUT, Capture::Discard, Feed::Nothing)
}

/// The child itself: the program, the arguments, the working directory and the
/// environment. Everything above differs only in what it makes of the exit code
/// and in how long it is willing to wait.
fn spawn(
    repo: &Path,
    args: &[&str],
    ceiling: Duration,
    stdout: Capture,
) -> Result<Output, VcsError> {
    bounded(local(repo, args), ceiling, stdout, Feed::Nothing)
}

/// The command every call in this file starts from, remote ones included.
/// Written once so the two decisions in it — the working directory, and the
/// `PATH` a person actually has — cannot come apart between the local spawn and
/// the networked one.
fn local(repo: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(GIT);
    command.args(args).current_dir(repo).env("GIT_OPTIONAL_LOCKS", "0");
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }
    command
}

/// Whether the caller has any use for what the child writes to standard output.
///
/// Not a convenience. A piped stream nobody drains fills at 64 KiB and stops
/// the child in `write` — the deadlock `bounded` is written around — so this is
/// the difference between an answer coming back and a deadline firing on a call
/// that had already done its work. `Discard` is `/dev/null` and cannot fill;
/// `Keep` costs a draining thread of its own.
enum Capture {
    Discard,
    Keep,
}

/// What the child reads on standard input.
///
/// `Nothing` is `/dev/null` and is what every call here but one wants — see
/// `bounded` for why that is a ceiling rather than a default. `Bytes` is a pipe
/// written and then closed, which is the only way to ask git a question longer
/// than a command line may be.
enum Feed<'a> {
    Nothing,
    Bytes(&'a [u8]),
}

/// Run a child with a ceiling on how long it may take, and come back with what
/// it said.
///
/// **The pipes are the whole of this function, and they are why it is not the
/// loop in `agents::oneshot::ask`.** That loop polls `try_wait` and reads both
/// pipes only once the child is gone, and its own comment says exactly what
/// makes that safe: the output is bounded — one line asked for — so neither
/// pipe can fill. Nothing here is bounded. `git pull` writes a merge diffstat
/// of one line per changed file, `git fetch --prune` a line per updated ref,
/// and `git diff HEAD` the whole patch; past the 64 KiB a pipe holds, git
/// blocks in `write`, `try_wait` never answers `Some`, and the deadline kills a
/// git that had already finished its work — under a sentence saying this app
/// stopped it.
///
/// So **every pipe that is opened is drained on a thread of its own** while the
/// wait happens. That is the precondition `oneshot`'s loop states and this one
/// cannot: with it, the child can write as much as it likes and the only thing
/// holding it up is its own work. A caller with nothing to do with standard
/// output says so (`Capture::Discard`) and it is never opened at all, which is
/// the cheaper half of the same rule rather than a different one.
///
/// **Standard input is `/dev/null` unless a caller hands over bytes**, and that
/// is a ceiling of its own: git with an *inherited* stdin waits on the editor or
/// the prompt it opened, and no terminal is attached to this process for anybody
/// to answer it in. `Feed::Bytes` is not that — it is a pipe this function owns,
/// written whole and then dropped, and the drop is the end of file git is
/// waiting for. It is written **after** both readers are running, for the reason
/// they exist: a child blocked in `write` because nobody is draining its output
/// would never get round to reading its input, and the two waits would hold each
/// other until the deadline. A failed write is dropped on the floor — a git that
/// closed the pipe early has an exit code, and the exit code is what the caller
/// reads.
///
/// **The child is stopped with its group, and then reaped** — `terminate`, which
/// is where the care is.
///
/// The readers are joined on the way out of the ordinary path and deliberately
/// **not** on the timeout path: they end by themselves when the last writer is
/// gone, which is what signalling the whole group guarantees, and a join there
/// would be this function waiting again on exactly the thing whose wait it just
/// gave up on.
///
/// **That join is unbounded, on purpose, and its premise is thinner than it was
/// when it was written.** It rests on git being the last writer to the pipe, so
/// the read ends when git does. A `pre-commit` hook that leaves a background
/// process behind breaks it — the process inherited the stderr this thread is
/// reading, git exits, and the join sits there with no ceiling over it, because
/// the ceiling above applies to the child and not to this. It is named here
/// rather than fixed: the fix is a channel with a timeout and a leaked thread,
/// which is more machinery than the case has earned so far. What it must not
/// cost is somebody's afternoon looking at the poll loop above.
fn bounded(
    mut command: Command,
    timeout: Duration,
    stdout: Capture,
    input: Feed<'_>,
) -> Result<Output, VcsError> {
    command
        .stdin(match input {
            Feed::Nothing => Stdio::null(),
            Feed::Bytes(_) => Stdio::piped(),
        })
        .stderr(Stdio::piped())
        .stdout(match stdout {
            Capture::Discard => Stdio::null(),
            Capture::Keep => Stdio::piped(),
        });
    group_of_its_own(&mut command);

    // The only command that ever reaches this in the app is git, which is what
    // makes a missing binary `NoGit` here rather than a plain `Io`.
    let mut child = command.spawn().map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound => VcsError::NoGit(GIT.to_string()),
        _ => VcsError::Io(err.to_string()),
    })?;

    let out = drain(child.stdout.take());
    let err = drain(child.stderr.take());

    // Taken rather than borrowed, so the handle is dropped at the end of this
    // block: that drop is the end of file, and without it git waits on a pipe
    // nobody will write to again.
    if let (Feed::Bytes(bytes), Some(mut pipe)) = (input, child.stdin.take()) {
        let _ = pipe.write_all(bytes);
    }

    let deadline = Instant::now() + timeout;
    // The poll starts short and backs off, which it did not have to while the
    // only calls waiting here reached a remote: a flat 100 ms is nothing out of
    // sixty seconds, and it is ten times the whole of a `git status` that
    // answers in ten. Every local call comes through here now, and the panel
    // makes several of them per refresh, so the first few naps are the ones
    // that matter and the rest only have to be cheap.
    let mut nap = Duration::from_millis(2);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                terminate(&mut child);
                return Err(VcsError::Timeout(timeout.as_secs()));
            }
            Ok(None) => {
                std::thread::sleep(nap);
                nap = (nap * 2).min(Duration::from_millis(100));
            }
            Err(err) => {
                terminate(&mut child);
                return Err(VcsError::Io(err.to_string()));
            }
        }
    };

    // A thread that panicked leaves this call with no output rather than with
    // no answer: what it was carrying is a message to read, and the exit code
    // is the fact the caller branches on.
    Ok(Output {
        status,
        stdout: out.join().unwrap_or_default(),
        stderr: err.join().unwrap_or_default(),
    })
}

/// One stream, read to the end on a thread of its own. A stream that was never
/// piped is the empty answer rather than a second shape for the caller to
/// handle.
fn drain<S: Read + Send + 'static>(pipe: Option<S>) -> JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut said = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut said);
        }
        said
    })
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

/// How long a git that has been asked to stop is given before it is killed
/// outright. It has one thing to do in that time and it is not its work — see
/// `terminate`, which also records why the whole of it is slept rather than cut
/// short the moment git is gone. Unlinking the lock files measures at well
/// under a millisecond; the rest is headroom for a machine under load.
#[cfg(unix)]
const CLEANUP_GRACE: Duration = Duration::from_secs(2);

/// End the call and whatever it started, then reap it.
///
/// **SIGTERM first, and it is not politeness.** git removes the `*.lock` files
/// it is holding from a signal handler and re-raises; SIGKILL cannot be caught,
/// so a write killed outright leaves `.git/index.lock` on the disk and every
/// later git command in that repository refuses with "Another git process seems
/// to be running" until somebody deletes the file by hand — which is a worse
/// state than the hang this ceiling exists to end, and one nothing in this app
/// would explain. Measured on git 2.34.1 against a `pre-commit` hook that
/// sleeps: SIGTERM to the group leaves no lock behind, SIGKILL leaves one.
/// SIGKILL still follows the grace, because a hook that ignores SIGTERM must
/// not turn the ceiling back into a wait with no end.
///
/// **`runs/preflight.rs::terminate` deliberately stays on a bare SIGKILL**, and
/// the difference is what the child is holding rather than an oversight in
/// either place: a declared command holds no lock file of ours and leaves
/// nothing on the disk that refuses the next call, while a stop there is a
/// person pressing stop and must not be held open for two seconds by a program
/// that has already been asked once.
///
/// **The signal goes to the group**, for the reason
/// `runs/preflight.rs::terminate` records: what is actually blocked is often a
/// child of git's — `ssh` or `git-remote-https` on a fetch, the hook itself on
/// a commit — and signalling git alone leaves it holding the connection, or the
/// tree, and the pipes.
///
/// **The grace is slept through whole, and the child is deliberately not
/// watched during it.** A version that returned as soon as `try_wait` reaped
/// git was written first and is wrong in exactly the case the SIGKILL exists
/// for: git dies promptly on SIGTERM, so that version returns while the hook
/// that *ignored* SIGTERM is still running, and the kill meant for it never
/// arrives. Waiting for the reap and then signalling is not the fix either —
/// a reaped pid can be recycled, and the group named would be somebody else's.
/// So nothing is watched, the group is signalled twice, and the cost is a flat
/// two seconds on a path that has already spent thirty or three hundred.
///
/// **The reap comes last**, for that same reason: the pid must still be the
/// child's every time this function names it. The same ordering, and the same
/// reason, as `runs/preflight.rs::terminate`.
#[cfg(unix)]
fn terminate(child: &mut Child) {
    let pid = child.id() as libc::pid_t;
    unsafe { libc::killpg(pid, libc::SIGTERM) };
    std::thread::sleep(CLEANUP_GRACE);
    unsafe { libc::killpg(pid, libc::SIGKILL) };
    let _ = child.kill();
    let _ = child.wait();
}

/// Windows has no signal to ask with: `kill` is `TerminateProcess`, which is
/// the ungraceful half and the only half there is, so a lock file left behind
/// there is a state this cannot prevent.
#[cfg(not(unix))]
fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn refusal(out: &Output) -> VcsError {
    VcsError::Git {
        // `None` is a signal: git was killed rather than exiting. `-1` says
        // so without a second field nothing else would read.
        status: out.status.code().unwrap_or(-1),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

/// What `bounded` and `terminate` have to get right, none of which shows up in
/// any outcome a person sees: a child that can write more than a pipe holds is
/// still waited on correctly, a child that outstays its ceiling is gone
/// afterwards rather than defunct, and a *write* that outstays it leaves the
/// repository usable.
///
/// Two kinds of test, and the split is deliberate. The first four drive `sh`,
/// because what is under test there is this module's handling of a process and
/// a script can be made to flood a pipe or to outlive a deadline on demand. The
/// last two drive **real git in a real repository**, because what is under test
/// there is git's own behaviour under the signal — that a hung read and a hung
/// commit both come back, and that the commit leaves no `index.lock` behind.
/// Unix only, because the assertions are about process groups, signals and
/// reaping, which is what these functions have two halves for.
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
        let out = bounded(
            script("yes 0123456789 | head -c 200000 1>&2"),
            Duration::from_secs(20),
            Capture::Discard,
            Feed::Nothing,
        )
        .expect("the child finished inside its deadline");

        assert!(out.status.success(), "the child exited of its own accord");
        assert_eq!(out.stderr.len(), 200_000, "every byte it wrote came back");
    }

    /// The same deadlock on the other pipe, which is the one a read opens.
    /// `git diff HEAD` is a patch of any size at all, so a caller that asked
    /// for standard output has to be as safe against a flood as the caller that
    /// did not.
    #[test]
    fn a_caller_that_asked_for_standard_output_gets_all_of_it() {
        let out = bounded(
            script("yes 0123456789 | head -c 200000"),
            Duration::from_secs(20),
            Capture::Keep,
            Feed::Nothing,
        )
        .expect("the child finished inside its deadline");

        assert!(out.status.success());
        assert_eq!(out.stdout.len(), 200_000, "every byte it wrote came back");
    }

    /// A caller with no use for standard output never opens it, so a child
    /// that floods it is not the second half of the deadlock above.
    #[test]
    fn standard_output_is_discarded_when_nobody_asked_for_it() {
        let out = bounded(
            script("yes 0123456789 | head -c 200000"),
            Duration::from_secs(20),
            Capture::Discard,
            Feed::Nothing,
        )
        .expect("the child finished inside its deadline");

        assert!(out.status.success());
        assert!(out.stdout.is_empty(), "nothing asked for it");
    }

    /// The two halves of `Feed::Bytes`: the child reads what it was handed, and
    /// it reaches the end of the input at all. `cat` never exits on a pipe that
    /// stays open, so a version that forgot to drop the handle would not come
    /// back with the wrong bytes — it would sit here until the deadline.
    #[test]
    fn a_child_reads_what_it_was_fed_and_then_reaches_the_end_of_it() {
        let out = bounded(
            script("cat"),
            Duration::from_secs(20),
            Capture::Keep,
            Feed::Bytes(b"one\0two\0"),
        )
        .expect("the child finished inside its deadline");

        assert!(out.status.success(), "the pipe was closed, so cat saw end of file");
        assert_eq!(out.stdout, b"one\0two\0", "every byte went through");
    }

    /// More than a pipe holds, in the other direction. The write happens after
    /// both readers are running, so a child that echoes its input can never
    /// block this thread by filling the output nobody is draining.
    #[test]
    fn a_feed_larger_than_a_pipe_does_not_deadlock_against_the_answer() {
        let fed = vec![b'x'; 200_000];

        let out = bounded(script("cat"), Duration::from_secs(20), Capture::Keep, Feed::Bytes(&fed))
            .expect("the child finished inside its deadline");

        assert!(out.status.success());
        assert_eq!(out.stdout.len(), 200_000, "every byte came back");
    }

    /// `Child` has no reaping `Drop`, so a kill with no `wait` behind it leaves
    /// a defunct process for the lifetime of the app — and this is the call the
    /// five-minute sweep makes against a remote that never answers.
    #[test]
    fn a_child_that_outstays_its_deadline_is_killed_and_reaped() {
        let dir = scratch("deadline");
        let pidfile = dir.join("pid");
        let command = script(&format!("echo $$ > {}; sleep 30", pidfile.display()));

        let err = bounded(command, Duration::from_millis(400), Capture::Discard, Feed::Nothing)
            .expect_err("the deadline fired");
        assert_eq!(err.kind(), "timeout", "this app stopped it, and git said nothing");

        let pid = pid_from(&pidfile);
        assert!(gone_within(pid, Duration::from_secs(5)), "the child was reaped, not left defunct");

        let _ = fs::remove_dir_all(&dir);
    }

    /// The process actually blocked is often a child of git's — `ssh` on a
    /// fetch, the hook on a commit — so a signal to git alone leaves it running
    /// with the connection, or the tree, and the pipes still in its hands.
    #[test]
    fn the_kill_reaches_what_the_child_started() {
        let dir = scratch("group");
        let pidfile = dir.join("pid");
        let command = script(&format!("sleep 30 & echo $! > {}; wait", pidfile.display()));

        let err = bounded(command, Duration::from_millis(400), Capture::Discard, Feed::Nothing)
            .expect_err("the deadline fired");
        assert_eq!(err.kind(), "timeout");

        let grandchild = pid_from(&pidfile);
        assert!(
            gone_within(grandchild, Duration::from_secs(5)),
            "what the child started died with it"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// The half of `terminate` the test above cannot reach: a grandchild that
    /// **ignores SIGTERM**. A plain `sleep` dies on the first signal, so the
    /// SIGKILL behind the grace is never needed there and a version that
    /// skipped it would pass — which is exactly the version that was written
    /// first. git itself dies promptly on SIGTERM, so watching the child during
    /// the grace and returning as soon as it is reaped leaves this process
    /// running with nothing left to signal it.
    ///
    /// `trap "" TERM` is inherited across fork and exec, so the `sleep` under
    /// the inner shell ignores it too and the pair survives on nothing but the
    /// kill.
    #[test]
    fn the_kill_still_reaches_a_grandchild_that_refuses_to_stop() {
        let dir = scratch("stubborn");
        let pidfile = dir.join("pid");
        let command = script(&format!(
            "sh -c 'trap \"\" TERM; echo $$ > {}; sleep 30' & wait",
            pidfile.display()
        ));

        let err = bounded(command, Duration::from_millis(400), Capture::Discard, Feed::Nothing)
            .expect_err("the deadline fired");
        assert_eq!(err.kind(), "timeout");

        let grandchild = pid_from(&pidfile);
        assert!(
            gone_within(grandchild, Duration::from_secs(5)),
            "the kill behind the grace reached what the signal could not"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    /// A repository with one commit in it and one modified file — the state
    /// both tests below need, and the smallest one git will answer questions
    /// about.
    fn repository(name: &str) -> PathBuf {
        let repo = scratch(name);
        git_write(&repo, &["init", "--quiet"]).expect("git init");
        git_write(&repo, &["config", "user.email", "test@example.com"]).expect("set the email");
        git_write(&repo, &["config", "user.name", "Test"]).expect("set the name");
        fs::write(repo.join("a.txt"), "first\n").expect("write the file");
        git_write(&repo, &["add", "a.txt"]).expect("stage it");
        git_write(&repo, &["commit", "-m", "first"]).expect("commit it");
        fs::write(repo.join("a.txt"), "second\n").expect("change the file");
        repo
    }

    /// A helper git will run and wait for, which writes down its own pid first
    /// so the test can prove the signal reached it.
    fn hangs(path: &std::path::Path, pidfile: &std::path::Path) {
        fs::write(path, format!("#!/bin/sh\necho $$ > {}\nsleep 30\n", pidfile.display()))
            .expect("write the helper");
        let mut mode = fs::metadata(path).expect("stat the helper").permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut mode, 0o755);
        fs::set_permissions(path, mode).expect("make the helper executable");
    }

    /// The ceiling the two tests below run under. **Headroom, not a
    /// measurement**: what has to happen inside it is git starting and reaching
    /// the helper it forks, which is milliseconds when the machine is idle and
    /// was measured failing at one second on a cold `cargo test` after a
    /// rebuild — roughly one run in eight, with no pidfile written at all. A
    /// flaky green gate is worse than a slow one, so do not trim this back.
    const CEILING: Duration = Duration::from_secs(5);

    /// A real read, hung the way a read is actually hung: on a helper somebody
    /// configured. A `textconv` is run once per file to turn a blob into text
    /// and git waits for it with no limit of its own, so without the ceiling
    /// here `git diff HEAD` never comes back at all.
    #[test]
    fn a_read_that_outstays_its_ceiling_comes_back_instead_of_hanging() {
        let repo = repository("read-ceiling");
        let pidfile = repo.join("helper-pid");
        let helper = repo.join("hangs.sh");
        hangs(&helper, &pidfile);
        fs::write(repo.join(".gitattributes"), "*.txt diff=slow\n").expect("write the attributes");

        let setting = format!("diff.slow.textconv={}", helper.display());
        let started = Instant::now();
        let err = spawn(&repo, &["-c", &setting, "diff", "HEAD"], CEILING, Capture::Keep)
            .expect_err("the ceiling fired");

        assert_eq!(err.kind(), "timeout", "this app stopped it, and git said nothing");
        assert!(started.elapsed() < Duration::from_secs(20), "it came back near its ceiling");
        assert!(
            gone_within(pid_from(&pidfile), Duration::from_secs(5)),
            "the helper git was waiting on died with it"
        );

        let _ = fs::remove_dir_all(&repo);
    }

    /// A real write, hung the way a write is actually hung: on a hook. The
    /// second half is what makes SIGTERM load-bearing — `git commit` holds
    /// `index.lock` while the hook runs, and a child killed outright leaves it
    /// there, after which every later git command in that repository refuses.
    #[test]
    fn a_write_that_outstays_its_ceiling_leaves_the_repository_usable() {
        let repo = repository("write-ceiling");
        let pidfile = repo.join("hook-pid");
        let hook = repo.join(".git/hooks/pre-commit");
        hangs(&hook, &pidfile);

        let err = spawn(&repo, &["commit", "-am", "hangs"], CEILING, Capture::Discard)
            .expect_err("the ceiling fired");

        assert_eq!(err.kind(), "timeout");
        assert!(
            gone_within(pid_from(&pidfile), Duration::from_secs(5)),
            "the hook died with the git that ran it"
        );
        assert!(
            !repo.join(".git/index.lock").exists(),
            "git took its own lock back off the disk before it went"
        );

        // The state that matters is not the file's absence but what it costs:
        // a repository nothing can be committed to is the failure this whole
        // grace period exists to prevent.
        fs::remove_file(&hook).expect("take the hook away");
        git_write(&repo, &["commit", "-am", "after"]).expect("the repository still takes a commit");

        let _ = fs::remove_dir_all(&repo);
    }
}
