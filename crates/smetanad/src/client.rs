//! The client every subcommand but `run` is: a desktop-kind device that reaches the server over its
//! loopback endpoint and speaks the ordinary bridge.
//!
//! It pairs itself with the local credential (`local.rs`) the first time, with a key of its own kept
//! beside the server's (`local-client-key`), and remembers that it did in `local-client-paired` so the
//! next call goes straight to `Hello`. The same type is the "client-test" of the integration tests:
//! what pairs through the file there is exactly what the command line pairs through.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::{presets, Connection, ConnectionError, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, RelayMode};
use remote_protocol::{
    encode_hello, read_frame, write_frame, BridgeError, ClientMsg, Hello, HelloInfo, HelloReply, PairReply,
    PairRequest, Platform, ServerMsg, ALPN, CLOSE_PROTOCOL, CLOSE_UNKNOWN, MAX_FRAME,
};
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

use crate::local;

pub const KEY_FILE: &str = "local-client-key";
pub const PAIRED_FILE: &str = "local-client-paired";

/// What a call's error says when the request could not be written to the connection, so it never
/// reached the server. `Client::call` makes this message; a caller that has to tell it from the other
/// loss (the connection closing with the request already sent) compares against these two.
pub const SEND_FAILED: &str = "the connection is gone";

/// What a call's error says when the connection ended after the request was sent and before the
/// answer came: the server may have done what was asked.
pub const CLOSED_BEFORE_REPLY: &str = "the connection closed before the reply";

/// What the device is called in the server's list.
const DEVICE_NAME: &str = "smetanad (this machine)";
const STEP_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("{0}")]
    Setup(String),
    #[error("the server refused the pairing: {0}")]
    Refused(String),
    #[error("the connection closed: {0}")]
    Closed(String),
    /// The server ended the connection with one of the protocol's `CLOSE_*` codes, which says why
    /// better than any message: an unknown key, a revoked one, a major version it does not speak.
    #[error("the server closed the connection with code {0}")]
    Code(u32),
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, BridgeError>>>>>;

pub struct Client {
    conn: Connection,
    send: tokio::sync::Mutex<SendStream>,
    pending: Pending,
    next: AtomicU64,
    /// `None` once [`Client::take_events`] has handed the receiver to a task of its own.
    events: Option<mpsc::UnboundedReceiver<(String, Value)>>,
    pub hello: HelloReply,
    _endpoint: Endpoint,
}

fn setup(err: impl std::fmt::Display) -> ClientError {
    ClientError::Setup(err.to_string())
}

/// The address a client on this machine dials, from the credential.
pub fn address(credential: &local::Credential) -> EndpointAddr {
    EndpointAddr::from_parts(credential.node, credential.addrs.iter().map(|addr| iroh::TransportAddr::Ip(*addr)))
}

/// The code of a close, when the peer closed the connection with one.
pub fn close_code(err: &ConnectionError) -> Option<u32> {
    match err {
        ConnectionError::ApplicationClosed(close) => u32::try_from(close.error_code.into_inner()).ok(),
        _ => None,
    }
}

async fn bind_client(key: &iroh::SecretKey) -> Result<Endpoint, ClientError> {
    Endpoint::builder(presets::Minimal)
        .secret_key(key.clone())
        .relay_mode(RelayMode::Disabled)
        .clear_ip_transports()
        .bind_addr("127.0.0.1:0")
        .map_err(setup)?
        .bind()
        .await
        .map_err(setup)
}

fn hello() -> Hello {
    Hello {
        protocol_major: remote_server::PROTOCOL_MAJOR,
        protocol_minor: remote_server::PROTOCOL_MINOR,
        app_version: crate::VERSION.to_owned(),
        capabilities: Vec::new(),
    }
}

async fn timed<T>(what: &str, work: impl std::future::Future<Output = T>) -> Result<T, ClientError> {
    tokio::time::timeout(STEP_TIMEOUT, work)
        .await
        .map_err(|_| ClientError::Closed(format!("timed out {what}")))
}

/// Pairs with the local credential. `Ok` when the server said `Accepted`.
async fn pair(endpoint: &Endpoint, addr: &EndpointAddr, secret: [u8; 32]) -> Result<(), ClientError> {
    pair_within(endpoint, addr, secret, STEP_TIMEOUT).await
}

