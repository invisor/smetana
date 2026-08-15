//! Thin commands with no state of their own, shaped exactly like `files/`'s.
//!
//! The project and the repository arrive as paths from the front end, which
//! knows both anyway: keeping a second copy of that knowledge here would mean
//! taking a dependency on the tracker for a value that is not this module's.

use std::path::Path;

use super::model::{Branch, Repo, VcsError, WorkingTree};
use super::{model, repos, run};
use crate::files::model::{looks_binary, BINARY_SNIFF_BYTES, MAX_FILE_BYTES};
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

/// Switch the repository to a branch it already has.
///
/// `git checkout --no-guess <branch> --` and nothing more. **Never `--force`**,
/// and the omission is the feature: the two refusals worth having are exactly
/// the ones force would drive over — a branch already checked out in another
/// worktree, which is what a run's `provisioning` phase cuts, and local changes
/// the checkout would have to overwrite.
///
/// Neither is pre-empted here. Asking git whether it would refuse, and then
/// asking it to do the thing, is two answers about one moment with a window
/// between them; and the sentence git gives is better than one written here,
/// because the person reading it knows git. `run.rs` carries it through
/// untouched with git's exit status beside it.
///
/// **The two extra words are what make the first line true, and each closes a
/// different way of not switching a branch at all.** The list a row was picked
/// from has no watcher behind it, and a run's `merging` phase deletes branches,
/// so the name pressed can be one git no longer has:
///
/// - Without `--no-guess`, git's DWIM *creates* a local branch from
///   `origin/<name>` at the remote's tip and checks that out — measured at exit
///   0. Creating a branch is outside this epic, and somebody who asked to
///   switch to something they could see would be handed a different commit with
///   nothing saying so.
/// - Without the trailing `--`, a name that also exists as a **path** is taken
///   as a path: measured on git 2.34.1, a top-level file named `shadow` with
///   the branch `shadow` gone made `git checkout --no-guess shadow` print
///   "Updated 1 path from the index" and **exit 0**, restoring that file from
///   the index over an uncommitted edit. This command would have answered
///   `Ok(())`, the panel would have refreshed, the tick would not have moved,
///   and somebody's work would be gone with nothing on screen about it.
///
/// With both, a name git cannot resolve as a branch is `fatal: invalid
/// reference: <name>` at exit 128 — a refusal in git's own words, which is what
/// this command commits to everywhere else. Verified against git 2.34.1 that an
/// ordinary local branch still switches at exit 0 with a dirty tree, and that
/// both refusals above still arrive unchanged.
#[tauri::command]
pub async fn vcs_checkout(repo: String, branch: String) -> Result<(), VcsError> {
    run::git(Path::new(&repo), &["checkout", "--no-guess", &branch, "--"])?;
    Ok(())
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
