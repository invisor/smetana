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
    apply_environment(&mut cmd);
    // The environment half of running without a person. The argument half is
    // applied by the profile itself, because it has to go in front of the
    // positional prompt and `CommandBuilder` only appends; the environment has
    // no order and belongs here, beside the other variables every agent gets.
    // Only a `Run` has a mode, and only a `Run` gets any of this.
    if let Intent::Run { settings, .. } = &launch.intent {
        for (key, value) in launch.profile.autonomy(settings.mode).env {
            cmd.env(key, value);
        }
        // The run's name in bd's audit trail, and the whole of what makes bd's
        // claim a mutual exclusion. bd refuses `--claim` only when the issue is
        // held by a *different* actor, and its default actor — `$BEADS_ACTOR`,
        // else `git user.name`, else `$USER` — is identical for two runs on one
        // machine, so without a per-run name both would "successfully" claim
        // the same task. The session id is unique within this app instance,
        // which is what makes the name unique per batch here; ids restart at 1
        // on every launch, so a run after a restart — or in a second app
        // instance — can mint the same name. That cross-instance gap is open
        // and recorded rather than solved.
        //
        // The environment variable rather than `bd --actor` on every call: the
        // skills would have to thread the flag through each bd invocation they
        // document, and one forgotten call silently reverts to the shared
        // default. And only for a `Run`: a person filing or editing a task
        // through an agent keeps their own name in the audit trail.
        // `run_actor` and not a format string here: the runs worker derives
        // the same name to find what this session claimed, and two copies of
        // the format would drift silently.
        cmd.env("BEADS_ACTOR", crate::terminal::model::run_actor(id));
    }
    cmd
}

/// The person's own shell, in the project's root, with the same environment an
/// agent gets. There is no profile and no intent behind it: what a shell is for
/// is whatever the person types into it, and this app has nothing to tell it.
///
/// A terminal a person opens is interactive by construction — the process's
/// stdin is a PTY — so no flag is passed to say so, and none is passed to make
/// it a login shell either: what is wanted is the shell they would get from
/// their own terminal application, not a second reading of their profile files.
///
/// `cwd` is the project root, the same directory an agent starts in. A task's
/// worktree would be another answer and is not this one — see the rule file.
pub fn build_shell_command(program: &str, cwd: &Path) -> CommandBuilder {
    let mut cmd = CommandBuilder::new(program);
    cmd.cwd(cwd);
    apply_environment(&mut cmd);
    cmd
}

