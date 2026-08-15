//! Thin commands with no state of their own, shaped exactly like `files/`'s.
//!
//! The project and the repository arrive as paths from the front end, which
//! knows both anyway: keeping a second copy of that knowledge here would mean
//! taking a dependency on the tracker for a value that is not this module's.

use std::path::Path;

use super::model::{Repo, VcsError, WorkingTree};
use super::{model, repos, run};

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
