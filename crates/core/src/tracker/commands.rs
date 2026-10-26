use crate::context::Context;
use tokio::sync::oneshot;


use super::access;
use super::access::AccessRepair;
use super::model::{Failure, Health, Issue, IssuePatch, Repair, Snapshot, TrackerError};
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

/// The body of `tracker_health`, for the phone bridge: the allowlisted commands
/// are plain functions that the Tauri wrapper and `remote::bridge` both call.
pub async fn health(handle: &TrackerHandle) -> Result<Health, TrackerError> {
    ask(handle, Request::Health).await
}

/// The body of `tracker_snapshot`; see `health`.
pub async fn snapshot(handle: &TrackerHandle) -> Result<Snapshot, TrackerError> {
    ask(handle, Request::Snapshot).await
}

pub async fn tracker_resync(handle: &TrackerHandle) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::Resync).await?
}

pub async fn tracker_update(
    handle: &TrackerHandle,
    id: String,
    patch: IssuePatch,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Update(id, patch, tx)).await?
}

pub async fn tracker_close(
    handle: &TrackerHandle,
    id: String,
    reason: Option<String>,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Close(id, reason, tx)).await?
}

pub async fn tracker_reopen(handle: &TrackerHandle, id: String) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Reopen(id, tx)).await?
}

/// Irreversible, and the only write that answers with nothing: a deleted issue
/// has no shape to hand back. The disappearance reaches the board as a delta.
pub async fn tracker_delete(handle: &TrackerHandle, id: String) -> Result<(), TrackerError> {
    ask(&handle, |tx| Request::Delete(id, tx)).await?
}

/// The body of `tracker_set_project`; see `health`.
pub async fn set_project(
    handle: &TrackerHandle,
    path: Option<String>,
) -> Result<Snapshot, TrackerError> {
    let dir = path.map(std::path::PathBuf::from);
    ask(handle, |tx| Request::SetProject(dir, tx)).await
}

/// `bd init` in the active project's directory. Success returns the board
/// right away: the folder became a repository and the worker has already
/// re-read it.
pub async fn tracker_init(handle: &TrackerHandle) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::InitTracker).await?
}

/// Copy `.beads` beside itself and run bd's own migrations over the original.
///
/// Offered on any tracker failure rather than on a recognized one: there is no
/// verdict to recognize — `bd doctor` is not supported in embedded mode and
/// `bd migrate` ignores `--json` — and a recognizer built out of a grep over
/// prose is one that stops matching on the next bd release without saying so.
/// Both migrations are idempotent by bd's own documentation, so running them
/// against a tracker broken some other way costs a few seconds and changes
/// nothing.
///
/// Success answers with the board as well as with the copy's path: the worker
/// reopens the folder itself, so there is nothing for the front end to ask for
/// afterwards.
pub async fn tracker_repair(handle: &TrackerHandle) -> Result<Repair, TrackerError> {
    ask(&handle, Request::Repair).await?
}

/// The whole of the last tracker failure, in one answer: the folder, the bd
/// this build ships, the bd command that failed and what it printed.
///
/// It exists for the second button on that screen — the one that hands the
/// failure to an agent — and it is one call rather than four reads for the
/// reason the intent it feeds is complete at send time: the tracker is what is
/// broken, so nothing can be asked again once the session has started, and two
/// calls could describe two different moments.
pub async fn tracker_failure(handle: &TrackerHandle) -> Result<Failure, TrackerError> {
    ask(&handle, Request::Failure).await
}

/// What can be done about a refused folder — for **this** project.
///
/// A read, and per folder rather than per build, which is why the front end asks
/// it again on a project switch: `~/Desktop/a` gets a button and `~/code/b` gets
/// a sentence about System Settings, in the same launch. The folder comes from
/// the worker for the reason the reset's does — the two must be the same folder.
///
/// It exists as a command because the front end may not ask the platform itself
/// — `stores/tracker.js` is the only file on that side allowed to know there is
/// a desktop at all — and because guessing from the user agent would offer the
/// button under `npm run dev` in a browser on a Mac, where there is no app to
/// reset anything for.
///
/// `HealthState::FolderRefused` is reported whatever this answers. What this
/// decides is only which of three sentences goes under it, and whether one of
/// them has a button.
pub async fn tracker_access_repair(handle: &TrackerHandle) -> Result<AccessRepair, TrackerError> {
    let (dir, _health, _snapshot) = ask(&handle, Request::Current).await?;
    Ok(match dir {
        Some(dir) => access::repair_for(&dir, access::home().as_deref()),
        // Nothing is open, so this notice is not being drawn at all. The
        // conservative answer is the one that offers nothing.
        None => AccessRepair::Unavailable,
    })
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
///
/// **`project` is `Option<String>` deliberately** — the front end does not send
/// it yet, so an absent argument reads the root exactly as before, and the
/// command is ready for a project's own `agents` block once something starts
/// passing it.
pub async fn tracker_search_semantic(
    ctx: &Context,
    handle: &TrackerHandle,
    query: String,
    project: Option<String>,
) -> Result<Vec<String>, OneshotError> {
    let snapshot = ask(&handle, Request::Snapshot)
        .await
        .map_err(|err| OneshotError::Io(err.to_string()))?;

    let ctx = ctx.clone();
    tokio::task::spawn_blocking(move || {
        // The Default row, for the reason `vcs_suggest_message` records: a
        // one-shot has no session and therefore no `Intent` to ask a role with.
        let (agent, model) = crate::settings::default_pair(&ctx, project.as_deref());
        // `pick_with_model` for the reason `vcs_suggest_message` records: the
        // substitution stays, the model does not cross it.
        let (profile, model) =
            crate::agents::pick_with_model(&agent, model, crate::shell_env::path())
                .ok_or_else(|| OneshotError::NoAgent(agent.clone()))?;
        // The merge lock is coordination and not work, and it is out of every
        // list on screen — so it is out of the question too, rather than only
        // out of the answer. See `search::is_lock`.
        let issues: Vec<Issue> =
            snapshot.issues.into_iter().filter(|issue| !search::is_lock(issue)).collect();
        let question = search::prompt(&query, &issues);
        // Never the project's own root and never '/' — see `runs::usage`'s
        // header for why a headless probe gets an empty folder of the app's
        // own to run in.
        let cwd = crate::agents::probe_dir(&ctx).map_err(OneshotError::Io)?;
        let raw = agent_oneshot::ask_raw(profile, model.as_deref(), &question, &cwd)?;
        let known: std::collections::HashSet<String> =
            issues.iter().map(|issue| issue.id.clone()).collect();
        Ok(search::parse(&raw, &known))
    })
    .await
    .map_err(|err| OneshotError::Io(err.to_string()))?
}

/// Whether these folders have a tracker. A question about the filesystem, not
/// about bd: the worker is not called here and the call costs a few metadata
/// reads per path. Without it a person would only learn that a folder has no
/// tracker by clicking on it.
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
/// next to it would create a second `.beads` inside the first. That climb
/// stops at the nearest `.git` **directory** on the way up (see
/// `nearest_tracked_ancestor` for the detail): a subfolder of a *nested*
/// repository does not climb past its own root into a tracker that belongs to
/// the repository around it. A `.git` **file** is not that stop — it is a
/// linked worktree, and the climb takes one jump through its
/// `gitdir:`/`commondir` into the main checkout and carries on from there,
/// the same repository bd itself answers for.
///
/// A question for the filesystem, not for bd — the worker is not called here.
/// If there is nothing tracked above, we return the path as is: that is a
/// legitimate "not a repository yet", and the `bd init` offer refers to it.
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
