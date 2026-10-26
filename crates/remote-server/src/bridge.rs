//! The command bridge: the loop that serves a paired device after the handshake,
//! and the one place events leave the desktop for both the window and the devices.
//!
//! A device sends `Invoke`, `Listen` and `Unlisten` on the stream the handshake
//! used. An `Invoke` is checked against the list `remote_protocol` holds for the device's
//! kind (a phone's allowlist, or every server command for a desktop) and
//! only then handed to a [`Dispatch`], which calls the same function the Tauri
//! command of that name calls. A command that is not on the list never reaches
//! the dispatcher: the answer is `Forbidden` (the desktop has a command of that
//! name) or `UnknownCommand` (it has not), and nothing runs.
//!
//! Calls run concurrently, because one can take seconds (a bd write) and the
//! phone should not wait behind it to read a snapshot. Replies carry the phone's
//! own id and arrive in any order. Everything written to the stream goes through
//! one task and one bounded queue, which is also what an event is put on: a phone
//! that cannot keep up is closed (`CLOSE_SLOW`) and reads its snapshots again on
//! reconnecting, as after any dropped connection. Events are never buffered
//! without bound on behalf of a phone that has stopped reading.
//!
//! Events come from the context's sink, which every event is emitted through:
//! it emits to the windows exactly as `app.emit` did and then offers the same
//! payload to the devices listening for it.
//!
//! **Terminal output is the exception, and the reason is its volume.** It does not travel as
//! one event per chunk through the queue above. A device that attaches to a session
//! (`terminal_attach`) is registered for that session's output, and each chunk the worker commits
//! goes into a per-device, per-session buffer ([`Outbox`]) that the connection's writer drains
//! whenever it can write: chunks that piled up while the connection was busy leave as one
//! event, so output is coalesced per session and per device. The buffer has a ceiling. A device
//! that falls behind by more than that has the buffer dropped and receives the session's ring
//! afresh (`terminal:output` with `reset: true`) in place of an ever-growing queue, and none of
//! this waits for the device: the worker's `publish_terminal_chunk` never blocks.

use std::collections::{HashMap, HashSet, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use iroh::endpoint::{Connection, RecvStream, SendStream};
use remote_protocol::{
    encode_frame, is_command_allowed_for, is_event_allowed_for, read_frame, BridgeError, ClientMsg,
    DeviceKind, FrameError, PublicKey, ServerMsg, CLOSE_PROTOCOL, CLOSE_SLOW,
};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Notify};
use tokio::task::JoinSet;

use super::Remote;

/// The largest request a phone may send. Arguments are a few hundred bytes; the
/// largest in practice is the text of a message to an agent.
const CLIENT_FRAME_LIMIT: usize = 1 << 20;
/// Messages waiting to be written to one phone. A phone that lets this fill with
/// events is too slow to be kept in step and is closed.
const QUEUE: usize = 512;
/// Calls one phone may have running at once.
const MAX_IN_FLIGHT: usize = 32;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Who is calling: the connection (its number is what the worker keeps a terminal's size by) and
/// what the device may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caller {
    pub id: u64,
    pub kind: DeviceKind,
}

/// Whatever calls the desktop's handlers on the bridge's behalf. Production is
/// `dispatch::Services`; a test stands in a recorder to prove that a refused
/// command never reaches this far.
pub trait Dispatch: Send + Sync {
    fn call<'a>(
        &'a self,
        caller: &'a Caller,
        cmd: &'a str,
        args: Value,
    ) -> BoxFuture<'a, Result<Value, BridgeError>>;

    /// Whether this host can honour `cmd`. A command it cannot is left out of the capabilities it
    /// advertises, so a client does not draw a control that can only fail. Everything by default.
    fn serves(&self, _cmd: &str) -> bool {
        true
    }

    /// The ring of a terminal session as of now: the number of the last chunk it holds, and the
    /// bytes in base64. What a device that fell behind is sent in place of the chunks it missed.
    /// `None` when the session is gone, or when this dispatcher has no terminals.
    fn terminal_ring<'a>(&'a self, _session: u64) -> BoxFuture<'a, Option<(u64, String)>> {
        Box::pin(async { None })
    }

    /// A device's connection ended: whatever the workers keep for it can go.
    fn client_gone<'a>(&'a self, _caller: &'a Caller) -> BoxFuture<'a, ()> {
        Box::pin(async {})
    }
}