/// [`pair`] with a longer wait for the answer: a ticket is confirmed by a person in the terminal that
/// ran `smetanad pair`, and that takes as long as it takes (the ticket itself lasts two minutes).
async fn pair_within(
    endpoint: &Endpoint,
    addr: &EndpointAddr,
    secret: [u8; 32],
    answer_within: Duration,
) -> Result<(), ClientError> {
    let conn = timed("connecting to pair", endpoint.connect(addr.clone(), ALPN))
        .await?
        .map_err(|err| ClientError::Closed(err.to_string()))?;
    let (mut send, mut recv) = conn.open_bi().await.map_err(setup)?;
    let request = PairRequest {
        secret,
        device_name: DEVICE_NAME.to_owned(),
        platform: Platform::Desktop,
        push_token: None,
        push_key: [0; 32],
        app_version: crate::VERSION.to_owned(),
    };
    write_frame(&mut send, &request).await.map_err(setup)?;
    let reply = tokio::time::timeout(answer_within, read_frame::<PairReply>(&mut recv, MAX_FRAME))
        .await
        .map_err(|_| ClientError::Closed("timed out waiting for the pairing answer".to_owned()))?
        .map_err(setup)?;
    conn.close(0u32.into(), b"done");
    match reply {
        PairReply::Accepted => Ok(()),
        PairReply::Rejected { reason } => Err(ClientError::Refused(reason)),
    }
}

/// Says `Hello` on a new connection and returns the open stream with the server's answer; `Err` with
/// the close code when the server closed instead.
async fn say_hello(
    endpoint: &Endpoint,
    addr: &EndpointAddr,
) -> Result<(Connection, SendStream, RecvStream, HelloReply), Result<u32, ClientError>> {
    let conn = timed("connecting", endpoint.connect(addr.clone(), ALPN))
        .await
        .map_err(Err)?
        .map_err(|err| Err(ClientError::Closed(err.to_string())))?;
    let opened = async {
        let (mut send, mut recv) = conn.open_bi().await.map_err(|err| (close_code(&err), err.to_string()))?;
        let frame = encode_hello(&hello(), &HelloInfo { platform: Platform::Desktop }).map_err(|e| (None, e.to_string()))?;
        send.write_all(&frame).await.map_err(|err| (None, err.to_string()))?;
        let reply = read_frame::<HelloReply>(&mut recv, MAX_FRAME).await.map_err(|e| (None, e.to_string()))?;
        Ok::<_, (Option<u32>, String)>((send, recv, reply))
    };
    match timed("waiting for the server's hello", opened).await {
        Ok(Ok((send, recv, reply))) => Ok((conn, send, recv, reply)),
        Ok(Err((code, why))) => {
            // A close that arrives while the frame is being read is what tells a known key from an
            // unknown one; the stream error alone would not carry it.
            let code = match code {
                Some(code) => Some(code),
                None => tokio::time::timeout(Duration::from_secs(2), conn.closed()).await.ok().and_then(|err| close_code(&err)),
            };
            match code {
                Some(code) => Err(Ok(code)),
                None => Err(Err(ClientError::Closed(why))),
            }
        }
        Err(err) => Err(Err(err)),
    }
}

/// A server this machine is paired with that is not on it: where to find it, and nothing else. The
/// secret of the ticket is spent at pairing and kept nowhere.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RemoteRecord {
    /// The server's public key.
    pub node: String,
    /// The relay the ticket named, which is how a client finds a server it has no address for.
    pub relay: String,
}

/// How long a person has to confirm a pairing in the terminal that ran `smetanad pair`.
pub const CONFIRM_WITHIN: Duration = Duration::from_secs(150);

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Binds the endpoint a client dials a remote server from: the relay the ticket named, every
/// interface, so a direct path can be found when there is one.
async fn bind_remote(key: &iroh::SecretKey, relay: &iroh::RelayUrl) -> Result<Endpoint, ClientError> {
    Endpoint::builder(presets::Minimal)
        .secret_key(key.clone())
        .relay_mode(RelayMode::custom([relay.clone()]))
        .bind()
        .await
        .map_err(setup)
}

fn remote_address(record: &RemoteRecord) -> Result<(EndpointAddr, iroh::RelayUrl), ClientError> {
    let relay: iroh::RelayUrl =
        record.relay.parse().map_err(|_| ClientError::Setup(format!("the relay {:?} is not usable", record.relay)))?;
    let node: iroh::PublicKey =
        record.node.parse().map_err(|_| ClientError::Setup("the server's key is not readable".to_owned()))?;
    Ok((EndpointAddr::new(node).with_relay_url(relay.clone()), relay))
}

/// What is wrong with a ticket before anything is dialled, in words for the person who pasted it.
pub fn check_ticket(text: &str) -> Result<(remote_protocol::PairingTicket, RemoteRecord), String> {
    let (ticket, kind) = remote_protocol::PairingTicket::decode_any(text).map_err(|err| err.to_string())?;
    if kind != remote_protocol::DeviceKind::Desktop {
        return Err("This is a code for a phone. Run `smetanad pair --kind desktop` on the server for this app.".to_owned());
    }
    if ticket.expires_at <= now_secs() {
        return Err("This code has expired. Run `smetanad pair --kind desktop` again for a new one.".to_owned());
    }
    ticket.relay.parse::<iroh::RelayUrl>().map_err(|_| "The code names a relay that is not usable.".to_owned())?;
    let record = RemoteRecord { node: ticket.desktop.to_string(), relay: ticket.relay.clone() };
    Ok((ticket, record))
}

