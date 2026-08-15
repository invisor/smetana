//! Thin commands with no state of their own, shaped exactly like `files/`'s.
//!
//! The project and the repository arrive as paths from the front end, which
//! knows both anyway: keeping a second copy of that knowledge here would mean
//! taking a dependency on the tracker for a value that is not this module's.

use std::path::Path;

use super::model::{Repo, VcsError, WorkingTree};
use super::{model, repos, run};
use crate::files::model::{looks_binary, BINARY_SNIFF_BYTES, MAX_FILE_BYTES};

/// The repositories of a project. Never a refusal: a folder that is not a
/// repository, or holds none, is an empty list, which the panel draws as an
/// empty state of its own.
#[tauri::command]
pub async fn vcs_repos(project: String) -> Vec<Repo> {
    repos::discover(Path::new(&project))
}

/// The working tree of one repository.
///
/// `--untracked-files=normal`, git's own default: `all` would walk into every
/// untracked directory, and a person who wants that opens the file tree.
#[tauri::command]
pub async fn vcs_status(repo: String) -> Result<WorkingTree, VcsError> {
    let out = run::git(
        Path::new(&repo),
        &["status", "--porcelain=v2", "-z", "--branch", "--untracked-files=normal"],
    )?;
    Ok(model::parse_status(&out))
}

/// git's own exit code for a name that resolves to nothing. `--quiet` is what
/// makes it a clean answer rather than a failure: git prints not a word, and 1
/// is a code no other outcome of `rev-parse --verify` uses — a repository it
/// could not read at all exits 128 with its own message.
const NO_SUCH_OBJECT: i32 = 1;

/// One file as `HEAD` has it, for the diff against the working tree. `path` is
/// relative to `repo`, exactly as `vcs_status` reported it.
///
/// `Ok(None)` is a file `HEAD` does not have — an added or an untracked one,
/// and every file of a repository with no commit in it yet. That is not a
/// failure and must not read as one: the panel diffs such a file against an
/// empty document, which is what it is.
///
/// **Three calls and each is a machine-readable question.** `git show
/// HEAD:<path>` was the obvious single call and cannot answer the first one: it
/// exits 128 for an absent path and 128 for a folder that is not a repository
/// alike, so telling the two apart would mean reading git's prose — which moves
/// between versions, the reason `model.rs` parses `--porcelain=v2` and never
/// `git status`. `rev-parse --verify --quiet` says the same thing in an exit
/// code.
///
/// The two calls after it are given the **object name** rather than
/// `HEAD:<path>` a second and a third time. HEAD can move while this is in
/// flight — an agent committing in the same tree is the ordinary case here —
/// and asking again by name would let the size belong to one blob and the bytes
/// to another.
///
/// The ceiling and the binary sniff are `files/`'s own, and deliberately: this
/// opens in the same editor, and a file it already refuses to open above 2 MiB
/// must not arrive through a second door. The size is asked for before the
/// bytes are, the way `files/fs.rs` reads the metadata before it reads the
/// file.
#[tauri::command]
pub async fn vcs_file_at_head(repo: String, path: String) -> Result<Option<String>, VcsError> {
    let dir = Path::new(&repo);
    let object = format!("HEAD:{path}");
    let Some(name) = run::git_maybe(
        dir,
        &["rev-parse", "--verify", "--quiet", &object],
        NO_SUCH_OBJECT,
    )?
    else {
        return Ok(None);
    };
    let name = name.trim().to_owned();

    // An answer that will not parse is not a reason to refuse the file: it
    // costs the cheap check and leaves the one below it, which reads the bytes
    // that actually arrived and cannot be wrong about them.
    if let Ok(bytes) = run::git(dir, &["cat-file", "-s", &name])?.trim().parse::<u64>() {
        if bytes > MAX_FILE_BYTES {
            return Err(VcsError::TooLarge { path, bytes });
        }
    }

    let bytes = run::git_bytes(dir, &["cat-file", "blob", &name])?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(VcsError::TooLarge { path, bytes: bytes.len() as u64 });
    }
    if looks_binary(&bytes[..bytes.len().min(BINARY_SNIFF_BYTES)]) {
        return Err(VcsError::Binary(path));
    }
    // Refused rather than made lossy, for the reason the editor refuses it: a
    // replacement character is a change to the text, and a diff drawn over one
    // would report a difference nobody made.
    String::from_utf8(bytes).map(Some).map_err(|_| VcsError::NotUtf8(path))
}
