//! The command bridge: the loop that serves a paired phone after the handshake,
//! and the one place events leave the desktop for both the window and the phones.
//!
//! A phone sends `Invoke`, `Listen` and `Unlisten` on the stream the handshake
//! used. An `Invoke` is checked against the allowlist in `remote_protocol` and
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
//! Events come from [`emit`], which every allowlisted event is emitted through:
//! it emits to the windows exactly as `app.emit` did and then offers the same
//! payload to the phones listening for it.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use iroh::endpoint::{Connection, RecvStream, SendStream};
use remote_protocol::{
    encode_frame, is_allowed_command, is_allowed_event, read_frame, BridgeError, ClientMsg, FrameError,
    PublicKey, ServerMsg, CLOSE_PROTOCOL, CLOSE_SLOW,
};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
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

/// Whatever calls the desktop's handlers on the bridge's behalf. Production is
/// `dispatch::Services`; a test stands in a recorder to prove that a refused
/// command never reaches this far.
pub trait Dispatch: Send + Sync {
    fn call<'a>(&'a self, cmd: &'a str, args: Value) -> BoxFuture<'a, Result<Value, BridgeError>>;
}

/// Every command the desktop registers with Tauri, allowlisted or not. It exists
/// to tell `Forbidden` from `UnknownCommand`, and `lib.rs`'s `generate_handler!`
/// is the truth it is held to: `desktop_commands_are_those_lib_rs_registers`
/// fails when the two drift, in either direction.
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
];

/// Emits an allowlisted event: to the windows, exactly as `app.emit` does, and to
/// every phone that has listened for it. The only way these events leave the
/// desktop, so that a phone and the window cannot be told different things.
///
/// An event that is not on the allowlist reaches the windows and no phone.
pub fn emit<S: Serialize + ?Sized>(app: &AppHandle, name: &str, payload: &S) {
    let _ = app.emit(name, payload);
    if let Some(remote) = app.try_state::<Arc<Remote>>() {
        remote.hub.publish(name, payload);
    }
}

struct Subscriber {
    events: HashSet<String>,
    queue: mpsc::Sender<ServerMsg>,
    conn: Connection,
}

/// The phones connected right now and what each listens to, plus the dispatcher
/// their calls go to. Owned by `Remote`.
#[derive(Default)]
pub struct Hub {
    subscribers: Mutex<HashMap<u64, Subscriber>>,
    next: AtomicU64,
    dispatch: OnceLock<Arc<dyn Dispatch>>,
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