/// Everything about the environment that is true of every process this app
/// starts under a PTY, agent or shell alike: the terminal type, the locale, and
/// the `PATH` with the bundled `bd` in front of it.
///
/// One function and not two copies, and the reason is the last of those three.
/// A shell built from a second copy of these decisions would drift from this
/// one, and the most expensive way to drift is the quietest: a shell without the
/// sidecar in front of its `PATH` runs whatever `bd` the machine happens to have
/// against a board whose version this app pins and checks.
fn apply_environment(cmd: &mut CommandBuilder) {
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
    // What every child's own `PATH` is built on: the login shell's, because a
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

/// Whether the program can be run at all, answered in the parent and before
/// anything is spawned. The rule is `execvp`'s own — a name with a `/` in it is
/// a path and nothing is searched, a bare name is looked for along `PATH`, the
/// first candidate that could be executed wins — asked of the filesystem here
/// instead of the kernel.
///
/// **Why this app asks a question the OS is about to answer for it.**
/// `portable-pty`'s `pre_exec` hook calls `close_random_fds()`, which closes
/// every descriptor above 2 in the forked child. One of them is the
/// close-on-exec pipe `std::process` keeps for precisely this purpose: the
/// child writes the `errno` of a failed `execvp` into it, and the parent reads
/// either that error or end-of-file, where end-of-file means the `exec`
/// landed. The pipe is shut inside `pre_exec`, which runs *before* the `exec`
/// is attempted, so the parent sees end-of-file whatever happens next and
/// `spawn_command` answers `Ok` for a process that is already dying. What
/// reached a person instead of "there is no such program" was a session that
/// appeared and ended a moment later — and, for a missing interpreter, the line
/// `fatal runtime error: assertion failed: output.write(&bytes).is_ok()`
/// painted into their terminal by the child's own runtime as it failed to
/// report the failure.
///
/// **Of the two ways out, this is the cheap one, and the other one has nowhere
/// to live.** A notification channel of our own would have to be a descriptor
/// `close_random_fds` does not reach, opened by a hook that runs after it; the
/// hook that closes them is `portable-pty`'s, registered inside
/// `spawn_command`, and `CommandBuilder` accepts no hook of ours — so having
/// one means forking the crate, which this task explicitly does not. Checking
/// first leaves a race, since the program can go between this answer and the
/// `exec`, and that race is the ordinary state of a filesystem rather than
/// something introduced here: nothing is promised that was not promised before,
/// and what is bought is that the failure people actually meet — an agent that
/// was never installed, a `node` that an npm shebang points at and that is no
/// longer there — is an error naming the program rather than a session with a
/// corpse behind it.
///
/// **Only the shebang is new information; the rest is better wording.**
/// `portable-pty`'s own `search_path` already refuses a program that is
/// missing, that is a directory or that carries no execute permission, and it
/// does it before the fork, so those three were never the `Ok`-on-a-dead-process
/// case — what they were is a multi-line refusal quoting the whole of `PATH`
/// back at whoever read it. A script whose interpreter has gone passes every one
/// of those tests and fails in the child, which is the half that was invisible.
/// Only an absolute interpreter is checked, a relative one being resolved by the
/// kernel against a directory this function is not the authority on, and only
/// one level, since no kernel here runs a script whose interpreter is itself a
/// script. `#!/usr/bin/env node` is deliberately as far as this reads: `env`
/// itself exists, and whether it finds `node` is `env`'s own search along the
/// same `PATH`, which is not repeated here.
///
/// An execute bit — any of the three — rather than `access(X_OK)`: the whole
/// point is to refuse what plainly cannot run and never to refuse what could,
/// and the finer question of whether *this* user may run this file is left to
/// `portable-pty`'s own `access` a few lines later, which asks the kernel and
/// answers for the right uid.
#[cfg(unix)]
pub fn resolve_program(program: &OsStr, path: Option<&OsStr>, cwd: &Path) -> Result<PathBuf, String> {
    use std::os::unix::ffi::OsStrExt;

    let named = program.to_string_lossy().into_owned();
    if program.as_bytes().contains(&b'/') {
        // A path, not a name. `join` on an absolute path answers the absolute
        // path, so this one line covers both a `/usr/bin/…` shell and the
        // `./agent` a person could type.
        return match inspect(&cwd.join(program)) {
            Ok(exe) => Ok(exe),
            Err(Unusable::Absent) => Err(format!("{named} is not on disk")),
            Err(Unusable::Because(why)) => Err(why),
        };
    }
    let Some(path) = path else {
        return Err(format!("{named} was looked for, but this session was given no PATH"));
    };
    // A candidate that exists and cannot be run does not end the search —
    // `execvp` walks on to the next directory and so does this — but it is the
    // more useful thing to say afterwards than "not found", so the first such
    // reason is kept.
    let mut refused: Option<String> = None;
    for dir in std::env::split_paths(path) {
        // `cwd.join(dir)` and not `dir`: a relative entry in `PATH` is resolved
        // against the child's working directory, which is the one the command
        // carries rather than this process's.
        match inspect(&cwd.join(dir).join(program)) {
            Ok(exe) => return Ok(exe),
            Err(Unusable::Absent) => {}
            Err(Unusable::Because(why)) => {
                refused.get_or_insert(why);
            }
        }
    }
    Err(refused.unwrap_or_else(|| format!("{named} is not on PATH")))
}

/// Why one candidate cannot be run. The two are apart because a `PATH` search
/// treats them differently: nothing at that path is the ordinary case and the
/// search goes on, while something unusable is worth repeating to a person.
#[cfg(unix)]
enum Unusable {
    Absent,
    Because(String),
}

/// One candidate, measured. `metadata` and not `symlink_metadata`: a symlink is
/// followed by the kernel too, and a dangling one is simply absent.
#[cfg(unix)]
fn inspect(path: &Path) -> Result<PathBuf, Unusable> {
    use std::os::unix::fs::PermissionsExt;

    let meta = std::fs::metadata(path).map_err(|_| Unusable::Absent)?;
    let shown = path.display();
    if meta.is_dir() {
        return Err(Unusable::Because(format!("{shown} is a directory")));
    }
    if meta.permissions().mode() & 0o111 == 0 {
        return Err(Unusable::Because(format!("{shown} is not executable")));
    }
    if let Some(interpreter) = interpreter(path) {
        if !runnable(&interpreter) {
            return Err(Unusable::Because(format!(
                "{shown} runs under {}, which is not there",
                interpreter.display()
            )));
        }
    }
    Ok(path.to_path_buf())
}

/// The interpreter an absolute `#!` line names, if there is one. Anything that
/// is not a shebang — every real binary — answers `None` and is left alone.
#[cfg(unix)]
fn interpreter(path: &Path) -> Option<PathBuf> {
    use std::io::Read;
    use std::os::unix::ffi::OsStrExt;

    // Kernels read a couple of hundred bytes of this line and no more (127 on
    // Linux, 512 on macOS), so a buffer of the larger of the two is the whole
    // of what can be executed. The end of the line has to be *in* it: a first
    // line that runs off the end of the buffer leaves a truncated path, and a
    // truncated path is exactly the way this check could refuse a program that
    // would have run. Answering `None` there is the safe direction — nothing is
    // refused that was not refused before.
    let mut head = [0u8; 512];
    let read = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    let line = head.get(..read)?.strip_prefix(b"#!")?;
    let line = &line[..line.iter().position(|b| *b == b'\n')?];
    let word = line
        .split(|b| *b == b' ' || *b == b'\t' || *b == b'\r')
        .find(|word| !word.is_empty())?;
    let interpreter = PathBuf::from(OsStr::from_bytes(word));
    interpreter.is_absolute().then_some(interpreter)
}

/// The interpreter itself, held to the same two rules as the program: something
/// is there, and it carries an execute bit.
#[cfg(unix)]
fn runnable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    std::fs::metadata(path)
        .map(|meta| !meta.is_dir() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

/// The refusal `Pty::start` makes on its own behalf, in the words of the
/// program that was asked for. `resolve_program` above carries the whole of why
/// it exists.
///
/// Windows has no part in this: there is no fork, `CreateProcess` reports a
/// failure to the caller synchronously, and nothing between the two ends closes
/// a channel the way `close_random_fds` does here.
fn preflight(command: &CommandBuilder) -> Result<(), TerminalError> {
    #[cfg(not(unix))]
    let _ = command;
    #[cfg(unix)]
    {
        // An empty argv is `portable-pty`'s own "run the default shell", which
        // this app never asks for and which names no program to check.
        let Some(program) = command.get_argv().first() else { return Ok(()) };
        // The directory a relative program name or a relative `PATH` entry would
        // be resolved against, which is the command's own and not this
        // process's. A working directory that is not there is dropped, exactly
        // as `as_command` drops it a moment later; where the two then differ —
        // it falls back to the home directory and this to wherever this process
        // stands — is a difference of nothing, since every program this app runs
        // is either an absolute path or a bare name looked for along an absolute
        // `PATH`.
        let cwd = command
            .get_cwd()
            .map(PathBuf::from)
            .filter(|dir| dir.is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("/"));
        resolve_program(program, command.get_env("PATH"), &cwd)
            .map(|_| ())
            .map_err(TerminalError::Spawn)?;
    }
    Ok(())
}

pub struct Pty {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn std::io::Write + Send>,
    child: Box<dyn Child + Send + Sync>,
}

impl Pty {
    /// A session running a coding agent.
    pub fn spawn(
        id: SessionId,
        launch: &Launch,
        cols: u16,
        rows: u16,
        out: mpsc::UnboundedSender<Chunk>,
    ) -> Result<Self, TerminalError> {
        Self::start(id, build_command(id, launch), launch.profile.binary(), cols, rows, out)
    }

    /// A session running the person's own shell, with no agent behind it. The
    /// PTY, the reader thread and everything after the spawn are the same —
    /// what a session runs is not this file's business past `build_command`.
    pub fn spawn_shell(
        id: SessionId,
        cwd: &Path,
        cols: u16,
        rows: u16,
        out: mpsc::UnboundedSender<Chunk>,
    ) -> Result<Self, TerminalError> {
        let program = crate::shell_env::shell();
        Self::start(id, build_shell_command(&program, cwd), &program, cols, rows, out)
    }

    /// The spawn itself. `what` names the program only so a failure can say what
    /// it was that did not start.
    fn start(
        id: SessionId,
        command: CommandBuilder,
        what: &str,
        cols: u16,
        rows: u16,
        out: mpsc::UnboundedSender<Chunk>,
    ) -> Result<Self, TerminalError> {
        // Before anything is opened or forked: a program that cannot be
        // executed is refused here, because the `Ok` `spawn_command` answers a
        // few lines down proves nothing about it. `resolve_program` above is
        // the whole of that mechanism and of why it is a check in the parent
        // rather than a channel out of the child.
        preflight(&command)?;

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
            .spawn_command(command)
            .map_err(|e| TerminalError::Spawn(format!("{what}: {e}")))?;

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

    /// The child's pid, which is also the id of the process group everything it
    /// starts belongs to — `spawn_command` calls `setsid`, which is the same
    /// fact `hangup` below rests on.
    ///
    /// It keeps answering after the child has been reaped, so a caller writing
    /// it down has to pair it with something that says *which* process that pid
    /// was: the run registry pairs it with the process's start time
    /// (`runs::recovery::group`), because a bare pid written to a file and read
    /// after a restart names whoever holds it by then.
    pub fn pid(&self) -> Option<u32> {
        self.child.process_id()
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
            languages: agents::Languages::default(),
            caveman_level: String::new(),
            agent_prompt: String::new(),
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
            reports: std::path::PathBuf::from("/p/.smetana/runs/1"),
            batch: 1,
            remove_worktrees: true,
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

    /// The shell branch, and what it is worth a test for is not the shell — it
    /// is that the environment is one piece of code and not two. Every one of
    /// these three is a decision the agent branch above is already tested for,
    /// asked again of the branch that has no profile behind it.
    #[test]
    fn a_shell_is_given_the_same_environment_an_agent_is() {
        let cmd = build_shell_command("/bin/zsh", Path::new("/tmp/project"));

        // What runs is the person's own shell, with no argument telling it what
        // to be: a PTY is what makes it interactive.
        assert_eq!(cmd.get_argv(), &vec![OsString::from("/bin/zsh")]);
        assert_eq!(cmd.get_cwd(), Some(&OsString::from("/tmp/project")));

        // The bundled bd in front, which is the expensive one to lose: without
        // it a shell runs whatever bd the machine has against a board whose
        // version this app pins and checks.
        let dir = sidecar_dir().expect("the test binary has a location");
        let path = cmd.get_env("PATH").expect("PATH must reach the shell");
        assert_eq!(std::env::split_paths(path).next(), Some(dir));

        // `iter_extra_env_as_str` and never `get_env` for these two, for the
        // reason the agent's locale test spells out: `get_env` answers out of a
        // snapshot of this process's own environment, where both are likely to
        // be set already and an assertion on them would pass with the lines it
        // guards deleted.
        let told: Vec<_> = cmd.iter_extra_env_as_str().collect();
        assert!(told.contains(&("TERM", "xterm-256color")), "{told:?}");
        assert_eq!(
            told.iter().filter(|(key, _)| crate::shell_env::LOCALE_KEYS.contains(key)).count(),
            1,
            "{told:?} — a shell told nothing about the encoding runs in the C locale too"
        );
    }

    /// The other half of that: what a shell is *not* given. Everything the
    /// agent branch adds beyond the shared piece is about an agent or about a
    /// run, and a shell has neither — a `BEADS_ACTOR` leaking into one would
    /// put a person's own bd commands into a run's audit trail under a session
    /// id that means nothing to them.
    #[test]
    fn a_shell_is_told_nothing_about_agents_or_runs() {
        let cmd = build_shell_command("/bin/zsh", Path::new("/tmp/project"));
        let keys: Vec<_> = cmd.iter_extra_env_as_str().map(|(key, _)| key).collect();
        assert_eq!(keys.len(), 3, "{keys:?} — the shared piece is TERM, a locale and PATH");
        assert!(!keys.contains(&"BEADS_ACTOR"), "{keys:?}");
    }

    #[test]
    fn the_prompt_is_an_argument_and_not_bytes_written_afterwards() {
        let cmd = build_command(7, &launch("codex"));
        let argv = cmd.get_argv();
        // What is checked is where the prompt is, not how it is worded: the
        // wording belongs to `prompt.rs` and is pinned by its own tests, and a
        // second copy of it here would only have to be edited twice.
        assert!(
            argv.last().unwrap().to_string_lossy().contains("Update bd issue smetana-7 (\"x y\")"),
            "{argv:?}"
        );
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
            Intent::NewTask {
                brainstorm: agents::Stage::Off,
                spec: agents::Stage::Off,
                plan: agents::Stage::Off,
                draft: agents::TaskDraft {
                    text: "the tab bar overlaps the board".into(),
                    issue_type: None,
                    priority: None,
                    images: vec![],
                    parent: None,
                },
            },
            Intent::EditTask { id: "smetana-7".into(), title: "x y".into() },
            Intent::Setup,
        ];
        for intent in intents {
            let cmd = build_command(42, &with_intent("claude", intent));
            let actor = cmd.iter_extra_env_as_str().find(|(key, _)| *key == "BEADS_ACTOR");
            assert_eq!(actor, None);
        }
    }

    /// A directory of this test's own, named after the test so two of them
    /// running at once cannot write over each other. The same shape
    /// `service.rs` uses for the shell tests.
    #[cfg(unix)]
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-pty-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the temp directory");
        dir.canonicalize().expect("canonicalize the temp directory")
    }

    #[cfg(unix)]
    fn executable(path: &Path, body: &str) {
        use std::os::unix::fs::PermissionsExt;
        std::fs::write(path, body).expect("write the program");
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
            .expect("make the program executable");
    }

    /// The failure this whole mechanism exists for, asked of the road the app
    /// itself takes: `Pty::start`, the one function `spawn` and `spawn_shell`
    /// both end in. A program that is not on disk must answer with an error and
    /// never with a session — `spawn_command`'s own `Ok` says nothing about it,
    /// because `close_random_fds` has already shut the channel the child would
    /// have reported a failed `exec` on.
    ///
    /// Nothing is spawned by this test, in either outcome: the refusal happens
    /// before the PTY is opened, and the assertion is that it happened.
    #[cfg(unix)]
    #[test]
    fn a_program_that_is_not_on_disk_is_refused_rather_than_started() {
        let dir = scratch("start-missing");
        let program = dir.join("an-agent-that-was-never-installed").display().to_string();
        let (chunks, _rx) = mpsc::unbounded_channel();
        match Pty::start(1, build_shell_command(&program, &dir), &program, 120, 30, chunks) {
            Err(TerminalError::Spawn(why)) => assert!(why.contains(&program), "{why}"),
            Err(other) => panic!("{other}"),
            Ok(_) => panic!("a program that is not on disk answered with a session"),
        }
    }

    /// The half of it that `portable-pty` cannot see, and the reason the check
    /// reads the shebang: this program exists, is a regular file and carries
    /// every execute bit, so `search_path` waves it through — and then the
    /// `exec` fails in the child, where the report of it has nowhere to go. On
    /// the code this test was written against the call answered `Ok`, the child
    /// died at once with status 1, and what a person saw in the terminal was
    /// `fatal runtime error: assertion failed: output.write(&bytes).is_ok()`
    /// from the child's own runtime failing to write down why it could not
    /// start.
    ///
    /// This is the shape of a real failure rather than an invented one: an
    /// agent installed by npm is a script whose first line names `node`, and a
    /// person who changed node versions has exactly this file.
    #[cfg(unix)]
    #[test]
    fn a_program_whose_interpreter_is_gone_is_refused_rather_than_started() {
        let dir = scratch("start-shebang");
        let script = dir.join("agent");
        executable(&script, "#!/nonexistent/interpreter\nexit 0\n");
        let program = script.display().to_string();
        let (chunks, _rx) = mpsc::unbounded_channel();
        match Pty::start(1, build_shell_command(&program, &dir), &program, 120, 30, chunks) {
            Err(TerminalError::Spawn(why)) => {
                assert!(why.contains("/nonexistent/interpreter"), "{why}")
            }
            Err(other) => panic!("{other}"),
            Ok(_) => panic!("a program whose interpreter is gone answered with a session"),
        }
    }

    /// The positive control, and the reason it stops at `resolve_program`
    /// rather than going through `Pty::start`: a program that passes the check
    /// is spawned, and no test in this file starts a process. Both halves are
    /// here — a real binary, and a script whose interpreter is where it says it
    /// is — because a check that refused either of them would break every
    /// session in the app while leaving the two tests above green.
    #[cfg(unix)]
    #[test]
    fn a_program_that_can_run_is_not_refused() {
        let dir = scratch("resolve-good");
        let script = dir.join("agent");
        executable(&script, "#!/bin/sh\nexit 0\n");
        assert_eq!(resolve_program(script.as_os_str(), None, &dir), Ok(script));

        let shell = Path::new("/bin/sh");
        assert_eq!(resolve_program(shell.as_os_str(), None, &dir), Ok(shell.to_path_buf()));
    }

    /// A bare name is `PATH`'s to answer, which is how every agent in this app
    /// is named — the profile gives `claude`, not a path to it.
    #[cfg(unix)]
    #[test]
    fn a_bare_name_is_looked_for_along_the_path() {
        let dir = scratch("resolve-path");
        let found = dir.join("agent");
        executable(&found, "#!/bin/sh\nexit 0\n");
        let path = OsString::from(format!("/nonexistent/bin:{}", dir.display()));
        assert_eq!(resolve_program(OsStr::new("agent"), Some(&path), &dir), Ok(found));

        let missing = resolve_program(OsStr::new("no-such-agent"), Some(&path), &dir);
        assert_eq!(missing, Err("no-such-agent is not on PATH".to_string()));
    }

    /// `execvp` walks past a candidate it cannot execute and so does this: a
    /// stray unreadable `agent` early in somebody's `PATH` must not hide the
    /// real one later in it.
    #[cfg(unix)]
    #[test]
    fn a_candidate_that_cannot_run_does_not_end_the_search() {
        let root = scratch("resolve-shadowed");
        let first = root.join("first");
        let second = root.join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        std::fs::write(first.join("agent"), "not executable\n").unwrap();
        let real = second.join("agent");
        executable(&real, "#!/bin/sh\nexit 0\n");
        let path = OsString::from(format!("{}:{}", first.display(), second.display()));
        assert_eq!(resolve_program(OsStr::new("agent"), Some(&path), &root), Ok(real));
    }

    /// The two refusals `portable-pty` already makes, worded here instead: they
    /// were never the `Ok`-on-a-dead-process case, and what they gain is a
    /// sentence a person can read in place of a multi-line dump of `PATH`.
    #[cfg(unix)]
    #[test]
    fn a_file_that_is_not_executable_and_a_directory_are_both_refused() {
        let dir = scratch("resolve-unusable");
        let plain = dir.join("agent");
        std::fs::write(&plain, "just a file\n").unwrap();
        assert_eq!(
            resolve_program(plain.as_os_str(), None, &dir),
            Err(format!("{} is not executable", plain.display()))
        );
        assert_eq!(
            resolve_program(dir.as_os_str(), None, &dir),
            Err(format!("{} is a directory", dir.display()))
        );
    }

    /// What the shebang reading deliberately stops short of. `env` exists, so
    /// the program is accepted, and whether `node` is on the `PATH` is `env`'s
    /// own search — repeating it here would be this function guessing at
    /// another program's rules.
    #[cfg(unix)]
    #[test]
    fn an_interpreter_that_is_there_is_as_far_as_the_shebang_is_read() {
        let dir = scratch("resolve-env-shebang");
        let script = dir.join("agent");
        executable(&script, "#!/usr/bin/env node\nconsole.log(1)\n");
        assert_eq!(resolve_program(script.as_os_str(), None, &dir), Ok(script));
    }
}