impl Client {
    /// Pairs this machine with the server a desktop ticket names, and answers where it is. The
    /// confirmation is the person's, at the terminal that issued the ticket, so this waits for it.
    pub async fn pair_remote(identity: &Path, ticket_text: &str) -> Result<RemoteRecord, ClientError> {
        let (ticket, record) = check_ticket(ticket_text).map_err(ClientError::Setup)?;
        let key = remote_server::endpoint::load_or_create_key(&identity.join(KEY_FILE)).map_err(ClientError::Setup)?;
        let (addr, relay) = remote_address(&record)?;
        let endpoint = bind_remote(&key, &relay).await?;
        let outcome = pair_within(&endpoint, &addr, ticket.secret, CONFIRM_WITHIN).await;
        endpoint.close().await;
        outcome.map(|()| record)
    }

    /// Connects to a server this machine has been paired with by [`Client::pair_remote`].
    pub async fn connect_remote(identity: &Path, record: &RemoteRecord) -> Result<Client, ClientError> {
        let key = remote_server::endpoint::load_or_create_key(&identity.join(KEY_FILE)).map_err(ClientError::Setup)?;
        let (addr, relay) = remote_address(record)?;
        let endpoint = bind_remote(&key, &relay).await?;
        match say_hello(&endpoint, &addr).await {
            Ok(opened) => Ok(Self::from_open(opened, endpoint)),
            Err(Ok(code)) => Err(ClientError::Code(code)),
            Err(Err(err)) => Err(err),
        }
    }

    /// Connects to the server whose data directory is `dir`, pairing first if this machine's client
    /// has not been paired with it.
    pub async fn connect(dir: &Path) -> Result<Client, ClientError> {
        Self::connect_as(dir, dir).await
    }

    /// [`Client::connect`] with this client's own key and paired marker kept in `identity` rather
    /// than in the server's directory: a second device on the same machine, which is a second key.
    pub async fn connect_as(dir: &Path, identity: &Path) -> Result<Client, ClientError> {
        let credential = local::read(dir).map_err(ClientError::Setup)?;
        let key = remote_server::endpoint::load_or_create_key(&identity.join(KEY_FILE)).map_err(ClientError::Setup)?;
        let endpoint = bind_client(&key).await?;
        let addr = address(&credential);
        let marker = identity.join(PAIRED_FILE);
        let node = credential.node.to_string();
        let was_paired = std::fs::read_to_string(&marker).is_ok_and(|text| text.trim() == node);
        if !was_paired {
            pair(&endpoint, &addr, credential.secret).await?;
            std::fs::write(&marker, &node).map_err(setup)?;
        }
        let opened = match say_hello(&endpoint, &addr).await {
            Ok(opened) => opened,
            // Paired by an earlier run and no longer known: the server's data was reset, or this
            // device was revoked. The credential is still good, so pair again.
            Err(Ok(code)) if was_paired && (code == CLOSE_UNKNOWN || code == CLOSE_PROTOCOL) => {
                pair(&endpoint, &addr, credential.secret).await?;
                say_hello(&endpoint, &addr).await.map_err(|err| match err {
                    Ok(code) => ClientError::Code(code),
                    Err(err) => err,
                })?
            }
            Err(Ok(code)) => return Err(ClientError::Code(code)),
            Err(Err(err)) => return Err(err),
        };
        Ok(Self::from_open(opened, endpoint))
    }

    /// Says `Hello` as `endpoint`'s key and returns the open bridge. For a device that is already
    /// paired by whatever means — a test's phone, say; `connect` is the one that pairs.
    pub async fn open(endpoint: &Endpoint, addr: &EndpointAddr) -> Result<Client, ClientError> {
        match say_hello(endpoint, addr).await {
            Ok(opened) => Ok(Self::from_open(opened, endpoint.clone())),
            Err(Ok(code)) => Err(ClientError::Code(code)),
            Err(Err(err)) => Err(err),
        }
    }

    fn from_open(opened: (Connection, SendStream, RecvStream, HelloReply), endpoint: Endpoint) -> Client {
        let (conn, send, recv, hello) = opened;
        let pending: Pending = Arc::default();
        let (events_tx, events) = mpsc::unbounded_channel();
        tokio::spawn(read_loop(recv, pending.clone(), events_tx));
        Client {
            conn,
            send: tokio::sync::Mutex::new(send),
            pending,
            next: AtomicU64::new(1),
            events: Some(events),
            hello,
            _endpoint: endpoint,
        }
    }

