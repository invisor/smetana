use tauri::State;
use tokio::sync::oneshot;

use super::model::{Health, Issue, IssuePatch, Snapshot, TrackerError};
use super::search;
use super::service::{Request, TrackerHandle};
use crate::agents::oneshot::{self as agent_oneshot, OneshotError};

/// The commands are deliberately thin: all they do is put a request on the
/// worker's queue and await the answer. The outer Result is about delivery to
/// the worker, the inner one (where there is one) is about the bd call itself.
async fn ask<T>(
    handle: &TrackerHandle,
    make: impl FnOnce(oneshot::Sender<T>) -> Request,
) -> Result<T, TrackerError> {
    let (tx, rx) = oneshot::channel();
    handle
        .0
        .send(make(tx))
        .await
        .map_err(|_| TrackerError::Spawn("the tracker worker is not running".into()))?;
    rx.await
        .map_err(|_| TrackerError::Spawn("the tracker worker did not answer".into()))
}

/// The tracker:health event may fire before the front end subscribes — this
/// command hands the last state to whoever missed it.
#[tauri::command]
pub async fn tracker_health(handle: State<'_, TrackerHandle>) -> Result<Health, TrackerError> {
    ask(&handle, Request::Health).await
}

#[tauri::command]
pub async fn tracker_snapshot(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::Snapshot).await
}

#[tauri::command]
pub async fn tracker_resync(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::Resync).await?
}

#[tauri::command]
pub async fn tracker_update(
    handle: State<'_, TrackerHandle>,
    id: String,
    patch: IssuePatch,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Update(id, patch, tx)).await?
}

#[tauri::command]
pub async fn tracker_close(
    handle: State<'_, TrackerHandle>,
    id: String,
    reason: Option<String>,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Close(id, reason, tx)).await?
}

#[tauri::command]
pub async fn tracker_reopen(
    handle: State<'_, TrackerHandle>,
    id: String,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Reopen(id, tx)).await?
}

/// Irreversible, and the only write that answers with nothing: a deleted issue
/// has no shape to hand back. The disappearance reaches the board as a delta.
#[tauri::command]
pub async fn tracker_delete(
    handle: State<'_, TrackerHandle>,
    id: String,
) -> Result<(), TrackerError> {
    ask(&handle, |tx| Request::Delete(id, tx)).await?
}

/// Moving to another directory. `None` means "no projects are left": the board
/// empties while the worker stays alive and waits for the next project. The
/// answer is the new project's snapshot in full: the front end does not listen
/// to deltas arriving on the way for the duration of the switch.
#[tauri::command]
pub async fn tracker_set_project(
    handle: State<'_, TrackerHandle>,
    path: Option<String>,
) -> Result<Snapshot, TrackerError> {
    let dir = path.map(std::path::PathBuf::from);
    ask(&handle, |tx| Request::SetProject(dir, tx)).await
}

/// `bd init` in the active project's directory. Success returns the board
/// right away: the folder became a repository and the worker has already
/// re-read it.
#[tauri::command]
pub async fn tracker_init(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::InitTracker).await?
}

/// Which issues a person meant, asked of the agent rather than of a substring.
///
/// The instant search in the front end runs over this very snapshot and answers
/// before a keystroke lands; this is the other tier and it is opt-in, because it
/// costs a model call and several seconds. Nothing is claimed and nothing is
/// written — the answer is a list of ids, and the rows on screen are still drawn
/// from the store's own issues, so an id the model invented cannot reach it.
///
/// `spawn_blocking` around the whole body, the same rule `vcs_suggest_message`
/// records: reading the settings file, probing the login shell for a `PATH` and
/// then waiting on a model are every one of them blocking.
///
/// The snapshot is read **before** the blocking half rather than inside it: the
/// worker is a tokio task and asking it from a blocking thread would be a
/// second runtime entry for no gain.
#[tauri::command]
pub async fn tracker_search_semantic(
    app: tauri::AppHandle,
    handle: State<'_, TrackerHandle>,
    query: String,
) -> Result<Vec<String>, OneshotError> {
    let snapshot = ask(&handle, Request::Snapshot)
        .await
        .map_err(|err| OneshotError::Io(err.to_string()))?;

    tokio::task::spawn_blocking(move || {
        let agent = crate::settings::agent(&app);
        let profile = crate::agents::pick(&agent, crate::shell_env::path())
            .ok_or_else(|| OneshotError::NoAgent(agent.clone()))?;
        // The merge lock is coordination and not work, and it is out of every
        // list on screen — so it is out of the question too, rather than only
        // out of the answer. See `search::is_lock`.
        let issues: Vec<Issue> =
            snapshot.issues.into_iter().filter(|issue| !search::is_lock(issue)).collect();
        let question = search::prompt(&query, &issues);
        let raw = agent_oneshot::ask_raw(profile, &question)?;
        let known: std::collections::HashSet<String> =
            issues.iter().map(|issue| issue.id.clone()).collect();
        Ok(search::parse(&raw, &known))
    })
    .await
    .map_err(|err| OneshotError::Io(err.to_string()))?
}

/// Whether these folders have a tracker. A question about the filesystem, not
/// about bd: the worker is not called here and the call costs one `is_dir` per
/// path. Without it a person would only learn that a folder has no tracker by
/// clicking on it.
#[tauri::command]
pub async fn tracker_probe(paths: Vec<String>) -> Vec<ProjectProbe> {
    paths
        .into_iter()
        .map(|path| {
            let tracked = crate::project::has_tracker(std::path::Path::new(&path));
            ProjectProbe { path, tracked }
        })
        .collect()
}

/// The folder that is actually being opened. Point at a subfolder of a tracked
/// repository and its root becomes the project: otherwise the board would say
/// "there is no tracker here" about a repository that has one, and the button
/// next to it would create a second `.beads` inside the first.
///
/// A question for the filesystem, not for bd — the worker is not called here.
/// If there is nothing tracked above, we return the path as is: that is a
/// legitimate "not a repository yet", and the `bd init` offer refers to it.
#[tauri::command]
pub async fn project_root(path: String) -> String {
    crate::project::nearest_tracked_ancestor(std::path::Path::new(&path))
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or(path)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProbe {
    pub path: String,
    pub tracked: bool,
}
