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

use std::path::PathBuf;
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
    let data = crate::dirs::data_dir(app)?;
    let settings = crate::dirs::config_dir(app).ok().map(|dir| dir.join("settings.json"));
    let mut config = placement(data, settings, crate::dirs::DEVELOPMENT);
    config.exe = spawn::sidecar("smetanad");
    config.bd = crate::host::sidecar_path();
    config.resource_dir = app.path().resource_dir().ok();
    Ok(config)
}

/// Where everything of the local placement is, for a data directory and a device settings file.
///
/// A release build's local server inherits what the release before the switch-over kept in this
/// same data directory (`--inherit-from`), and the device file's server half is handed to it
/// (`migrate.rs`). A **development build does neither**: its data directory is its own
/// (`dirs::development_dir`), so there is nothing of a release in it to take, and the migration is
/// switched off outright, so that no slip in where a directory points can ever send a development
/// build's migration at the release's live files.
fn placement(data: PathBuf, settings: Option<PathBuf>, development: bool) -> Config {
    Config {
        server_dir: data.join(SERVER_DIR),
        identity_dir: data.clone(),
        inherit_from: (!development).then(|| data.clone()),
        exe: None,
        bd: None,
        resource_dir: None,
        log: data.join("smetanad.log"),
        settings: if development { None } else { settings },
    }
}

