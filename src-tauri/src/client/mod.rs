//! The desktop as a client of one `smetanad`.
//!
//! `docs/superpowers/specs/2026-10-25-headless-server-design.md`. The shell starts none of the workers
//! and serves no phones: it holds one iroh connection to a server, and
//!
//! - `forward.rs` answers every `invoke` that is not the machine's own by sending it over that
//!   connection and handing the server's reply back;
//! - `link.rs` keeps the connection up, re-emits the server's events into the webview under their
//!   existing names, and tells the webview what state it is in (`state.rs`);
//! - `spawn.rs` finds the local server and, when there is none, starts one of its own, detached.
//!
//! Everything in the app that is not this directory is the machine's own: windows, dialogs, the
//! clipboard, the opener, the updater and the login item.

mod forward;
mod link;
mod migrate;
mod spawn;
mod state;
mod target;

use std::sync::Arc;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};

pub use forward::handler;
use link::Link;
use spawn::Config;
use target::Target;

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
    // Which server this device uses is read before the first attempt: a person who chose a remote one
    // must not see the local one started for a moment first.
    let chosen = app.path().app_data_dir().map(|dir| target::load(&dir)).unwrap_or(Target::Local);
    let link = Link::with_target(
        Arc::new(move |name, payload| {
            let _ = sink_app.emit(name, payload);
        }),
        chosen,
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

/// `server_target`: which server this device uses, and whether it is connected to it.
pub fn target_view(app: &AppHandle) -> Value {
    match link(app) {
        Some(link) => json!({ "target": serde_json::to_value(link.target()).unwrap_or(Value::Null) }),
        None => Value::Null,
    }
}

fn identity_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path().app_data_dir().map_err(|err| err.to_string())
}

/// `server_use_local`: back to the server on this machine, which is started again if it is not
/// running (the loop does that, not this call).
pub fn use_local(app: &AppHandle) -> Result<Value, String> {
    let link = link(app).ok_or("The connection has not been set up.")?;
    target::save(&identity_dir(app)?, &Target::Local)?;
    link.set_target(Target::Local);
    Ok(target_view(app))
}

/// The live work of the server on this machine, for the question the switch asks: empty when this
/// device is not using it or cannot reach it (a work list is no reason to refuse a switch).
pub async fn local_work(app: &AppHandle) -> Value {
    let Some(link) = link(app) else { return json!([]) };
    if !link.target().is_local() {
        return json!([]);
    }
    link.call("server_work", json!({})).await.unwrap_or_else(|_| json!([]))
}

/// `server_use_remote`: pairs with the server a desktop ticket names, and moves this device to it.
///
/// Pairing comes first and is the only step that can fail the whole switch: a bad or spent ticket
/// leaves the device exactly where it was. Then, if the person asked for it, the local server is told
/// to stop once its work is done (`stop-when-idle`, which ends nothing); that is best effort, and its
/// outcome is reported rather than made a condition. The local server is otherwise left running.
pub async fn use_remote(app: &AppHandle, ticket: String, stop_local: bool) -> Result<Value, String> {
    let link = link(app).ok_or("The connection has not been set up.")?;
    let dir = identity_dir(app)?;
    let record = smetanad::client::Client::pair_remote(&dir, &ticket).await.map_err(|err| err.to_string())?;
    let mut stop = Value::Null;
    if stop_local && link.target().is_local() {
        stop = match link.call("server_restart", json!({ "mode": "stop-when-idle", "confirmed": null })).await {
            Ok(reply) => reply.get("outcome").cloned().unwrap_or(Value::Null),
            Err(why) => json!({ "failed": why }),
        };
    }
    let target = Target::remote(record);
    target::save(&dir, &target)?;
    link.set_target(target);
    let mut answer = target_view(app);
    answer["localStop"] = stop;
    Ok(answer)
}
