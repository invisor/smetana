//! Bringing the project up before the first batch.
//!
//! The port of `holiday-curb`'s `preflight.mjs`, minus everything in it that
//! was a fact about that project — the Node major it demanded, its containers,
//! its lockfile stamp. What is left is the shape: run what the config declares,
//! then wait until what the config declares healthy answers.
//!
//! This runs **once**, before the first batch, not per batch. A run against
//! infrastructure that never came up produces red gates nobody caused, and an
//! agent that then parks every task for a reason that is not about the code.
//!
//! The split here is the same one the source made and for the same reason: the
//! decision is pure and carries the tests, the waiting is I/O and does not.

use std::io;
use std::io::Read;
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::runs::config::HealthCheck;

/// How long a health check may take to come good, and how often to ask. Both
/// are the source's numbers: two minutes is a cold dependency install plus a
/// compile, and asking more often than every couple of seconds only spends the
/// machine that is trying to start.
pub const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(2);

/// A declared command may take as long as it likes to *start* things, but not
/// forever: a command that blocks is indistinguishable from one that hung, and
/// the run would sit there with nothing on screen to say why.
pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(600);

/// How long the two readers are given once the child itself has already
/// exited.
///
/// Not a work budget, and nothing waits this out in the ordinary case: the
/// child is the last writer, both pipes are at EOF the moment it goes, and the
/// wait is zero. It is a grace for the one case where something the command
/// left running still holds the pipe it inherited — a second is far more than a
/// reader already at EOF needs, and far less than a run notices.
const OUTPUT_GRACE: Duration = Duration::from_secs(1);

/// The stop button, reaching a command that is already running.
///
/// A flag rather than a channel because the thread that owns the child is a
/// blocking one: it already asks every 200 ms whether the child is done, and
/// this rides on that same look rather than adding anything to wait on. Cloned
/// across the two sides — the caller holds one and the thread holds the other.
#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// Ask the command in flight to stop. It is asked once and never unasked:
    /// a run that was stopped stays stopped.
    pub fn ask(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    pub fn asked(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

/// How a declared command finished.
///
/// `Cancelled` is neither a success nor a failure, and that is why it is not an
/// `Err`: nothing about the project was learned, nothing is wrong with it, and
/// the run that asked for it is already over. Reporting it as a preflight
/// failure would put "the project would not come up" in the bar over a stop
/// somebody pressed themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ran {
    Done,
    Cancelled,
}

/// What to do after one look at a health check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Poll {
    Healthy,
    /// The budget is spent.
    GiveUp,
    Again,
}

/// The pure half. **The check is read before the clock**, which is why a
/// zero-length budget still asks once: something already up must not be
/// reported as never having come up, and on a warm machine that is the common
/// case rather than an edge one.
pub fn poll_step(healthy: bool, elapsed: Duration, timeout: Duration) -> Poll {
    if healthy {
        Poll::Healthy
    } else if elapsed >= timeout {
        Poll::GiveUp
    } else {
        Poll::Again
    }
}

/// What went wrong, in words a person can act on. Every variant names the thing
/// that failed rather than saying the preflight failed, because the whole point
/// of the phase is to say which piece is not there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreflightError {
    /// A declared command exited non-zero. Carries the tail of what it printed.
    Command { command: String, code: Option<i32>, output: String },
    /// A command could not be started at all — not on PATH, not executable.
    Spawn { command: String, detail: String },
    /// It never answered inside the budget.
    Unhealthy { check: String },
}

impl std::fmt::Display for PreflightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreflightError::Command { command, code, output } => {
                let code = code.map_or_else(|| "a signal".to_string(), |c| c.to_string());
                write!(f, "`{command}` exited {code}: {output}")
            }
            PreflightError::Spawn { command, detail } => write!(f, "could not run `{command}`: {detail}"),
            PreflightError::Unhealthy { check } => {
                write!(f, "{check} never answered within {}s", HEALTH_TIMEOUT.as_secs())
            }
        }
    }
}

