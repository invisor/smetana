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

/// The codeset an agent is told about when nobody on the machine named a UTF-8
/// locale of their own — which on a bundled app is every time, since launchd
/// hands it no locale at all. That makes this the shipped path rather than an
/// edge case, and worth a measurement per platform instead of one value that
/// reads well.
///
/// macOS 26.5.2, as `LC_CTYPE=<value> locale charmap`:
///
///   `UTF-8` -> UTF-8, `C.UTF-8` -> UTF-8
///
/// `C.UTF-8` is present here, but macOS has not always had it and the bundle
/// names no `minimumSystemVersion`, so it inherits Tauri's default of 10.13.
/// Where it is absent it resolves to US-ASCII — the bug unfixed — and adds a
/// five-line `Setting locale failed` block from every Perl-based tool, printed
/// into the surface `screen.rs` reads. `UTF-8` has been understood by macOS
/// throughout, so it is the one that cannot regress.
///
/// It is also why `shell_env::FALLBACK_KEY` is `LC_CTYPE`, and why an invented
/// answer is never written to the variable it replaced: a bare codeset is legal
/// in only one position. Measured the same way, and the single copy of this —
/// `LC_CTYPE=UTF-8` -> UTF-8, against `LC_ALL=UTF-8` -> US-ASCII and
/// `LANG=UTF-8` -> US-ASCII.
#[cfg(target_os = "macos")]
const UTF8_LOCALE: &str = "UTF-8";

