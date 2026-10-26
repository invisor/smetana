//! The iroh endpoint: binding it, accepting on it, and deciding what each
//! connection may do.
//!
//! A connection falls into one of three cases, decided by the phone's key and
//! nothing the phone has said:
//!
//! - **A key on the device list** gets the handshake: `Hello` in, `HelloReply`
//!   out, and the same stream then carries the command bridge (`bridge.rs`).
//! - **A key the list does not know, while a pairing code is outstanding**, gets
//!   to send exactly one `PairRequest`. This is the only case in which anything
//!   is read from an unknown key, and it is bounded: a small frame, one request,
//!   a timeout, and a secret that has to match.
//! - **Any other key** is closed before a byte of it is read.

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::{presets, Connection};
use iroh::{Endpoint, RelayMode, RelayUrl, SecretKey};
use remote_protocol::{
    decode_hello, read_frame, read_frame_bytes, write_frame, DeviceKind, HelloReply, PairReply,
    PairRequest, PublicKey, ALPN, CLOSE_DONE, CLOSE_PROTOCOL, CLOSE_REVOKED, CLOSE_UNKNOWN,
    CLOSE_VERSION,
};

#[cfg(not(unix))]
use super::devices::write_atomic;
use super::devices::Device;
use super::pairing::Spent;
use super::{Event, Remote, MIN_MOBILE_VERSION, PROTOCOL_MAJOR, PROTOCOL_MINOR, RECOMMENDED_MOBILE_VERSION};

pub const KEY_FILE: &str = "remote-key";

/// How many connections from keys the list does not know are served at once,
/// while a pairing code is out. Pairing needs one phone; the rest is margin for a
/// retry, and a cap keeps a stranger who has the endpoint's address from holding
/// sockets open for the length of the first-frame timeout.
pub const MAX_PAIRING_CONNECTIONS: usize = 4;

/// How long a phone gets to open its stream and send its first frame.
const FIRST_FRAME_TIMEOUT: Duration = Duration::from_secs(15);
/// How long a closing side waits for the peer to take what was just written.
const LINGER: Duration = Duration::from_secs(2);
/// The largest `PairRequest` or `Hello` read. Both are a few hundred bytes.
const FIRST_FRAME_LIMIT: usize = 16 * 1024;

const NAME_LIMIT: usize = 64;
pub(crate) const TOKEN_LIMIT: usize = 4096;

/// The device key, read from `path` or made and written there. Created at first
/// launch and never rotated: it is the desktop's identity, and every ticket and
/// every paired phone is bound to it. Lower-case hex, readable only by the user
/// on platforms that have the notion.
pub fn load_or_create_key(path: &Path) -> Result<SecretKey, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let bytes = data_encoding::HEXLOWER
                .decode(text.trim().as_bytes())
                .map_err(|err| format!("{}: {err}", path.display()))?;
            let bytes: [u8; 32] = bytes
                .try_into()
                .map_err(|_| format!("{}: not a 32 byte key", path.display()))?;
            Ok(SecretKey::from_bytes(&bytes))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let key = SecretKey::generate();
            let text = data_encoding::HEXLOWER.encode(&key.to_bytes());
            write_private(path, text.as_bytes())?;
            Ok(key)
        }
        Err(err) => Err(format!("{}: {err}", path.display())),
    }
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    // Created with the mode it will keep, not narrowed afterwards: there is no
    // moment at which the key exists and anybody else can read it. `create_new`
    // so a racing second launch cannot replace a key the first has already used.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|err| format!("{}: {err}", path.display()))?;
    file.write_all(bytes).map_err(|err| err.to_string())?;
    file.sync_all().map_err(|err| err.to_string())
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_atomic(path, bytes)
}