/// How a health check reads in a message. Kept beside the check itself so the
/// error names what the config named, not an index into a list.
pub fn describe(check: &HealthCheck) -> String {
    match check {
        HealthCheck::Url { url } => url.clone(),
        HealthCheck::Tcp { tcp } => format!("port {tcp}"),
    }
}

/// Is it up?
///
/// A URL goes through `curl` rather than an HTTP client. One GET is not worth a
/// dependency the size of a TLS stack and an async runtime's worth of transitive
/// crates, and anything that has a health URL already assumes something can
/// fetch it. A machine with no `curl` reads as not healthy, which surfaces as a
/// named preflight failure rather than as silence — the same trade `library.rs`
/// makes when a file it cannot read answers "no".
pub fn is_healthy(check: &HealthCheck) -> bool {
    match check {
        HealthCheck::Url { url } => curl(url).status().is_ok_and(|s| s.success()),
        HealthCheck::Tcp { tcp } => tcp_open(*tcp),
    }
}

/// The probe itself, built rather than run, so the `PATH` it is given is a
/// thing a test can read. `curl` lives in `/usr/bin` on macOS and is therefore
/// found either way today — it gets the login shell's `PATH` regardless,
/// because the alternative is a rule that holds in one of the two places it is
/// written and nobody finds out which.
fn curl(url: &str) -> Command {
    let mut cmd = Command::new("curl");
    cmd.args(["-sfS", "-o", "/dev/null", "--max-time", "5", url])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    with_login_path(&mut cmd);
    cmd
}

/// The `PATH` anything the preflight starts is given: the login shell's, for
/// the reason `terminal/pty.rs` records at its own copy of this. A bundled app
/// started from Finder inherits launchd's, which holds nothing a person
/// installed — so `docker`, `mise`, `nvm`'s shims and everything else a
/// declared command reaches for are simply not there, and the phase whose whole
/// job is to say which piece is missing reports the tool instead of the piece.
/// `shell_env::path` already falls back to the inherited value, so this is
/// never a narrowing.
fn with_login_path(cmd: &mut Command) {
    if let Some(path) = crate::shell_env::path() {
        cmd.env("PATH", path);
    }
}

fn tcp_open(port: u16) -> bool {
    // Resolved rather than constructed, so a machine that maps localhost to
    // IPv6 only is not reported as having nothing listening.
    let Ok(addresses) = ("localhost", port).to_socket_addrs() else {
        return false;
    };
    let addresses: Vec<SocketAddr> = addresses.collect();
    addresses
        .iter()
        .any(|address| TcpStream::connect_timeout(address, Duration::from_secs(2)).is_ok())
}

