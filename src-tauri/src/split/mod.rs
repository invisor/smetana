//! The desktop as a client of one `smetanad`: the `split` build (`src-tauri` feature `split`).
//!
//! Step 5 of `docs/superpowers/specs/2026-10-25-headless-server-design.md`. In this build the shell
//! starts none of the workers and serves no phones: it holds one iroh connection to a server, and
//!
//! - `forward.rs` answers every `invoke` that is not the machine's own by sending it over that
//!   connection and handing the server's reply back;
//! - `link.rs` keeps the connection up, re-emits the server's events into the webview under their
//!   existing names, and tells the webview what state it is in (`state.rs`);
//! - `spawn.rs` finds the local server and, when there is none, starts one of its own, detached.
//!
//! Everything in the app that is not this directory is the same code the release runs. The build
//! without the feature never compiles it in.

mod forward;
mod link;
mod migrate;
mod spawn;
mod state;

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

pub use forward::handler;
use link::Link;
use spawn::Config;

/// Where the local server keeps what it keeps, below the app's own data directory: a directory that
/// belongs to the server alone, so the shell's files (the window's geometry, its device key) and the
/// server's (`settings.json`, `remote-key`, the runs' registry) cannot be mistaken for each other.
const SERVER_DIR: &str = "server";

fn config(app: &AppHandle) -> Result<Config, String> {
    let data = app.path().app_data_dir().map_err(|err| err.to_string())?;
    Ok(Config {
        server_dir: data.join(SERVER_DIR),
        identity_dir: data.clone(),
        exe: spawn::sidecar("smetanad"),
        bd: crate::host::sidecar_path(),
        resource_dir: app.path().resource_dir().ok(),
        log: data.join("smetanad.log"),
        settings: app.path().app_config_dir().ok().map(|dir| dir.join("settings.json")),
    })
}

/// Sets the connection up and starts keeping it. Called once, from `setup`, before the first window
/// can make a call.
pub fn start(app: &AppHandle) {
    let sink_app = app.clone();
    let link = Link::new(
        Arc::new(move |name, payload| {
            let _ = sink_app.emit(name, payload);
        }),
        true,
    );
    app.manage(link.clone());
    match config(app) {
        Ok(config) => {
            tauri::async_runtime::spawn(link::supervise(link, config));
        }
        Err(reason) => {
            log::error!("split: no data directory, no server to reach: {reason}");
            link.broken(format!("there is no data directory to keep a server in: {reason}"));
        }
    }
}

fn link(app: &AppHandle) -> Option<Arc<Link>> {
    app.try_state::<Arc<Link>>().map(|link| link.inner().clone())
}

/// `connection_state`: what the connection is doing now.
pub fn snapshot(app: &AppHandle) -> Value {
    match link(app) {
        Some(link) => serde_json::to_value(link.view()).unwrap_or(Value::Null),
        None => Value::Null,
    }
}

/// `connection_retry`: try now instead of at the end of the wait.
pub fn retry(app: &AppHandle) {
    if let Some(link) = link(app) {
        link.retry_now();
    }
}

/// A pairing code shown in the settings window is withdrawn when the window goes. The code lives in
/// the server, so this is a call, and one nobody waits for: a window that is closing has no one to
/// tell if it fails.
pub fn cancel_pairing(app: &AppHandle) {
    if let Some(link) = link(app) {
        tauri::async_runtime::spawn(async move {
            let _ = link.call("remote_pair_cancel", serde_json::json!({})).await;
        });
    }
}
