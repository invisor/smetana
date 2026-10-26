//! `smetanad run`: the core's workers, the iroh endpoints and the restart gateway, in one process
//! with no window.
//!
//! The data directory is an argument and is never guessed. Everything the server keeps lives in it:
//! `settings.json`, the attachments, transcripts and runs' registries the workers write, the device
//! key (`remote-key`) and the device list (`remote-devices.json`), the local pairing credential and the
//! lock that says one server owns the directory.
//!
//! Two endpoints serve one protocol. A **loopback** endpoint is always up: it is how a client on this
//! machine reaches the server, with no relay, and it is what the local pairing credential names. The
//! **public** endpoint, which contacts a relay, starts late for the reason the desktop's does — a
//! server that has only ever been used by the machine it sits on has no business opening a socket to
//! the internet — at `smetanad pair` and at start-up only if some device was paired by a ticket.

use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, Weak};
use std::time::Duration;

use remote_server::devices::{self, Devices};
use remote_server::dispatch::Services;
use remote_server::gateway::{deferred_payload, Gateway, LiveWork, WorkKind as LiveKind, DEFERRED_EVENT};
use remote_server::pairing::{self, Pairing};
use remote_server::{endpoint, Event, Remote, DEFAULT_RELAY_URL};
use smetana_core::context::{Context, Sink};
use smetana_core::runs::service::Request as RunRequest;
use smetana_core::session::service::SessionHandle;
use smetana_core::terminal::model::{SessionState, WorkKind};
use smetana_core::terminal::service::{Request as TerminalRequest, TerminalHandle};
use smetana_core::{attachments, runs, session, settings, shell_env, terminal, tracker};
use tokio::sync::{oneshot, Notify};

use crate::local;

pub const LOCK_FILE: &str = "server.lock";

/// How often a `when-idle` restart looks again for work that is still alive.
pub const RESTART_POLL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct Options {
    /// Where everything lives. Required and never derived.
    pub data_dir: PathBuf,
    /// The `bd` binary. Absent, the one beside this executable, then the one on `PATH`.
    pub bd: Option<PathBuf>,
    /// The skill library's directory (the bundle's resources). Absent, the library is not found and
    /// the agents work without it.
    pub resource_dir: Option<PathBuf>,
    /// An older release's data directory to take the key, the paired devices and the attachments
    /// from, once: `inherit.rs`. Never guessed; the desktop that has one names it.
    pub inherit_from: Option<PathBuf>,
    pub restart_poll: Duration,
}

impl Options {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir, bd: None, resource_dir: None, inherit_from: None, restart_poll: RESTART_POLL }
    }
}

/// The server is not to run as root: it starts agents that run arbitrary commands, with the account's
/// credentials, on a machine the person may share with other things. `euid` is the effective user id.
pub fn refuse_root(euid: u32) -> Result<(), String> {
    if euid == 0 {
        Err("smetanad refuses to run as root: the agents it starts run commands with the account's \
             authority, and a root shell would give them the whole machine. Run it as an ordinary user \
             (for a service, `User=` in the unit)."
            .to_owned())
    } else {
        Ok(())
    }
}

/// The effective user id, where there is one.
pub fn effective_uid() -> Option<u32> {
    #[cfg(unix)]
    {
        // SAFETY: `geteuid` takes no arguments, touches no memory and cannot fail.
        Some(unsafe { libc::geteuid() })
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Where `bd` is: the flag, else beside this executable (the layout the desktop bundle has), else on
/// the `PATH`. When none exists the beside-the-executable path is returned anyway, so the tracker's own
/// health says that bd is missing and where it looked.
pub fn find_bd(flag: Option<&Path>) -> PathBuf {
    if let Some(path) = flag {
        return path.to_path_buf();
    }
    let name = if cfg!(windows) { "bd.exe" } else { "bd" };
    let beside = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|dir| dir.join(name)));
    if let Some(path) = beside.as_ref().filter(|path| path.is_file()) {
        return path.clone();
    }
    if let Some(path) = smetana_core::health::which(name, std::env::var("PATH").ok().as_deref()) {
        return path;
    }
    beside.unwrap_or_else(|| PathBuf::from(name))
}