/// Sets the connection up and starts keeping it. Called once, from `setup`, before the first window
/// can make a call.
pub fn start(app: &AppHandle) {
    let sink_app = app.clone();
    // Which server this device uses is read before the first attempt: a person who chose a remote one
    // must not see the local one started for a moment first.
    let chosen = crate::dirs::data_dir(app).map(|dir| target::load(&dir)).unwrap_or(Target::Local);
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
            log::error!("client: no data directory, no server to reach: {reason}");
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
    crate::dirs::data_dir(app)
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

/// `tracker_access_reset`: make macOS forget its stored refusal of the open project's folder, then
/// stop the server on this machine so that a fresh one is started and asked again.
///
/// **Who holds the grant.** The folder is read by `smetanad`, not by this process. A server this app
/// started is a child of the app, and macOS attributes a child's file access to the app bundle that
/// spawned it (the responsible process; nothing here disclaims it), so the stored refusal is the
/// app's identifier's — the same one the release that read folders in-process held. The reset is
/// therefore `tccutil reset <service> <this app's identifier>`, as it always was, and it is the
/// shell's to make: the server does not know which bundle it belongs to.
///
/// **Why the server stops rather than this app.** A reset takes effect only for a process that has
/// not been refused yet. The process that was refused is the server, so it is the server that has to
/// be new: it is asked to stop (`server_restart` `stop-when-idle`, which ends nothing and stops at
/// once when nothing is alive), and the connection loop finds the directory free and starts a fresh
/// one from this app, which makes it the app's child again. Not `now`, which re-executes the same
/// process.
///
/// **Refused** for a server on another machine (its permissions are that machine's), when the server
/// says this folder has no promptable permission, and while anything is alive on the server: stopping
/// it ends every run, session and shell it holds, and that list is what the refusal names.
pub async fn access_reset(app: &AppHandle) -> Result<(), String> {
    let link = link(app).ok_or("The connection has not been set up.")?;
    let local = link.target().is_local();
    let repair = if local { link.call("tracker_access_repair", json!({})).await.map_err(said)? } else { Value::Null };
    let settings = if local { link.call("settings_load", json!({ "project": null })).await.map_err(said)? } else { Value::Null };
    let work = if local { link.call("server_work", json!({})).await.map_err(said)? } else { Value::Null };
    let dir = reset_plan(local, &repair, &settings["activeProject"], &work)?;
    let identifier = app.config().identifier.clone();
    tauri::async_runtime::spawn_blocking(move || smetana_core::tracker::access::reset(&dir, &identifier))
        .await
        .map_err(|err| format!("the reset did not finish: {err}"))??;
    link.call("server_restart", json!({ "mode": "stop-when-idle", "confirmed": null })).await.map_err(said)?;
    Ok(())
}

/// A rejection from the server, as the words the front end puts in front of its caption.
fn said(value: Value) -> String {
    match value {
        Value::String(text) => text,
        other => other.get("message").and_then(Value::as_str).map(str::to_owned).unwrap_or_else(|| other.to_string()),
    }
}

/// The checks `access_reset` makes before anything is changed, pure: the folder to reset, or why not.
fn reset_plan(local: bool, repair: &Value, active: &Value, work: &Value) -> Result<PathBuf, String> {
    if !local {
        return Err("The server is on another machine, so its folder permissions are that machine's to reset.".to_owned());
    }
    if repair.as_str() != Some("reset") {
        return Err("No permission macOS will ask about again governs this folder, so there is nothing here to reset.".to_owned());
    }
    let dir = active.as_str().filter(|dir| !dir.is_empty()).ok_or("No project is open.")?;
    let live: Vec<String> = work
        .as_array()
        .map(|list| list.iter().map(|item| item["label"].as_str().or(item["project"].as_str()).unwrap_or("work").to_owned()).collect())
        .unwrap_or_default();
    if !live.is_empty() {
        return Err(format!(
            "Still running on the server: {}. Resetting the permission restarts the server, which would end it.",
            live.join(", ")
        ));
    }
    Ok(PathBuf::from(dir))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_folder_permission_is_reset_only_for_the_local_server_with_nothing_alive_on_it() {
        let reset = json!("reset");
        let dir = json!("/Users/me/Desktop/p");
        let idle = json!([]);
        assert_eq!(reset_plan(true, &reset, &dir, &idle), Ok(PathBuf::from("/Users/me/Desktop/p")));
        assert!(reset_plan(false, &reset, &dir, &idle).unwrap_err().contains("another machine"));
        assert!(reset_plan(true, &json!("full-disk-access"), &dir, &idle).unwrap_err().contains("nothing here to reset"));
        assert!(reset_plan(true, &reset, &Value::Null, &idle).unwrap_err().contains("No project"));
        let busy = json!([{ "kind": "run", "id": "r1", "project": "/Users/me/notes", "label": "notes run", "owner": null }]);
        let refused = reset_plan(true, &reset, &dir, &busy).unwrap_err();
        assert!(refused.contains("notes run") && refused.contains("restarts the server"), "{refused}");
    }

    #[test]
    fn a_release_build_inherits_its_own_data_directory_and_hands_its_settings_over() {
        let data = PathBuf::from("/support/com.invisor.smetana");
        let config = placement(data.clone(), Some(data.join("settings.json")), false);
        assert_eq!(config.server_dir, data.join("server"));
        assert_eq!(config.inherit_from.as_deref(), Some(data.as_path()));
        assert_eq!(config.settings, Some(data.join("settings.json")));
        let args: Vec<String> = spawn::arguments(&config).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(args.windows(2).any(|pair| pair == ["--inherit-from", "/support/com.invisor.smetana"]));
    }

    #[test]
    fn a_development_build_inherits_nothing_and_migrates_nothing_and_keeps_out_of_the_release_directory() {
        let release = PathBuf::from("/support/com.invisor.smetana");
        let data = crate::dirs::development_dir(&release);
        let config = placement(data.clone(), Some(data.join("settings.json")), true);
        assert_eq!(config.inherit_from, None, "no key or paired phone of the release is taken");
        assert_eq!(config.settings, None, "no settings file is relieved of its server half");
        for path in [&config.server_dir, &config.identity_dir, &config.log] {
            assert!(!path.starts_with(&release), "{} is inside the release's directory", path.display());
        }
        let args: Vec<String> = spawn::arguments(&config).iter().map(|a| a.to_string_lossy().into_owned()).collect();
        assert!(!args.iter().any(|arg| arg == "--inherit-from"));
    }
}
