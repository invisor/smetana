//! File commands: thin, with no state of their own — same as the settings ones.
//!
//! `root` comes from the front end: it knows the active project anyway, and
//! keeping a second copy of that knowledge here would mean taking a dependency
//! on the tracker for a value that is not its own. Every command checks that the
//! path lies inside the root it was sent.

use std::path::PathBuf;

use super::fs;
use super::model::{FileText, FilesError, Listing, Stat};

/// **The work here that does not return promptly, run off the runtime.** The
/// rule is `vcs/commands.rs`'s and it names this module as the victim: every
/// IPC call in the app — the file tree, the editor, the tracker, the terminals
/// — shares the runtime these commands are polled on, so work parked in the
/// body of an `async fn` takes a worker out of everything else on screen with
/// nothing saying why. The blocking pool is where a thread is meant to be
/// parked, whether it is waiting on a process or grinding through a directory.
///
/// It did not apply to this module while a listing was a `read_dir` measured in
/// fractions of a millisecond; parking a runtime worker on that was free. It
/// applies now that `list_dir` ends in a git spawn with `READ_CEILING` — thirty
/// seconds — over it, and `refreshDirs` in `stores/files.js` is a `Promise.all`
/// over every open folder fired on **every window focus**, so the calls arrive N
/// at a time against a runtime holding one worker per core.
///
/// `files_copy` and `files_move` take it for a different reason and the same
/// arithmetic: no process, but a walk of a folder and then the bytes of it, up
/// to the ceilings `MAX_COPY_ENTRIES` and `MAX_COPY_BYTES` put on a copy —
/// which is a second of disk on a warm cache and a great deal more over a
/// network mount. That is a worker parked on a copy while the board, the
/// terminals and the editor wait behind it.
///
/// The other five need nothing. None of them waits on a child — the trash is
/// the `trash` crate through `NsFileManager` rather than the `osascript` Finder
/// method, which `fs.rs` chose for its own reasons and which happens to keep
/// this true as well — and `files_rename` is one `rename` syscall, the same
/// order of cost as `files_create`, however large the folder behind it.
async fn off_the_runtime<T, F>(work: F) -> Result<T, FilesError>
where
    F: FnOnce() -> Result<T, FilesError> + Send + 'static,
    T: Send + 'static,
{
    // A blocking task that panicked, or a runtime shutting down under it. `Io`
    // in this app's own words, the way `vcs/commands.rs` answers the same case:
    // there is no filesystem error to report, because nothing got as far as the
    // filesystem.
    tokio::task::spawn_blocking(work)
        .await
        .unwrap_or_else(|err| Err(FilesError::Io(err.to_string())))
}

#[tauri::command]
pub async fn files_list(root: String, dir: String) -> Result<Listing, FilesError> {
    off_the_runtime(move || fs::list_dir(&PathBuf::from(root), &dir)).await
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

/// A copy of an entry into another folder. The name is not the caller's to
/// choose: `fs::copy_entry` takes the first of `copy_candidates` that is still
/// free — `report copy.md`, then `report copy 2.md` — so nothing is ever
/// overwritten and nothing is ever asked. The answer is the new path from the
/// root.
#[tauri::command]
pub async fn files_copy(root: String, src: String, dst_dir: String) -> Result<String, FilesError> {
    off_the_runtime(move || fs::copy_entry(&PathBuf::from(root), &src, &dst_dir)).await
}

/// The same journey with nothing left behind. Dropped into the folder it is
/// already in, it answers with the path it came in with and touches nothing.
#[tauri::command]
pub async fn files_move(root: String, src: String, dst_dir: String) -> Result<String, FilesError> {
    off_the_runtime(move || fs::move_entry(&PathBuf::from(root), &src, &dst_dir)).await
}

/// A new name in the same folder. A command of its own rather than a flag on
/// `files_move`, because its argument is a **name** and what checks it is
/// `resolve_new_within` — the same split `files_create` and `files_mkdir` are
/// built on — and not the containment check a move makes about two folders.
#[tauri::command]
pub async fn files_rename(
    root: String,
    path: String,
    name: String,
) -> Result<String, FilesError> {
    fs::rename_entry(&PathBuf::from(root), &path, &name)
}

/// Into the system trash, not gone. Whole path here rather than a pair: the
/// target exists, so the ordinary `resolve_within` is what checks it.
#[tauri::command]
pub async fn files_trash(root: String, path: String) -> Result<(), FilesError> {
    fs::move_to_trash(&PathBuf::from(root), &path)
}