/// Binds the endpoint with the given relay. No address lookup service is
/// configured: the phone is told where to go by the ticket, and nothing about
/// this desktop is published anywhere.
pub async fn bind(key: &SecretKey, relay: &str) -> Result<Endpoint, String> {
    let url: RelayUrl = relay.parse().map_err(|err| format!("relay URL {relay:?}: {err}"))?;
    Endpoint::builder(presets::Minimal)
        .secret_key(key.clone())
        .alpns(vec![ALPN.to_vec()])
        .relay_mode(RelayMode::custom([url]))
        .bind()
        .await
        .map_err(|err| format!("could not start the endpoint: {err}"))
}

/// Binds the endpoint a client on this machine dials: loopback only, no relay, an address the
/// system picks. The same key as the public endpoint and the same protocol; what differs is who can
/// reach it, which is why the pairing a client does here may be the local credential's.
pub async fn bind_loopback(key: &SecretKey) -> Result<Endpoint, String> {
    Endpoint::builder(presets::Minimal)
        .secret_key(key.clone())
        .alpns(vec![ALPN.to_vec()])
        .relay_mode(RelayMode::Disabled)
        .clear_ip_transports()
        .bind_addr("127.0.0.1:0")
        .map_err(|err| format!("could not bind the loopback address: {err}"))?
        .bind()
        .await
        .map_err(|err| format!("could not start the loopback endpoint: {err}"))
}

