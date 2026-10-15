//! The phone's half of the bridge, with no Tauri in it.
//!
//! A `Core` owns the pairing, the connection to the desktop and the calls made
//! over it. The Tauri layer (`lib.rs`) is a thin set of commands over this, and
//! `core_tests.rs` drives the same code against a stand-in desktop on a local
//! relay.
//!
//! **One link, kept up.** Once paired, a supervisor task keeps one iroh
//! connection to the desktop: dial, `Hello`, read the answer, then serve calls and
//! events until it drops, then dial again after a pause that doubles up to a
//! minute. The state of that link is a `Status`, published to the front end as
//! `bridge:status`; after a drop the front end reads its snapshots again, because
//! the desktop does not replay what was missed.
//!
//! **A phone that is too old calls nothing.** If the desktop's `min_mobile_version`
//! is above this build's, the connection is closed straight after the handshake
//! and the status says `update-required`; no call ever leaves.

use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::{presets, Connection, ConnectionError};
use iroh::{Endpoint, EndpointAddr, RelayMode, RelayUrl};
use remote_protocol::{
    is_allowed_command, is_allowed_event, read_frame, write_frame, BridgeError, ClientMsg, Hello,
    HelloReply, PairReply, PairRequest, PairingTicket, Platform, PublicKey, ServerMsg, ALPN,
    CLOSE_REVOKED, CLOSE_UNKNOWN, CLOSE_VERSION, MAX_FRAME,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot, Notify};

use crate::secrets::{names, Identity, SecretStore};
use crate::version::older_than;

/// The protocol this build speaks. `PROTOCOL_MAJOR` must equal the desktop's
/// (`src-tauri/src/remote/mod.rs`; `tests/versions.rs` reads the file). The minor
/// is the newest this build knows: 1 added `UpdatePushToken`.
pub const PROTOCOL_MAJOR: u16 = 1;
pub const PROTOCOL_MINOR: u16 = 1;
/// The first minor whose desktop reads `ClientMsg::UpdatePushToken`.
const PUSH_TOKEN_MINOR: u16 = 1;

/// What the front end hears as `bridge:status`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum Status {
    Connecting,
    Online,
    /// `last_seen` is milliseconds since the epoch, or absent when this phone has
    /// never reached the desktop.
    Unreachable { last_seen: Option<u64> },
    /// The desktop wants a newer app than this one. The only thing the phone does
    /// then is show where to get it.
    UpdateRequired { store_url: String },
    /// Working, with a newer app on offer.
    UpdateRecommended { store_url: String, recommended: String },
    /// The desktop no longer lists this phone. Retrying cannot help.
    Revoked,
}

/// What the Rust core hands outward.
#[derive(Debug, Clone)]
pub enum Out {
    Event { name: String, payload: Value },
    Status(Status),
}

pub type Sink = Arc<dyn Fn(Out) + Send + Sync>;

/// Why a call did not return a value. `to_value` is what the front end receives as
/// the rejection of `invoke`: the desktop's own error where it gave one, a plain
/// string where it did not, which is how Tauri itself rejects.
#[derive(Debug, Clone, PartialEq)]
pub enum CallError {
    Offline,
    Forbidden,
    UnknownCommand,
    Handler(Value),
    Internal(String),
}

impl CallError {
    pub fn to_value(&self) -> Value {
        match self {
            CallError::Offline => Value::String("The workstation is not reachable.".into()),
            CallError::Forbidden => Value::String("forbidden".into()),
            CallError::UnknownCommand => Value::String("unknown command".into()),
            CallError::Handler(value) => value.clone(),
            CallError::Internal(message) => Value::String(message.clone()),
        }
    }
}

impl From<BridgeError> for CallError {
    fn from(err: BridgeError) -> Self {
        match err {
            BridgeError::Forbidden => CallError::Forbidden,
            BridgeError::UnknownCommand => CallError::UnknownCommand,
            BridgeError::Handler(value) => CallError::Handler(value),
            BridgeError::Internal(message) => CallError::Internal(message),
        }
    }
}

#[derive(Clone)]
pub struct Config {
    pub app_version: String,
    pub platform: Platform,
    pub device_name: String,
    /// Where `update-required` points.
    pub store_url: String,
    pub backoff_start: Duration,
    pub backoff_max: Duration,
    pub connect_timeout: Duration,
    pub handshake_timeout: Duration,
    pub call_timeout: Duration,
    /// How long to wait for the person at the desktop to press confirm.
    pub pair_timeout: Duration,
    /// Tests reach a relay with a self-signed certificate. Compiled out of every
    /// other build: the switch that turns certificate checking off does not exist
    /// in a binary anyone installs.
    #[cfg(test)]
    pub insecure_tls: bool,
}

