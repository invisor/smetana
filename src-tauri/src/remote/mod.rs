//! The desktop's half of the companion: what needs Tauri. The endpoint, pairing, the device list, the
//! bridge and the dispatcher are `crates/remote-server`, which the headless `smetanad` serves from as
//! well; this module makes a `Remote` out of an `AppHandle`, manages it, and wraps its commands for the
//! webview.
//!
//! The endpoint is not started with the app. Binding one contacts a relay, and this app does not open a
//! socket by itself on behalf of a feature nobody has used: it starts when a phone is first paired, and
//! at launch only if there is already a phone to serve.

pub mod commands;

use std::path::PathBuf;
use std::sync::Arc;

use remote_server::devices::{self, Devices};
use remote_server::dispatch::Services;
use remote_server::pairing::{self, Pairing};
use remote_server::{
    device_views, endpoint, Event, EventSink, PairRequestPayload, DEVICES_EVENT, PAIR_ENDED_EVENT,
    PAIR_REQUEST_EVENT,
};
use tauri::{AppHandle, Emitter, Manager};

pub use remote_server::{push, Remote, DEFAULT_RELAY_URL};

/// The directory the key and the device list live in.
fn data_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok()
}

/// The managed state of a running app as the dispatcher's services. `None` while any of it is
/// missing, which at the end of `setup` it never is.
fn services(app: &AppHandle, remote: Option<std::sync::Weak<Remote>>) -> Option<Services> {
    let ctx = crate::host::context(app);
    Some(Services {
        remote,
        tracker: app.try_state::<crate::tracker::service::TrackerHandle>()?.inner().clone(),
        runs: app.try_state::<crate::runs::service::RunHandle>()?.inner().clone(),
        sessions: app.try_state::<crate::session::service::SessionHandle>()?.inner().clone(),
        terminal: app.try_state::<crate::terminal::service::TerminalHandle>()?.inner().clone(),
        settings_path: crate::settings::commands::settings_path(&ctx).map_err(|err| err.to_string()),
        attachments_root: crate::attachments::store_root(&ctx).map_err(|err| err.to_string()),
        project_switched: Some(Services::announce_project_switch(ctx.clone())),
        ctx: Some(ctx),
        // The desktop is not a server one restarts from a client: it has a window to close.
        gateway: None,
    })
}

/// Builds the service, manages it, and starts the endpoint if there is a phone
/// to serve. Anything that goes wrong here is logged and leaves the feature off:
/// the rest of the app has no use for it.
pub fn start(app: AppHandle) {
    let Some(dir) = data_dir(&app) else {
        log::warn!("remote: no app data directory, the companion is off");
        return;
    };
    let key = match endpoint::load_or_create_key(&dir.join(endpoint::KEY_FILE)) {
        Ok(key) => key,
        Err(err) => {
            log::warn!("remote: no device key, the companion is off: {err}");
            return;
        }
    };
    let settings_path = crate::host::context(&app).settings_path();
    let relay = Box::new(move || {
        settings_path
            .as_ref()
            .map(|path| crate::settings::file::load(path).0.remote.relay_url)
            .unwrap_or_else(|| DEFAULT_RELAY_URL.to_owned())
    });
    let sink_app = app.clone();
    let events: EventSink = Arc::new(move |event| match event {
        Event::PairRequest { device_name, platform } => {
            let _ = sink_app.emit(PAIR_REQUEST_EVENT, PairRequestPayload { device_name, platform });
        }
        Event::DevicesChanged(list) => {
            let _ = sink_app.emit(DEVICES_EVENT, device_views(&list));
        }
        Event::PairEnded => {
            let _ = sink_app.emit(PAIR_ENDED_EVENT, ());
        }
    });
    let remote = Remote::new(
        key,
        Devices::load(dir.join(devices::FILE_NAME)),
        Pairing::new(pairing::system_clock()),
        app.package_info().version.to_string(),
        events,
        relay,
    );
    match services(&app, Some(Arc::downgrade(&remote))) {
        Some(services) => remote.hub.set_dispatch(Arc::new(services)),
        None => log::warn!("remote: the workers are not all managed, phones can call nothing"),
    }
    let serve_now = !remote.devices.is_empty();
    app.manage(remote.clone());
    if serve_now {
        tauri::async_runtime::spawn(async move {
            if let Err(err) = remote.ensure_running(None).await {
                log::warn!("remote: could not start the endpoint: {err}");
            }
        });
    }
}

/// Withdraws a pairing code when the settings window is destroyed. The code is
/// a secret on a screen that is gone; whoever photographed it would still have
/// the minutes it has left. Called from the window's own `Destroyed` event,
/// since the page's unmount hooks do not run when the OS window is torn down.
pub fn cancel_pairing(app: &AppHandle) {
    if let Some(remote) = app.try_state::<Arc<Remote>>() {
        remote.pairing.cancel();
    }
}

/// Closes the endpoint on the way out so connected phones are told rather than
/// left to time out.
pub fn shutdown(app: &AppHandle) {
    if let Some(remote) = app.try_state::<Arc<Remote>>() {
        let remote = remote.inner().clone();
        tauri::async_runtime::block_on(async move { remote.stop().await });
    }
}
