//! Noninteractive server-side command execution, with a bounded child tree.
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::context::Context;

pub const LIMIT: Duration = Duration::from_secs(60);
const STDERR_LIMIT: usize = 2048;

pub type CommandFuture<'a> = Pin<Box<dyn Future<Output = Result<(), Error>> + Send + 'a>>;

pub trait Runner: Send + Sync {
    fn run<'a>(
        &'a self,
        ctx: &'a Context,
        dir: &'a Path,
        operation: &'a str,
        command: &'a str,
    ) -> CommandFuture<'a>;
}

pub struct Shell;

impl Runner for Shell {
    fn run<'a>(
        &'a self,
        ctx: &'a Context,
        dir: &'a Path,
        operation: &'a str,
        command: &'a str,
    ) -> CommandFuture<'a> {
        Box::pin(execute(ctx, dir, operation, command))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub operation: String,
    pub code: Option<i32>,
    pub timed_out: bool,
    pub stderr: String,
}

fn brief(bytes: &[u8]) -> String {
    String::from_utf8_lossy(&bytes[..bytes.len().min(STDERR_LIMIT)])
        .trim()
        .to_owned()
}

/// A shell is needed because these are deliberately free-form project commands.
pub async fn execute(
    ctx: &Context,
    dir: &Path,
    operation: &str,
    command: &str,
) -> Result<(), Error> {
    execute_with_limit(ctx, dir, operation, command, LIMIT).await
}

async fn execute_with_limit(
    ctx: &Context,
    dir: &Path,
    operation: &str,
    command: &str,
    limit: Duration,
) -> Result<(), Error> {
    // The bundled executable may have an architecture suffix in development.
    // A shell function makes the default `bd dolt ...` commands resolve to it.
    #[cfg(unix)]
    let command = if let Some(sidecar) = ctx.sidecar() {
        let quoted = sidecar.to_string_lossy().replace('\'', "'\\''");
        format!("bd() {{ '{quoted}' \"$@\"; }}\n{command}")
    } else {
        command.to_owned()
    };
    #[cfg(windows)]
    let command = command.to_owned();
    let mut child = if cfg!(windows) {
        let mut cmd = tokio::process::Command::new("cmd.exe");
        cmd.args(["/D", "/S", "/C", &command]);
        cmd
    } else {
        let mut cmd = tokio::process::Command::new("/bin/sh");
        cmd.args(["-c", &command]);
        cmd
    };
    child
        .current_dir(dir)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if dir.join(".beads").is_dir() {
        child.env("BEADS_DIR", dir.join(".beads"));
    }
    if let Some(path) = crate::shell_env::path() {
        let combined = match ctx.sidecar_dir() {
            Some(sidecar) => format!(
                "{}{}{}",
                sidecar.display(),
                if cfg!(windows) { ";" } else { ":" },
                path
            ),
            None => path.to_owned(),
        };
        child.env("PATH", combined);
    } else if let Some(sidecar) = ctx.sidecar_dir() {
        child.env("PATH", sidecar);
    }
    #[cfg(unix)]
    unsafe {
        child.pre_exec(|| {
            if libc::setsid() == -1 {
                Err(std::io::Error::last_os_error())
            } else {
                Ok(())
            }
        });
    }
    #[cfg(windows)]
    child.creation_flags(0x0800_0000);
    let child = child.spawn().map_err(|err| Error {
        operation: operation.into(),
        code: None,
        timed_out: false,
        stderr: err.to_string(),
    })?;
    let pid = child.id();
    let result = tokio::time::timeout(limit, child.wait_with_output()).await;
    match result {
        Ok(Ok(output)) if output.status.success() => Ok(()),
        Ok(Ok(output)) => Err(Error {
            operation: operation.into(),
            code: output.status.code(),
            timed_out: false,
            stderr: brief(&output.stderr),
        }),
        Ok(Err(err)) => Err(Error {
            operation: operation.into(),
            code: None,
            timed_out: false,
            stderr: err.to_string(),
        }),
        Err(_) => {
            if let Some(pid) = pid {
                #[cfg(unix)]
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                }
                #[cfg(windows)]
                {
                    let _ = tokio::process::Command::new("taskkill")
                        .args(["/T", "/F", "/PID", &pid.to_string()])
                        .stdin(std::process::Stdio::null())
                        .output()
                        .await;
                }
            }
            Err(Error {
                operation: operation.into(),
                code: None,
                timed_out: true,
                stderr: format!("command exceeded {} seconds", limit.as_secs_f64()),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shell_preserves_quotes_cwd_and_reports_exit() {
        let dir = tempfile::tempdir().unwrap();
        let result = execute(
            &Context::detached(),
            dir.path(),
            "pull",
            "printf '%s' 'two words' > result; exit 7",
        )
        .await;
        assert_eq!(
            std::fs::read_to_string(dir.path().join("result")).unwrap(),
            "two words"
        );
        assert_eq!(result.unwrap_err().code, Some(7));
    }

    #[tokio::test]
    async fn timeout_ends_a_command_with_a_live_child() {
        let dir = tempfile::tempdir().unwrap();
        let start = std::time::Instant::now();
        let result = execute_with_limit(
            &Context::detached(),
            dir.path(),
            "pull",
            "sleep 5 & wait",
            Duration::from_millis(100),
        )
        .await;
        assert!(result.unwrap_err().timed_out);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn default_bd_name_resolves_the_bundled_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("bd-with-architecture-suffix");
        std::fs::write(&sidecar, "#!/bin/sh\nprintf '%s' \"$*\" > invoked\n").unwrap();
        std::fs::set_permissions(&sidecar, std::fs::Permissions::from_mode(0o755)).unwrap();
        let ctx = Context::new(
            Err("x".into()),
            Err("x".into()),
            Err("x".into()),
            Some(sidecar),
            std::sync::Arc::new(TestSink),
        );
        execute(&ctx, dir.path(), "pull", "bd dolt pull")
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join("invoked")).unwrap(),
            "dolt pull"
        );
    }

    #[cfg(unix)]
    struct TestSink;
    #[cfg(unix)]
    impl crate::context::Sink for TestSink {
        fn emit(&self, _: &str, _: serde_json::Value) {}
        fn push(&self, _: crate::remote::push::Message) {}
    }
}
