//! Thin commands with no state of their own, shaped exactly like `files/`'s.
//!
//! The project and the repository arrive as paths from the front end, which
//! knows both anyway: keeping a second copy of that knowledge here would mean
//! taking a dependency on the tracker for a value that is not this module's.

use std::path::Path;

use super::model::{Branch, Repo, VcsError, WorkingTree};
use super::{model, repos, run};
use crate::git;

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

/// The local branches of one repository, most recently worked on first.
///
/// **This one spawns nothing**, and it is the exception the module header's
/// rule survives rather than one it breaks: a branch list is `refs/heads/`,
/// `packed-refs` and the reflogs, all of which `git.rs` already reads off the
/// disk. The command sits here because it is a section of this panel and
/// because `vcs_checkout` beside it is what a person does with the answer —
/// but not one line of the reading is written a second time, and `git.rs` is
/// not touched: `branches_with_recency` gathers the three sources through
/// `parse_commondir`, so a linked worktree offers the whole repository's list
/// and not the one branch it is itself on, and `by_recency` is what orders
/// them. Nothing here re-sorts that: the branch somebody merges into every day
/// is nowhere in particular alphabetically.
///
/// Never a refusal, the same as `vcs_repos` and for the same reason: a folder
/// outside git has no branches to offer, which is an empty list and not an
/// error.
#[tauri::command]
pub async fn vcs_branches(repo: String) -> Vec<Branch> {
    branch_list(Path::new(&repo))
}

/// The command's whole body, synchronous so a test can call it: an `async fn`
/// would need a runtime here for three file reads that never yield.
fn branch_list(path: &Path) -> Vec<Branch> {
    // HEAD is read here rather than compared on the front end, and it is read
    // per worktree while the refs are shared — the distinction `parse_commondir`
    // exists for.
    let current = git::head(path).branch;
    git::by_recency(git::branches_with_recency(path))
        .into_iter()
        .map(|name| Branch { current: current.as_deref() == Some(name.as_str()), name })
        .collect()
}

/// The one thing in this file worth pinning: the mapping above, over the layout
/// that broke `git.rs` once already. Everything else here is an argument list
/// handed to `run.rs`, which is the process table and carries no tests.
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-vcs-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    /// A project opened as a linked worktree offers the whole repository's
    /// branches, and exactly one of them is the one it is on.
    ///
    /// The refs live in the common directory and HEAD does not, so reading both
    /// beside the worktree's own git directory would answer with the single
    /// branch nobody needs — smetana-5t7, one level up.
    #[test]
    fn a_worktree_lists_every_branch_and_marks_the_one_it_is_on() {
        let root = scratch("worktree-branches");
        let repo = root.join("repo");
        let linked = root.join("wt");
        let wt_git = repo.join(".git/worktrees/wt");
        fs::create_dir_all(repo.join(".git/refs/heads/feat")).expect("create refs/heads/feat");
        fs::create_dir_all(&wt_git).expect("create the worktree git directory");
        fs::create_dir_all(&linked).expect("create the worktree");
        for name in ["main", "staging"] {
            fs::write(repo.join(".git/refs/heads").join(name), "a1b2c3\n").expect("write the ref");
        }
        fs::write(repo.join(".git/refs/heads/feat/x"), "d4e5f6\n").expect("write the nested ref");
        fs::write(repo.join(".git/HEAD"), "ref: refs/heads/main\n").expect("write the main HEAD");
        fs::write(wt_git.join("HEAD"), "ref: refs/heads/feat/x\n").expect("write the worktree HEAD");
        fs::write(wt_git.join("commondir"), "../..\n").expect("write commondir");
        fs::write(linked.join(".git"), format!("gitdir: {}\n", wt_git.display()))
            .expect("write the .git file");

        let branches = branch_list(&linked);
        let names: Vec<&str> = branches.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(names, ["feat/x", "main", "staging"]);
        let current: Vec<&str> =
            branches.iter().filter(|b| b.current).map(|b| b.name.as_str()).collect();
        assert_eq!(current, ["feat/x"]);

        let _ = fs::remove_dir_all(&root);
    }

    /// A folder outside git is an empty list, never a refusal — the answer
    /// `vcs_repos` gives about the same folder.
    #[test]
    fn a_folder_outside_git_has_no_branches_and_no_error() {
        let root = scratch("no-git");
        assert!(branch_list(&root).is_empty());
        let _ = fs::remove_dir_all(&root);
    }
}
