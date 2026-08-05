//! The only file that knows portable-pty exists. Reading is blocking, so
//! each session gets its own thread: it reads and forwards chunks into the
//! worker's shared channel. Mutable state still belongs to a single worker.

use std::ffi::{OsStr, OsString};
use std::io::Read;
use std::path::{Path, PathBuf};

use portable_pty::{Child, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use tokio::sync::mpsc;

use super::model::{SessionId, TerminalError};
use crate::agents::{Intent, Launch};

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
///
/// What to run and what to say to it belongs to the agent's profile
/// (`agents/`); the working directory and the terminal type belong to every
/// agent alike and stay here.
///
/// An opening prompt travels as the agent's positional argument rather than as
/// bytes written into the PTY afterwards. The agent takes a moment to come up,
/// and anything sent into an input that is not reading yet is simply lost —
/// there is no acknowledgement to wait for and no way to tell that it went. As
/// an argument it is handed over by the OS before the process starts.
pub fn build_command(launch: &Launch) -> CommandBuilder {
    let mut cmd = launch.profile.command(launch);
    cmd.cwd(&launch.cwd);
    cmd.env("TERM", "xterm-256color");
    // The environment half of running without a person. The argument half is
    // applied by the profile itself, because it has to go in front of the
    // positional prompt and `CommandBuilder` only appends; the environment has
    // no order and belongs here, beside the other two variables every agent
    // gets. Only a `Run` has a mode, and only a `Run` gets any of this.
    if let Intent::Run { settings } = &launch.intent {
        for (key, value) in launch.profile.autonomy(settings.mode).env {
            cmd.env(key, value);
        }
    }
    // What the agent's own `PATH` is built on: the login shell's, because a
    // bundled app inherits launchd's, which holds nothing a person installed —
    // an agent started with that finds neither `git` nor `node` nor the helpers
    // it shells out to. `crate::shell_env::path` already falls back to the
    // inherited value, and `cmd.get_env` behind it covers the one case it
    // cannot answer: `CommandBuilder::new` has snapshotted the parent's
    // environment, so this is the value the child would otherwise have had.
    let base = crate::shell_env::path()
        .map(OsString::from)
        .or_else(|| cmd.get_env("PATH").map(OsStr::to_owned));
    // Filing a task means the agent running `bd`, and this app's bd is a
    // sidecar inside the bundle: on a machine that never installed one there is
    // nothing on `PATH` to find, and "command not found" is now the whole
    // feature failing. One directory in front of that base, and nothing else
    // about the environment is touched.
    let path = match sidecar_dir() {
        Some(dir) => Some(path_with(&dir, base.as_deref())),
        None => base,
    };
    if let Some(path) = path {
        cmd.env("PATH", path);
    }
    cmd
}

/// The directory the bundled `bd` sits in — `None` only when the running
/// executable has no discoverable location, in which case the agent is left
/// with whatever `PATH` it inherited, which is where it was before.
///
/// `tracker/bd.rs` reaches bd through `app.shell().sidecar("bd")`, and that is
/// `dirname(current_exe())` joined with the name and nothing else —
/// `relative_command_path` in tauri-plugin-shell is those two lines. Deriving
/// the directory the same way is what makes it the same directory by
/// construction rather than by a rule copied out of that crate: in a bundle it
/// is `smetana.app/Contents/MacOS`, beside the app executable, where the
/// bundler drops the external binary with its target triple stripped; under
/// `npm run tauri dev` it is `src-tauri/target/debug`, where the Tauri CLI
/// drops the same file.
///
/// `tauri::utils::platform::current_exe` rather than `std::env::current_exe`,
/// again because it is what the plugin calls: it answers with the path captured
/// at start-up, before anything could have moved, and it resolves an AppImage's
/// mount point back to the image.
fn sidecar_dir() -> Option<PathBuf> {
    let exe = tauri::utils::platform::current_exe().ok()?;
    let dir = exe.parent()?;
    // A test binary lives one level below the sidecar, in `target/debug/deps`.
    // The plugin steps up out of that directory; not doing the same here would
    // make the two disagree in precisely the case a test can observe.
    Some(if dir.ends_with("deps") {
        dir.parent().unwrap_or(dir).to_path_buf()
    } else {
        dir.to_path_buf()
    })
}

/// `PATH` with `dir` at the front. Pure, and the part of this worth a test.
///
/// Prepended and never appended: the app pins a bd version and checks it
/// (`EXPECTED_BD_VERSION` in `tracker/service.rs`), so an agent that found some
/// other bd first would be writing to the board through a version that
/// handshake never verified.
///
/// A `join_paths` that fails leaves the inherited value alone. It can only fail
/// on a directory that itself contains the separator, and of the two ways to be
/// wrong there, an unreachable bd costs this one feature while a mangled `PATH`
/// costs the agent everything else it runs.
pub fn path_with(dir: &Path, inherited: Option<&OsStr>) -> OsString {
    let rest = inherited.into_iter().flat_map(std::env::split_paths);
    std::env::join_paths(std::iter::once(dir.to_path_buf()).chain(rest))
        .unwrap_or_else(|_| inherited.unwrap_or(OsStr::new("")).to_owned())
}

pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Pty {
    pub fn spawn(
        id: SessionId,
        launch: &Launch,
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
            .spawn_command(build_command(launch))
            .map_err(|e| TerminalError::Spawn(format!("{}: {e}", launch.profile.binary())))?;

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
    use crate::agents::library::Skills;
    use crate::agents::{self, Intent, Launch};

    fn launch(id: &str) -> Launch {
        Launch {
            profile: agents::resolve(id).unwrap(),
            cwd: std::path::PathBuf::from("/tmp/project"),
            intent: Intent::EditTask { id: "smetana-7".into(), title: "x y".into() },
            skills: Skills {
                smetana: std::path::PathBuf::from("/app/resources/smetana"),
                superpowers: std::path::PathBuf::from("/app/resources/superpowers"),
                superpowers_installed: true,
            },
            facts: None,
        }
    }

    #[test]
    fn the_binary_comes_from_the_profile() {
        assert_eq!(build_command(&launch("claude")).get_argv()[0], "claude");
        assert_eq!(build_command(&launch("codex")).get_argv()[0], "codex");
    }

    #[test]
    fn every_agent_is_given_a_terminal_it_can_paint_in() {
        for id in agents::IDS {
            let cmd = build_command(&launch(id));
            // `portable-pty` 0.9.0 has no `iter_env`; `get_env` reaches the
            // same value.
            let term = cmd.get_env("TERM").map(|v| v.to_string_lossy().into_owned());
            assert_eq!(term.as_deref(), Some("xterm-256color"), "{id}");
        }
    }

    // `:` separates PATH entries on Unix and is an ordinary character on
    // Windows, so the literal below only proves what it claims on Unix.
    #[cfg(unix)]
    #[test]
    fn the_bundled_bd_is_put_in_front_of_the_persons_own() {
        let dir = std::path::Path::new("/app/Contents/MacOS");
        assert_eq!(
            path_with(dir, Some(OsStr::new("/usr/local/bin:/usr/bin"))),
            OsString::from("/app/Contents/MacOS:/usr/local/bin:/usr/bin")
        );
        // Nothing inherited is still a reachable bd, not an empty PATH.
        assert_eq!(path_with(dir, None), OsString::from("/app/Contents/MacOS"));
    }

    #[test]
    fn every_agent_can_reach_the_bundled_bd() {
        // Without this the agent's `bd create` is "command not found" on any
        // machine that never installed bd — which is every machine the 128 MB
        // sidecar exists to serve.
        let dir = sidecar_dir().expect("the test binary has a location");
        for id in agents::IDS {
            let cmd = build_command(&launch(id));
            let path = cmd.get_env("PATH").expect("PATH must reach the agent");
            let mut entries = std::env::split_paths(path);
            assert_eq!(entries.next(), Some(dir.clone()), "{id}");
        }
    }

    #[test]
    fn the_rest_of_the_environment_is_left_alone() {
        // One directory in front, and nothing else about PATH rewritten: an
        // agent that lost the person's own tools would be worse off than one
        // that could not find bd. The base is the login shell's PATH rather
        // than this process's — a bundled app inherits launchd's, which has
        // none of the person's own directories in it.
        let base: Vec<_> = crate::shell_env::path()
            .map(|p| std::env::split_paths(p).collect())
            .unwrap_or_default();
        let cmd = build_command(&launch("claude"));
        let given: Vec<_> =
            std::env::split_paths(cmd.get_env("PATH").expect("PATH must reach the agent")).collect();
        assert_eq!(given[1..], base[..]);
    }

    #[test]
    fn the_prompt_is_an_argument_and_not_bytes_written_afterwards() {
        let cmd = build_command(&launch("codex"));
        let argv = cmd.get_argv();
        assert_eq!(argv.last().unwrap().to_string_lossy(), "Update bd issue smetana-7 (\"x y\"): ");
    }
}
