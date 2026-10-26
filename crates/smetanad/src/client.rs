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
}

type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, BridgeError>>>>>;

pub struct Client {
    conn: Connection,
    send: tokio::sync::Mutex<SendStream>,
    pending: Pending,
    next: AtomicU64,
    events: mpsc::UnboundedReceiver<(String, Value)>,
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
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
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
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
    };
    write_frame(&mut send, &request).await.map_err(setup)?;
    let reply = timed("waiting for the pairing answer", read_frame::<PairReply>(&mut recv, MAX_FRAME))
        .await?
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

impl Client {
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
                    Ok(code) => ClientError::Closed(format!("closed with code {code}")),
                    Err(err) => err,
                })?
            }
            Err(Ok(code)) => return Err(ClientError::Closed(format!("closed with code {code}"))),
            Err(Err(err)) => return Err(err),
        };
        Ok(Self::from_open(opened, endpoint))
    }

    /// Says `Hello` as `endpoint`'s key and returns the open bridge. For a device that is already
    /// paired by whatever means — a test's phone, say; `connect` is the one that pairs.
    pub async fn open(endpoint: &Endpoint, addr: &EndpointAddr) -> Result<Client, ClientError> {
        match say_hello(endpoint, addr).await {
            Ok(opened) => Ok(Self::from_open(opened, endpoint.clone())),
            Err(Ok(code)) => Err(ClientError::Closed(format!("closed with code {code}"))),
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
            events,
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
            return Err(BridgeError::Internal(format!("the connection is gone: {err}")));
        }
        rx.await.unwrap_or_else(|_| Err(BridgeError::Internal("the connection closed before the reply".into())))
    }

    /// Starts receiving an event.
    pub async fn listen(&self, event: &str) -> Result<(), ClientError> {
        write_frame(&mut *self.send.lock().await, &ClientMsg::Listen { event: event.to_owned() })
            .await
            .map_err(setup)
    }

    /// The next event, or `None` when the connection has ended.
    pub async fn next_event(&mut self) -> Option<(String, Value)> {
        self.events.recv().await
    }

    /// The next event within `within`.
    pub async fn event_within(&mut self, within: Duration) -> Option<(String, Value)> {
        tokio::time::timeout(within, self.events.recv()).await.ok().flatten()
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
