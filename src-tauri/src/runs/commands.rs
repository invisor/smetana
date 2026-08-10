//! The thin layer over `config.rs`. There is no worker to queue behind and no
//! state to guard: reading one file costs milliseconds, the same reasoning
//! that keeps `files/` and `git.rs` out of a worker.

use std::path::Path;

use tauri::{AppHandle, Manager, State};
use tokio::sync::oneshot;

use super::browser::{self, BrowserTools};
use super::config::{self, ConfigState, LiveCheckMode};
use super::model::{Run, RunError, RunSettings};
use super::service::{Request, RunHandle};

/// Every outcome is a state, so this cannot fail: a project with no config is
/// the ordinary case, and an unreadable one is `Broken` with what the OS said.
#[tauri::command]
pub fn project_config(project: String) -> ConfigState {
    config::load(Path::new(&project))
}

/// Whether there is anything on this machine to drive a browser with, asked
/// before the run dialog opens so its live-check toggle can be switched off and
/// blocked with a reason rather than starting a run that fails inside the check.
///
/// Infallible for `project_config`'s reason: every outcome here is a state. A
/// machine with no Playwright and no extension is the answer, not a failure —
/// and the one thing that genuinely could fail, asking the run worker about
/// busy-ness, falls back to "nothing is holding it". That is the lenient
/// direction, and it is the right one for this fact: the tool-presence half is
/// what the "unobservable reads as no" rule is about, and a worker that cannot
/// answer is a worker no run could be going through anyway.
///
/// `AppHandle` rather than `State<'_, RunHandle>`: an async command borrowing
/// state has to return a `Result`, and there is nothing here to put in one.
#[tauri::command]
pub async fn browser_tools(app: AppHandle, project: String) -> BrowserTools {
    // `try_state` rather than `state`: the latter panics when the worker is not
    // managed yet, and a panic inside a read that answers "what does this
    // machine have" is the one outcome this command has no honest shape for.
    let candidates = match app.try_state::<RunHandle>() {
        Some(handle) => {
            let handle = handle.inner().clone();
            ask(&handle, Request::BrowserBusy).await.unwrap_or_default()
        }
        None => Vec::new(),
    };

    // The worker knows a run asked for a live check; only the project's own
    // config says whether that check opens a browser. A run whose live check is
    // a declared command needs no browser and is holding nothing, and naming it
    // as the reason this toggle is blocked would be an invention.
    let holder = candidates.into_iter().find(|other| {
        matches!(
            config::load(Path::new(other)),
            ConfigState::Ok { config }
                if config.live_check.as_ref().map(|live| live.mode) == Some(LiveCheckMode::Browser)
        )
    });

    browser::detect(Path::new(&project), holder)
}

/// How everything here reaches the worker, shaped exactly like the tracker's:
/// put a request on the worker's queue and await the reply. The outer failure is
/// delivery to the worker; the inner one, where there is one, is about the run
/// itself. `browser_tools` above is the one caller that swallows the outer
/// failure rather than passing it on, for the reason recorded on it.
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
///
/// Named by the run's token rather than the project: a project holds several
/// runs now, and the stop has to reach exactly the one whose bar segment was
/// pressed. `None` back is a run that ended before the stop arrived.
#[tauri::command]
pub async fn run_stop(handle: State<'_, RunHandle>, token: u64) -> Result<Option<Run>, RunError> {
    ask(&handle, |tx| Request::Stop(token, tx)).await
}

/// The `run:state` event fires before the webview can subscribe — the same
/// shape `tracker_health` has, and for the same reason. The set rather than
/// one run: the project may hold several, and `runs.js` keeps them whole the
/// way it kept the single one.
#[tauri::command]
pub async fn run_state(handle: State<'_, RunHandle>, project: String) -> Result<Vec<Run>, RunError> {
    ask(&handle, |tx| Request::State(project, tx)).await
}