impl Config {
    pub fn new(app_version: &str, platform: Platform, device_name: &str, store_url: &str) -> Self {
        Self {
            app_version: app_version.to_owned(),
            platform,
            device_name: device_name.to_owned(),
            store_url: store_url.to_owned(),
            backoff_start: Duration::from_secs(1),
            backoff_max: Duration::from_secs(60),
            connect_timeout: Duration::from_secs(15),
            handshake_timeout: Duration::from_secs(15),
            call_timeout: Duration::from_secs(120),
            pair_timeout: Duration::from_secs(150),
            #[cfg(test)]
            insecure_tls: false,
        }
    }
}

/// Who the phone is paired with, as kept in the vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingRecord {
    pub desktop: String,
    pub relay: String,
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, BridgeError>>>>>;

#[derive(Clone)]
struct Live {
    tx: mpsc::Sender<ClientMsg>,
    pending: Pending,
    minor: u16,
}

pub struct Core {
    store: Arc<dyn SecretStore>,
    sink: Sink,
    config: Config,
    status: Mutex<Status>,
    live: Mutex<Option<Live>>,
    listening: Mutex<HashSet<String>>,
    next_id: AtomicU64,
    last_seen: AtomicU64,
    retry: Notify,
    supervisor: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn now_secs() -> u64 {
    now_ms() / 1000
}

async fn bind(identity: &Identity, relay: &RelayUrl, config: &Config) -> Result<Endpoint, String> {
    let builder = Endpoint::builder(presets::Minimal)
        .secret_key(identity.iroh.clone())
        .relay_mode(RelayMode::custom([relay.clone()]));
    #[cfg(test)]
    let builder = if config.insecure_tls {
        builder.ca_tls_config(iroh::tls::CaTlsConfig::insecure_skip_verify())
    } else {
        builder
    };
    #[cfg(not(test))]
    let _ = config;
    builder.bind().await.map_err(|err| format!("could not start the endpoint: {err}"))
}

fn close_code(err: &ConnectionError) -> Option<u32> {
    match err {
        ConnectionError::ApplicationClosed(close) => u32::try_from(close.error_code.into_inner()).ok(),
        _ => None,
    }
}

/// How an attempt to hold the link ended.
enum Attempt {
    /// Never got as far as a working link.
    Failed,
    /// Had one, and lost it.
    Lost,
    Revoked,
    UpdateRequired,
}

impl Core {
    pub fn new(store: Arc<dyn SecretStore>, sink: Sink, config: Config) -> Arc<Self> {
        let last_seen = store.get(names::LAST_SEEN).and_then(|t| t.parse().ok()).unwrap_or(0);
        Arc::new(Self {
            store,
            sink,
            config,
            status: Mutex::new(Status::Connecting),
            live: Mutex::new(None),
            listening: Mutex::new(HashSet::new()),
            next_id: AtomicU64::new(1),
            last_seen: AtomicU64::new(last_seen),
            retry: Notify::new(),
            supervisor: Mutex::new(None),
        })
    }

