use tauri::State;
use tokio::sync::oneshot;

use crate::runs::service::{Request as RunRequest, RunHandle};

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
#[tauri::command]
pub async fn tracker_repair(handle: State<'_, TrackerHandle>) -> Result<Repair, TrackerError> {
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
#[tauri::command]
pub async fn tracker_failure(handle: State<'_, TrackerHandle>) -> Result<Failure, TrackerError> {
    ask(&handle, Request::Failure).await
}

/// How long the run worker is given to say which projects are live.
///
/// It answers from memory — the registry is already in hand — so this is not a
/// budget for the work, it is the point at which "the worker is wedged" stops
/// being indistinguishable from "the worker is about to answer". Generous
/// enough that a machine under load never trips it, short enough that a person
/// who pressed a button gets a sentence rather than a control stuck on
/// "Resetting…" for the life of the window.
const RUNS_ANSWER: std::time::Duration = std::time::Duration::from_secs(5);

/// Which projects hold a run right now, asked of the one thing that knows.
///
/// The same call `updates::live_runs` makes, and deliberately not shared with
/// it: that one answers in `UpdateError` and this one in `TrackerError`, and a
/// helper generic over both would be more machinery than the four lines it
/// saves. What must not diverge is the **direction of the refusal**, and that is
/// stated in both places — a worker that cannot be reached refuses, because
/// silence is not permission when the cost of being wrong is an agent killed
/// mid-task.
///
/// There are three ways to hear nothing and all three refuse: the channel is
/// closed, the sender is dropped without an answer, and — the one a bare
/// `await` misses — the worker is alive and wedged, so the answer simply never
/// comes. The timeout wraps the send as well as the receive, since a bounded
/// channel with a stuck worker at the far end blocks on the way out too.
async fn live_runs(runs: &RunHandle) -> Result<Vec<String>, TrackerError> {
    let (tx, rx) = oneshot::channel();
    let asked = tokio::time::timeout(RUNS_ANSWER, async move {
        runs.0.send(RunRequest::LiveProjects(tx)).await.map_err(|_| {
            TrackerError::Access(
                "the run worker is not running, so nothing here can promise a run is not going"
                    .into(),
            )
        })?;
        rx.await.map_err(|_| {
            TrackerError::Access(
                "the run worker did not answer, so nothing here can promise a run is not going"
                    .into(),
            )
        })
    })
    .await;
    asked.unwrap_or_else(|_| {
        Err(TrackerError::Access(format!(
            "the run worker did not answer within {} seconds, so nothing here can promise a run \
             is not going",
            RUNS_ANSWER.as_secs()
        )))
    })
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
#[tauri::command]
pub async fn tracker_access_repair(
    handle: State<'_, TrackerHandle>,
) -> Result<AccessRepair, TrackerError> {
    let (dir, _health, _snapshot) = ask(&handle, Request::Current).await?;
    Ok(match dir {
        Some(dir) => access::repair_for(&dir, access::home().as_deref()),
        // Nothing is open, so this notice is not being drawn at all. The
        // conservative answer is the one that offers nothing.
        None => AccessRepair::Unavailable,
    })
}

/// Make macOS forget its stored refusal of the project's folder, then restart.
///
/// The folder is read from the worker rather than taken from the front end: the
/// two must name the same directory, and the one that matters is the one bd was
/// actually failing in. `Request::Current` answers it beside the health that
/// explains it, in a single message, which is what stops a project switch
/// between two reads from resetting a grant for a folder nobody was looking at.
///
/// The bundle identifier comes from the running app. It is already in
/// `tauri.conf.json` and repeated once in `runs::awake`, and a third literal
/// copy is a string that would go stale in silence — `app.config().identifier`
/// cannot name an app other than the one asking.
///
/// **The run gate is the same one `updates_install` keeps, and it is here for
/// the same reason.** This restarts the app, a restart kills every PTY child,
/// and under a run those children are the agents it is driving — so a press
/// here can end an unattended batch that has been going for hours. The gate
/// knows nothing about which project is active, and that is the point: this
/// notice is drawn for a folder whose own project cannot be running anything
/// (nothing there can be read), so the run this would kill is always somebody
/// else's, in a project nobody is looking at. `.claude/rules/updates.md`
/// carries the whole argument.
///
/// **It restarts rather than answering and leaving that to the front end.** A
/// reset only takes effect for a process that has not yet been refused in this
/// launch, so the restart is not a courtesy but the second half of the repair,
/// and a returned "now restart" would be a second control for something that has
/// to happen anyway — with a window in between during which the board is broken
/// and the person has been told it is fixed. The copy on the button says the app
/// restarts, which is what stands in for the confirmation dialog there is not.
///
/// `request_restart` and not `restart`, which is `updates_install`'s call and
/// its reasoning verbatim: it goes through the event loop whatever thread it is
/// called from, so `RunEvent::Exit` in `lib.rs` fires and the PTY children are
/// killed exactly as they are on an ordinary quit. The direct `restart` skips
/// both when it happens to be called on the main thread, which would orphan
/// every open terminal's process instead.
///
/// `spawn_blocking` for the `tccutil` call itself: it spawns a process and
/// waits, and the tracker worker is the one task answering every other command.
#[tauri::command]
pub async fn tracker_access_reset(
    app: tauri::AppHandle,
    handle: State<'_, TrackerHandle>,
    runs: State<'_, RunHandle>,
) -> Result<(), TrackerError> {
    let (dir, _health, _snapshot) = ask(&handle, Request::Current).await?;
    let dir = dir.ok_or_else(|| TrackerError::Access("no project is open".into()))?;

    let live = live_runs(&runs).await?;
    if !live.is_empty() {
        return Err(TrackerError::RunLive { projects: live.join(", ") });
    }

    let identifier = app.config().identifier.clone();
    tokio::task::spawn_blocking(move || access::reset(&dir, &identifier))
        .await
        .map_err(|err| TrackerError::Access(format!("the reset did not finish: {err}")))?
        .map_err(TrackerError::Access)?;

    app.request_restart();
    Ok(())
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
