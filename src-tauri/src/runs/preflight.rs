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
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
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

    let deadline = Instant::now() + COMMAND_TIMEOUT;
    loop {
        if cancel.asked() {
            terminate(&mut child);
            return Ok(Ran::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(Ran::Done),
            Ok(Some(status)) => {
                let output = child.wait_with_output().map(|o| tail(&o.stderr, &o.stdout)).unwrap_or_default();
                return Err(PreflightError::Command {
                    command: command.to_string(),
                    code: status.code(),
                    output,
                });
            }
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
                return Err(PreflightError::Spawn {
                    command: command.to_string(),
                    detail: err.to_string(),
                })
            }
        }
    }
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
    // signal would hold the stop open for as long as it liked.
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