/// A context for the workers (or just for reading, as `status` does).
pub fn context(options: &Options, sink: Arc<dyn Sink>) -> Context {
    Context::new(
        Ok(options.data_dir.clone()),
        // `settings.json` sits in the data directory: the server has one directory, not the
        // platform's three.
        Ok(options.data_dir.clone()),
        options.resource_dir.clone().ok_or_else(|| "no --resource-dir was given".to_owned()),
        Some(find_bd(options.bd.as_deref())),
        sink,
    )
}

/// What the workers say, delivered to every device that listens.
struct ServerSink {
    remote: OnceLock<Weak<Remote>>,
}

impl ServerSink {
    fn remote(&self) -> Option<Arc<Remote>> {
        self.remote.get().and_then(Weak::upgrade)
    }
}

impl Sink for ServerSink {
    fn emit(&self, name: &str, payload: serde_json::Value) {
        if let Some(remote) = self.remote() {
            remote.hub.publish(name, &payload);
        }
    }

    fn terminal_chunk(&self, session: u64, seq: u64, bytes: &[u8]) {
        if let Some(remote) = self.remote() {
            remote.hub.publish_terminal_chunk(session, seq, bytes);
        }
    }

    fn terminal_log(&self, session: u64, entries: &serde_json::Value) {
        if let Some(remote) = self.remote() {
            remote.hub.publish_terminal_log(session, entries);
        }
    }

    fn push(&self, message: smetana_core::remote::push::Message) {
        if let Some(remote) = self.remote() {
            remote_server::push::dispatch(&remote, message);
        }
    }
}

/// A sink with nowhere to send: for a context that only reads, as `status` builds.
pub struct NoSink;

impl Sink for NoSink {
    fn emit(&self, _name: &str, _payload: serde_json::Value) {}
    fn push(&self, _message: smetana_core::remote::push::Message) {}
}

/// How a server's life ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// A signal: the operator stopped it.
    Stop,
    /// The restart gateway asked for a restart.
    Restart,
}

pub struct Server {
    pub remote: Arc<Remote>,
    pub gateway: Arc<Gateway>,
    pub data_dir: PathBuf,
    /// The loopback address the local credential names.
    pub addr: iroh::EndpointAddr,
    terminal: TerminalHandle,
    sessions: SessionHandle,
    restart: Arc<Notify>,
    /// The gateway asked for the server to end (`server_restart` with `stop-when-idle`).
    stop: Arc<Notify>,
    _lock: std::fs::File,
}

/// Takes the directory's lock, or says who has it.
fn lock_directory(dir: &Path) -> Result<std::fs::File, String> {
    use std::io::Write;
    let path = dir.join(LOCK_FILE);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(&path)
        .map_err(|err| format!("{}: {err}", path.display()))?;
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            let holder = std::fs::read_to_string(&path).unwrap_or_default();
            return Err(format!(
                "another smetanad already owns {} (pid {})",
                dir.display(),
                holder.trim()
            ));
        }
        Err(std::fs::TryLockError::Error(err)) => return Err(format!("{}: {err}", path.display())),
    }
    file.set_len(0).map_err(|err| err.to_string())?;
    write!(file, "{}", std::process::id()).map_err(|err| err.to_string())?;
    Ok(file)
}

/// Whether a server owns `dir` right now: the lock is held by a live process. A directory that has
/// never held a server, or whose server is gone however it ended, is not owned. The lock is taken and
/// let go again for the length of the question, which a server starting in that instant would see as
/// "already owns" and retry or refuse; a caller that spawns servers asks once per spawn.
pub fn is_owned(dir: &Path) -> bool {
    let Ok(file) = std::fs::OpenOptions::new().read(true).write(true).open(dir.join(LOCK_FILE)) else {
        return false;
    };
    matches!(file.try_lock(), Err(std::fs::TryLockError::WouldBlock))
}