/// Every command the desktop's webview can call, allowlisted or not: the shell answers the ones it
/// registers itself (`src-tauri/src/lib.rs`'s `generate_handler!`) and forwards the rest to the
/// server. It exists to tell `Forbidden` from `UnknownCommand`, and the shell's registrations are
/// what it is held to: `remote_bridge_desktop_commands_cover_what_lib_rs_registers` fails when a
/// registered command is missing here or a listed one is neither registered nor the server's. Each
/// of them is either a server command
/// (`remote_protocol::SERVER_COMMANDS`) or a [`CLIENT_COMMANDS`] one, and
/// `every_desktop_command_is_a_server_command_or_a_client_one` fails for a new command that
/// is neither, so that choosing is not skipped.
pub const DESKTOP_COMMANDS: &[&str] = &[
    "tracker_health",
    "tracker_snapshot",
    "tracker_resync",
    "tracker_update",
    "tracker_close",
    "tracker_reopen",
    "tracker_delete",
    "tracker_set_project",
    "tracker_init",
    "tracker_repair",
    "tracker_failure",
    "tracker_access_repair",
    "tracker_access_reset",
    "tracker_probe",
    "tracker_search_semantic",
    "project_root",
    "files_list",
    "files_read",
    "files_write",
    "files_stat",
    "files_create",
    "files_mkdir",
    "files_trash",
    "files_copy",
    "files_copy_external",
    "files_move",
    "files_rename",
    "files_clipboard_write",
    "files_clipboard_read",
    "attachment_import",
    "attachment_reopen",
    "attachment_write",
    "attachments_survey",
    "attachments_clean",
    "image_read",
    "git_head",
    "vcs_repos",
    "vcs_status",
    "vcs_in_progress",
    "vcs_branches",
    "vcs_remote_branches",
    "vcs_last_fetch",
    "vcs_tracking",
    "vcs_fetch",
    "vcs_pull",
    "vcs_push",
    "vcs_checkout",
    "vcs_checkout_remote",
    "vcs_create_branch",
    "vcs_rename_branch",
    "vcs_delete_branch",
    "vcs_merge",
    "vcs_rebase",
    "vcs_abort",
    "vcs_commit",
    "vcs_discard",
    "vcs_suggest_message",
    "vcs_file_at_head",
    "vcs_file_at_rev",
    "vcs_compare",
    "project_config",
    "project_config_save_defaults",
    "browser_tools",
    "run_start",
    "run_stop",
    "run_release",
    "run_state",
    "run_reports",
    "target_branches",
    "agent_usage",
    "agents_catalog",
    "codex_models",
    "agents_installed",
    "sessions_list",
    "sessions_open_log",
    "sessions_open_cwd",
    "sessions_reveal",
    "sessions_delete",
    "settings_load",
    "settings_save",
    "settings_window_open",
    "window_chrome",
    "home_dir",
    "drag_drop_space",
    "compare_window_open",
    "image_window_open",
    "dialog_window_open",
    "dialog_window_close",
    "dialog_window_size",
    "dialog_window_closable",
    "window_show_ready",
    "autostart_state",
    "autostart_set",
    "updates_state",
    "updates_check",
    "updates_install",
    "terminal_list",
    "terminal_marks",
    "terminal_create",
    "terminal_shell",
    "terminal_remove",
    "terminal_attach",
    "terminal_detach",
    "terminal_resize",
    "terminal_write",
    "terminal_clear",
    "terminal_run_capture",
    "terminal_restorable",
    "terminal_forget",
    "session_start",
    "crew_tree",
    "crew_attach",
    "crew_send",
    "crew_stop",
    "crew_clear",
    "session_attach",
    "session_since",
    "session_send",
    "session_answer",
    "session_stop",
    "session_close",
    "remote_pair_begin",
    "remote_pair_cancel",
    "remote_pair_decide",
    "remote_devices",
    "remote_device_revoke",
    "connection_state",
    "connection_retry",
    "device_settings_load",
    "device_settings_save",
    // Which server this device uses, and the files on this machine that a remote server cannot reach.
    "server_target",
    "server_use_local",
    "server_use_remote",
    "server_local_work",
    "client_file_read",
    "client_open_document",
    "client_open_fetched",
];

