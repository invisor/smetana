//! The `#[tauri::command]` wrappers of what only this machine can answer: the system clipboard, the
//! opener, the device's half of the settings, a file this desktop hands to a remote server.
//!
//! Every other command the webview calls is the server's: the shell forwards it over the connection
//! (`client/forward.rs`) and the logic is in the matching `commands.rs` of `crates/core`.

pub mod client_files;
pub mod files;
pub mod sessions;
pub mod settings;