fn create_data_dir(dir: &Path) -> Result<(), String> {
    if dir.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // Made by this call, so its mode is this call's to choose: the key and the credential inside
        // are 0600 anyway, but a directory nobody else can list is one less thing to reason about.
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    Ok(())
}

/// The live work a restart would end: runs, agent sessions (PTY), driven sessions and shells.
///
/// A run is named by its token, and its current batch (the session the run records, in whichever
/// worker its transport says) carries that token as its `owner`, so that confirming a run covers the
/// batches it goes on to start and nothing else on the same project.
pub async fn live_work(
    runs: &runs::service::RunHandle,
    terminal: &TerminalHandle,
    sessions: &SessionHandle,
) -> Vec<LiveWork> {
    use smetana_core::runs::session::Transport;
    let mut work = Vec::new();
    // (driven?, session id) -> the run's token
    let mut owners: std::collections::HashMap<(bool, u64), u64> = std::collections::HashMap::new();
    let (tx, rx) = oneshot::channel();
    if runs.0.send(RunRequest::LiveProjects(tx)).await.is_ok() {
        for project in rx.await.unwrap_or_default() {
            let (tx, rx) = oneshot::channel();
            if runs.0.send(RunRequest::State(project, tx)).await.is_err() {
                continue;
            }
            for run in rx.await.unwrap_or_default() {
                if let Some(session) = run.session {
                    owners.insert((run.transport != Transport::Pty, session), run.token);
                }
                work.push(LiveWork {
                    kind: LiveKind::Run,
                    id: Some(run.token),
                    project: run.project,
                    label: "run".to_owned(),
                    owner: None,
                });
            }
        }
    }
    let (tx, rx) = oneshot::channel();
    if terminal.0.send(TerminalRequest::Marks(tx)).await.is_ok() {
        for mark in rx.await.unwrap_or_default() {
            if mark.state == SessionState::Exited {
                continue;
            }
            let (kind, label) = match mark.kind {
                WorkKind::Shell => (LiveKind::Shell, "shell"),
                _ => (LiveKind::Agent, "agent"),
            };
            work.push(LiveWork {
                kind,
                id: Some(mark.id),
                project: mark.project,
                label: label.to_owned(),
                owner: owners.get(&(false, mark.id)).copied(),
            });
        }
    }
    if let Ok(live) = session::commands::plain::live(sessions).await {
        for found in live {
            work.push(LiveWork {
                kind: LiveKind::Session,
                id: Some(found.id),
                project: found.project,
                label: found.agent,
                owner: owners.get(&(true, found.id)).copied(),
            });
        }
    }
    work
}

/// The relay the public endpoint binds: the server's own setting when it has a settings file, else the
/// one the old release paired its phones through (`inherit.rs`), else the default. A server that
/// inherited phones binds before the shell has handed the settings over, and the phones are told to
/// look for it on the old release's relay.
fn relay_url(settings: Option<&Path>, inherited: Option<&str>) -> String {
    match settings {
        Some(path) if path.is_file() => settings::file::load(path).0.remote.relay_url,
        _ => inherited.map(str::to_owned).unwrap_or_else(|| DEFAULT_RELAY_URL.to_owned()),
    }
}

