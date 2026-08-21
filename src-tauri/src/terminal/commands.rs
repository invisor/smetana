//! The commands are deliberately thin: they put a request on the worker's
//! queue and wait for the reply — exactly as the tracker's do. The outer
//! Result is about delivery to the worker, the inner one (where there is one)
//! about the operation itself.

use tauri::State;
use tokio::sync::oneshot;

use super::model::{Session, SessionId, SessionMark, TerminalError};
use super::service::{Attached, Request, TerminalHandle};
use crate::agents::Intent;

async fn ask<T>(
    handle: &TerminalHandle,
    make: impl FnOnce(oneshot::Sender<T>) -> Request,
) -> Result<T, TerminalError> {
    let (tx, rx) = oneshot::channel();
    handle
        .0
        .send(make(tx))
        .await
        .map_err(|_| TerminalError::Spawn("the terminal worker is not running".into()))?;
    rx.await
        .map_err(|_| TerminalError::Spawn("the terminal worker did not answer".into()))
}

/// The requests that carry no reply channel: nothing to wait for, so only
/// delivery to the worker can fail.
async fn tell(handle: &TerminalHandle, request: Request) -> Result<(), TerminalError> {
    handle
        .0
        .send(request)
        .await
        .map_err(|_| TerminalError::Spawn("the terminal worker is not running".into()))
}

#[tauri::command]
pub async fn terminal_list(
    handle: State<'_, TerminalHandle>,
    project: String,
) -> Result<Vec<Session>, TerminalError> {
    ask(&handle, |tx| Request::List(project, tx)).await
}

/// Every session the worker holds, of every project, as the project rail draws
/// them. Read once when a window opens. There is no second read and no polling:
/// the `terminal:state` and `terminal:removed` events are emitted for every
/// session of every project already, so the front end maintains this from
/// them — this command exists because a window that has just opened has been
/// told about nothing.
#[tauri::command]
pub async fn terminal_marks(
    handle: State<'_, TerminalHandle>,
) -> Result<Vec<SessionMark>, TerminalError> {
    ask(&handle, Request::Marks).await
}

/// `intent` is why the session is being started; the profile turns it into a
/// command line. `agent` is the id from settings — an unknown or uninstalled
/// one falls back to whatever is installed rather than failing.
#[tauri::command]
pub async fn terminal_create(
    handle: State<'_, TerminalHandle>,
    project: String,
    agent: String,
    intent: Intent,
) -> Result<Session, TerminalError> {
    ask(&handle, |tx| Request::Create(project, agent, intent, tx)).await?
}

/// A shell in the project, with no agent and no intent behind it. Its own
/// command rather than an `Intent` variant on the one above: `terminal_create`
/// takes an agent id and turns it into a command line through a profile, and a
/// shell has none of that — the request it puts on the queue is a different one.
///
/// `cwd` is a path relative to the project's root, or absent for the root
/// itself. Absent is what the `+` menu's "New terminal" means and what this
/// command meant before the file tree's own menu could name a folder; a path
/// that is not a folder inside the root is refused and no session is made, in
/// `shell_cwd`.
#[tauri::command]
pub async fn terminal_shell(
    handle: State<'_, TerminalHandle>,
    project: String,
    cwd: Option<String>,
) -> Result<Session, TerminalError> {
    ask(&handle, |tx| Request::CreateShell(project, cwd, tx)).await?
}

#[tauri::command]
pub async fn terminal_remove(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
) -> Result<(), TerminalError> {
    ask(&handle, |tx| Request::Remove(id, tx)).await
}

/// The ring snapshot plus the current `seq`: everything the session has said,
/// and the number output arriving after it continues from.
#[tauri::command]
pub async fn terminal_attach(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
) -> Result<Attached, TerminalError> {
    ask(&handle, |tx| Request::Attach(id, tx)).await?
}

/// Takes the id being left, not just "stop sending": switching tabs is a
/// detach and an attach that reach the worker in no guaranteed order, and a
/// detach without an id could unset the attach that overtook it.
#[tauri::command]
pub async fn terminal_detach(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
) -> Result<(), TerminalError> {
    tell(&handle, Request::Detach(id)).await
}

#[tauri::command]
pub async fn terminal_resize(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
    cols: u16,
    rows: u16,
) -> Result<(), TerminalError> {
    tell(&handle, Request::Resize(id, cols, rows)).await
}

#[tauri::command]
pub async fn terminal_write(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
    data: String,
) -> Result<(), TerminalError> {
    ask(&handle, |tx| Request::Write(id, data, tx)).await?
}

/// Send and wait for the output to stop, then hand back the screen. Refuses
/// with `busy` when the session is waiting for a human: see the comment in
/// service.rs.
#[tauri::command]
pub async fn terminal_run_capture(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
    input: String,
    settle_ms: u64,
    timeout_ms: u64,
) -> Result<Vec<String>, TerminalError> {
    ask(&handle, |tx| Request::RunCapture(id, input, settle_ms, timeout_ms, tx)).await?
}