/// Run one declared command in the project root and wait for it.
///
/// Through a shell, deliberately: what the config holds is a command line a
/// person wrote and tested in their own terminal (`docker compose up -d`), and
/// splitting it on spaces ourselves would break the first one that carries a
/// quoted argument or a pipe. The same reasoning applies to the gates, which
/// the agent runs the same way.
///
/// `cancel` is read on the same 200 ms poll that asks whether the child is
/// done, so a stop pressed during the preflight takes effect within one poll
/// rather than within `COMMAND_TIMEOUT` (smetana-16w). The command is killed
/// where it stands: a declared command brings infrastructure up and is expected
/// to be run again from the top next time, which is not the merge-in-progress
/// the cooperative stop between batches exists to protect.
///
/// **Both pipes are drained while the wait happens, and that is the whole of
/// why this is not a loop over `try_wait` alone** (smetana-5fj). A pipe holds
/// 64 KiB; a child that writes more of it than that and is not being read
/// blocks in `write` for good, `try_wait` never answers `Some`, and the
/// deadline then reports "still running after 600s" about a command that did
/// its work in milliseconds. `npm install` on a cold tree is well past 64 KiB,
/// and it is the first declared command of this project. `vcs/run.rs::bounded`
/// is the same rule against the same failure and differs in one thing only: no
/// caller there has ever read standard output, so it goes to `/dev/null`, while
/// what a person is shown here is `tail` over **both** streams — so both are
/// read, on a thread apiece, and neither may be discarded.
pub fn run_command(root: &Path, command: &str, cancel: &Cancel) -> Result<Ran, PreflightError> {
    // Asked before anything is started, so a stop that lands between two
    // declared commands does not start the next one at all.
    if cancel.asked() {
        return Ok(Ran::Cancelled);
    }
    let mut child = shell(root, command)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err: io::Error| PreflightError::Spawn {
            command: command.to_string(),
            detail: err.to_string(),
        })?;

    // Started before the first look at the child, so nothing it writes has to
    // wait for one.
    let reading_stdout = drain(child.stdout.take());
    let reading_stderr = drain(child.stderr.take());

    let deadline = Instant::now() + COMMAND_TIMEOUT;
    let status = loop {
        if cancel.asked() {
            terminate(&mut child);
            return Ok(Ran::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                terminate(&mut child);
                return Err(PreflightError::Command {
                    command: command.to_string(),
                    code: None,
                    output: format!("still running after {}s", COMMAND_TIMEOUT.as_secs()),
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(err) => {
                // The one arm that leaves both the child and its readers where
                // they are. `try_wait` fails when the kernel will not say what
                // became of this pid — `ECHILD` if something else reaped it —
                // and a pid that may already have been reaped is exactly the
                // pid `terminate` may not name, since the group it stands for
                // could be somebody else's by now. A reader left holding a
                // buffer nobody reads is the smaller of those two.
                return Err(PreflightError::Spawn {
                    command: command.to_string(),
                    detail: err.to_string(),
                })
            }
        }
    };

    // The ordinary path, and the only one that waits on the readers at all —
    // the same division, and the same reason, as `vcs/run.rs::bounded`. The
    // child has exited, so in the ordinary case the write ends are closed, both
    // readers are at EOF already and this costs nothing; the two give-up paths
    // above wait on neither, because a reader ends only once the last writer
    // has, which there is what `terminate`'s kill of the whole process group
    // brings about rather than anything waiting here could.
    //
    // **The wait is bounded, and that is the one place this parts company with
    // `bounded`, which joins outright.** The child is not always the last
    // writer: a declared command that leaves something running behind it
    // (`something &`, rather than `docker compose up -d`, whose containers are
    // the daemon's children and hold nothing of ours) leaves that descendant
    // holding the pipe it inherited, and nothing here can close a write end it
    // does not own. An outright join there never returns — past
    // `COMMAND_TIMEOUT`, past the next look at `cancel`, with the stop button
    // dead and `service.rs`'s "bounded by one poll of `run_command`" no longer
    // true of the phase it is written about. So the readers are given
    // `OUTPUT_GRACE` and then abandoned, exactly as the give-up paths abandon
    // them and for the reason written beside them. What that costs is a
    // truncated or empty tail in that one case; what it buys is a call that
    // always comes back. `bounded` still has the unbounded form, deliberately
    // and not yet: the difference between the two files is this paragraph.
    //
    // A reader that panicked drops its sender, which disconnects the channel
    // and lands in the same fallback: the exit code is the fact the caller
    // branches on, and the tail is a message to read.
    //
    // One deadline across both, rather than one apiece: what is granted is a
    // moment for the pipes to close, and two graces in a row would make the
    // ceiling twice the number the constant states.
    let grace = Instant::now() + OUTPUT_GRACE;
    let stdout = collect(&reading_stdout, grace);
    let stderr = collect(&reading_stderr, grace);

    if status.success() {
        return Ok(Ran::Done);
    }
    Err(PreflightError::Command {
        command: command.to_string(),
        code: status.code(),
        output: tail(&stderr, &stdout),
    })
}

/// One reader's answer, or nothing if it has not come by the deadline the
/// caller set. `Duration::ZERO` is not a special case for `recv_timeout`: it
/// looks once and gives up, which is exactly what a spent grace means.
fn collect(reader: &mpsc::Receiver<Vec<u8>>, by: Instant) -> Vec<u8> {
    reader.recv_timeout(by.saturating_duration_since(Instant::now())).unwrap_or_default()
}

/// One pipe, read to the end on a thread of its own, so the child is never
/// waiting on this one to catch up. Nothing bounds what it collects: `tail`
/// keeps ten lines of it, but a command is entitled to write a build log first
/// and holding that in memory for the length of one declared command is the
/// cheaper half of the trade against reading it in pieces here.
///
/// A channel rather than a `JoinHandle`, for one reason: a join cannot be given
/// up on and a `recv_timeout` can. Which matters only in the case the caller's
/// comment describes — a descendant still holding the write end — and matters
/// completely there, since that is the difference between a call that returns
/// with less to show and one that does not return.
fn drain<R: Read + Send + 'static>(pipe: Option<R>) -> mpsc::Receiver<Vec<u8>> {
    let (reader, read) = mpsc::channel();
    std::thread::spawn(move || {
        let mut said = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut said);
        }
        // Nobody left to hand it to is an ordinary end, not a failure: the
        // caller gave up on this reader and said so by dropping its receiver.
        let _ = reader.send(said);
    });
    read
}