    pub fn pairing(&self) -> Option<PairingRecord> {
        serde_json::from_str(&self.store.get(names::PAIRING)?).ok()
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    fn set_status(&self, next: Status) {
        {
            let mut status = self.status.lock().unwrap();
            if *status == next {
                return;
            }
            *status = next.clone();
        }
        (self.sink)(Out::Status(next));
    }

    fn unreachable(&self) -> Status {
        let seen = self.last_seen.load(Ordering::Relaxed);
        Status::Unreachable { last_seen: (seen > 0).then_some(seen) }
    }

    fn touch_seen(&self) {
        let now = now_ms();
        self.last_seen.store(now, Ordering::Relaxed);
        let _ = self.store.set(names::LAST_SEEN, &now.to_string());
    }

    fn token(&self) -> Option<String> {
        self.store.get(names::PUSH_TOKEN).filter(|t| !t.is_empty())
    }

    // ----- pairing ---------------------------------------------------------

    /// Pairs with the desktop a ticket names. Returns once the person at the
    /// desktop has confirmed, or with the reason it did not happen. On success
    /// the pairing is in the vault; the caller then calls `connect`.
    pub async fn pair(&self, ticket: &str) -> Result<(), String> {
        let ticket = PairingTicket::decode(ticket).map_err(|err| err.to_string())?;
        if ticket.expires_at <= now_secs() {
            return Err("This code has expired. Show a new one on the desktop.".into());
        }
        let relay: RelayUrl = ticket.relay.parse().map_err(|_| "The code names a relay that is not usable.".to_owned())?;
        let identity = Identity::load_or_create(&*self.store)?;
        let endpoint = bind(&identity, &relay, &self.config).await?;
        let token = self.token();
        let request = PairRequest {
            secret: ticket.secret,
            device_name: self.config.device_name.clone(),
            platform: self.config.platform,
            push_token: token.clone(),
            push_key: identity.push_key(),
            app_version: self.config.app_version.clone(),
        };
        let addr = EndpointAddr::new(ticket.desktop).with_relay_url(relay.clone());
        let outcome = self.pair_over(&endpoint, addr, &request).await;
        endpoint.close().await;
        outcome?;
        let record = PairingRecord { desktop: ticket.desktop.to_string(), relay: relay.to_string() };
        self.store.set(names::PAIRING, &serde_json::to_string(&record).map_err(|e| e.to_string())?)?;
        Ok(())
    }

    async fn pair_over(&self, endpoint: &Endpoint, addr: EndpointAddr, request: &PairRequest) -> Result<(), String> {
        let conn = tokio::time::timeout(self.config.connect_timeout, endpoint.connect(addr, ALPN))
            .await
            .map_err(|_| "The desktop did not answer. Is it running, and online?".to_owned())?
            .map_err(|err| format!("Could not reach the desktop: {err}"))?;
        let (mut send, mut recv) = conn.open_bi().await.map_err(|err| err.to_string())?;
        write_frame(&mut send, request).await.map_err(|err| err.to_string())?;
        let reply = tokio::time::timeout(self.config.pair_timeout, read_frame::<PairReply>(&mut recv, MAX_FRAME)).await;
        let outcome = match reply {
            Ok(Ok(PairReply::Accepted)) => Ok(()),
            Ok(Ok(PairReply::Rejected { reason })) => Err(rejection_text(&reason)),
            Ok(Err(_)) => {
                // The desktop closed without a reply. `CLOSE_PROTOCOL` is ambiguous: it is
                // what a desktop answers a key it already lists, and also what it sends when
                // the first frame did not arrive in time. Nothing is stored on either; the
                // person can show a new code and try again.
                let closed = tokio::time::timeout(Duration::from_secs(1), conn.closed()).await.ok();
                match closed.as_ref().and_then(close_code) {
                    Some(code) if code == remote_protocol::CLOSE_PROTOCOL => {
                        Err("The desktop closed the connection without answering. Show a new code on the desktop and try again.".into())
                    }
                    Some(code) if code == CLOSE_UNKNOWN => Err("The desktop is not showing a pairing code any more.".into()),
                    _ => Err("The connection to the desktop dropped before it answered.".into()),
                }
            }
            Err(_) => Err("The desktop did not confirm in time.".into()),
        };
        conn.close(remote_protocol::CLOSE_DONE.into(), b"done");
        outcome
    }

    /// Forgets the pairing. The phone's own keys stay: pairing again with the same
    /// key is a replacement on the desktop's list, not a second device.
    pub fn unpair(&self) {
        if let Some(handle) = self.supervisor.lock().unwrap().take() {
            handle.abort();
        }
        *self.live.lock().unwrap() = None;
        for name in [names::PAIRING, names::LAST_SEEN] {
            self.store.delete(name);
        }
        self.last_seen.store(0, Ordering::Relaxed);
        self.set_status(Status::Connecting);
    }

    // ----- the link --------------------------------------------------------

    /// Starts keeping the link up, or asks a waiting supervisor to try now.
    pub fn connect(self: &Arc<Self>) -> Result<(), String> {
        let record = self.pairing().ok_or("This phone is not paired with a workstation.")?;
        let mut slot = self.supervisor.lock().unwrap();
        if slot.as_ref().is_some_and(|handle| !handle.is_finished()) {
            self.retry.notify_one();
            return Ok(());
        }
        let identity = Identity::load_or_create(&*self.store)?;
        let core = self.clone();
        *slot = Some(tokio::spawn(async move { core.supervise(identity, record).await }));
        Ok(())
    }

    async fn supervise(self: Arc<Self>, identity: Identity, record: PairingRecord) {
        let (Ok(relay), Ok(desktop)) = (record.relay.parse::<RelayUrl>(), PublicKey::from_str(&record.desktop)) else {
            log::warn!("companion: the stored pairing is not readable");
            self.set_status(self.unreachable());
            return;
        };
        let addr = EndpointAddr::new(desktop).with_relay_url(relay.clone());
        let mut delay = self.config.backoff_start;
        let mut retried = false;
        let endpoint = loop {
            match bind(&identity, &relay, &self.config).await {
                Ok(endpoint) => break endpoint,
                Err(err) => {
                    log::warn!("companion: {err}");
                    self.set_status(self.unreachable());
                    self.pause(&mut delay).await;
                }
            }
        };
        loop {
            // An explicit retry always shows `connecting` again, even from `unreachable`:
            // the front end set it optimistically, and a failed attempt that ends in the
            // same `unreachable` would otherwise emit nothing and leave it there.
            if retried || !matches!(self.status(), Status::Unreachable { .. }) {
                self.set_status(Status::Connecting);
            }
            match self.attempt(&endpoint, addr.clone()).await {
                Attempt::Revoked => {
                    self.set_status(Status::Revoked);
                    break;
                }
                Attempt::UpdateRequired => {
                    self.set_status(Status::UpdateRequired { store_url: self.config.store_url.clone() });
                    break;
                }
                Attempt::Lost => {
                    delay = self.config.backoff_start;
                    self.set_status(self.unreachable());
                }
                Attempt::Failed => self.set_status(self.unreachable()),
            }
            retried = self.pause(&mut delay).await;
        }
        endpoint.close().await;
    }

    /// Waits out the pause (or is told to try now) and doubles it for next time.
    /// Returns whether it was cut short by a request to retry.
    async fn pause(&self, delay: &mut Duration) -> bool {
        let retried = tokio::select! {
            _ = tokio::time::sleep(*delay) => false,
            _ = self.retry.notified() => true,
        };
        *delay = (*delay * 2).min(self.config.backoff_max);
        retried
    }

    async fn attempt(&self, endpoint: &Endpoint, addr: EndpointAddr) -> Attempt {
        let conn = match tokio::time::timeout(self.config.connect_timeout, endpoint.connect(addr, ALPN)).await {
            Ok(Ok(conn)) => conn,
            _ => return Attempt::Failed,
        };
        let Some((send, mut recv, reply)) = self.handshake(&conn).await else {
            return match tokio::time::timeout(Duration::from_secs(1), conn.closed()).await.ok().as_ref().and_then(close_code) {
                Some(code) if code == CLOSE_REVOKED || code == CLOSE_UNKNOWN => Attempt::Revoked,
                Some(code) if code == CLOSE_VERSION => Attempt::UpdateRequired,
                _ => Attempt::Failed,
            };
        };
        if reply.protocol_major != PROTOCOL_MAJOR || older_than(&self.config.app_version, &reply.min_mobile_version) {
            // Nothing is called on a desktop this build is too old for.
            conn.close(remote_protocol::CLOSE_DONE.into(), b"update required");
            return Attempt::UpdateRequired;
        }

        let (tx, mut outgoing) = mpsc::channel::<ClientMsg>(64);
        let pending: Pending = Arc::default();
        *self.live.lock().unwrap() = Some(Live { tx: tx.clone(), pending: pending.clone(), minor: reply.protocol_minor });
        let writer = tokio::spawn(async move {
            let mut send = send;
            while let Some(msg) = outgoing.recv().await {
                if write_frame(&mut send, &msg).await.is_err() {
                    break;
                }
            }
        });
        for event in self.listening.lock().unwrap().clone() {
            let _ = tx.try_send(ClientMsg::Listen { event });
        }
        self.sync_token();
        self.touch_seen();
        if older_than(&self.config.app_version, &reply.recommended_mobile_version) {
            self.set_status(Status::UpdateRecommended {
                store_url: self.config.store_url.clone(),
                recommended: reply.recommended_mobile_version.clone(),
            });
        } else {
            self.set_status(Status::Online);
        }

        // Read until the link goes.
        loop {
            match read_frame::<ServerMsg>(&mut recv, MAX_FRAME).await {
                Ok(ServerMsg::Reply { id, result }) => {
                    if let Some(waiting) = pending.lock().unwrap().remove(&id) {
                        let _ = waiting.send(result);
                    }
                }
                Ok(ServerMsg::Event { name, payload }) => (self.sink)(Out::Event { name, payload }),
                Err(_) => break,
            }
        }
        *self.live.lock().unwrap() = None;
        pending.lock().unwrap().clear();
        writer.abort();
        self.touch_seen();
        let code = tokio::time::timeout(Duration::from_secs(1), conn.closed()).await.ok().as_ref().and_then(close_code);
        match code {
            Some(c) if c == CLOSE_REVOKED || c == CLOSE_UNKNOWN => Attempt::Revoked,
            Some(c) if c == CLOSE_VERSION => Attempt::UpdateRequired,
            _ => Attempt::Lost,
        }
    }

    async fn handshake(&self, conn: &Connection) -> Option<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream, HelloReply)> {
        let hello = Hello {
            protocol_major: PROTOCOL_MAJOR,
            protocol_minor: PROTOCOL_MINOR,
            app_version: self.config.app_version.clone(),
            capabilities: Vec::new(),
        };
        let exchange = async {
            let (mut send, mut recv) = conn.open_bi().await.ok()?;
            write_frame(&mut send, &hello).await.ok()?;
            let reply = read_frame::<HelloReply>(&mut recv, MAX_FRAME).await.ok()?;
            Some((send, recv, reply))
        };
        tokio::time::timeout(self.config.handshake_timeout, exchange).await.ok().flatten()
    }