/// The commands that act on the machine the person sits at, and so are not the server's: the
/// windows and their chrome, the dialogs, the system clipboard, the opener, the client's own
/// updater and login item, and the tracker's access reset, which ends in a restart of the app.
/// A device that calls one is answered `Forbidden`.
#[cfg_attr(not(test), allow(dead_code))]
pub const CLIENT_COMMANDS: &[&str] = &[
    "tracker_access_reset",
    "files_clipboard_write",
    "files_clipboard_read",
    "sessions_open_log",
    "sessions_open_cwd",
    "sessions_reveal",
    "settings_window_open",
    "window_chrome",
    "drag_drop_space",
    "compare_window_open",
    "image_window_open",
    "dialog_window_open",
    "dialog_window_close",
    "dialog_window_size",
    "dialog_window_closable",
    "window_show_ready",
    "autostart_state",
    "autostart_set",
    "updates_state",
    "updates_check",
    "updates_install",
    // The desktop's own connection to its server (`src-tauri/src/connection.rs`): there is nothing
    // of the server's to ask, so a device that calls one is `Forbidden`.
    "connection_state",
    "connection_retry",
    // The client build's own half of the settings (`settings/split.rs`): the file on this machine.
    "device_settings_load",
    "device_settings_save",
    // Which server this device uses, and the files on this machine that a remote server cannot reach.
    "server_target",
    "server_use_local",
    "server_use_remote",
    "server_local_work",
    "client_file_read",
    "client_open_document",
    "client_open_fetched",
];

/// What the output buffer of one device for one session may hold before the device is sent the
/// ring afresh instead. Half a megabyte: a few hundred screens of text, which a connection that
/// is merely slow clears in a moment, and far less than the ring the fresh start costs.
pub const OUTBOX_CAP: usize = 512 * 1024;

/// One device's follow of one terminal session.
struct Watch {
    /// Not drained until the reply to the attach is on its way: the device must hear the ring
    /// before the chunks that follow it.
    armed: bool,
    /// The number of the last chunk the device has already been given in a reply; chunks at or
    /// below it are in what it holds.
    floor: u64,
    chunks: VecDeque<(u64, Vec<u8>)>,
    bytes: usize,
    /// The buffer overflowed and was dropped; the device gets the ring before anything else.
    resync: bool,
    /// The newest chunk the overflow threw away. A ring older than this would leave a gap.
    dropped_through: u64,
}

/// What one device has not yet been sent of the sessions it follows.
#[derive(Default)]
pub struct Outbox {
    watches: Mutex<HashMap<u64, Watch>>,
    wake: Notify,
    cap: AtomicUsize,
}