/// End the command and whatever it started, then reap it.
///
/// The signal goes to the process *group* and not to the child, for the reason
/// `terminal/pty.rs::hangup` records: the child is a shell, and the work is in
/// the processes it started — `npm install` is node and everything node forks.
/// Killing the shell alone leaves those running with nobody waiting on them,
/// which is the orphan this is here to avoid. `shell` asks for a group of the
/// child's own, so the group id is the child's pid and the signal reaches
/// nothing of ours.
///
/// It is safe to name that group by pid only because the child has not been
/// reaped yet — a reaped pid can be reused and would name somebody else's
/// group, which is why the `wait` comes after.
#[cfg(unix)]
fn terminate(child: &mut Child) {
    // SIGKILL rather than the SIGHUP a session gets: nothing here has a screen
    // to restore or a buffer to flush, and a declared command that ignored the
    // signal would hold the stop open for as long as it liked. `vcs/run.rs`'s
    // twin of this function deliberately asks with SIGTERM first, and the
    // difference is what its child is holding: git unlinks its `*.lock` files
    // from a signal handler, so a git killed outright leaves a repository that
    // refuses every later command. Nothing here holds a lock of ours.
    unsafe { libc::killpg(child.id() as libc::pid_t, libc::SIGKILL) };
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(not(unix))]
fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(unix)]
fn shell(root: &Path, command: &str) -> Command {
    use std::os::unix::process::CommandExt;

    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command).current_dir(root);
    with_login_path(&mut cmd);
    // A group of its own, which is what makes `terminate` able to name the
    // whole tree the command started. Standard input goes with it: a command
    // in a background group that reads the terminal is stopped by the kernel
    // (SIGTTIN) and would then sit there until `COMMAND_TIMEOUT`, and a
    // declared command has nobody to type at it either way.
    cmd.process_group(0).stdin(Stdio::null());
    cmd
}

#[cfg(windows)]
fn shell(root: &Path, command: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(command).current_dir(root).stdin(Stdio::null());
    with_login_path(&mut cmd);
    cmd
}

