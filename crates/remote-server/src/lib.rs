//! The server half of the remote protocol: an iroh endpoint with a persistent key, pairing by ticket,
//! and the list of devices allowed to connect. The headless
//! `smetanad` serves from it, and it has no window.
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
pub mod gateway;
pub mod pairing;
pub mod push;

#[cfg(test)]
mod contract;
#[cfg(test)]
mod tests;


use std::sync::Arc;

use remote_protocol::{DeviceKind, Platform, PublicKey};
use serde::Serialize;
// The workers live in `smetana-core`; these bring its modules in under the names the desktop shell has
// always used, so `crate::tracker::...` reads the same here as it does there.
use smetana_core::{agents, attachments, context, placement, runs, session, settings, terminal, tracker};

use devices::{Device, Devices};
use pairing::Pairing;

pub use smetana_core::remote::DEFAULT_RELAY_URL;

/// The protocol this desktop speaks. A phone on another major is answered with
/// this and disconnected.
pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 11;

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
    pub key: iroh::SecretKey,
    pub devices: Devices,
    pub pairing: Pairing,
    pub connections: endpoint::Connections,
    pub pairing_slots: Arc<tokio::sync::Semaphore>,
    pub hub: bridge::Hub,
    pub app_version: String,
    events: EventSink,
    relay: Box<dyn Fn() -> String + Send + Sync>,
    running: tokio::sync::Mutex<Option<Running>>,
    /// The endpoint bound on loopback with no relay, for a client on this machine. A second endpoint
    /// with the same key, not a second transport: the protocol on it is the one on the first.
    local: tokio::sync::Mutex<Option<Running>>,
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
            local: tokio::sync::Mutex::new(None),
        })
    }

    /// Tells the window (through the sink) and every desktop device listening for it.
    pub fn emit(&self, event: Event) {
        match &event {
            Event::PairRequest { device_name, platform } => self.hub.publish(
                PAIR_REQUEST_EVENT,
                &PairRequestPayload { device_name: device_name.clone(), platform: *platform },
            ),
            Event::DevicesChanged(list) => self.hub.publish(DEVICES_EVENT, &device_views(list)),
            Event::PairEnded => self.hub.publish(PAIR_ENDED_EVENT, &()),
        }
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
        *running = Some(self.serve(endpoint, wanted.clone(), endpoint::Door::Public));
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
        *running = Some(self.serve(endpoint, relay, endpoint::Door::Public));
    }

    /// Binds the loopback endpoint and serves on it, and returns the address a client on this machine
    /// dials: the key plus a direct socket address, and no relay anywhere. A second call replaces the
    /// first endpoint.
    pub async fn serve_local(self: &Arc<Self>) -> Result<iroh::EndpointAddr, String> {
        let endpoint = endpoint::bind_loopback(&self.key).await?;
        let addr = endpoint::loopback_addr(&endpoint).await?;
        let mut local = self.local.lock().await;
        if let Some(old) = local.take() {
            old.task.abort();
            old.endpoint.close().await;
        }
        *local = Some(self.serve(endpoint, "loopback".to_owned(), endpoint::Door::Loopback));
        Ok(addr)
    }

    fn serve(self: &Arc<Self>, endpoint: iroh::Endpoint, relay: String, door: endpoint::Door) -> Running {
        let task = tokio::spawn(endpoint::accept_loop(self.clone(), endpoint.clone(), door));
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
    pub async fn pair_begin(self: &Arc<Self>, relay: &str, kind: DeviceKind) -> Result<PairBegin, String> {
        if !crate::settings::model::valid_relay_url(relay.trim()) {
            return Err("That is not a usable relay address.".into());
        }
        let relay = self.ensure_running(Some(relay)).await?;
        let ticket = self.pairing.begin_for(self.public_key(), &relay, kind);
        Ok(PairBegin { ticket: ticket.encode_for(kind), expires_at: ticket.expires_at })
    }

    /// Stops the endpoint, closing every connection with it.
    pub async fn stop(&self) {
        if let Some(old) = self.running.lock().await.take() {
            old.task.abort();
            old.endpoint.close().await;
        }
        if let Some(old) = self.local.lock().await.take() {
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
    /// What the device may do; fixed at pairing. Added in protocol 1.8.
    pub kind: DeviceKind,
    /// Paired with the local credential: a client on the server's own machine. Absent (false) for
    /// every ticket-paired device, so what a phone reads is unchanged.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub local: bool,
}

impl From<&Device> for DeviceView {
    fn from(device: &Device) -> Self {
        Self {
            key: device.key.clone(),
            name: device.name.clone(),
            platform: device.platform,
            paired_at: device.paired_at,
            last_contact: device.last_contact,
            kind: device.kind,
            local: device.local,
        }
    }
}

pub fn device_views(list: &[Device]) -> Vec<DeviceView> {
    list.iter().map(DeviceView::from).collect()
}
