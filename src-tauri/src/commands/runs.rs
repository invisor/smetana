use smetana_core::runs::browser::BrowserTools;
use smetana_core::runs::commands as logic;
use smetana_core::runs::commands::ProjectConfigReply;
use smetana_core::runs::config;
use smetana_core::runs::model::{Run, RunError, RunSettings};
use smetana_core::runs::reports::ReportEntry;
use smetana_core::runs::service::RunHandle;
use smetana_core::runs::usage::AgentUsage;
use tauri::{AppHandle, Manager, State};

/// See `runs::commands::project_config` in the core crate.
#[tauri::command]
pub fn project_config(project: String) -> ProjectConfigReply {
    logic::project_config(project)
}

/// See `runs::commands::project_config_save_defaults` in the core crate.
#[tauri::command]
pub fn project_config_save_defaults(project: String, defaults: config::Defaults) -> Result<(), String> {
    logic::project_config_save_defaults(project, defaults)
}

/// See `runs::commands::target_branches` in the core crate.
#[tauri::command]
pub fn target_branches(project: String) -> Vec<smetana_core::git::BranchOption> {
    logic::target_branches(project)
}

#[tauri::command]
pub async fn run_start(
    handle: State<'_, RunHandle>,
    project: String,
    settings: RunSettings,
) -> Result<Run, RunError> {
    logic::start(&handle, project, settings).await
}

/// Cooperative: this answers as soon as the worker has noted the request, and
/// the batch in flight is still going. `Run.stopping` is what says so, and the
/// run's own event says when it is actually over.
///
/// Named by the run's token rather than the project: a project holds several
/// runs now, and the stop has to reach exactly the one whose bar segment was
/// pressed. `None` back is a run that ended before the stop arrived.
#[tauri::command]
pub async fn run_stop(handle: State<'_, RunHandle>, token: u64) -> Result<Option<Run>, RunError> {
    logic::stop(&handle, token).await
}

/// See `runs::commands::run_release` in the core crate.
#[tauri::command]
pub async fn run_release(handle: State<'_, RunHandle>) -> Result<(), RunError> {
    logic::run_release(&handle).await
}

/// The `run:state` event fires before the webview can subscribe — the same
/// shape `tracker_health` has, and for the same reason. The set rather than
/// one run: the project may hold several, and `runs.js` keeps them whole the
/// way it kept the single one.
#[tauri::command]
pub async fn run_state(handle: State<'_, RunHandle>, project: String) -> Result<Vec<Run>, RunError> {
    logic::state(&handle, project).await
}

/// See `runs::commands::run_reports` in the core crate.
#[tauri::command]
pub async fn run_reports(project: String) -> Vec<ReportEntry> {
    logic::run_reports(project).await
}

/// See `runs::commands::browser_tools` in the core crate.
#[tauri::command]
pub async fn browser_tools(app: AppHandle, project: String) -> BrowserTools {
    let handle = app.try_state::<RunHandle>();
    logic::browser_tools(handle.as_deref(), project).await
}

/// See `runs::commands::agent_usage` in the core crate.
#[tauri::command]
pub async fn agent_usage(app: AppHandle, agent: Option<String>, project: Option<String>) -> AgentUsage {
    logic::agent_usage(crate::host::context(&app), agent, project).await
}