    fn register(&self, queue: mpsc::Sender<ServerMsg>, conn: Connection) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.lock().insert(id, Subscriber { events: HashSet::new(), queue, conn });
        id
    }

    fn unregister(&self, id: u64) {
        self.lock().remove(&id);
    }

    fn listen(&self, id: u64, event: &str, on: bool) {
        if on && !is_allowed_event(event) {
            return;
        }
        if let Some(sub) = self.lock().get_mut(&id) {
            if on {
                sub.events.insert(event.to_owned());
            } else {
                sub.events.remove(event);
            }
        }
    }

    /// Offers an event to every phone listening for it. The payload is turned
    /// into JSON once, and not at all when nobody is listening.
    pub fn publish<S: Serialize + ?Sized>(&self, name: &str, payload: &S) {
        if !is_allowed_event(name) {
            return;
        }
        let mut subs = self.lock();
        if !subs.values().any(|sub| sub.events.contains(name)) {
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
            if !sub.events.contains(name) {
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

    #[cfg(test)]
    pub fn listeners(&self, event: &str) -> usize {
        self.lock().values().filter(|sub| sub.events.contains(event)).count()
    }
}

/// What a call to `cmd` answers when the command is not allowlisted.
pub fn refusal(cmd: &str) -> BridgeError {
    if DESKTOP_COMMANDS.contains(&cmd) {
        BridgeError::Forbidden
    } else {
        BridgeError::UnknownCommand
    }
}

/// One call, from the allowlist check to the answer. The only way a request
/// becomes a handler call.
pub async fn invoke(dispatch: Option<&Arc<dyn Dispatch>>, cmd: &str, args: Value) -> Result<Value, BridgeError> {
    if !is_allowed_command(cmd) {
        return Err(refusal(cmd));
    }
    match dispatch {
        Some(dispatch) => dispatch.call(cmd, args).await,
        None => Err(BridgeError::Internal("the desktop is not ready to take calls".into())),
    }
}

/// Serves one phone on its stream until the stream ends, the device is revoked or
/// the phone sends something that is not a message. Returns with the connection
/// still open when the phone merely stopped sending; the caller decides.
pub(crate) async fn serve(
    remote: &Arc<Remote>,
    conn: &Connection,
    key: &PublicKey,
    send: SendStream,
    mut recv: RecvStream,
) {
    let (queue, outgoing) = mpsc::channel::<ServerMsg>(QUEUE);
    let id = remote.hub.register(queue.clone(), conn.clone());
    let writer = tokio::spawn(write_loop(send, outgoing));
    let mut calls = JoinSet::new();
    loop {
        let msg = match read_frame::<ClientMsg>(&mut recv, CLIENT_FRAME_LIMIT).await {
            Ok(msg) => msg,
            // The phone closed its side, or the connection went.
            Err(FrameError::Io(_)) => break,
            Err(err) => {
                log::debug!("remote: a phone sent a frame that is not a message: {err}");
                conn.close(CLOSE_PROTOCOL.into(), b"not a message");
                break;
            }
        };
        match msg {
            ClientMsg::Listen { event } => remote.hub.listen(id, &event, true),
            ClientMsg::Unlisten { event } => remote.hub.listen(id, &event, false),
            ClientMsg::UpdatePushToken { token } => {
                if token.is_empty() || token.len() > super::endpoint::TOKEN_LIMIT {
                    log::debug!("remote: a phone sent a push token of an unusable size");
                } else if let Err(err) = remote.devices.set_push_token(key, token) {
                    log::warn!("remote: could not save a phone's new push token: {err}");
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
                let key = *key;
                calls.spawn(async move {
                    // A revoked device may have a connection that has not been
                    // closed yet; the list is the authority, not the connection.
                    let result = if remote.devices.contains(&key) {
                        invoke(remote.hub.dispatch.get(), &cmd, args).await
                    } else {
                        Err(BridgeError::Forbidden)
                    };
                    let _ = queue.send(ServerMsg::Reply { id: call, result }).await;
                });
            }
        }
    }
    remote.hub.unregister(id);
    // Calls still running belong to a phone that is gone; their answers have
    // nowhere to go. What they already asked of the workers is not undone.
    calls.abort_all();
    drop(queue);
    let _ = writer.await;
}

async fn write_loop(mut send: SendStream, mut outgoing: mpsc::Receiver<ServerMsg>) {
    while let Some(msg) = outgoing.recv().await {
        let frame = match encode_frame(&msg) {
            Ok(frame) => frame,
            // A reply too large for a frame becomes an error the phone can read,
            // rather than a call that is never answered. An event that large is
            // dropped: the phone's next snapshot covers it.
            Err(FrameError::TooLarge(size)) => match msg {
                ServerMsg::Reply { id, .. } => {
                    let result = Err(BridgeError::Internal(format!(
                        "the reply is {size} bytes, more than one frame holds"
                    )));
                    match encode_frame(&ServerMsg::Reply { id, result }) {
                        Ok(frame) => frame,
                        Err(_) => continue,
                    }
                }
                ServerMsg::Event { name, .. } => {
                    log::warn!("remote: event {name} of {size} bytes is too large to send");
                    continue;
                }
            },
            Err(err) => {
                log::warn!("remote: a message to a phone does not encode: {err}");
                continue;
            }
        };
        if let Err(err) = send.write_all(&frame).await {
            log::debug!("remote: could not write to a phone: {err}");
            return;
        }
    }
    let _ = send.finish();
}
