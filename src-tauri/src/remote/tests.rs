//! The companion end to end: two iroh endpoints in one process, no relay, the
//! phone dialling the desktop's loopback address.
//!
//! What is checked here is the policy — who is let in, what is stored, what is
//! closed — and not iroh. The pieces that need no network (the clock, the list
//! on disk, the ticket) are tested beside their modules.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::{presets, Connection, ConnectionError};
use iroh::{Endpoint, EndpointAddr, RelayMode, SecretKey};
use remote_protocol::{
    read_frame, write_frame, Hello, HelloReply, PairReply, PairRequest, Platform, ALPN, MAX_FRAME,
};

use super::devices::{Device, Devices};
use super::endpoint::{decide_pairing, CLOSE_REVOKED, CLOSE_UNKNOWN, CLOSE_VERSION};
use super::pairing::{Pairing, SECRET_TTL_SECS};
use super::{Event, Remote, PROTOCOL_MAJOR};

const START: u64 = 1_000_000;

struct Harness {
    remote: Arc<Remote>,
    addr: EndpointAddr,
    events: Arc<Mutex<Vec<Event>>>,
    now: Arc<AtomicU64>,
    dir: std::path::PathBuf,
    _endpoint: Endpoint,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

async fn bind_local(key: &SecretKey, alpns: Vec<Vec<u8>>) -> Endpoint {
    Endpoint::builder(presets::Minimal)
        .secret_key(key.clone())
        .alpns(alpns)
        .relay_mode(RelayMode::Disabled)
        .clear_ip_transports()
        .bind_addr("127.0.0.1:0")
        .unwrap()
        .bind()
        .await
        .unwrap()
}

async fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..500 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

async fn harness() -> Harness {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir()
        .join(format!("smetana-remote-{}-{}", std::process::id(), N.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir_all(&dir).unwrap();
    let now = Arc::new(AtomicU64::new(START));
    let clock = now.clone();
    let events = Arc::new(Mutex::new(Vec::new()));
    let sink = events.clone();
    let key = SecretKey::generate();
    let remote = Remote::new(
        key.clone(),
        Devices::load(dir.join(super::devices::FILE_NAME)),
        Pairing::new(Arc::new(move || clock.load(Ordering::SeqCst))),
        "9.9.9".into(),
        Arc::new(move |event| sink.lock().unwrap().push(event)),
        Box::new(|| "loopback".into()),
    );
    let endpoint = bind_local(&key, vec![ALPN.to_vec()]).await;
    remote.attach(endpoint.clone(), "loopback".into()).await;
    let mut addr = endpoint.addr();
    for _ in 0..200 {
        if addr.ip_addrs().next().is_some() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
        addr = endpoint.addr();
    }
    assert!(addr.ip_addrs().next().is_some(), "the endpoint has no loopback address");
    Harness { remote, addr, events, now, dir, _endpoint: endpoint }
}

struct Phone {
    key: SecretKey,
    endpoint: Endpoint,
}

async fn phone() -> Phone {
    let key = SecretKey::generate();
    let endpoint = bind_local(&key, Vec::new()).await;
    Phone { key, endpoint }
}

impl Phone {
    async fn connect(&self, h: &Harness) -> Connection {
        self.endpoint.connect(h.addr.clone(), ALPN).await.unwrap()
    }
}

fn request(secret: [u8; 32]) -> PairRequest {
    PairRequest {
        secret,
        device_name: "Pixel 8".into(),
        platform: Platform::Android,
        push_token: Some("token".into()),
        push_key: [5; 32],
        app_version: "1.0.0".into(),
    }
}

fn hello(major: u16) -> Hello {
    Hello { protocol_major: major, protocol_minor: 0, app_version: "1.0.0".into(), capabilities: vec![] }
}

fn code(err: ConnectionError) -> Option<u64> {
    match err {
        ConnectionError::ApplicationClosed(close) => Some(close.error_code.into_inner()),
        _ => None,
    }
}

async fn pair(h: &Harness, phone: &Phone, secret: [u8; 32]) -> (Connection, PairReply) {
    let conn = phone.connect(h).await;
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    write_frame(&mut send, &request(secret)).await.unwrap();
    let reply = read_frame::<PairReply>(&mut recv, MAX_FRAME).await.unwrap();
    (conn, reply)
}

/// Pairs a phone by the real route and returns when it is on the list.
async fn pair_accepted(h: &Harness, phone: &Phone) {
    let ticket = h.remote.pairing.begin(h.remote.public_key(), "loopback");
    let answer = async {
        wait_until("the pair request", || {
            h.events.lock().unwrap().iter().any(|e| matches!(e, Event::PairRequest { .. }))
        })
        .await;
        assert!(h.remote.pairing.decide(true));
    };
    let ((conn, reply), ()) = tokio::join!(pair(h, phone, ticket.secret), answer);
    assert_eq!(reply, PairReply::Accepted);
    conn.close(0u32.into(), b"done");
}

#[tokio::test]
async fn remote_pairing_accepted_by_the_person_saves_the_device() {
    let h = harness().await;
    let phone = phone().await;
    pair_accepted(&h, &phone).await;

    assert!(h.remote.devices.contains(&phone.key.public()));
    let device = &h.remote.devices.all()[0];
    assert_eq!(device.name, "Pixel 8");
    assert_eq!(device.platform, Platform::Android);
    assert_eq!(device.push_token.as_deref(), Some("token"));
    assert_eq!(device.push_key, "05".repeat(32));
    assert_eq!(device.paired_at, START);

    let events = h.events.lock().unwrap();
    assert!(events.contains(&Event::PairRequest { device_name: "Pixel 8".into(), platform: Platform::Android }));
    assert!(events.iter().any(|e| matches!(e, Event::DevicesChanged(list) if list.len() == 1)));
    drop(events);

    // Spent: the same code does not pair a second phone.
    assert!(!h.remote.pairing.is_open());
}

#[tokio::test]
async fn remote_pairing_rejected_by_the_person_saves_nothing() {
    let h = harness().await;
    let phone = phone().await;
    let ticket = h.remote.pairing.begin(h.remote.public_key(), "loopback");
    let answer = async {
        wait_until("the pair request", || !h.events.lock().unwrap().is_empty()).await;
        assert!(h.remote.pairing.decide(false));
    };
    let ((conn, reply), ()) = tokio::join!(pair(&h, &phone, ticket.secret), answer);
    assert!(matches!(reply, PairReply::Rejected { .. }));
    conn.close(0u32.into(), b"done");
    assert!(h.remote.devices.is_empty());
    assert!(!h.remote.devices.contains(&phone.key.public()));
    assert!(!h.dir.join(super::devices::FILE_NAME).exists(), "nothing was written either");
}

#[tokio::test]
async fn remote_pairing_with_the_wrong_secret_is_rejected_without_asking_the_person() {
    let h = harness().await;
    let phone = phone().await;
    h.remote.pairing.begin(h.remote.public_key(), "loopback");
    let (conn, reply) = pair(&h, &phone, [0; 32]).await;
    assert!(matches!(reply, PairReply::Rejected { .. }));
    conn.close(0u32.into(), b"done");
    assert!(h.events.lock().unwrap().is_empty(), "the person was not bothered");
    assert!(h.remote.devices.is_empty());
}

#[tokio::test]
async fn remote_pairing_left_unanswered_is_rejected_when_the_secret_runs_out() {
    let h = harness().await;
    let phone = phone().await;
    let ticket = h.remote.pairing.begin(h.remote.public_key(), "loopback");
    // One second of the secret's term left when the phone presents it.
    h.now.store(START + SECRET_TTL_SECS - 1, Ordering::SeqCst);
    let (conn, reply) = pair(&h, &phone, ticket.secret).await;
    assert!(matches!(reply, PairReply::Rejected { .. }));
    conn.close(0u32.into(), b"done");
    assert!(h.remote.devices.is_empty());
}

#[tokio::test]
async fn remote_a_secret_used_once_or_expired_is_rejected() {
    let h = harness().await;
    let key = SecretKey::generate().public();
    let rejected = |reply: PairReply| matches!(reply, PairReply::Rejected { .. });

    let used = h.remote.pairing.begin(h.remote.public_key(), "loopback");
    assert!(h.remote.pairing.spend(&used.secret).is_ok());
    assert!(rejected(decide_pairing(&h.remote, &key, request(used.secret)).await), "the second use");

    let late = h.remote.pairing.begin(h.remote.public_key(), "loopback");
    h.now.store(START + SECRET_TTL_SECS + 1, Ordering::SeqCst);
    assert!(rejected(decide_pairing(&h.remote, &key, request(late.secret)).await), "past its term");
    assert!(h.remote.devices.is_empty());
}

#[tokio::test]
async fn remote_an_unpaired_key_is_closed_before_hello_is_read() {
    let h = harness().await;
    let phone = phone().await;
    let conn = phone.connect(&h).await;
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    let _ = write_frame(&mut send, &hello(PROTOCOL_MAJOR)).await;

    let closed = tokio::time::timeout(Duration::from_secs(5), conn.closed()).await.unwrap();
    assert_eq!(code(closed), Some(CLOSE_UNKNOWN as u64));
    assert!(read_frame::<HelloReply>(&mut recv, MAX_FRAME).await.is_err(), "no reply was given");
    assert!(h.remote.devices.is_empty());
}

#[tokio::test]
async fn remote_an_unknown_key_gets_one_bounded_frame_while_a_code_is_out_and_no_more() {
    let h = harness().await;
    let phone = phone().await;
    h.remote.pairing.begin(h.remote.public_key(), "loopback");
    let conn = phone.connect(&h).await;
    let (mut send, _recv) = conn.open_bi().await.unwrap();
    // A length prefix claiming four gigabytes: refused without being read.
    send.write_all(&[0xff, 0xff, 0xff, 0xff]).await.unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(5), conn.closed()).await.unwrap();
    assert!(code(closed).is_some());
    assert!(h.remote.devices.is_empty());
}

async fn handshake(conn: &Connection, major: u16) -> HelloReply {
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    write_frame(&mut send, &hello(major)).await.unwrap();
    read_frame::<HelloReply>(&mut recv, MAX_FRAME).await.unwrap()
}

#[tokio::test]
async fn remote_a_paired_phone_completes_the_handshake() {
    let h = harness().await;
    let phone = phone().await;
    pair_accepted(&h, &phone).await;

    h.now.store(START + 60, Ordering::SeqCst);
    let conn = phone.connect(&h).await;
    let reply = handshake(&conn, PROTOCOL_MAJOR).await;
    assert_eq!(reply.protocol_major, PROTOCOL_MAJOR);
    assert_eq!(reply.app_version, "9.9.9");
    assert_eq!(reply.min_mobile_version, super::MIN_MOBILE_VERSION);
    assert_eq!(reply.recommended_mobile_version, super::RECOMMENDED_MOBILE_VERSION);
    wait_until("the contact to be recorded", || h.remote.devices.all()[0].last_contact == Some(START + 60)).await;
    assert!(
        tokio::time::timeout(Duration::from_millis(300), conn.closed()).await.is_err(),
        "the connection is held open"
    );
}

#[tokio::test]
async fn remote_revoking_closes_the_open_connection_and_refuses_the_next() {
    let h = harness().await;
    let phone = phone().await;
    pair_accepted(&h, &phone).await;

    let conn = phone.connect(&h).await;
    handshake(&conn, PROTOCOL_MAJOR).await;
    assert_eq!(h.remote.connections.count(&phone.key.public()), 1);

    h.remote.revoke(&phone.key.public()).unwrap();
    let closed = tokio::time::timeout(Duration::from_secs(5), conn.closed()).await.unwrap();
    assert_eq!(code(closed), Some(CLOSE_REVOKED as u64));
    assert!(h.remote.devices.is_empty());
    assert!(
        h.events.lock().unwrap().iter().rev().any(|e| matches!(e, Event::DevicesChanged(list) if list.is_empty())),
        "the list change was announced"
    );

    let again = phone.connect(&h).await;
    let (mut send, mut recv) = again.open_bi().await.unwrap();
    let _ = write_frame(&mut send, &hello(PROTOCOL_MAJOR)).await;
    let closed = tokio::time::timeout(Duration::from_secs(5), again.closed()).await.unwrap();
    assert_eq!(code(closed), Some(CLOSE_UNKNOWN as u64));
    assert!(read_frame::<HelloReply>(&mut recv, MAX_FRAME).await.is_err());
}

#[tokio::test]
async fn remote_a_protocol_major_that_does_not_match_is_answered_and_closed() {
    let h = harness().await;
    let phone = phone().await;
    pair_accepted(&h, &phone).await;

    let conn = phone.connect(&h).await;
    let reply = handshake(&conn, PROTOCOL_MAJOR + 1).await;
    assert_eq!(reply.protocol_major, PROTOCOL_MAJOR, "the phone is told what this desktop speaks");
    let closed = tokio::time::timeout(Duration::from_secs(10), conn.closed()).await.unwrap();
    assert_eq!(code(closed), Some(CLOSE_VERSION as u64));
    assert_eq!(h.remote.devices.all()[0].last_contact, None, "a refused handshake is not a contact");
}

#[tokio::test]
async fn remote_a_device_listed_on_disk_is_served_after_a_restart() {
    let h = harness().await;
    let phone = phone().await;
    h.remote
        .devices
        .add(Device {
            key: phone.key.public().to_string(),
            name: "Old".into(),
            platform: Platform::Ios,
            push_token: None,
            push_key: "00".repeat(32),
            paired_at: 1,
            last_contact: None,
        })
        .unwrap();
    let conn = phone.connect(&h).await;
    assert_eq!(handshake(&conn, PROTOCOL_MAJOR).await.protocol_major, PROTOCOL_MAJOR);
}