/// The last few lines of what a failed command said. Standard error first,
/// since that is where a failure usually explains itself; the whole of both
/// would put a build log into an error message.
fn tail(stderr: &[u8], stdout: &[u8]) -> String {
    let text = if stderr.is_empty() { stdout } else { stderr };
    let text = String::from_utf8_lossy(text);
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(10)..].join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn something_already_up_is_never_reported_as_never_having_started() {
        // The check is read before the clock. On a warm machine this is the
        // common path, not an edge case.
        assert_eq!(poll_step(true, Duration::from_secs(999), Duration::ZERO), Poll::Healthy);
    }

    #[test]
    fn a_spent_budget_gives_up_and_an_unspent_one_asks_again() {
        assert_eq!(poll_step(false, Duration::from_secs(120), HEALTH_TIMEOUT), Poll::GiveUp);
        assert_eq!(poll_step(false, Duration::from_secs(119), HEALTH_TIMEOUT), Poll::Again);
    }

    #[test]
    fn the_budget_is_spent_at_the_boundary_not_after_it() {
        // Off by one here costs one extra poll interval on every failure, which
        // is the difference between a two-minute wait and a two-minute-two.
        assert_eq!(poll_step(false, HEALTH_TIMEOUT, HEALTH_TIMEOUT), Poll::GiveUp);
    }

    #[test]
    fn a_check_names_itself_the_way_the_config_wrote_it() {
        assert_eq!(
            describe(&HealthCheck::Url { url: "http://localhost:4001/health".into() }),
            "http://localhost:4001/health"
        );
        assert_eq!(describe(&HealthCheck::Tcp { tcp: 5433 }), "port 5433");
    }

    #[test]
    fn an_error_says_which_piece_is_missing() {
        // "preflight failed" sends somebody looking; naming the command does not.
        let err = PreflightError::Command {
            command: "docker compose up -d".into(),
            code: Some(1),
            output: "no such service: worker".into(),
        };
        let text = err.to_string();
        assert!(text.contains("docker compose up -d"), "{text}");
        assert!(text.contains("no such service"), "{text}");

        let err = PreflightError::Unhealthy { check: "port 5433".into() };
        assert!(err.to_string().contains("port 5433"));
    }

    #[test]
    fn a_command_that_cannot_start_is_told_apart_from_one_that_failed() {
        // Different fixes: one is a missing tool, the other is a broken project.
        let root = std::env::temp_dir();
        let err = run_command(&root, "definitely-not-a-real-command-8a3f", &Cancel::default())
            .expect_err("no such command");
        // A shell reports "not found" as a non-zero exit of the shell itself,
        // so this is a Command failure carrying the shell's own message.
        assert!(matches!(err, PreflightError::Command { .. }), "{err:?}");
        assert!(err.to_string().contains("definitely-not-a-real-command-8a3f"));
    }

    /// The bug this whole `PATH` line exists for. A bundled app on macOS is
    /// handed launchd's environment, which on a stock machine is
    /// `/usr/bin:/bin:/usr/sbin:/sbin` — so `docker compose -f … up -d`, the
    /// very first declared command of the project this was ported from, exits
    /// 127 against infrastructure that is up and answering, because Docker
    /// installs itself into `/usr/local/bin`. Invisible in development, where
    /// the binary is started from a terminal that already has the person's own
    /// `PATH`, which is why it is pinned here rather than left to be noticed.
    #[test]
    fn a_declared_command_and_a_probe_both_get_the_login_shells_path() {
        use std::ffi::OsStr;

        let expected = crate::shell_env::path().map(OsStr::new);
        for cmd in [shell(Path::new("/"), "docker compose up -d"), curl("http://localhost:4001/health")] {
            let given =
                cmd.get_envs().find(|(key, _)| *key == OsStr::new("PATH")).and_then(|(_, value)| value);
            assert_eq!(given, expected, "{:?}", cmd.get_program());
        }
    }

    #[test]
    fn a_command_that_succeeds_returns_nothing_to_report() {
        assert_eq!(run_command(&std::env::temp_dir(), "exit 0", &Cancel::default()), Ok(Ran::Done));
    }

    #[test]
    fn a_failing_command_carries_its_own_output() {
        let err = run_command(&std::env::temp_dir(), "echo the-reason >&2; exit 3", &Cancel::default())
            .expect_err("exit 3 is a failure");
        match err {
            PreflightError::Command { code, output, .. } => {
                assert_eq!(code, Some(3));
                assert!(output.contains("the-reason"), "{output}");
            }
            other => panic!("expected a command failure, got {other:?}"),
        }
    }

    /// The regression the two reader threads exist for (smetana-5fj). A pipe
    /// holds 64 KiB; a command that writes more of it than that and is not
    /// being read blocks in `write` for good, `try_wait` never answers `Some`,
    /// and the run fails at `COMMAND_TIMEOUT` with "still running after 600s"
    /// about a command that did its work in milliseconds. `npm install` on a
    /// cold tree writes well past 64 KiB, and it is the first declared command
    /// of this very project.
    #[test]
    fn a_command_that_writes_more_than_a_pipe_holds_still_finishes() {
        // Both pipes, because either one of them is enough to wedge the wait
        // and only one of the two can be answered by discarding it.
        for command in ["yes 0123456789 | head -c 200000", "yes 0123456789 | head -c 200000 1>&2"] {
            assert_eq!(within(Duration::from_secs(30), command), Ok(Ran::Done), "{command}");
        }
    }

    /// What a person is shown when a declared command fails is built from both
    /// pipes, which is why neither of them may be sent to `/dev/null` the way
    /// `vcs/run.rs::bounded` sends standard output. Standard error wins when
    /// both said something — `tail`'s own rule, unchanged — and a failure that
    /// only ever wrote to standard output is still explained, which is the half
    /// that proves standard output is read rather than discarded.
    #[test]
    fn a_failing_commands_tail_carries_what_both_pipes_said() {
        let err = within(Duration::from_secs(30), "echo on-stdout; echo on-stderr >&2; exit 1")
            .expect_err("exit 1 is a failure");
        match err {
            PreflightError::Command { code, output, .. } => {
                assert_eq!(code, Some(1));
                assert!(output.contains("on-stderr"), "{output}");
                // `tail` picks one stream, it does not join the two — assert
                // the half that is a rule, or a `tail` rewritten to
                // concatenate would pass this test unchanged.
                assert!(!output.contains("on-stdout"), "{output}");
            }
            other => panic!("expected a command failure, got {other:?}"),
        }

        let err = within(Duration::from_secs(30), "echo only-on-stdout; exit 1")
            .expect_err("exit 1 is a failure");
        match err {
            PreflightError::Command { output, .. } => {
                assert!(output.contains("only-on-stdout"), "{output}");
            }
            other => panic!("expected a command failure, got {other:?}"),
        }
    }

    /// A command that floods a pipe **and** fails: the tail is the last ten
    /// lines of it and not the 200 KB that came before, and it arrives at all,
    /// which the deadlocked version could not manage either.
    #[test]
    fn a_flood_that_then_fails_still_comes_back_as_a_tail() {
        let err = within(Duration::from_secs(30), "yes the-flood | head -c 200000 >&2; echo the-reason >&2; exit 2")
            .expect_err("exit 2 is a failure");
        match err {
            PreflightError::Command { code, output, .. } => {
                assert_eq!(code, Some(2));
                assert!(output.contains("the-reason"), "{output}");
                assert!(output.len() < 4096, "the tail is a tail, not a build log: {} bytes", output.len());
            }
            other => panic!("expected a command failure, got {other:?}"),
        }
    }

    /// The regression the bounded wait exists for, and the one case where the
    /// child is not the last writer: a command that exits zero having left
    /// something running behind it, holding the standard output it inherited.
    /// An outright join on the readers would wait for the descendant instead of
    /// for the command — past `COMMAND_TIMEOUT`, past the next look at
    /// `cancel`, breaking what `service.rs` states about `bring_up`: bounded by
    /// one poll of `run_command`, not by the command. So what is asserted is
    /// the clock and not only the answer.
    #[cfg(unix)]
    #[test]
    fn a_command_that_leaves_something_holding_the_pipe_comes_back_anyway() {
        // Warmed before the clock starts, because the first call anywhere in
        // the process runs an interactive login shell and takes about a second
        // of its own — which is not what this test is timing.
        let _ = crate::shell_env::path();

        let started = Instant::now();
        // Five seconds of descendant against a one-second grace: long enough
        // that waiting it out is unmistakable on the clock, short enough that
        // nothing is left behind worth the name.
        let ran = within(Duration::from_secs(30), "sleep 5 & exit 0");
        let waited = started.elapsed();

        assert_eq!(ran, Ok(Ran::Done));
        assert!(
            waited < OUTPUT_GRACE + Duration::from_secs(2),
            "it waited for what the command left running rather than for the command: {waited:?}"
        );
    }

    /// Run a declared command on a thread of the test's own, so a `run_command`
    /// that waits on a child whose pipes nobody is reading fails this in
    /// seconds rather than sitting on `COMMAND_TIMEOUT` for ten minutes. The
    /// thread is abandoned on failure deliberately: nothing can interrupt a
    /// wait that is already blocked, and the harness is on its way out anyway.
    fn within(patience: Duration, command: &str) -> Result<Ran, PreflightError> {
        let (answer, answered) = std::sync::mpsc::channel();
        let owned = command.to_string();
        std::thread::spawn(move || {
            let _ = answer.send(run_command(&std::env::temp_dir(), &owned, &Cancel::default()));
        });
        answered.recv_timeout(patience).unwrap_or_else(|_| {
            panic!("`{command}` never came back: nothing was reading the pipe it filled")
        })
    }

    #[test]
    fn a_stop_between_commands_does_not_start_the_next_one() {
        // The cheap half of smetana-16w: an already-asked cancel spends
        // nothing at all, so a preflight with five declared commands does not
        // run four of them after the button was pressed.
        let dir = scratch("between");
        let marker = dir.join("ran");
        let cancel = Cancel::default();
        cancel.ask();

        let command = format!("touch {}", marker.display());
        assert_eq!(run_command(&dir, &command, &cancel), Ok(Ran::Cancelled));
        assert!(!marker.exists(), "the command was started after the stop");
    }

    #[cfg(unix)]
    #[test]
    fn a_cancelled_command_ends_now_and_takes_what_it_started_with_it() {
        // The expensive half, and the whole acceptance criterion: a command
        // with minutes left in it ends when the stop arrives rather than when
        // the command does. The process it started goes with it — a signal to
        // the shell alone would leave `npm install`'s node running with nobody
        // waiting on it, which is the orphan the group kill exists to prevent.
        let dir = scratch("cancel");
        let pid_file = dir.join("grandchild.pid");
        let cancel = Cancel::default();

        let asking = cancel.clone();
        let watched = pid_file.clone();
        let waiter = std::thread::spawn(move || {
            // Asked only once the grandchild is up, so there is something for
            // the kill to miss if it names the wrong process.
            for _ in 0..100 {
                if std::fs::read_to_string(&watched).is_ok_and(|text| !text.trim().is_empty()) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            asking.ask();
        });

        let started = Instant::now();
        let command = format!("sleep 60 & echo $! > {}; wait", pid_file.display());
        let ran = run_command(&dir, &command, &cancel).expect("a stop is not a failure");
        waiter.join().expect("the thread that asks");

        assert_eq!(ran, Ran::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(30), "it waited the command out");

        let pid: i32 = std::fs::read_to_string(&pid_file)
            .expect("the pid the shell wrote")
            .trim()
            .parse()
            .expect("a pid");
        // Not our child any more once the shell is gone, so this asks the
        // kernel rather than waiting: 0 is the signal that would be sent.
        let gone = (0..100).any(|_| {
            if unsafe { libc::kill(pid, 0) } != 0 {
                return true;
            }
            std::thread::sleep(Duration::from_millis(50));
            false
        });
        assert!(gone, "the process the command started outlived it");
    }

    /// A directory of this test's own under the system temp dir. Named after
    /// the process as well as the case, so two `cargo test` runs at once do not
    /// read each other's files.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-preflight-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        dir
    }

    #[test]
    fn nothing_is_listening_on_a_port_nothing_bound() {
        // Port 1 needs root to bind, so on a developer's machine and in CI
        // alike this is reliably closed.
        assert!(!tcp_open(1));
    }
}