    /// Calls a command. `Err` is the bridge's refusal or the handler's own error, in the form the
    /// server gave it.
    pub async fn call(&self, cmd: &str, args: Value) -> Result<Value, BridgeError> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().unwrap_or_else(|p| p.into_inner()).insert(id, tx);
        let sent = write_frame(&mut *self.send.lock().await, &ClientMsg::Invoke { id, cmd: cmd.to_owned(), args }).await;
        if let Err(err) = sent {
            self.pending.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
            return Err(BridgeError::Internal(format!("{SEND_FAILED}: {err}")));
        }
        rx.await.unwrap_or_else(|_| Err(BridgeError::Internal(CLOSED_BEFORE_REPLY.into())))
    }

    /// Starts receiving an event.
    pub async fn listen(&self, event: &str) -> Result<(), ClientError> {
        write_frame(&mut *self.send.lock().await, &ClientMsg::Listen { event: event.to_owned() })
            .await
            .map_err(setup)
    }

    /// The next event, or `None` when the connection has ended.
    pub async fn next_event(&mut self) -> Option<(String, Value)> {
        self.events.as_mut()?.recv().await
    }

    /// Hands the event receiver to the caller, so one task can follow the events while others call
    /// commands on the same shared client (`call` and `listen` take `&self`). `None` the second time.
    pub fn take_events(&mut self) -> Option<mpsc::UnboundedReceiver<(String, Value)>> {
        self.events.take()
    }

    /// The next event within `within`.
    pub async fn event_within(&mut self, within: Duration) -> Option<(String, Value)> {
        tokio::time::timeout(within, self.events.as_mut()?.recv()).await.ok().flatten()
    }

    /// Resolves with the close code when the server ends this connection.
    pub async fn closed(&self) -> Option<u32> {
        close_code(&self.conn.closed().await)
    }

    /// This client's own key, as the server's device list shows it.
    pub fn id(&self) -> String {
        self._endpoint.id().to_string()
    }

    pub fn close(&self) {
        self.conn.close(0u32.into(), b"done");
    }
}

async fn read_loop(
    mut recv: RecvStream,
    pending: Pending,
    events: mpsc::UnboundedSender<(String, Value)>,
) {
    while let Ok(message) = read_frame::<ServerMsg>(&mut recv, MAX_FRAME).await {
        match message {
            ServerMsg::Reply { id, result } => {
                let waiting = pending.lock().unwrap_or_else(|p| p.into_inner()).remove(&id);
                if let Some(waiting) = waiting {
                    let _ = waiting.send(result);
                }
            }
            ServerMsg::Event { name, payload } => {
                let _ = events.send((name, payload));
            }
        }
    }
    // Whoever is still waiting is told by its sender being dropped.
    pending.lock().unwrap_or_else(|p| p.into_inner()).clear();
}

#[cfg(test)]
mod remote_tests {
    use super::*;
    use remote_protocol::{DeviceKind, PairingTicket};

    fn ticket(expires_at: u64) -> PairingTicket {
        PairingTicket {
            desktop: iroh::SecretKey::generate().public(),
            relay: "https://relay.example.com".to_owned(),
            secret: [7; 32],
            expires_at,
        }
    }

    #[test]
    fn a_desktop_ticket_that_has_not_expired_names_the_server_to_dial() {
        let ticket = ticket(now_secs() + 60);
        let (read, record) = check_ticket(&ticket.encode_for(DeviceKind::Desktop)).unwrap();
        assert_eq!(read, ticket);
        assert_eq!(record.node, ticket.desktop.to_string());
        assert_eq!(record.relay, "https://relay.example.com");
        assert!(remote_address(&record).is_ok());
    }

    #[test]
    fn a_phones_ticket_is_refused_with_the_command_that_makes_the_right_one() {
        let err = check_ticket(&ticket(now_secs() + 60).encode_for(DeviceKind::Phone)).unwrap_err();
        assert!(err.contains("--kind desktop"), "{err}");
    }

    #[test]
    fn an_expired_ticket_and_a_garbled_one_are_told_apart() {
        let expired = check_ticket(&ticket(now_secs().saturating_sub(5)).encode_for(DeviceKind::Desktop)).unwrap_err();
        assert!(expired.contains("expired"), "{expired}");
        let garbled = check_ticket("smetana2:not-base32!").unwrap_err();
        assert!(!garbled.contains("expired"), "{garbled}");
        assert!(check_ticket("hello").is_err());
    }

    #[test]
    fn a_ticket_naming_an_unusable_relay_is_refused_before_anything_is_dialled() {
        let mut bad = ticket(now_secs() + 60);
        bad.relay = "not a url".to_owned();
        assert!(check_ticket(&bad.encode_for(DeviceKind::Desktop)).unwrap_err().contains("relay"));
    }
}
