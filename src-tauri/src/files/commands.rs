//! File commands: thin, with no state of their own — same as the settings ones.
//!
//! `root` comes from the front end: it knows the active project anyway, and
//! keeping a second copy of that knowledge here would mean taking a dependency
//! on the tracker for a value that is not its own. Every command checks that the
//! path lies inside the root it was sent.

use std::path::PathBuf;

use super::fs;
use super::model::{FileText, FilesError, Listing, Stat};

#[tauri::command]
pub async fn files_list(root: String, dir: String) -> Result<Listing, FilesError> {
    fs::list_dir(&PathBuf::from(root), &dir)
}

#[tauri::command]
pub async fn files_read(root: String, path: String) -> Result<FileText, FilesError> {
    fs::read_text(&PathBuf::from(root), &path)
}

/// Answers with the new timestamp: the front end puts it in the buffer and
/// sends it back with the next write.
#[tauri::command]
pub async fn files_write(
    root: String,
    path: String,
    text: String,
    expected_mtime: i64,
) -> Result<i64, FilesError> {
    fs::write_text(&PathBuf::from(root), &path, &text, expected_mtime)
}

/// No refusals: a vanished file arrives as `mtime: null`.
#[tauri::command]
pub async fn files_stat(root: String, paths: Vec<String>) -> Vec<Stat> {
    fs::stat_many(&PathBuf::from(root), &paths)
}

/// A new empty file. `dir` and `name` rather than one path, because that split
/// is the check: the directory has to exist to be resolved and the name has to
/// be a name — see `fs::resolve_new_within`. The answer is the new path from
/// the root, which is what the front end opens a tab on.
#[tauri::command]
pub async fn files_create(root: String, dir: String, name: String) -> Result<String, FilesError> {
    fs::create_file(&PathBuf::from(root), &dir, &name)
}

/// The same, for a directory. The answer is again the new path: the tree
/// expands it.
#[tauri::command]
pub async fn files_mkdir(root: String, dir: String, name: String) -> Result<String, FilesError> {
    fs::create_dir(&PathBuf::from(root), &dir, &name)
}

/// Into the system trash, not gone. Whole path here rather than a pair: the
/// target exists, so the ordinary `resolve_within` is what checks it.
#[tauri::command]
pub async fn files_trash(root: String, path: String) -> Result<(), FilesError> {
    fs::move_to_trash(&PathBuf::from(root), &path)
}
