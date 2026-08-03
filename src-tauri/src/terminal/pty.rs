//! The only file that knows portable-pty exists. Reading is blocking, so
//! each session gets its own thread: it reads and forwards chunks into the
//! worker's shared channel. Mutable state still belongs to a single worker.

use std::io::Read;
use std::path::Path;

use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use tokio::sync::mpsc;

use super::model::{SessionId, TerminalError};

/// One piece of a session's life, arriving at the worker from the reader thread.
pub enum Chunk {
    Data(SessionId, Vec<u8>),
    /// End of stream, and nothing else: the exit code is not carried here
    /// because it is not known here — see the comment at the send site.
    Gone(SessionId),
}

/// The pure part of spawning: exactly what we run and where. Pulled out for
/// the test — actually spawning a process is not covered by tests, the same
/// as bd's calls aren't.
pub fn build_command(agent: &str, cwd: &Path) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(agent);
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd
}

pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Pty {
    pub fn spawn(
        id: SessionId,
        agent: &str,
        cwd: &Path,
        cols: u16,
        rows: u16,
        out: mpsc::UnboundedSender<Chunk>,
    ) -> Result<Self, TerminalError> {
        let system = NativePtySystem::default();
        let pair = system
            .openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })
            .map_err(|e| TerminalError::Spawn(e.to_string()))?;

        // Both come from the master, not from the child, so grab them before
        // spawning: if either fails there is no process yet to leak. Doing
        // this after spawn_command would mean an early `?` return drops the
        // already-running child without killing or reaping it.
        let writer = pair.master.take_writer().map_err(|e| TerminalError::Spawn(e.to_string()))?;
        let mut reader = pair.master.try_clone_reader().map_err(|e| TerminalError::Spawn(e.to_string()))?;

        let child = pair
            .slave
            .spawn_command(build_command(agent, cwd))
            .map_err(|e| TerminalError::Spawn(format!("{agent}: {e}")))?;

        // The slave side must be dropped here, not kept alongside the master:
        // a PTY only signals EOF to the master's reader once every open
        // handle to the slave is closed. The child duplicates the slave fd
        // for its own stdio, but if the parent also keeps one open, the
        // reader below would block forever after the child exits instead of
        // seeing end-of-stream.
        drop(pair.slave);

        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if out.send(Chunk::Data(id, buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                }
            }
            // The worker learns the exit code through its own wait: this is
            // only the end of the stream, and it arrives before the process
            // has necessarily been reaped.
            let _ = out.send(Chunk::Gone(id));
        });

        Ok(Self { master: pair.master, writer, child })
    }

    pub fn write(&mut self, bytes: &[u8]) {
        // A write error here means the pipe is already broken, i.e. the
        // agent is already gone — liveness is discovered through
        // `Chunk::Gone` and `exit_code()`, not through write results, so
        // there is nothing further to do with the error at this call site.
        use std::io::Write;
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.master.resize(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 });
    }

    /// The exit code, if the process has already died; `None` — still alive.
    pub fn exit_code(&mut self) -> Option<i32> {
        self.child.try_wait().ok().flatten().map(|status| status.exit_code() as i32)
    }

    /// The soft signal, sent before any killing: an agent given no warning
    /// flushes nothing, and this is the path that runs every time the app
    /// closes. It goes to the process *group*, not to the child: the child is
    /// a session leader (`spawn_command` calls `setsid`), so its pid is also
    /// its group id, and whatever it started — a build, a test run — is in
    /// that group unless it asked for one of its own. `kill()` reaches the
    /// agent alone and leaves those behind as orphans, which is the very
    /// thing the exit path exists to prevent.
    ///
    /// Answers whether a signal actually went out, so the caller knows
    /// whether there is anything to wait for. The caller is also the one that
    /// must check the process is still alive: `process_id` keeps answering
    /// after the child has been reaped, and a pid that has been reused names
    /// somebody else's process group.
    #[cfg(unix)]
    pub fn hangup(&mut self) -> bool {
        let Some(pid) = self.child.process_id() else { return false };
        // SIGHUP rather than SIGTERM because it is what the kernel itself
        // delivers when a terminal window closes — the case every CLI
        // already handles.
        unsafe { libc::killpg(pid as libc::pid_t, libc::SIGHUP) == 0 }
    }

    /// Windows has no signal to send here, and saying so is what lets the
    /// caller skip a grace period that could not help anyone.
    #[cfg(not(unix))]
    pub fn hangup(&mut self) -> bool {
        false
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn команда_это_агент_в_каталоге_проекта() {
        let cmd = build_command("claude", std::path::Path::new("/tmp/project"));
        assert_eq!(cmd.get_argv()[0], "claude");
        assert_eq!(cmd.get_cwd().map(|c| c.to_string_lossy().into_owned()), Some("/tmp/project".to_owned()));
    }

    #[test]
    fn терминал_объявлен_цветным() {
        // Without TERM the agent decides colours are unsupported and sends
        // plain text — and then there is nothing for screen.rs to parse and
        // nothing for the profile to recognise.
        let cmd = build_command("claude", std::path::Path::new("/tmp/project"));
        let term = cmd.iter_extra_env_as_str().find(|(k, _)| *k == "TERM").map(|(_, v)| v.to_owned());
        assert_eq!(term.as_deref(), Some("xterm-256color"));
    }
}
