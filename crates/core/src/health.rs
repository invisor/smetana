//! The server's report on its own environment: is bd there and the version this build expects, is
//! git there, and for every agent profile, is its binary on the `PATH` the agents will inherit and
//! does it answer.
//!
//! One report for two readers. `smetanad status` prints it, and a client asks for it over the
//! connection (`server_health`) and shows it where `tracker_health` is shown. It is separate from the
//! tracker's health on purpose: that one is about a folder's tracker and moves with the project, this
//! one is about the machine and is the same whichever project is open. Nothing is retried and nothing
//! is repaired; a missing tool is a line in the report, never a failure of the call that asks.
//!
//! "Answers" means `--version` exits successfully within a few seconds. It cannot say whether an agent
//! is logged in — there is no command that says so for every CLI — and the report does not pretend to.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Serialize;

use crate::context::Context;
use crate::tracker::service::EXPECTED_BD_VERSION;

/// How long a binary gets to answer `--version`. A CLI that needs longer than this to say its name is
/// not one a run can use.
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// One binary: where it was found and whether it answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tool {
    /// The name looked for.
    pub name: String,
    pub found: bool,
    pub path: Option<String>,
    /// Whether `--version` exited successfully in time. `false` when it was not found.
    pub answers: bool,
    /// The first line it printed.
    pub version: Option<String>,
    /// What is wrong, in a sentence for a person; `None` when nothing is.
    pub problem: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCheck {
    pub id: String,
    pub label: String,
    pub binary: Tool,
}

/// The whole report. `ok` is `problems.is_empty()`, and `problems` is what `smetanad status` prints
/// first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub ok: bool,
    pub problems: Vec<String>,
    pub bd: Tool,
    /// The version of bd this build of the server is written against.
    pub bd_expected: String,
    pub git: Tool,
    pub agents: Vec<AgentCheck>,
    /// The `PATH` the agents inherit, which is where their binaries were looked for.
    pub path: Option<String>,
}

/// Looks for `binary` in each directory of `path_var`.
pub fn which(binary: &str, path_var: Option<&str>) -> Option<PathBuf> {
    let paths = path_var?;
    std::env::split_paths(paths)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(binary))
        .find(|candidate| candidate.is_file())
}

/// Runs `program --version` and returns the first line of what it printed, or why it did not answer.
/// Blocking; the report runs it on a blocking thread.
fn probe(program: &Path) -> Result<String, String> {
    let mut child = Command::new(program)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("could not be started: {err}"))?;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut stdout = child.stdout.take();
    std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(out) = stdout.as_mut() {
            let _ = std::io::Read::read_to_string(out, &mut text);
        }
        let _ = tx.send(text);
    });
    match rx.recv_timeout(PROBE_TIMEOUT) {
        Ok(text) => {
            let status = child.wait().map_err(|err| err.to_string())?;
            if status.success() {
                Ok(text.lines().next().unwrap_or("").trim().to_owned())
            } else {
                Err(format!("exited with {status} when asked for its version"))
            }
        }
        Err(_) => {
            let _ = child.kill();
            let _ = child.wait();
            Err(format!("did not answer within {} seconds", PROBE_TIMEOUT.as_secs()))
        }
    }
}

/// The report for `name`, found at `path` (or not found at all).
fn check(name: &str, path: Option<PathBuf>) -> Tool {
    let Some(path) = path else {
        return Tool {
            name: name.to_owned(),
            found: false,
            path: None,
            answers: false,
            version: None,
            problem: Some(format!("`{name}` was not found")),
        };
    };
    let shown = path.to_string_lossy().into_owned();
    match probe(&path) {
        Ok(version) => Tool {
            name: name.to_owned(),
            found: true,
            path: Some(shown),
            answers: true,
            version: Some(version),
            problem: None,
        },
        Err(why) => Tool {
            name: name.to_owned(),
            found: true,
            path: Some(shown),
            answers: false,
            version: None,
            problem: Some(format!("`{name}` {why}")),
        },
    }
}