    // ----- calls and events ------------------------------------------------

    /// Calls a desktop command. Nothing is sent blind: with no working link the
    /// call fails at once, and a command off the phone's allowlist never leaves.
    pub async fn invoke(&self, cmd: &str, args: Value) -> Result<Value, CallError> {
        if !is_allowed_command(cmd) {
            return Err(CallError::Forbidden);
        }
        let live = self.live.lock().unwrap().clone().ok_or(CallError::Offline)?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        live.pending.lock().unwrap().insert(id, tx);
        if live.tx.send(ClientMsg::Invoke { id, cmd: cmd.to_owned(), args }).await.is_err() {
            live.pending.lock().unwrap().remove(&id);
            return Err(CallError::Offline);
        }
        match tokio::time::timeout(self.config.call_timeout, rx).await {
            Ok(Ok(result)) => result.map_err(CallError::from),
            // The sender was dropped: the link went while the call was out.
            Ok(Err(_)) => Err(CallError::Offline),
            Err(_) => {
                live.pending.lock().unwrap().remove(&id);
                Err(CallError::Internal("The workstation did not answer in time.".into()))
            }
        }
    }

    /// Subscribes to a desktop event. Remembered, and sent again after a reconnect.
    pub fn listen(&self, event: &str) -> Result<(), CallError> {
        if !is_allowed_event(event) {
            return Err(CallError::Forbidden);
        }
        self.listening.lock().unwrap().insert(event.to_owned());
        if let Some(live) = self.live.lock().unwrap().as_ref() {
            let _ = live.tx.try_send(ClientMsg::Listen { event: event.to_owned() });
        }
        Ok(())
    }

