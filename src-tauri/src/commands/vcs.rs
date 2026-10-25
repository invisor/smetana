use smetana_core::agents::oneshot::OneshotError;
use smetana_core::context::Context;
use smetana_core::git;
use smetana_core::vcs::commands as logic;
use smetana_core::vcs::model::{
    Branch, Comparison, InProgress, Landed, MergeOutcome, OpKind, ProjectRepos, Tracking, VcsError,
    WorkingTree,
};
use tauri::State;

/// See `vcs::commands::vcs_repos` in the core crate.
#[tauri::command]
pub async fn vcs_repos(project: String) -> ProjectRepos {
    logic::vcs_repos(project).await
}

/// See `vcs::commands::vcs_status` in the core crate.
#[tauri::command]
pub async fn vcs_status(repo: String) -> Result<WorkingTree, VcsError> {
    logic::vcs_status(repo).await
}

/// See `vcs::commands::vcs_branches` in the core crate.
#[tauri::command]
pub async fn vcs_branches(repo: String) -> Vec<Branch> {
    logic::vcs_branches(repo).await
}

/// See `vcs::commands::vcs_remote_branches` in the core crate.
#[tauri::command]
pub async fn vcs_remote_branches(repo: String) -> Vec<git::RemoteBranch> {
    logic::vcs_remote_branches(repo).await
}

/// See `vcs::commands::vcs_last_fetch` in the core crate.
#[tauri::command]
pub async fn vcs_last_fetch(repo: String) -> Option<i64> {
    logic::vcs_last_fetch(repo).await
}

/// See `vcs::commands::vcs_tracking` in the core crate.
#[tauri::command]
pub async fn vcs_tracking(repo: String) -> Vec<Tracking> {
    logic::vcs_tracking(repo).await
}

/// See `vcs::commands::vcs_checkout` in the core crate.
#[tauri::command]
pub async fn vcs_checkout(repo: String, branch: String) -> Result<(), VcsError> {
    logic::vcs_checkout(repo, branch).await
}

/// See `vcs::commands::vcs_checkout_remote` in the core crate.
#[tauri::command]
pub async fn vcs_checkout_remote(repo: String, branch: String) -> Result<(), VcsError> {
    logic::vcs_checkout_remote(repo, branch).await
}

/// See `vcs::commands::vcs_create_branch` in the core crate.
#[tauri::command]
pub async fn vcs_create_branch(
    repo: String,
    name: String,
    start: String,
    switch: bool,
) -> Result<(), VcsError> {
    logic::vcs_create_branch(repo, name, start, switch).await
}

/// See `vcs::commands::vcs_delete_branch` in the core crate.
#[tauri::command]
pub async fn vcs_delete_branch(
    repo: String,
    branch: String,
    force: bool,
) -> Result<(), VcsError> {
    logic::vcs_delete_branch(repo, branch, force).await
}

/// See `vcs::commands::vcs_rename_branch` in the core crate.
#[tauri::command]
pub async fn vcs_rename_branch(
    repo: String,
    from: String,
    to: String,
) -> Result<(), VcsError> {
    logic::vcs_rename_branch(repo, from, to).await
}

/// See `vcs::commands::vcs_merge` in the core crate.
#[tauri::command]
pub async fn vcs_merge(repo: String, branch: String) -> Result<MergeOutcome, VcsError> {
    logic::vcs_merge(repo, branch).await
}

/// See `vcs::commands::vcs_rebase` in the core crate.
#[tauri::command]
pub async fn vcs_rebase(repo: String, onto: String) -> Result<MergeOutcome, VcsError> {
    logic::vcs_rebase(repo, onto).await
}

/// See `vcs::commands::vcs_fetch` in the core crate.
#[tauri::command]
pub async fn vcs_fetch(repo: String) -> Result<(), VcsError> {
    logic::vcs_fetch(repo).await
}

/// See `vcs::commands::vcs_pull` in the core crate.
#[tauri::command]
pub async fn vcs_pull(repo: String) -> Result<MergeOutcome, VcsError> {
    logic::vcs_pull(repo).await
}

/// See `vcs::commands::vcs_push` in the core crate.
#[tauri::command]
pub async fn vcs_push(repo: String, set_upstream: bool) -> Result<Landed, VcsError> {
    logic::vcs_push(repo, set_upstream).await
}

/// See `vcs::commands::vcs_abort` in the core crate.
#[tauri::command]
pub async fn vcs_abort(repo: String, op: OpKind) -> Result<(), VcsError> {
    logic::vcs_abort(repo, op).await
}

/// See `vcs::commands::vcs_in_progress` in the core crate.
#[tauri::command]
pub async fn vcs_in_progress(repo: String) -> Result<Option<InProgress>, VcsError> {
    logic::vcs_in_progress(repo).await
}

/// See `vcs::commands::vcs_commit` in the core crate.
#[tauri::command]
pub async fn vcs_commit(repo: String, message: String) -> Result<(), VcsError> {
    logic::vcs_commit(repo, message).await
}

/// See `vcs::commands::vcs_discard` in the core crate.
#[tauri::command]
pub async fn vcs_discard(
    repo: String,
    path: String,
    orig_path: Option<String>,
) -> Result<(), VcsError> {
    logic::vcs_discard(repo, path, orig_path).await
}

/// See `vcs::commands::vcs_suggest_message` in the core crate.
#[tauri::command]
pub async fn vcs_suggest_message(
    ctx: State<'_, Context>,
    repo: String,
    project: Option<String>,
) -> Result<String, OneshotError> {
    logic::vcs_suggest_message(&ctx, repo, project).await
}

/// See `vcs::commands::vcs_file_at_head` in the core crate.
#[tauri::command]
pub async fn vcs_file_at_head(repo: String, path: String) -> Result<Option<String>, VcsError> {
    logic::vcs_file_at_head(repo, path).await
}

/// See `vcs::commands::vcs_file_at_rev` in the core crate.
#[tauri::command]
pub async fn vcs_file_at_rev(
    repo: String,
    rev: String,
    path: String,
) -> Result<Option<String>, VcsError> {
    logic::vcs_file_at_rev(repo, rev, path).await
}

/// See `vcs::commands::vcs_compare` in the core crate.
#[tauri::command]
pub async fn vcs_compare(
    repo: String,
    branch: String,
    mode: String,
) -> Result<Comparison, VcsError> {
    logic::vcs_compare(repo, branch, mode).await
}
