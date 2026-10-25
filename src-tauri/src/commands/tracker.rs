use smetana_core::agents::oneshot::OneshotError;
use smetana_core::context::Context;
use smetana_core::runs::service::RunHandle;
use smetana_core::tracker::access::AccessRepair;
use smetana_core::tracker::commands as logic;
use smetana_core::tracker::commands::ProjectProbe;
use smetana_core::tracker::model::{Failure, Health, Issue, IssuePatch, Repair, Snapshot, TrackerError};
use smetana_core::tracker::service::TrackerHandle;
use tauri::State;

/// The tracker:health event may fire before the front end subscribes — this
/// command hands the last state to whoever missed it.
#[tauri::command]
pub async fn tracker_health(handle: State<'_, TrackerHandle>) -> Result<Health, TrackerError> {
    logic::health(&handle).await
}

#[tauri::command]
pub async fn tracker_snapshot(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    logic::snapshot(&handle).await
}

/// See `tracker::commands::tracker_resync` in the core crate.
#[tauri::command]
pub async fn tracker_resync(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    logic::tracker_resync(&handle).await
}

/// See `tracker::commands::tracker_update` in the core crate.
#[tauri::command]
pub async fn tracker_update(
    handle: State<'_, TrackerHandle>,
    id: String,
    patch: IssuePatch,
) -> Result<Issue, TrackerError> {
    logic::tracker_update(&handle, id, patch).await
}

/// See `tracker::commands::tracker_close` in the core crate.
#[tauri::command]
pub async fn tracker_close(
    handle: State<'_, TrackerHandle>,
    id: String,
    reason: Option<String>,
) -> Result<Issue, TrackerError> {
    logic::tracker_close(&handle, id, reason).await
}

/// See `tracker::commands::tracker_reopen` in the core crate.
#[tauri::command]
pub async fn tracker_reopen(handle: State<'_, TrackerHandle>, id: String) -> Result<Issue, TrackerError> {
    logic::tracker_reopen(&handle, id).await
}

/// See `tracker::commands::tracker_delete` in the core crate.
#[tauri::command]
pub async fn tracker_delete(handle: State<'_, TrackerHandle>, id: String) -> Result<(), TrackerError> {
    logic::tracker_delete(&handle, id).await
}

/// Moving to another directory. `None` means "no projects are left": the board
/// empties while the worker stays alive and waits for the next project. The
/// answer is the new project's snapshot in full: the front end does not listen
/// to deltas arriving on the way for the duration of the switch.
#[tauri::command]
pub async fn tracker_set_project(handle: State<'_, TrackerHandle>, path: Option<String>) -> Result<Snapshot, TrackerError> {
    logic::set_project(&handle, path).await
}

/// See `tracker::commands::tracker_init` in the core crate.
#[tauri::command]
pub async fn tracker_init(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    logic::tracker_init(&handle).await
}

/// See `tracker::commands::tracker_repair` in the core crate.
#[tauri::command]
pub async fn tracker_repair(handle: State<'_, TrackerHandle>) -> Result<Repair, TrackerError> {
    logic::tracker_repair(&handle).await
}

/// See `tracker::commands::tracker_failure` in the core crate.
#[tauri::command]
pub async fn tracker_failure(handle: State<'_, TrackerHandle>) -> Result<Failure, TrackerError> {
    logic::tracker_failure(&handle).await
}

/// See `tracker::commands::tracker_access_repair` in the core crate.
#[tauri::command]
pub async fn tracker_access_repair(handle: State<'_, TrackerHandle>) -> Result<AccessRepair, TrackerError> {
    logic::tracker_access_repair(&handle).await
}

/// See `tracker::commands::tracker_search_semantic` in the core crate.
#[tauri::command]
pub async fn tracker_search_semantic(
    ctx: State<'_, Context>,
    handle: State<'_, TrackerHandle>,
    query: String,
    project: Option<String>,
) -> Result<Vec<String>, OneshotError> {
    logic::tracker_search_semantic(&ctx, &handle, query, project).await
}

/// See `tracker::commands::tracker_probe` in the core crate.
#[tauri::command]
pub async fn tracker_probe(paths: Vec<String>) -> Vec<ProjectProbe> {
    logic::tracker_probe(paths).await
}

/// See `tracker::commands::project_root` in the core crate.
#[tauri::command]
pub async fn project_root(path: String) -> String {
    logic::project_root(path).await
}

/// Make macOS forget its stored refusal of the project's folder, then restart.
///
/// The reset is `tracker::commands::access_reset`, which carries the reasoning; this is the half
/// that needs the running application: the bundle identifier it is run for, and the restart.
/// `request_restart` and not `restart`, for `updates_install`'s reason verbatim: it goes through the
/// event loop whatever thread it is called from, so `RunEvent::Exit` in `lib.rs` fires and the PTY
/// children are killed exactly as they are on an ordinary quit.
#[tauri::command]
pub async fn tracker_access_reset(
    app: tauri::AppHandle,
    handle: State<'_, TrackerHandle>,
    runs: State<'_, RunHandle>,
) -> Result<(), TrackerError> {
    let identifier = app.config().identifier.clone();
    logic::access_reset(&handle, &runs, identifier).await?;
    app.request_restart();
    Ok(())
}