    pub fn unlisten(&self, event: &str) {
        self.listening.lock().unwrap().remove(event);
        if let Some(live) = self.live.lock().unwrap().as_ref() {
            let _ = live.tx.try_send(ClientMsg::Unlisten { event: event.to_owned() });
        }
    }

    // ----- push token ------------------------------------------------------

    /// The platform handed over a push token (or a new one). Kept for the next
    /// pairing and reported to the desktop now if there is a link to report on.
    pub fn set_push_token(&self, token: &str) {
        if token.is_empty() {
            return;
        }
        let _ = self.store.set(names::PUSH_TOKEN, token);
        self.sync_token();
    }

    /// Tells the desktop the token. Sent at every handshake and at every change, with no
    /// record of what the desktop was told before: the message is idempotent there, and a
    /// record kept on the phone could say "reported" for a token that never arrived. Only
    /// to a desktop whose protocol knows the message: an older one would read an unknown
    /// variant as a broken frame and close.
    fn sync_token(&self) {
        let Some(token) = self.token() else { return };
        let guard = self.live.lock().unwrap();
        let Some(live) = guard.as_ref() else { return };
        if live.minor < PUSH_TOKEN_MINOR {
            return;
        }
        let _ = live.tx.try_send(ClientMsg::UpdatePushToken { token });
    }
}

/// The desktop's reason, in words for a person. Its own wording is kept when it
/// has one worth showing.
fn rejection_text(reason: &str) -> String {
    let reason = reason.trim();
    if reason.is_empty() {
        "The desktop refused the pairing.".into()
    } else {
        let mut text = reason.to_owned();
        if let Some(first) = text.get_mut(0..1) {
            first.make_ascii_uppercase();
        }
        if !text.ends_with('.') {
            text.push('.');
        }
        text
    }
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