impl Server {
    pub async fn start(options: Options) -> Result<Server, String> {
        create_data_dir(&options.data_dir)?;
        let data_dir = options.data_dir.canonicalize().map_err(|err| format!("{}: {err}", options.data_dir.display()))?;
        let lock = lock_directory(&data_dir)?;
        // Before the key is read: taking over an older release's data is what decides which key this
        // is, and a key made first could not be replaced by the one the phones already trust. It never
        // stops the start (`inherit.rs`): what failed is logged and tried again next time.
        let inherited = options.inherit_from.as_ref().map(|from| {
            let report = crate::inherit::inherit(from, &data_dir);
            for problem in &report.problems {
                log::warn!("inheriting from {}: {problem}", from.display());
            }
            log::info!("inherited from {}: {report:?}", from.display());
            report
        });
        let key_file = data_dir.join(endpoint::KEY_FILE);
        let unkept = inherited.as_ref().is_some_and(|report| report.key_pending) && !key_file.exists();
        let key = if unkept {
            // The old release's key is there and could not be taken this time. A key made and kept now
            // would stand in its place for good and unpair every phone; one held only for this run
            // leaves the next start free to take it. The inherited phones are not on the list yet, so
            // nothing dials this run's key from outside, and the public side is refused below so that
            // nothing pairs with it either.
            log::warn!("serving this run under a key that is not kept, until the old release's key can be taken");
            iroh::SecretKey::generate()
        } else {
            endpoint::load_or_create_key(&key_file)?
        };
        // Which relay the public endpoint uses before the server has settings of its own: the old
        // release's, read by the inheritance (this start's report, or the marker of an earlier one).
        let inherited_relay = inherited
            .and_then(|report| report.relay)
            .or_else(|| crate::inherit::relay_hint(&data_dir));

        // First: a descriptor limit raised after a child exists is no use to that child.
        smetana_core::rlimit::raise();
        smetana_core::rlimit::report();
        smetana_core::context::install_runtime(tokio::runtime::Handle::current());
        shell_env::warm();

        let sink = Arc::new(ServerSink { remote: OnceLock::new() });
        let ctx = context(&options, sink.clone());
        let stored = ctx.settings_path().map(|path| settings::file::load(&path).0);
        // The last project the server had open, and no other: unlike the desktop it does not take the
        // directory it was started from, because a service has none worth taking.
        let initial = stored.as_ref().and_then(|stored| stored.last_project.clone()).map(PathBuf::from);
        let known: Vec<PathBuf> = stored
            .map(|stored| stored.open_projects)
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .chain(initial.clone())
            .collect();

        let tracker = tracker::service::start(ctx.clone(), initial);
        let terminal = terminal::service::start(ctx.clone(), known.clone());
        let sessions = session::service::start(ctx.clone());
        let runs = runs::service::start(ctx.clone(), tracker.clone(), terminal.clone(), sessions.clone(), known);

        let settings_path = ctx.settings_path();
        let relay = Box::new(move || relay_url(settings_path.as_deref(), inherited_relay.as_deref()));
        let remote = Remote::new(
            key,
            Devices::load(data_dir.join(devices::FILE_NAME)),
            Pairing::new(pairing::system_clock()),
            env!("CARGO_PKG_VERSION").to_owned(),
            // A pairing prompt reaches the terminal that asked through the hub, as an event; there is
            // no window to tell.
            Arc::new(|event| {
                if let Event::DevicesChanged(list) = event {
                    log::info!("remote: {} device(s) paired", list.len());
                }
            }),
            relay,
        );
        if unkept {
            remote.refuse_public(
                "Pairing is off until this server can read the key the old app kept; restart the server once that file is readable."
                    .to_owned(),
            );
        }
        let _ = sink.remote.set(Arc::downgrade(&remote));

        let restart = Arc::new(Notify::new());
        let stop = Arc::new(Notify::new());
        let gateway = {
            let (runs, terminal, sessions) = (runs.clone(), terminal.clone(), sessions.clone());
            let wake = restart.clone();
            let ending = stop.clone();
            let told = Arc::downgrade(&remote);
            Gateway::new(
                Box::new(move || {
                    let (runs, terminal, sessions) = (runs.clone(), terminal.clone(), sessions.clone());
                    Box::pin(async move { live_work(&runs, &terminal, &sessions).await })
                }),
                Arc::new(move || wake.notify_one()),
                Arc::new(move || ending.notify_one()),
                Arc::new(move |work| {
                    if let Some(remote) = told.upgrade() {
                        remote.hub.publish(DEFERRED_EVENT, &deferred_payload(work));
                    }
                }),
                options.restart_poll,
            )
        };

        remote.hub.set_dispatch(Arc::new(Services {
            tracker,
            runs,
            sessions: sessions.clone(),
            terminal: terminal.clone(),
            settings_path: settings::commands::settings_path(&ctx).map_err(|err| err.to_string()),
            attachments_root: attachments::store_root(&ctx).map_err(|err| err.to_string()),
            project_switched: Some(Services::announce_project_switch(ctx.clone())),
            ctx: Some(ctx),
            remote: Some(Arc::downgrade(&remote)),
            gateway: Some(gateway.clone()),
        }));

        let addr = remote.serve_local().await?;
        let credential = local::Credential {
            node: remote.public_key(),
            addrs: addr.ip_addrs().copied().collect(),
            secret: local::new_secret(),
        };
        remote.pairing.set_local(credential.secret);
        local::write(&data_dir, &credential)?;

        if remote.devices.has_remote() {
            if let Err(err) = remote.ensure_running(None).await {
                log::warn!("could not start the public endpoint: {err}");
            }
        }
        log::info!("serving {} as {}", data_dir.display(), remote.public_key());
        Ok(Server { remote, gateway, data_dir, addr, terminal, sessions, restart, stop, _lock: lock })
    }