/// bd's version is the number in its output, not the whole line.
fn bd_version(line: &str) -> Option<String> {
    line.split_whitespace().find(|word| word.chars().next().is_some_and(|c| c.is_ascii_digit())).map(str::to_owned)
}

/// Builds the report. Blocking work (running each binary once) goes to a blocking thread, so the
/// future may be awaited on the async workers' runtime.
pub async fn report(ctx: &Context) -> Report {
    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || report_blocking(&ctx))
        .await
        .unwrap_or_else(|err| failed(format!("the check did not finish: {err}")))
}

fn failed(problem: String) -> Report {
    let nothing = |name: &str| Tool {
        name: name.to_owned(),
        found: false,
        path: None,
        answers: false,
        version: None,
        problem: None,
    };
    Report {
        ok: false,
        problems: vec![problem],
        bd: nothing("bd"),
        bd_expected: EXPECTED_BD_VERSION.to_owned(),
        git: nothing("git"),
        agents: Vec::new(),
        path: None,
    }
}

pub fn report_blocking(ctx: &Context) -> Report {
    let path_var = crate::shell_env::path();
    let mut problems = Vec::new();

    let mut bd = check("bd", ctx.sidecar().filter(|path| path.is_file()).map(Path::to_path_buf));
    if let Some(line) = bd.version.clone() {
        let version = bd_version(&line);
        if version.as_deref() != Some(EXPECTED_BD_VERSION) {
            bd.problem = Some(format!("bd is {line:?}; this server is written against {EXPECTED_BD_VERSION}"));
        }
    }
    let git = check("git", which(if cfg!(windows) { "git.exe" } else { "git" }, path_var));
    let agents: Vec<AgentCheck> = crate::agents::IDS
        .iter()
        .filter_map(|id| crate::agents::resolve(id))
        .map(|profile| AgentCheck {
            id: profile.id().to_owned(),
            label: profile.label().to_owned(),
            binary: check(profile.binary(), which(profile.binary(), path_var)),
        })
        .collect();

    for tool in [&bd, &git] {
        if let Some(problem) = &tool.problem {
            problems.push(problem.clone());
        }
    }
    if !agents.iter().any(|agent| agent.binary.answers) {
        problems.push("no agent CLI was found and answers: install and log in to claude or codex".to_owned());
    }
    Report {
        ok: problems.is_empty(),
        problems,
        bd,
        bd_expected: EXPECTED_BD_VERSION.to_owned(),
        git,
        agents,
        path: path_var.map(str::to_owned),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bd_prints_its_version_inside_a_sentence() {
        assert_eq!(bd_version("bd version 1.1.2 (abc123)").as_deref(), Some("1.1.2"));
        assert_eq!(bd_version("nonsense"), None);
    }

    #[test]
    fn a_missing_binary_is_a_line_in_the_report_and_not_an_error() {
        let tool = check("no-such-tool", which("no-such-tool", Some("/definitely/not/a/dir")));
        assert!(!tool.found && !tool.answers);
        assert_eq!(tool.problem.as_deref(), Some("`no-such-tool` was not found"));
    }

    #[cfg(unix)]
    #[test]
    fn a_binary_that_does_not_answer_its_version_is_reported_as_found_but_silent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mute");
        std::fs::write(&path, "#!/bin/sh\nexit 3\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let tool = check("mute", which("mute", dir.path().to_str()));
        assert!(tool.found && !tool.answers);
        assert!(tool.problem.unwrap().contains("exited with"));
    }

    #[cfg(unix)]
    #[test]
    fn a_binary_that_answers_has_its_first_line_as_the_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("talker");
        std::fs::write(&path, "#!/bin/sh\necho 'talker 4.5.6'\necho second\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let tool = check("talker", which("talker", dir.path().to_str()));
        assert!(tool.answers);
        assert_eq!(tool.version.as_deref(), Some("talker 4.5.6"));
        assert_eq!(tool.problem, None);
    }
}
