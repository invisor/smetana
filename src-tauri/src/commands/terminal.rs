use smetana_core::agents::Intent;
use smetana_core::terminal::commands as logic;
use smetana_core::terminal::model::{Session, SessionId, SessionMark, TerminalError};
use smetana_core::terminal::service::{Attached, TerminalHandle};
use tauri::State;

#[tauri::command]
pub async fn terminal_list(handle: State<'_, TerminalHandle>, project: String) -> Result<Vec<Session>, TerminalError> {
    logic::plain::list(&handle, project).await
}

/// See `terminal::commands::terminal_marks` in the core crate.
#[tauri::command]
pub async fn terminal_marks(handle: State<'_, TerminalHandle>) -> Result<Vec<SessionMark>, TerminalError> {
    logic::terminal_marks(&handle).await
}

/// See `terminal::commands::terminal_create` in the core crate.
#[tauri::command]
pub async fn terminal_create(
    handle: State<'_, TerminalHandle>,
    project: String,
    intent: Intent,
) -> Result<Session, TerminalError> {
    logic::terminal_create(&handle, project, intent).await
}

/// See `terminal::commands::terminal_shell` in the core crate.
#[tauri::command]
pub async fn terminal_shell(
    handle: State<'_, TerminalHandle>,
    project: String,
    cwd: Option<String>,
) -> Result<Session, TerminalError> {
    logic::terminal_shell(&handle, project, cwd).await
}

/// See `terminal::commands::terminal_remove` in the core crate.
#[tauri::command]
pub async fn terminal_remove(handle: State<'_, TerminalHandle>, id: SessionId) -> Result<(), TerminalError> {
    logic::terminal_remove(&handle, id).await
}

/// The ring snapshot plus the current `seq`: everything the session has said,
/// and the number output arriving after it continues from.
#[tauri::command]
pub async fn terminal_attach(handle: State<'_, TerminalHandle>, id: SessionId) -> Result<Attached, TerminalError> {
    logic::plain::attach(&handle, id).await
}

/// See `terminal::commands::terminal_detach` in the core crate.
#[tauri::command]
pub async fn terminal_detach(handle: State<'_, TerminalHandle>, id: SessionId) -> Result<(), TerminalError> {
    logic::terminal_detach(&handle, id).await
}

/// See `terminal::commands::terminal_resize` in the core crate.
#[tauri::command]
pub async fn terminal_resize(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
    cols: u16,
    rows: u16,
) -> Result<(), TerminalError> {
    logic::terminal_resize(&handle, id, cols, rows).await
}

/// See `terminal::commands::terminal_write` in the core crate.
#[tauri::command]
pub async fn terminal_write(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
    data: String,
) -> Result<(), TerminalError> {
    logic::terminal_write(&handle, id, data).await
}

/// See `terminal::commands::terminal_clear` in the core crate.
#[tauri::command]
pub async fn terminal_clear(handle: State<'_, TerminalHandle>, id: SessionId) -> Result<(), TerminalError> {
    logic::terminal_clear(&handle, id).await
}

/// See `terminal::commands::terminal_run_capture` in the core crate.
#[tauri::command]
pub async fn terminal_run_capture(
    handle: State<'_, TerminalHandle>,
    id: SessionId,
    input: String,
    settle_ms: u64,
    timeout_ms: u64,
) -> Result<Vec<String>, TerminalError> {
    logic::terminal_run_capture(&handle, id, input, settle_ms, timeout_ms).await
}

/// See `terminal::commands::terminal_restorable` in the core crate.
#[tauri::command]
pub async fn terminal_restorable(project: String) -> Result<Vec<smetana_core::terminal::restore::Restorable>, TerminalError> {
    logic::terminal_restorable(project).await
}

/// See `terminal::commands::terminal_forget` in the core crate.
#[tauri::command]
pub async fn terminal_forget(project: String, session_id: String) -> Result<(), TerminalError> {
    logic::terminal_forget(project, session_id).await
}
