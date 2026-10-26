//! The two commands of the file tree that are about the machine's own clipboard. Everything else the
//! tree does is the server's (`smetana_core::files::commands`) and reaches it over the connection.

use crate::clipboard::{self, ClipboardFiles};
use smetana_core::files::commands as logic;
use smetana_core::files::model::FilesError;

/// The paths onto the machine's own clipboard, so that a copy in the tree can
/// be pasted in Finder. Absolute, because a clipboard is not about a project.
///
/// Off the runtime because a clipboard call is a platform call with a lock
/// behind it — on Windows it is a process-wide one, and on Linux it hops to the
/// main thread and waits for the answer (`clipboard.rs`).
#[tauri::command]
pub async fn files_clipboard_write(
    app: tauri::AppHandle,
    paths: Vec<String>,
    mode: String,
) -> Result<(), FilesError> {
    logic::off_the_runtime(move || clipboard::write_files(&app, &paths, &mode)).await
}

/// And what is on it now. A clipboard holding no files at all answers an empty
/// list rather than a refusal: that is not an error, it is Tuesday.
#[tauri::command]
pub async fn files_clipboard_read(app: tauri::AppHandle) -> Result<ClipboardFiles, FilesError> {
    logic::off_the_runtime(move || clipboard::read_files(&app)).await
}
