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
use std::process::{Command, Stdio};
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
        HealthCheck::Url { url } => Command::new("curl")
            .args(["-sfS", "-o", "/dev/null", "--max-time", "5", url])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success()),
        HealthCheck::Tcp { tcp } => tcp_open(*tcp),
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
pub fn run_command(root: &Path, command: &str) -> Result<(), PreflightError> {
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
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => {
                let output = child.wait_with_output().map(|o| tail(&o.stderr, &o.stdout)).unwrap_or_default();
                return Err(PreflightError::Command {
                    command: command.to_string(),
                    code: status.code(),
                    output,
                });
            }
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
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

#[cfg(unix)]
fn shell(root: &Path, command: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command).current_dir(root);
    cmd
}

#[cfg(windows)]
fn shell(root: &Path, command: &str) -> Command {
    let mut cmd = Command::new("cmd");
    cmd.arg("/C").arg(command).current_dir(root);
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
        let err = run_command(&root, "definitely-not-a-real-command-8a3f").expect_err("no such command");
        // A shell reports "not found" as a non-zero exit of the shell itself,
        // so this is a Command failure carrying the shell's own message.
        assert!(matches!(err, PreflightError::Command { .. }), "{err:?}");
        assert!(err.to_string().contains("definitely-not-a-real-command-8a3f"));
    }

    #[test]
    fn a_command_that_succeeds_returns_nothing_to_report() {
        assert_eq!(run_command(&std::env::temp_dir(), "exit 0"), Ok(()));
    }

    #[test]
    fn a_failing_command_carries_its_own_output() {
        let err = run_command(&std::env::temp_dir(), "echo the-reason >&2; exit 3")
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
    fn nothing_is_listening_on_a_port_nothing_bound() {
        // Port 1 needs root to bind, so on a developer's machine and in CI
        // alike this is reliably closed.
        assert!(!tcp_open(1));
    }
}
