//! The thin layer over `config.rs`. There is no worker to queue behind and no
//! state to guard: reading one file costs milliseconds, the same reasoning
//! that keeps `files/` and `git.rs` out of a worker.

use std::path::Path;

use tauri::State;
use tokio::sync::oneshot;

use super::config::{self, ConfigState};
use super::model::{Run, RunError, RunSettings};
use super::service::{Request, RunHandle};

/// Every outcome is a state, so this cannot fail: a project with no config is
/// the ordinary case, and an unreadable one is `Broken` with what the OS said.
#[tauri::command]
pub fn project_config(project: String) -> ConfigState {
    config::load(Path::new(&project))
}

/// The three run commands, shaped exactly like the tracker's: put a request on
/// the worker's queue and await the reply. The outer failure is delivery to the
/// worker; the inner one, where there is one, is about the run itself.
async fn ask<T>(
    handle: &RunHandle,
    make: impl FnOnce(oneshot::Sender<T>) -> Request,
) -> Result<T, RunError> {
    let (tx, rx) = oneshot::channel();
    handle
        .0
        .send(make(tx))
        .await
        .map_err(|_| RunError::Terminal("the run worker is not running".into()))?;
    rx.await.map_err(|_| RunError::Terminal("the run worker did not answer".into()))
}

#[tauri::command]
pub async fn run_start(
    handle: State<'_, RunHandle>,
    project: String,
    settings: RunSettings,
) -> Result<Run, RunError> {
    ask(&handle, |tx| Request::Start(project, Box::new(settings), tx)).await?
}

/// Cooperative: this answers as soon as the worker has noted the request, and
/// the batch in flight is still going. `Run.stopping` is what says so, and the
/// run's own event says when it is actually over.
#[tauri::command]
pub async fn run_stop(handle: State<'_, RunHandle>, project: String) -> Result<Option<Run>, RunError> {
    ask(&handle, |tx| Request::Stop(project, tx)).await
}

/// The `run:state` event fires before the webview can subscribe — the same
/// shape `tracker_health` has, and for the same reason.
#[tauri::command]
pub async fn run_state(handle: State<'_, RunHandle>, project: String) -> Result<Option<Run>, RunError> {
    ask(&handle, |tx| Request::State(project, tx)).await
}
