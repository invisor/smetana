//! The commands the front end's connection store calls: what the shell's one connection to a server is
//! doing, and which server this device uses. All of them are answered here and none is forwarded.

use serde_json::Value;
use tauri::AppHandle;

/// What the connection is doing: see `client::state::View`.
#[tauri::command]
pub fn connection_state(app: AppHandle) -> Value {
    crate::client::snapshot(&app)
}

/// Try to connect now rather than at the end of the wait.
#[tauri::command]
pub fn connection_retry(app: AppHandle) {
    crate::client::retry(&app);
}

/// Which server this device uses: `{ target: { kind: "local" } }`, or a remote one with where it is.
#[tauri::command]
pub fn server_target(app: AppHandle) -> Value {
    crate::client::target_view(&app)
}

/// Back to the server on this machine.
#[tauri::command]
pub fn server_use_local(app: AppHandle) -> Result<Value, String> {
    crate::client::use_local(&app)
}

/// To the server a desktop pairing ticket names. See `client::use_remote`.
#[tauri::command]
pub async fn server_use_remote(app: AppHandle, ticket: String, stop_local: bool) -> Result<Value, String> {
    crate::client::use_remote(&app, ticket, stop_local).await
}

/// What the server on this machine is doing that a switch away from it would leave running.
#[tauri::command]
pub async fn server_local_work(app: AppHandle) -> Value {
    crate::client::local_work(&app).await
}

/// Make macOS forget its refusal of the open project's folder and start the local server afresh. See
/// `client::access_reset`, which holds who the grant belongs to and why the server is what restarts.
#[tauri::command]
pub async fn tracker_access_reset(app: AppHandle) -> Result<(), String> {
    crate::client::access_reset(&app).await
}
