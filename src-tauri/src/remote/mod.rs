//! The desktop's half of the mobile companion: an iroh endpoint with a
//! persistent key, pairing by QR code, and the list of phones allowed to connect.
//!
//! This stage ends at the handshake. A paired phone connects, says `Hello`, is
//! answered with `HelloReply` and held open; the command bridge that will use
//! the connection from there is the next stage's. The design is
//! `docs/superpowers/specs/2026-10-15-mobile-companion-design.md`.
//!
//! The endpoint is not started with the app. Binding one contacts a relay, and
//! this app does not open a socket by itself on behalf of a feature nobody has
//! used: it starts when a phone is first paired, and at launch only if there is
//! already a phone to serve.

pub mod commands;
pub mod devices;
pub mod endpoint;
pub mod pairing;

#[cfg(test)]
mod tests;

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
/// **A placeholder.** The spec says the default is "ours" and gives no address;
/// `.example` is reserved and never resolves, so a build that ships this value
/// fails to reach a relay rather than quietly reaching somebody else's. Replace
/// it with the real URL before the first release that includes the companion.
/// It is written out again in the front end (`defaults()` in
/// `stores/settings.js`, `view` in `SettingsWindow.vue`, and the prop default of
/// `DevicesSettings.vue`) and those copies must agree with this one.
pub const DEFAULT_RELAY_URL: &str = "https://relay.smetana.example";

/// The protocol this desktop speaks. A phone on another major is answered with
/// this and disconnected.
pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 0;

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

    /// Starts the endpoint if it is not running with the relay the settings name
    /// now, and returns that relay. A changed relay URL takes effect here — at
    /// the next pairing — and ends the connections held on the old one.
    pub async fn ensure_running(self: &Arc<Self>) -> Result<String, String> {
        let wanted = (self.relay)();
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
    pub async fn pair_begin(self: &Arc<Self>) -> Result<PairBegin, String> {
        let relay = self.ensure_running().await?;
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
        self.connections.close_all(key, endpoint::CLOSE_REVOKED, b"revoked");
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
            let _ = sink_app.emit(DEVICES_EVENT, list);
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
    let serve_now = !remote.devices.is_empty();
    app.manage(remote.clone());
    if serve_now {
        tauri::async_runtime::spawn(async move {
            if let Err(err) = remote.ensure_running().await {
                log::warn!("remote: could not start the endpoint: {err}");
            }
        });
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
