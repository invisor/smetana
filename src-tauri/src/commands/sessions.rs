//! What "open a transcript" means on the machine a person sits at. Listing and deleting a transcript
//! are the server's; handing one to this desktop's own file manager is the shell's.

use std::path::Path;

use smetana_core::sessions::commands as logic;

/// What "open" means on the machine a person sits at: whatever the system has registered for the path.
fn open(path: &Path) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|err| err.to_string())
}

/// And "show", in the platform's own file manager.
fn reveal(path: &Path) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|err| err.to_string())
}

/// A session's transcript, handed to the desktop. The guard and the reasoning are
/// `sessions::commands::sessions_open_log` and `sessions::act`; the hand-over is this machine's.
#[tauri::command]
pub async fn sessions_open_log(path: String) -> Result<(), String> {
    logic::sessions_open_log(path, open).await
}

/// The directory a session ran in, opened in the platform's file manager; see `sessions_open_log`.
#[tauri::command]
pub async fn sessions_open_cwd(path: String) -> Result<(), String> {
    logic::sessions_open_cwd(path, open).await
}

/// A session's transcript, shown in the platform's own file manager; see `sessions_open_log`.
#[tauri::command]
pub async fn sessions_reveal(path: String) -> Result<(), String> {
    logic::sessions_reveal(path, reveal).await
}