impl Outbox {
    fn new(cap: usize) -> Self {
        Self { watches: Mutex::default(), wake: Notify::new(), cap: AtomicUsize::new(cap) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Watch>> {
        self.watches.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Starts following `session`, buffering its chunks but sending none until [`Outbox::arm`].
    fn watch(&self, session: u64) {
        self.lock().insert(
            session,
            Watch {
                armed: false,
                floor: 0,
                chunks: VecDeque::new(),
                bytes: 0,
                resync: false,
                dropped_through: 0,
            },
        );
    }

    fn unwatch(&self, session: u64) {
        self.lock().remove(&session);
    }

    /// The reply to an attach carried the ring up to chunk `seq`: what is buffered at or below
    /// it is in that reply already.
    fn set_floor(&self, session: u64, seq: u64) {
        if let Some(watch) = self.lock().get_mut(&session) {
            watch.floor = seq;
            watch.chunks.retain(|(held, _)| *held > seq);
            watch.bytes = watch.chunks.iter().map(|(_, bytes)| bytes.len()).sum();
        }
    }

    fn arm(&self, session: u64) {
        if let Some(watch) = self.lock().get_mut(&session) {
            watch.armed = true;
        }
        self.wake.notify_one();
    }

    fn push(&self, session: u64, seq: u64, bytes: &[u8]) {
        let cap = self.cap.load(Ordering::Relaxed);
        let mut watches = self.lock();
        let Some(watch) = watches.get_mut(&session) else { return };
        if seq <= watch.floor {
            return;
        }
        watch.chunks.push_back((seq, bytes.to_vec()));
        watch.bytes += bytes.len();
        if watch.bytes > cap {
            // The device is not keeping up. What it has not been sent is dropped, not queued
            // further, and it will be given the ring in its place.
            watch.chunks.clear();
            watch.bytes = 0;
            watch.resync = true;
            watch.dropped_through = seq;
        }
        let armed = watch.armed;
        drop(watches);
        if armed {
            self.wake.notify_one();
        }
    }

    /// Whether anything is waiting to be sent.
    #[cfg(test)]
    fn buffered_bytes(&self) -> usize {
        self.lock().values().map(|watch| watch.bytes).sum()
    }

    /// What is waiting, as events: per session, every chunk that piled up as one; and for a
    /// session whose buffer overflowed, a fresh ring from `dispatch`.
    async fn drain(&self, dispatch: Option<&Arc<dyn Dispatch>>) -> Vec<ServerMsg> {
        let mut out = Vec::new();
        let mut resyncs = Vec::new();
        {
            let mut watches = self.lock();
            for (session, watch) in watches.iter_mut() {
                if !watch.armed {
                    continue;
                }
                if watch.resync {
                    resyncs.push(*session);
                    continue;
                }
                let (Some(&(from, _)), Some(&(seq, _))) = (watch.chunks.front(), watch.chunks.back()) else {
                    continue;
                };
                let mut joined = Vec::with_capacity(watch.bytes);
                for (_, bytes) in watch.chunks.drain(..) {
                    joined.extend_from_slice(&bytes);
                }
                watch.bytes = 0;
                out.push(output_event(*session, from, seq, &joined));
            }
        }
        for session in resyncs {
            let ring = match dispatch {
                Some(dispatch) => dispatch.terminal_ring(session).await,
                None => None,
            };
            let mut watches = self.lock();
            let Some(ring) = ring else {
                // The session is gone, or there is nowhere to ask: there is nothing to follow.
                watches.remove(&session);
                continue;
            };
            let (seq, data) = ring;
            let Some(watch) = watches.get_mut(&session) else { continue };
            if watch.dropped_through > seq {
                // Overflowed again while the ring was on its way, and it is older than what was
                // dropped: ask once more rather than leave a gap.
                drop(watches);
                self.wake.notify_one();
                continue;
            }
            watch.floor = seq;
            watch.chunks.retain(|(held, _)| *held > seq);
            watch.bytes = watch.chunks.iter().map(|(_, bytes)| bytes.len()).sum();
            watch.resync = false;
            let more = !watch.chunks.is_empty();
            drop(watches);
            out.push(ServerMsg::Event {
                name: "terminal:output".into(),
                payload: output_payload(session, seq, seq, &data, true),
            });
            if more {
                self.wake.notify_one();
            }
        }
        out
    }
}

/// The payload of a `terminal:output` event to a device: the window's own, and for a fresh ring
/// one more key.
///
/// `from` is the number of the first chunk the data holds and `seq` the last, so a run of chunks
/// that were coalesced is `from..=seq` and a client can tell it is contiguous with what it has
/// (`from == last + 1`). A reset holds the ring as of `seq`, so `from == seq`, and `reset: true`
/// says it replaces the terminal instead of continuing it. The window's own events (no `from`)
/// are one chunk each and keep their shape.
pub(crate) fn output_payload(session: u64, from: u64, seq: u64, data: &str, reset: bool) -> Value {
    let mut payload = crate::terminal::service::output_payload(session, seq, data);
    payload["from"] = json!(from);
    if reset {
        payload["reset"] = json!(true);
    }
    payload
}

fn output_event(session: u64, from: u64, seq: u64, bytes: &[u8]) -> ServerMsg {
    let data = data_encoding::BASE64.encode(bytes);
    ServerMsg::Event { name: "terminal:output".into(), payload: output_payload(session, from, seq, &data, false) }
}

struct Subscriber {
    kind: DeviceKind,
    events: HashSet<String>,
    queue: mpsc::Sender<ServerMsg>,
    conn: Connection,
    outbox: Arc<Outbox>,
}

/// The devices connected right now and what each listens to, plus the dispatcher
/// their calls go to. Owned by `Remote`.
pub struct Hub {
    subscribers: Mutex<HashMap<u64, Subscriber>>,
    next: AtomicU64,
    dispatch: OnceLock<Arc<dyn Dispatch>>,
    outbox_cap: AtomicUsize,
}

impl Default for Hub {
    fn default() -> Self {
        Self {
            subscribers: Mutex::default(),
            // Number 0 is the window of this process, in the worker's terms.
            next: AtomicU64::new(1),
            dispatch: OnceLock::new(),
            outbox_cap: AtomicUsize::new(OUTBOX_CAP),
        }
    }
}

impl Hub {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<u64, Subscriber>> {
        self.subscribers.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Sets what calls are dispatched to. Once: a second call is ignored, since
    /// the first is the one connections already made are using.
    pub fn set_dispatch(&self, dispatch: Arc<dyn Dispatch>) {
        let _ = self.dispatch.set(dispatch);
    }

    /// What a device of `kind` is told it may do: its capabilities, less the commands this host's
    /// dispatcher says it cannot serve (a command it cannot run, an event it can never emit).
    pub fn capabilities_for(&self, kind: DeviceKind) -> Vec<String> {
        let dispatch = self.dispatch.get();
        remote_protocol::capabilities_for(kind)
            .into_iter()
            .filter(|name| dispatch.map_or(true, |dispatch| dispatch.serves(name)))
            .collect()
    }

    /// Sets the ceiling on what one device may have buffered for one session, for connections
    /// made after the call. A test makes it small; nothing else changes it.
    #[cfg(test)]
    pub fn set_outbox_cap(&self, cap: usize) {
        self.outbox_cap.store(cap, Ordering::Relaxed);
    }

    fn register(&self, queue: mpsc::Sender<ServerMsg>, conn: Connection, kind: DeviceKind) -> (u64, Arc<Outbox>) {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let outbox = Arc::new(Outbox::new(self.outbox_cap.load(Ordering::Relaxed)));
        self.lock().insert(
            id,
            Subscriber { kind, events: HashSet::new(), queue, conn, outbox: outbox.clone() },
        );
        (id, outbox)
    }

    fn unregister(&self, id: u64) {
        self.lock().remove(&id);
    }

    fn listen(&self, id: u64, event: &str, on: bool) {
        if let Some(sub) = self.lock().get_mut(&id) {
            if on {
                if is_event_allowed_for(sub.kind, event) {
                    sub.events.insert(event.to_owned());
                }
            } else {
                sub.events.remove(event);
                if event == "terminal:output" {
                    sub.outbox.lock().clear();
                }
            }
        }
    }

    /// Offers an event to every device listening for it. The payload is turned
    /// into JSON once, and not at all when nobody is listening.
    ///
    /// Terminal output and log are not offered here: they are volume, and go through
    /// [`Hub::publish_terminal_chunk`] and [`Hub::publish_terminal_log`] to the devices that
    /// follow the session.
    pub fn publish<S: Serialize + ?Sized>(&self, name: &str, payload: &S) {
        if matches!(name, "terminal:output" | "terminal:log") {
            return;
        }
        let mut subs = self.lock();
        if !subs.values().any(|sub| sub.events.contains(name) && is_event_allowed_for(sub.kind, name)) {
            return;
        }
        let payload = match serde_json::to_value(payload) {
            Ok(value) => value,
            Err(err) => {
                log::warn!("remote: event {name} does not serialize: {err}");
                return;
            }
        };
        let mut dropped = Vec::new();
        for (id, sub) in subs.iter() {
            if !sub.events.contains(name) || !is_event_allowed_for(sub.kind, name) {
                continue;
            }
            let msg = ServerMsg::Event { name: name.to_owned(), payload: payload.clone() };
            match sub.queue.try_send(msg) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    sub.conn.close(CLOSE_SLOW.into(), b"too slow");
                    dropped.push(*id);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => dropped.push(*id),
            }
        }
        for id in dropped {
            subs.remove(&id);
        }
    }

    /// One committed chunk of a session's output, for every device that follows that session
    /// (attached to it; only a desktop device can). Listening is not needed, and unlistening
    /// ends every follow. Never waits: a device that cannot keep up
    /// loses its buffer and is sent the ring (see [`Outbox`]), and the worker is not told.
    pub fn publish_terminal_chunk(&self, session: u64, seq: u64, bytes: &[u8]) {
        for sub in self.lock().values() {
            sub.outbox.push(session, seq, bytes);
        }
    }

    /// New entries of a timeline session's log, for the devices that follow it. They are few and small, and take the ordinary queue.
    pub fn publish_terminal_log(&self, session: u64, entries: &Value) {
        let mut dropped = Vec::new();
        let mut subs = self.lock();
        for (id, sub) in subs.iter() {
            if !sub.outbox.lock().get(&session).is_some_and(|w| w.armed) {
                continue;
            }
            let msg = ServerMsg::Event {
                name: "terminal:log".into(),
                payload: crate::terminal::service::log_payload(session, entries),
            };
            match sub.queue.try_send(msg) {
                Ok(()) => {}
                Err(mpsc::error::TrySendError::Full(_)) => {
                    sub.conn.close(CLOSE_SLOW.into(), b"too slow");
                    dropped.push(*id);
                }
                Err(mpsc::error::TrySendError::Closed(_)) => dropped.push(*id),
            }
        }
        for id in dropped {
            subs.remove(&id);
        }
    }

    #[cfg(test)]
    pub fn listeners(&self, event: &str) -> usize {
        self.lock().values().filter(|sub| sub.events.contains(event)).count()
    }

    /// The bytes one connection has buffered for sessions it follows.
    #[cfg(test)]
    pub fn buffered_bytes(&self) -> usize {
        self.lock().values().map(|sub| sub.outbox.buffered_bytes()).sum()
    }
}

/// What a call to `cmd` answers when the device's kind may not call it.
pub fn refusal(cmd: &str) -> BridgeError {
    if DESKTOP_COMMANDS.contains(&cmd) || remote_protocol::SERVER_ONLY_COMMANDS.contains(&cmd) {
        BridgeError::Forbidden
    } else {
        BridgeError::UnknownCommand
    }
}

/// One call, from the kind's list to the answer. The only way a request
/// becomes a handler call.
pub async fn invoke(
    dispatch: Option<&Arc<dyn Dispatch>>,
    caller: &Caller,
    cmd: &str,
    args: Value,
) -> Result<Value, BridgeError> {
    if !is_command_allowed_for(caller.kind, cmd) {
        return Err(refusal(cmd));
    }
    match dispatch {
        Some(dispatch) => dispatch.call(caller, cmd, args).await,
        None => Err(BridgeError::Internal("the desktop is not ready to take calls".into())),
    }
}

/// [`invoke`], and for a desktop device the bookkeeping that makes `terminal_attach` and
/// `terminal_detach` start and stop its follow of the session. The `Option` is the session to
/// arm once the reply is on its way.
async fn invoke_following(
    dispatch: Option<&Arc<dyn Dispatch>>,
    caller: &Caller,
    outbox: &Outbox,
    cmd: &str,
    args: Value,
) -> (Result<Value, BridgeError>, Option<u64>) {
    let session = args.get("id").and_then(Value::as_u64);
    let follows = caller.kind == DeviceKind::Desktop && is_command_allowed_for(caller.kind, cmd);
    match (follows, cmd, session) {
        (true, "terminal_detach", Some(session)) => outbox.unwatch(session),
        // Before the call, not after: a chunk committed between the worker answering and this
        // side registering would otherwise be nobody's.
        (true, "terminal_attach", Some(session)) => outbox.watch(session),
        _ => {}
    }
    let result = invoke(dispatch, caller, cmd, args).await;
    let mut arm = None;
    if let (true, "terminal_attach", Some(session)) = (follows, cmd, session) {
        match &result {
            Ok(reply) => {
                if let Some(seq) = reply.get("seq").and_then(Value::as_u64) {
                    outbox.set_floor(session, seq);
                }
                arm = Some(session);
            }
            Err(_) => outbox.unwatch(session),
        }
    }
    (result, arm)
}

/// Serves one device on its stream until the stream ends, the device is revoked or
/// the device sends something that is not a message. Returns with the connection
/// still open when the device merely stopped sending; the caller decides.
pub(crate) async fn serve(
    remote: &Arc<Remote>,
    conn: &Connection,
    key: &PublicKey,
    kind: DeviceKind,
    send: SendStream,
    mut recv: RecvStream,
) {
    let (queue, outgoing) = mpsc::channel::<ServerMsg>(QUEUE);
    let (id, outbox) = remote.hub.register(queue.clone(), conn.clone(), kind);
    let caller = Caller { id, kind };
    let writer = tokio::spawn(write_loop(send, outgoing, outbox.clone(), remote.hub.dispatch.get().cloned()));
    let mut calls = JoinSet::new();
    loop {
        let msg = match read_frame::<ClientMsg>(&mut recv, CLIENT_FRAME_LIMIT).await {
            Ok(msg) => msg,
            // The device closed its side, or the connection went.
            Err(FrameError::Io(_)) => break,
            Err(err) => {
                log::debug!("remote: a device sent a frame that is not a message: {err}");
                conn.close(CLOSE_PROTOCOL.into(), b"not a message");
                break;
            }
        };
        match msg {
            ClientMsg::Listen { event } => remote.hub.listen(id, &event, true),
            ClientMsg::Unlisten { event } => remote.hub.listen(id, &event, false),
            ClientMsg::UpdatePushToken { token } => {
                if token.is_empty() || token.len() > super::endpoint::TOKEN_LIMIT {
                    log::debug!("remote: a device sent a push token of an unusable size");
                } else if let Err(err) = remote.devices.set_push_token(key, token) {
                    log::warn!("remote: could not save a device's new push token: {err}");
                }
            }
            ClientMsg::Invoke { id: call, cmd, args } => {
                while calls.try_join_next().is_some() {}
                if calls.len() >= MAX_IN_FLIGHT {
                    let result = Err(BridgeError::Internal("too many calls in flight".into()));
                    let _ = queue.send(ServerMsg::Reply { id: call, result }).await;
                    continue;
                }
                let remote = remote.clone();
                let queue = queue.clone();
                let outbox = outbox.clone();
                let key = *key;
                calls.spawn(async move {
                    // A revoked device may have a connection that has not been
                    // closed yet; the list is the authority, not the connection. So is
                    // its kind: a device paired again as something else is not served as
                    // what it was.
                    let (result, arm) = if remote.devices.kind_of(&key) == Some(caller.kind) {
                        invoke_following(remote.hub.dispatch.get(), &caller, &outbox, &cmd, args).await
                    } else {
                        (Err(BridgeError::Forbidden), None)
                    };
                    let _ = queue.send(ServerMsg::Reply { id: call, result }).await;
                    if let Some(session) = arm {
                        outbox.arm(session);
                    }
                });
            }
        }
    }
    remote.hub.unregister(id);
    // Calls still running belong to a device that is gone; their answers have
    // nowhere to go. What they already asked of the workers is not undone.
    calls.abort_all();
    if let Some(dispatch) = remote.hub.dispatch.get() {
        dispatch.client_gone(&caller).await;
    }
    drop(queue);
    let _ = writer.await;
}

async fn write_loop(
    mut send: SendStream,
    mut outgoing: mpsc::Receiver<ServerMsg>,
    outbox: Arc<Outbox>,
    dispatch: Option<Arc<dyn Dispatch>>,
) {
    loop {
        let batch = tokio::select! {
            biased;
            msg = outgoing.recv() => match msg {
                Some(msg) => vec![msg],
                None => break,
            },
            () = outbox.wake.notified() => outbox.drain(dispatch.as_ref()).await,
        };
        for msg in batch {
            if !write_msg(&mut send, msg).await {
                return;
            }
        }
    }
    let _ = send.finish();
}

/// `false` when the stream can no longer be written to.
async fn write_msg(send: &mut SendStream, msg: ServerMsg) -> bool {
    let frame = match encode_frame(&msg) {
        Ok(frame) => frame,
        // A reply too large for a frame becomes an error the device can read,
        // rather than a call that is never answered. An event that large is
        // dropped: the device's next snapshot covers it.
        Err(FrameError::TooLarge(size)) => match msg {
            ServerMsg::Reply { id, .. } => {
                let result = Err(BridgeError::Internal(format!(
                    "the reply is {size} bytes, more than one frame holds"
                )));
                match encode_frame(&ServerMsg::Reply { id, result }) {
                    Ok(frame) => frame,
                    Err(_) => return true,
                }
            }
            ServerMsg::Event { name, .. } => {
                log::warn!("remote: event {name} of {size} bytes is too large to send");
                return true;
            }
        },
        Err(err) => {
            log::warn!("remote: a message to a device does not encode: {err}");
            return true;
        }
    };
    if let Err(err) = send.write_all(&frame).await {
        log::debug!("remote: could not write to a device: {err}");
        return false;
    }
    true
}
