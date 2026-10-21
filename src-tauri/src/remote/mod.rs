//! The desktop's half of the mobile companion: an iroh endpoint with a
//! persistent key, pairing by QR code, and the list of phones allowed to connect.
//!
//! A paired phone connects, says `Hello`, is answered with `HelloReply` and then
//! speaks the command bridge (`bridge.rs`) on the same stream: it calls the
//! allowlisted commands and listens to the allowlisted events. The design is
//! `docs/superpowers/specs/2026-10-15-mobile-companion-design.md`.
//!
//! The endpoint is not started with the app. Binding one contacts a relay, and
//! this app does not open a socket by itself on behalf of a feature nobody has
//! used: it starts when a phone is first paired, and at launch only if there is
//! already a phone to serve.

pub mod bridge;
pub mod commands;
pub mod devices;
pub mod dispatch;
pub mod endpoint;
pub mod pairing;
pub mod push;

#[cfg(test)]
mod contract;
#[cfg(test)]
mod tests;

pub use bridge::emit;

use std::path::PathBuf;
use std::sync::Arc;

use remote_protocol::{Platform, PublicKey};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use devices::{Device, Devices};
use pairing::Pairing;

/// The relay a desktop uses until its owner names another, and the one put in
/// every ticket it issues.
///
/// Ours: the relay in `server/`, behind Caddy on the project's own VPS
/// (`server/README.md`, "Our deployment").
/// It is written out twice in the tree, here and as `DEFAULT_RELAY_URL` in
/// `components/settings/devices.js`, which every front-end default imports;
/// `tests/components/settings/devices.test.js` holds the two equal.
pub const DEFAULT_RELAY_URL: &str = "https://relay.smetana.io";

/// The protocol this desktop speaks. A phone on another major is answered with
/// this and disconnected.
pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 6;

/// Below this the phone shows one screen, "Update the app", and calls nothing.
pub const MIN_MOBILE_VERSION: &str = "0.1.0";
/// Below this the phone shows a dismissible banner.
pub const RECOMMENDED_MOBILE_VERSION: &str = "0.1.0";

/// What the front end hears about, whatever carries it.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A phone with a valid code is asking to be paired.
    PairRequest { device_name: String, platform: Platform },
    /// The list of devices changed.
    DevicesChanged(Vec<Device>),
    /// The request that was on the screen is over — decided, timed out or the
    /// phone left — and the prompt should go.
    PairEnded,
}

pub type EventSink = Arc<dyn Fn(Event) + Send + Sync>;

struct Running {
    endpoint: iroh::Endpoint,
    relay: String,
    task: tokio::task::JoinHandle<()>,
}

pub struct Remote {
    pub(crate) key: iroh::SecretKey,
    pub(crate) devices: Devices,
    pub(crate) pairing: Pairing,
    pub(crate) connections: endpoint::Connections,
    pub(crate) pairing_slots: Arc<tokio::sync::Semaphore>,
    pub(crate) hub: bridge::Hub,
    pub(crate) app_version: String,
    events: EventSink,
    relay: Box<dyn Fn() -> String + Send + Sync>,
    running: tokio::sync::Mutex<Option<Running>>,
}

impl Remote {
    pub fn new(
        key: iroh::SecretKey,
        devices: Devices,
        pairing: Pairing,
        app_version: String,
        events: EventSink,
        relay: Box<dyn Fn() -> String + Send + Sync>,
    ) -> Arc<Self> {
        Arc::new(Self {
            key,
            devices,
            pairing,
            connections: endpoint::Connections::default(),
            pairing_slots: Arc::new(tokio::sync::Semaphore::new(endpoint::MAX_PAIRING_CONNECTIONS)),
            hub: bridge::Hub::default(),
            app_version,
            events,
            relay,
            running: tokio::sync::Mutex::new(None),
        })
    }

    pub(crate) fn emit(&self, event: Event) {
        (self.events)(event);
    }

    pub fn public_key(&self) -> PublicKey {
        self.key.public()
    }