/// glibc is the other way round: a bare `UTF-8` is inert there and resolves to
/// US-ASCII, while `C.UTF-8` is a real locale. WebKitGTK is a stated build
/// target, so both halves ship.
#[cfg(not(target_os = "macos"))]
const UTF8_LOCALE: &str = "C.UTF-8";

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
pub fn build_command(id: SessionId, launch: &Launch) -> CommandBuilder {
    let mut cmd = launch.profile.command(launch);
    cmd.cwd(&launch.cwd);
    cmd.env("TERM", "xterm-256color");
    // What encoding the bytes in that terminal are in, which is the second half
    // of `TERM` and is missing for the same launchd reason `PATH` is: a bundled
    // app is handed an environment with no locale in it at all, and a child told
    // nothing runs in the C locale. There macOS's own tools take their default
    // C-string encoding from `CFStringGetSystemEncoding()`, which answers
    // MacRoman — so an agent that runs `pbcopy` over the UTF-8 bytes of a
    // Russian greeting puts `–ü—Ä–∏–≤–µ—Ç` on the clipboard, losslessly and with
    // no error to notice. That is the whole of smetana-bn3: a coding agent
    // copies a mouse selection by shelling out to `pbcopy`, so text the agent
    // had drawn correctly in its own pane reached the clipboard as mojibake,
    // and nothing in the webview was ever involved.
    //
    // Saying what the encoding is is a statement of fact rather than a
    // preference: this PTY is a UTF-8 channel by construction, since
    // `terminal_write` sends a Rust string's own UTF-8 bytes and both
    // `screen.rs` and xterm.js decode UTF-8 at the other end. Which variable
    // carries it is `shell_env`'s to decide: theirs when their value is being
    // forwarded, and deliberately not theirs when it was rejected, because the
    // codeset invented for that case is legal in only one position.
    let locale = crate::shell_env::locale();
    // Anything that would outrank what is about to be set is dropped first, and
    // it does two jobs at once. The login shell's environment is this process's
    // plus whatever its rc files did, so a variable this process has and the
    // shell does not is one an rc file unset — leaving it in would let the value
    // the person removed decide the encoding after all. And when the resolved
    // locale was rejected rather than forwarded, the variable it came from is by
    // definition one of these, so this is also what takes the offending
    // `LC_ALL=C` out of the way of the `LC_CTYPE` replacing it.
    for key in crate::shell_env::outranking(locale.key) {
        cmd.env_remove(key);
    }
    cmd.env(locale.key, locale.value.as_deref().unwrap_or(UTF8_LOCALE));
    // The environment half of running without a person. The argument half is
    // applied by the profile itself, because it has to go in front of the
    // positional prompt and `CommandBuilder` only appends; the environment has
    // no order and belongs here, beside the other two variables every agent
    // gets. Only a `Run` has a mode, and only a `Run` gets any of this.
    if let Intent::Run { settings } = &launch.intent {
        for (key, value) in launch.profile.autonomy(settings.mode).env {
            cmd.env(key, value);
        }
        // The run's name in bd's audit trail, and the whole of what makes bd's
        // claim a mutual exclusion. bd refuses `--claim` only when the issue is
        // held by a *different* actor, and its default actor — `$BEADS_ACTOR`,
        // else `git user.name`, else `$USER` — is identical for two runs on one
        // machine, so without a per-run name both would "successfully" claim
        // the same task. The session id is already unique per session, which
        // makes it unique per batch.
        //
        // The environment variable rather than `bd --actor` on every call: the
        // skills would have to thread the flag through each bd invocation they
        // document, and one forgotten call silently reverts to the shared
        // default. And only for a `Run`: a person filing or editing a task
        // through an agent keeps their own name in the audit trail.
        cmd.env("BEADS_ACTOR", format!("smetana-run-{id}"));
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
            .spawn_command(build_command(id, launch))
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
        with_intent(id, Intent::EditTask { id: "smetana-7".into(), title: "x y".into() })
    }

    fn with_intent(id: &str, intent: Intent) -> Launch {
        Launch {
            profile: agents::resolve(id).unwrap(),
            cwd: std::path::PathBuf::from("/tmp/project"),
            intent,
            skills: Skills {
                smetana: std::path::PathBuf::from("/app/resources/smetana"),
                superpowers: std::path::PathBuf::from("/app/resources/superpowers"),
                superpowers_installed: true,
            },
            facts: None,
        }
    }

    fn run_intent() -> Intent {
        use crate::runs::model::{RunMode, RunScope, RunSettings};
        Intent::Run {
            settings: RunSettings {
                scope: RunScope::Queue,
                mode: RunMode::Auto,
                target_branch: "staging".into(),
                create_target: false,
                min_priority: Some(2),
                max_parallel_tasks: Some(3),
                live_check: false,
                file_findings: true,
            },
        }
    }

    #[test]
    fn the_binary_comes_from_the_profile() {
        assert_eq!(build_command(7, &launch("claude")).get_argv()[0], "claude");
        assert_eq!(build_command(7, &launch("codex")).get_argv()[0], "codex");
    }

    #[test]
    fn every_agent_is_given_a_terminal_it_can_paint_in() {
        for id in agents::IDS {
            let cmd = build_command(7, &launch(id));
            // `iter_extra_env_as_str` and not `get_env`, for the reason the
            // test below spells out: `get_env` answers out of the snapshot of
            // this process's own environment, where a `TERM` is all but certain.
            let term = cmd.iter_extra_env_as_str().find(|(key, _)| *key == "TERM");
            assert_eq!(term, Some(("TERM", "xterm-256color")), "{id}");
        }
    }

    /// The other half of that: a terminal type says nothing about what encoding
    /// the bytes in it are in, and a child left to guess guesses MacRoman on
    /// macOS. Which of the three variables carries the answer depends on the
    /// machine — what must never happen is that none of them does.
    ///
    /// Whether the value *resolves* is deliberately not asserted here. It is not
    /// a property of the string: `names_utf8` is a rule about forwarding
    /// somebody else's value and rejects a bare codeset for having no `.`, which
    /// is right, and which would fail this test on every path that falls back —
    /// including the bundle's own, where nobody names a locale at all. The
    /// question is answered one test down by asking the platform instead.
    ///
    /// Read through `iter_extra_env_as_str`, which yields only what `env()` set,
    /// and never `get_env`: `CommandBuilder::new` snapshots this process's
    /// environment and `get_env` answers out of that snapshot, so on any machine
    /// whose own environment has a locale — every developer terminal, most CI
    /// images — a `get_env` assertion passes on the tester's environment and
    /// would go on passing with the lines it guards deleted.
    #[test]
    fn every_agent_is_told_what_encoding_the_stream_carries() {
        for id in agents::IDS {
            let cmd = build_command(7, &launch(id));
            let told: Vec<_> = cmd
                .iter_extra_env_as_str()
                .filter(|(key, _)| crate::shell_env::LOCALE_KEYS.contains(key))
                .collect();
            assert_eq!(told.len(), 1, "{id} was told {told:?}, want exactly one locale variable");
        }
    }

    /// The pairing, asked of the platform rather than of our own reading of it.
    /// A key and a value can each be defensible and still resolve to US-ASCII
    /// together — `LC_ALL=UTF-8` is exactly that — and no pure function in this
    /// tree can notice, because whether a locale resolves is the system's answer
    /// and not a property of the string.
    #[cfg(unix)]
    #[test]
    fn what_the_agent_is_told_resolves_to_utf8_on_this_machine() {
        const LOCALE: &str = "/usr/bin/locale";
        if !std::path::Path::new(LOCALE).exists() {
            // Not a silent pass: it is loud where the tool exists, which is
            // every developer machine and the gate this project runs.
            eprintln!("{LOCALE} is absent; the pairing could not be checked");
            return;
        }
        for id in agents::IDS {
            let cmd = build_command(7, &launch(id));
            let (key, value) = cmd
                .iter_extra_env_as_str()
                .find(|(key, _)| crate::shell_env::LOCALE_KEYS.contains(key))
                .expect("one of the locale variables must have been set");
            let out = std::process::Command::new(LOCALE)
                .arg("charmap")
                // Cleared so the answer is about this pair and nothing else,
                // exactly as the values in `UTF8_LOCALE` were measured.
                .env_clear()
                .env(key, value)
                .output()
                .expect("locale must run");
            let charmap = String::from_utf8_lossy(&out.stdout).trim().to_ascii_uppercase();
            assert_eq!(charmap, "UTF-8", "{id} was told {key}={value}, which resolves to {charmap}");
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
            let cmd = build_command(7, &launch(id));
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
        let cmd = build_command(7, &launch("claude"));
        let given: Vec<_> =
            std::env::split_paths(cmd.get_env("PATH").expect("PATH must reach the agent")).collect();
        assert_eq!(given[1..], base[..]);
    }

    #[test]
    fn the_prompt_is_an_argument_and_not_bytes_written_afterwards() {
        let cmd = build_command(7, &launch("codex"));
        let argv = cmd.get_argv();
        assert_eq!(argv.last().unwrap().to_string_lossy(), "Update bd issue smetana-7 (\"x y\"): ");
    }

    /// The actor is what bd's `--claim` mutual exclusion rests on: its default
    /// — `$BEADS_ACTOR`, else `git user.name`, else `$USER` — is identical for
    /// two runs on one machine, so a run must carry a name of its own. Read
    /// through `iter_extra_env_as_str` and never `get_env`, for the reason the
    /// locale test spells out: `get_env` also answers out of the snapshot of
    /// this process's own environment.
    #[test]
    fn a_run_session_carries_the_runs_own_bd_actor() {
        for id in agents::IDS {
            let cmd = build_command(42, &with_intent(id, run_intent()));
            let actor = cmd.iter_extra_env_as_str().find(|(key, _)| *key == "BEADS_ACTOR");
            assert_eq!(actor, Some(("BEADS_ACTOR", "smetana-run-42")), "{id}");
        }
    }

    /// The other half of the rule: a person filing or editing a task through
    /// an agent keeps their own name in bd's audit trail, so no other intent
    /// is given an actor.
    #[test]
    fn no_other_intent_is_given_a_bd_actor() {
        let intents = [
            Intent::Bare,
            Intent::EditTask { id: "smetana-7".into(), title: "x y".into() },
            Intent::Setup,
        ];
        for intent in intents {
            let cmd = build_command(42, &with_intent("claude", intent));
            let actor = cmd.iter_extra_env_as_str().find(|(key, _)| *key == "BEADS_ACTOR");
            assert_eq!(actor, None);
        }
    }
}