/// The address of a loopback endpoint once it has one, which is a moment after it is bound.
pub async fn loopback_addr(endpoint: &Endpoint) -> Result<iroh::EndpointAddr, String> {
    for _ in 0..200 {
        let addr = endpoint.addr();
        if addr.ip_addrs().next().is_some() {
            return Ok(addr);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    Err("the loopback endpoint has no address".to_owned())
}

/// Open connections by phone, so revoking one can close them.
#[derive(Default)]
pub struct Connections {
    map: Mutex<HashMap<String, Vec<(u64, Connection)>>>,
    next: AtomicU64,
}

impl Connections {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<(u64, Connection)>>> {
        self.map.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn register(&self, key: &PublicKey, conn: Connection) -> u64 {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        self.lock().entry(key.to_string()).or_default().push((id, conn));
        id
    }

    fn unregister(&self, key: &PublicKey, id: u64) {
        let mut map = self.lock();
        if let Some(list) = map.get_mut(&key.to_string()) {
            list.retain(|(held, _)| *held != id);
            if list.is_empty() {
                map.remove(&key.to_string());
            }
        }
    }

    pub fn close_all(&self, key: &PublicKey, code: u32, reason: &[u8]) {
        let taken = self.lock().remove(&key.to_string()).unwrap_or_default();
        for (_, conn) in taken {
            conn.close(code.into(), reason);
        }
    }

    #[cfg(test)]
    pub fn count(&self, key: &PublicKey) -> usize {
        self.lock().get(&key.to_string()).map_or(0, Vec::len)
    }
}

pub async fn accept_loop(remote: Arc<Remote>, endpoint: Endpoint) {
    while let Some(incoming) = endpoint.accept().await {
        let remote = remote.clone();
        tokio::spawn(async move {
            let Ok(conn) = incoming.await else { return };
            handle(remote, conn).await;
        });
    }
}

async fn handle(remote: Arc<Remote>, conn: Connection) {
    let key = conn.remote_id();
    if remote.devices.contains(&key) {
        serve_paired(&remote, conn, key).await;
    } else if remote.pairing.accepts_requests() {
        // Held for the whole of the exchange, the wait for the person included.
        match remote.pairing_slots.clone().try_acquire_owned() {
            Ok(_permit) => serve_pairing(&remote, conn, key).await,
            Err(_) => conn.close(CLOSE_UNKNOWN.into(), b"busy"),
        }
    } else {
        conn.close(CLOSE_UNKNOWN.into(), b"unknown device");
    }
}

async fn serve_paired(remote: &Arc<Remote>, conn: Connection, key: PublicKey) {
    let id = remote.connections.register(&key, conn.clone());
    // The list is checked again once the connection is registered: a revoke that
    // landed between the first check and the registration found nothing to close.
    if let Some(kind) = remote.devices.kind_of(&key) {
        if let Some((send, recv)) = handshake(remote, &conn, &key, kind).await {
            super::bridge::serve(remote, &conn, &key, kind, send, recv).await;
        }
        conn.closed().await;
    } else {
        conn.close(CLOSE_REVOKED.into(), b"revoked");
    }
    remote.connections.unregister(&key, id);
}

/// The handshake. `Some` is the phone's stream, left open for the bridge.
async fn handshake(
    remote: &Remote,
    conn: &Connection,
    key: &PublicKey,
    kind: DeviceKind,
) -> Option<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream)> {
    let Some((mut send, recv, (hello, info))) = first_hello(conn).await else {
        conn.close(CLOSE_PROTOCOL.into(), b"expected a hello");
        return None;
    };
    // What the device says about itself is a note for the log. What it may do is the kind the
    // list holds for its key, set when it was paired.
    log::debug!("remote: hello from a {kind:?} device, which calls itself {:?}", info.map(|info| info.platform));
    let reply = HelloReply {
        protocol_major: PROTOCOL_MAJOR,
        protocol_minor: PROTOCOL_MINOR,
        app_version: remote.app_version.clone(),
        capabilities: remote_protocol::capabilities_for(kind),
        min_mobile_version: MIN_MOBILE_VERSION.to_owned(),
        recommended_mobile_version: RECOMMENDED_MOBILE_VERSION.to_owned(),
    };
    if write_frame(&mut send, &reply).await.is_err() {
        return None;
    }
    if hello.protocol_major != PROTOCOL_MAJOR {
        // The reply above is what tells the phone why; closing is what keeps it
        // from going on to speak a protocol this desktop no longer reads.
        let _ = send.finish();
        let _ = tokio::time::timeout(LINGER, conn.closed()).await;
        conn.close(CLOSE_VERSION.into(), b"unsupported protocol major");
        return None;
    }
    remote.devices.touch(key, remote.pairing.now());
    Some((send, recv))
}

/// [`first_frame`] for a `Hello`, which may carry a [`remote_protocol::HelloInfo`] behind it.
async fn first_hello(
    conn: &Connection,
) -> Option<(
    iroh::endpoint::SendStream,
    iroh::endpoint::RecvStream,
    (remote_protocol::Hello, Option<remote_protocol::HelloInfo>),
)> {
    let read = async {
        let (send, mut recv) = conn.accept_bi().await.ok()?;
        let body = read_frame_bytes(&mut recv, FIRST_FRAME_LIMIT).await.ok()?;
        Some((send, recv, decode_hello(&body).ok()?))
    };
    tokio::time::timeout(FIRST_FRAME_TIMEOUT, read).await.ok().flatten()
}

/// Opens the phone's first stream and reads the one frame it must begin with.
/// Both halves of the stream come back: on a paired connection it goes on to
/// carry the bridge.
async fn first_frame<T: serde::de::DeserializeOwned>(
    conn: &Connection,
) -> Option<(iroh::endpoint::SendStream, iroh::endpoint::RecvStream, T)> {
    let read = async {
        let (send, mut recv) = conn.accept_bi().await.ok()?;
        let frame = read_frame::<T>(&mut recv, FIRST_FRAME_LIMIT).await.ok()?;
        Some((send, recv, frame))
    };
    tokio::time::timeout(FIRST_FRAME_TIMEOUT, read).await.ok().flatten()
}

async fn serve_pairing(remote: &Arc<Remote>, conn: Connection, key: PublicKey) {
    let Some((mut send, _recv, request)) = first_frame::<PairRequest>(&conn).await else {
        conn.close(CLOSE_PROTOCOL.into(), b"expected a pair request");
        return;
    };
    let reply = decide_pairing(remote, &key, request).await;
    let delivered = conn.close_reason().is_none() && write_frame(&mut send, &reply).await.is_ok();
    if delivered {
        let _ = send.finish();
        let _ = tokio::time::timeout(LINGER, conn.closed()).await;
    } else if reply == PairReply::Accepted {
        // The phone never heard it. A device on the list that believes it was
        // refused could only be removed by hand, and a phone that tries again
        // would be told its key is already paired, so the entry goes.
        match remote.devices.remove(&key) {
            Ok(_) => remote.emit(Event::DevicesChanged(remote.devices.all())),
            Err(err) => log::warn!("remote: could not undo an undelivered pairing: {err}"),
        }
    }
    conn.close(CLOSE_DONE.into(), b"done");
}

/// The whole of the pairing decision for one request that has been read.
pub(crate) async fn decide_pairing(remote: &Remote, key: &PublicKey, request: PairRequest) -> PairReply {
    let rejected = |reason: &str| PairReply::Rejected { reason: reason.to_owned() };
    let spent = match remote.pairing.spend_any(&request.secret) {
        Ok(spent) => spent,
        Err(reason) => return rejected(reason),
    };
    if request.push_token.as_ref().is_some_and(|token| token.len() > TOKEN_LIMIT) {
        return rejected("the push token is too long");
    }
    let name = clean_name(&request.device_name);
    let kind = match spent {
        Spent::Ticket { remaining, kind } => {
            let waiting = match remote.pairing.wait_for_decision() {
                Ok(waiting) => waiting,
                Err(reason) => return rejected(reason),
            };
            let id = waiting.id;
            remote.emit(Event::PairRequest { device_name: name.clone(), platform: request.platform });
            let accepted = waiting.answer(remaining).await;
            remote.pairing.finish_waiting(id);
            // However it ended, the prompt on the screen is stale now.
            remote.emit(Event::PairEnded);
            if !accepted {
                return rejected("the desktop did not confirm the pairing");
            }
            kind
        }
        // The credential is in a file only this server's user reads: there is no one to ask.
        Spent::Local => DeviceKind::Desktop,
    };
    let device = Device {
        key: key.to_string(),
        name,
        platform: request.platform,
        push_token: request.push_token,
        push_key: data_encoding::HEXLOWER.encode(&request.push_key),
        paired_at: remote.pairing.now(),
        last_contact: None,
        // The ticket's, not the request's: `platform` above is what the device says it is.
        kind,
        local: matches!(spent, Spent::Local),
    };
    if let Err(err) = remote.devices.add(device) {
        log::warn!("remote: could not save a paired device: {err}");
        return rejected("the desktop could not save the pairing");
    }
    remote.emit(Event::DevicesChanged(remote.devices.all()));
    PairReply::Accepted
}

/// A name a phone chose, made safe to put in a dialog: no control characters,
/// no more than `NAME_LIMIT` characters, never empty.
fn clean_name(raw: &str) -> String {
    let cleaned: String = raw.chars().filter(|c| !c.is_control()).take(NAME_LIMIT).collect();
    let cleaned = cleaned.trim();
    if cleaned.is_empty() {
        "Unnamed phone".to_owned()
    } else {
        cleaned.to_owned()
    }
}

#[cfg(test)]
mod name_tests {
    use super::*;

    #[test]
    fn remote_device_names_are_made_safe_to_show() {
        assert_eq!(clean_name("  Pixel 8 \n"), "Pixel 8");
        assert_eq!(clean_name("\u{0}\u{7}"), "Unnamed phone");
        assert_eq!(clean_name(&"x".repeat(500)).chars().count(), NAME_LIMIT);
    }

    #[test]
    fn remote_key_is_created_once_and_read_back() {
        let dir = std::env::temp_dir().join(format!("smetana-key-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join(KEY_FILE);
        let first = load_or_create_key(&path).unwrap();
        let second = load_or_create_key(&path).unwrap();
        assert_eq!(first.public(), second.public());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "nobody else may read the key");
        }
        std::fs::write(&path, "nonsense").unwrap();
        assert!(load_or_create_key(&path).is_err(), "a damaged key is not silently replaced");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