    /// Starts the endpoint if it is not running with the relay wanted, and
    /// returns that relay. `wanted` is the address the caller has on screen; with
    /// none, the one `settings.json` holds (start-up). A changed relay takes
    /// effect here and ends the connections held on the old one.
    pub async fn ensure_running(self: &Arc<Self>, wanted: Option<&str>) -> Result<String, String> {
        let wanted = match wanted {
            Some(url) => url.trim().to_owned(),
            None => (self.relay)(),
        };
        let mut running = self.running.lock().await;
        if let Some(current) = running.as_ref() {
            if current.relay == wanted {
                return Ok(wanted);
            }
        }
        if let Some(old) = running.take() {
            old.task.abort();
            old.endpoint.close().await;
        }
        let endpoint = endpoint::bind(&self.key, &wanted).await?;
        *running = Some(self.serve(endpoint, wanted.clone()));
        Ok(wanted)
    }

    /// Serves on an endpoint somebody else bound. The production path goes
    /// through `ensure_running`; this is its seam, so a test can stand an
    /// endpoint on loopback with no relay.
    #[cfg(test)]
    pub async fn attach(self: &Arc<Self>, endpoint: iroh::Endpoint, relay: String) {
        let mut running = self.running.lock().await;
        if let Some(old) = running.take() {
            old.task.abort();
            old.endpoint.close().await;
        }
        *running = Some(self.serve(endpoint, relay));
    }

    fn serve(self: &Arc<Self>, endpoint: iroh::Endpoint, relay: String) -> Running {
        let task = tokio::spawn(endpoint::accept_loop(self.clone(), endpoint.clone()));
        Running { endpoint, relay, task }
    }

    /// Issues a ticket for a new pairing, starting the endpoint first: the relay
    /// in the ticket is the one the endpoint is actually on.
    ///
    /// The relay is an argument and not read from `settings.json`: an edit in the
    /// settings window reaches that file only after the app window's debounce,
    /// so a press that followed it closely would pair through the old relay while
    /// the field showed the new one. What the person sees is what is used. An
    /// address `valid_relay_url` refuses is an error rather than a fallback, for
    /// the same reason.
    pub async fn pair_begin(self: &Arc<Self>, relay: &str) -> Result<PairBegin, String> {
        if !crate::settings::model::valid_relay_url(relay.trim()) {
            return Err("That is not a usable relay address.".into());
        }
        let relay = self.ensure_running(Some(relay)).await?;
        let ticket = self.pairing.begin(self.public_key(), &relay);
        Ok(PairBegin { ticket: ticket.encode(), expires_at: ticket.expires_at })
    }

    /// Stops the endpoint, closing every connection with it.
    pub async fn stop(&self) {
        if let Some(old) = self.running.lock().await.take() {
            old.task.abort();
            old.endpoint.close().await;
        }
    }

    /// Removes a device and closes every connection it has open. The removal
    /// comes first: a connection racing in behind the close is refused by the
    /// list, not by luck.
    pub fn revoke(&self, key: &PublicKey) -> Result<(), String> {
        let removed = self.devices.remove(key)?;
        self.connections.close_all(key, remote_protocol::CLOSE_REVOKED, b"revoked");
        if removed {
            self.emit(Event::DevicesChanged(self.devices.all()));
        }
        Ok(())
    }
}

/// What `remote_pair_begin` answers.
#[derive(Debug, Clone, Serialize)]
pub struct PairBegin {
    pub ticket: String,
    pub expires_at: u64,
}

/// The payload of `remote:pair-request`.
#[derive(Debug, Clone, Serialize)]
pub struct PairRequestPayload {
    pub device_name: String,
    pub platform: Platform,
}

pub const PAIR_REQUEST_EVENT: &str = "remote:pair-request";
pub const DEVICES_EVENT: &str = "remote:devices";
pub const PAIR_ENDED_EVENT: &str = "remote:pair-ended";

/// A device as a webview may see it. The push token and push key stay on disk:
/// nothing in the interface uses them, and a webview has no business holding
/// the credentials that wake somebody's phone.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceView {
    pub key: String,
    pub name: String,
    pub platform: Platform,
    pub paired_at: u64,
    pub last_contact: Option<u64>,
}

impl From<&Device> for DeviceView {
    fn from(device: &Device) -> Self {
        Self {
            key: device.key.clone(),
            name: device.name.clone(),
            platform: device.platform,
            paired_at: device.paired_at,
            last_contact: device.last_contact,
        }
    }
}

pub fn device_views(list: &[Device]) -> Vec<DeviceView> {
    list.iter().map(DeviceView::from).collect()
}

/// The directory the key and the device list live in.
fn data_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_data_dir().ok()
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
    let settings_path = crate::settings::path(&app);
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
    match dispatch::Services::from_app(&app) {
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
