use crate::clipboard::{self, ClipboardFiles};
use smetana_core::files::commands as logic;
use smetana_core::files::model::{FileText, FilesError, Listing, Stat};

/// See `files::commands::files_list` in the core crate.
#[tauri::command]
pub async fn files_list(root: String, dir: String) -> Result<Listing, FilesError> {
    logic::files_list(root, dir).await
}

/// See `files::commands::files_read` in the core crate.
#[tauri::command]
pub async fn files_read(root: String, path: String) -> Result<FileText, FilesError> {
    logic::files_read(root, path).await
}

/// See `files::commands::files_write` in the core crate.
#[tauri::command]
pub async fn files_write(
    root: String,
    path: String,
    text: String,
    expected_mtime: i64,
) -> Result<i64, FilesError> {
    logic::files_write(root, path, text, expected_mtime).await
}

/// See `files::commands::files_stat` in the core crate.
#[tauri::command]
pub async fn files_stat(root: String, paths: Vec<String>) -> Vec<Stat> {
    logic::files_stat(root, paths).await
}

/// See `files::commands::files_create` in the core crate.
#[tauri::command]
pub async fn files_create(
    root: String,
    dir: String,
    name: String,
) -> Result<String, FilesError> {
    logic::files_create(root, dir, name).await
}

/// See `files::commands::files_mkdir` in the core crate.
#[tauri::command]
pub async fn files_mkdir(
    root: String,
    dir: String,
    name: String,
) -> Result<String, FilesError> {
    logic::files_mkdir(root, dir, name).await
}

/// See `files::commands::files_copy` in the core crate.
#[tauri::command]
pub async fn files_copy(
    root: String,
    src: String,
    dst_dir: String,
) -> Result<String, FilesError> {
    logic::files_copy(root, src, dst_dir).await
}

/// See `files::commands::files_copy_external` in the core crate.
#[tauri::command]
pub async fn files_copy_external(
    root: String,
    src: String,
    dst_dir: String,
) -> Result<String, FilesError> {
    logic::files_copy_external(root, src, dst_dir).await
}

/// See `files::commands::files_move` in the core crate.
#[tauri::command]
pub async fn files_move(
    root: String,
    src: String,
    dst_dir: String,
) -> Result<String, FilesError> {
    logic::files_move(root, src, dst_dir).await
}

/// See `files::commands::files_rename` in the core crate.
#[tauri::command]
pub async fn files_rename(
    root: String,
    path: String,
    name: String,
) -> Result<String, FilesError> {
    logic::files_rename(root, path, name).await
}

/// See `files::commands::files_trash` in the core crate.
#[tauri::command]
pub async fn files_trash(root: String, path: String) -> Result<(), FilesError> {
    logic::files_trash(root, path).await
}

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