    /// Resolves when the operator stops the server or the gateway restarts it.
    pub async fn wait(&self) -> Exit {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let (Ok(mut term), Ok(mut int)) = (signal(SignalKind::terminate()), signal(SignalKind::interrupt())) else {
                tokio::select! {
                    _ = self.restart.notified() => return Exit::Restart,
                    _ = self.stop.notified() => return Exit::Stop,
                }
            };
            tokio::select! {
                _ = term.recv() => Exit::Stop,
                _ = int.recv() => Exit::Stop,
                _ = self.stop.notified() => Exit::Stop,
                _ = self.restart.notified() => Exit::Restart,
            }
        }
        #[cfg(not(unix))]
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => Exit::Stop,
                _ = self.stop.notified() => Exit::Stop,
                _ = self.restart.notified() => Exit::Restart,
            }
        }
    }

    /// Ends every child the server owns, tells the connected devices, and removes the credential.
    pub async fn shutdown(self) {
        local::remove(&self.data_dir);
        self.remote.stop().await;
        let (terminal, sessions) = (self.terminal.clone(), self.sessions.clone());
        let _ = tokio::task::spawn_blocking(move || {
            terminal::service::shutdown(&terminal);
            session::service::shutdown(&sessions);
        })
        .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_the_server_has_settings_its_phones_are_served_on_the_relay_the_old_release_used() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let theirs = "https://relay.example.org";
        assert_eq!(relay_url(Some(&file), Some(theirs)), theirs, "no settings file yet: the inherited relay");
        assert_eq!(relay_url(Some(&file), None), DEFAULT_RELAY_URL);
        assert_eq!(relay_url(None, Some(theirs)), theirs);
        // Once the settings are the server's, they decide, whatever was inherited.
        std::fs::write(&file, r#"{"version":1,"remote":{"relayUrl":"https://relay.mine.org"}}"#).unwrap();
        assert_eq!(relay_url(Some(&file), Some(theirs)), "https://relay.mine.org");
    }

    #[test]
    fn root_is_refused_with_a_reason_and_anybody_else_is_not() {
        let message = refuse_root(0).unwrap_err();
        assert!(message.contains("refuses to run as root"), "{message}");
        assert!(refuse_root(1000).is_ok());
    }

    #[test]
    fn a_second_server_on_one_directory_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let first = lock_directory(dir.path()).unwrap();
        let err = lock_directory(dir.path()).unwrap_err();
        assert!(err.contains("already owns"), "{err}");
        drop(first);
        assert!(lock_directory(dir.path()).is_ok(), "the lock goes with the process that held it");
    }

    #[test]
    fn a_directory_is_owned_while_its_lock_is_held() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_owned(dir.path()), "no lock file at all");
        let held = lock_directory(dir.path()).unwrap();
        assert!(is_owned(dir.path()));
        drop(held);
        assert!(!is_owned(dir.path()), "the lock goes with the process that held it");
    }

    #[test]
    fn bd_is_the_flag_when_there_is_one() {
        assert_eq!(find_bd(Some(Path::new("/opt/bd"))), PathBuf::from("/opt/bd"));
    }
}
