//! The core against a stand-in desktop in the same process, joined through a
//! relay the test starts (iroh's own test relay, with a self-signed certificate:
//! `Config::insecure_tls`, which exists only in test builds).
//!
//! The stand-in is built from `remote-protocol` alone — the real desktop endpoint
//! is part of the Tauri application's crate and cannot be linked here — and it
//! speaks the desktop's side of the same frames: pairing by secret, `Hello`
//! answered with a `HelloReply`, `Invoke` answered, `Listen` remembered.
//! `src-tauri/src/remote/tests/phone_bridge.rs` tests the real endpoint from the
//! other end with the same frames.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::{presets, Connection};
use iroh::{Endpoint, RelayMode, RelayUrl, SecretKey};
use remote_protocol::{
    read_frame, write_frame, BridgeError, ClientMsg, Hello, HelloReply, PairReply, PairRequest,
    PairingTicket, Platform, ServerMsg, ALPN, CLOSE_REVOKED, MAX_FRAME,
};
use serde_json::{json, Value};

use super::*;
use crate::secrets::MemoryStore;

struct Desktop {
    endpoint: Endpoint,
    key: SecretKey,
    relay: RelayUrl,
    state: Arc<DesktopState>,
    _server: Box<dyn std::any::Any + Send>,
}

#[derive(Default)]
struct DesktopState {
    paired: Mutex<Vec<String>>,
    secret: Mutex<Option<[u8; 32]>>,
    requests: Mutex<Vec<PairRequest>>,
    calls: Mutex<Vec<(String, Value)>>,
    listens: Mutex<Vec<String>>,
    tokens: Mutex<Vec<String>>,
    hellos: Mutex<Vec<Hello>>,
    min_version: Mutex<String>,
    recommended: Mutex<String>,
    minor: Mutex<u16>,
    conns: Mutex<Vec<Connection>>,
    push_events: Mutex<Vec<(String, Value)>>,
    reject_pairing: AtomicBool,
    close_pairing_with_protocol: AtomicBool,
}

impl Desktop {
    async fn start() -> Desktop {
        let (_map, relay, server) = iroh::test_utils::run_relay_server().await.unwrap();
        let key = SecretKey::generate();
        let endpoint = Endpoint::builder(presets::Minimal)
            .secret_key(key.clone())
            .alpns(vec![ALPN.to_vec()])
            .relay_mode(RelayMode::custom([relay.clone()]))
            .ca_tls_config(iroh::tls::CaTlsConfig::insecure_skip_verify())
            .bind()
            .await
            .unwrap();
        let state = Arc::new(DesktopState::default());
        *state.min_version.lock().unwrap() = "0.1.0".into();
        *state.recommended.lock().unwrap() = "0.1.0".into();
        *state.minor.lock().unwrap() = 1;
        tokio::spawn(accept(endpoint.clone(), state.clone()));
        Desktop { endpoint, key, relay, state, _server: Box::new(server) }
    }

    fn ticket(&self) -> String {
        let secret = [9u8; 32];
        *self.state.secret.lock().unwrap() = Some(secret);
        PairingTicket {
            desktop: self.key.public(),
            relay: self.relay.to_string(),
            secret,
            expires_at: now_secs() + 120,
        }
        .encode()
    }

    /// Drops every open connection the way a network failure does, without a close code
    /// the phone would read as a verdict.
    fn drop_connections(&self) {
        for conn in self.state.conns.lock().unwrap().drain(..) {
            conn.close(0u32.into(), b"test: link lost");
        }
    }

    fn close_all_with(&self, code: u32) {
        for conn in self.state.conns.lock().unwrap().drain(..) {
            conn.close(code.into(), b"test");
        }
    }
}

async fn accept(endpoint: Endpoint, state: Arc<DesktopState>) {
    while let Some(incoming) = endpoint.accept().await {
        let state = state.clone();
        tokio::spawn(async move {
            let Ok(conn) = incoming.await else { return };
            let key = conn.remote_id().to_string();
            let known = state.paired.lock().unwrap().contains(&key);
            if known {
                state.conns.lock().unwrap().push(conn.clone());
                serve_paired(conn, state).await;
            } else {
                serve_pairing(conn, state, key).await;
            }
        });
    }
}

async fn serve_pairing(conn: Connection, state: Arc<DesktopState>, key: String) {
    let Ok((mut send, mut recv)) = conn.accept_bi().await else { return };
    let Ok(request) = read_frame::<PairRequest>(&mut recv, 16 * 1024).await else { return };
    if state.close_pairing_with_protocol.load(Ordering::SeqCst) {
        conn.close(remote_protocol::CLOSE_PROTOCOL.into(), b"expected a pair request");
        return;
    }
    let right = *state.secret.lock().unwrap() == Some(request.secret);
    state.requests.lock().unwrap().push(request);
    let reply = if state.reject_pairing.load(Ordering::SeqCst) || !right {
        PairReply::Rejected { reason: "the desktop did not confirm the pairing".into() }
    } else {
        state.paired.lock().unwrap().push(key);
        PairReply::Accepted
    };
    let _ = write_frame(&mut send, &reply).await;
    let _ = send.finish();
    let _ = tokio::time::timeout(Duration::from_secs(2), conn.closed()).await;
}

async fn serve_paired(conn: Connection, state: Arc<DesktopState>) {
    let Ok((mut send, mut recv)) = conn.accept_bi().await else { return };
    let Ok(hello) = read_frame::<Hello>(&mut recv, 16 * 1024).await else { return };
    state.hellos.lock().unwrap().push(hello);
    let reply = HelloReply {
        protocol_major: 1,
        protocol_minor: *state.minor.lock().unwrap(),
        app_version: "9.9.9".into(),
        capabilities: remote_protocol::capabilities(),
        min_mobile_version: state.min_version.lock().unwrap().clone(),
        recommended_mobile_version: state.recommended.lock().unwrap().clone(),
    };
    if write_frame(&mut send, &reply).await.is_err() {
        return;
    }
    let (out, mut outgoing) = mpsc::channel::<ServerMsg>(16);
    tokio::spawn(async move {
        while let Some(msg) = outgoing.recv().await {
            if write_frame(&mut send, &msg).await.is_err() {
                break;
            }
        }
    });
    while let Ok(msg) = read_frame::<ClientMsg>(&mut recv, MAX_FRAME).await {
        match msg {
            ClientMsg::Invoke { id, cmd, args } => {
                state.calls.lock().unwrap().push((cmd.clone(), args.clone()));
                let result = match cmd.as_str() {
                    "session_attach" => Err(BridgeError::Handler(json!({ "kind": "noSuchSession", "message": 9 }))),
                    _ => Ok(json!({ "echo": cmd, "args": args })),
                };
                let _ = out.send(ServerMsg::Reply { id, result }).await;
            }
            ClientMsg::Listen { event } => {
                state.listens.lock().unwrap().push(event.clone());
                let queued: Vec<_> = state.push_events.lock().unwrap().iter().filter(|(n, _)| *n == event).cloned().collect();
                for (name, payload) in queued {
                    let _ = out.send(ServerMsg::Event { name, payload }).await;
                }
            }
            ClientMsg::Unlisten { .. } => {}
            ClientMsg::UpdatePushToken { token } => state.tokens.lock().unwrap().push(token),
        }
    }
}

struct Phone {
    core: Arc<Core>,
    store: Arc<MemoryStore>,
    out: Arc<Mutex<Vec<Out>>>,
}

fn phone_with(app_version: &str) -> Phone {
    let store = Arc::new(MemoryStore::default());
    let out: Arc<Mutex<Vec<Out>>> = Arc::default();
    let sink_out = out.clone();
    let sink: Sink = Arc::new(move |o| sink_out.lock().unwrap().push(o));
    let mut config = Config::new(app_version, Platform::Android, "Pixel 8", "https://store.example/app");
    config.insecure_tls = true;
    config.backoff_start = Duration::from_millis(50);
    config.backoff_max = Duration::from_millis(400);
    config.connect_timeout = Duration::from_secs(10);
    let core = Core::new(store.clone(), sink, config);
    Phone { core, store, out }
}

fn phone() -> Phone {
    phone_with("0.1.0")
}

impl Phone {
    fn statuses(&self) -> Vec<Status> {
        self.out.lock().unwrap().iter().filter_map(|o| if let Out::Status(s) = o { Some(s.clone()) } else { None }).collect()
    }

    fn events(&self) -> Vec<(String, Value)> {
        self.out.lock().unwrap().iter().filter_map(|o| if let Out::Event { name, payload } = o { Some((name.clone(), payload.clone())) } else { None }).collect()
    }
}

async fn until(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..600 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("timed out waiting for {what}");
}

async fn paired_and_online(desktop: &Desktop, phone: &Phone) {
    phone.core.pair(&desktop.ticket()).await.unwrap();
    phone.core.connect().unwrap();
    until("online", || matches!(phone.core.status(), Status::Online | Status::UpdateRecommended { .. })).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn pairing_stores_the_desktop_and_sends_the_phones_own_keys() {
    let desktop = Desktop::start().await;
    let phone = phone();
    phone.core.set_push_token("apns-token");
    phone.core.pair(&desktop.ticket()).await.unwrap();

    let record = phone.core.pairing().expect("the pairing is kept");
    assert_eq!(record.desktop, desktop.key.public().to_string());
    assert_eq!(record.relay, desktop.relay.to_string());
    let request = desktop.state.requests.lock().unwrap()[0].clone();
    let identity = Identity::load_or_create(&*phone.store).unwrap();
    assert_eq!(request.push_key, identity.push_key());
    assert_ne!(request.push_key, identity.iroh.public().as_bytes().to_owned(), "the push key is not the iroh key");
    assert_eq!(request.device_name, "Pixel 8");
    assert_eq!(request.push_token.as_deref(), Some("apns-token"));
    assert_eq!(desktop.state.paired.lock().unwrap().as_slice(), [identity.iroh.public().to_string()]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_refused_pairing_is_an_error_and_stores_nothing() {
    let desktop = Desktop::start().await;
    desktop.state.reject_pairing.store(true, Ordering::SeqCst);
    let phone = phone();
    let err = phone.core.pair(&desktop.ticket()).await.unwrap_err();
    assert!(err.contains("did not confirm"), "{err}");
    assert!(phone.core.pairing().is_none());
}

#[tokio::test]
async fn a_bad_or_expired_ticket_fails_before_any_network() {
    let phone = phone();
    assert!(phone.core.pair("hello").await.is_err());
    let expired = PairingTicket {
        desktop: SecretKey::generate().public(),
        relay: "https://relay.example.com".into(),
        secret: [1; 32],
        expires_at: 5,
    }
    .encode();
    assert!(phone.core.pair(&expired).await.unwrap_err().contains("expired"));
    assert!(phone.core.pairing().is_none());
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_invoke_reaches_the_desktop_through_the_relay_and_returns_its_answer() {
    let desktop = Desktop::start().await;
    let phone = phone();
    paired_and_online(&desktop, &phone).await;

    let reply = phone.core.invoke("tracker_snapshot", json!({})).await.unwrap();
    assert_eq!(reply, json!({ "echo": "tracker_snapshot", "args": {} }));
    let reply = phone.core.invoke("run_state", json!({ "project": "/p" })).await.unwrap();
    assert_eq!(reply["args"], json!({ "project": "/p" }));

    // The desktop's own error comes back as it gave it.
    let err = phone.core.invoke("session_attach", json!({ "id": 9 })).await.unwrap_err();
    assert_eq!(err.to_value(), json!({ "kind": "noSuchSession", "message": 9 }));

    // The handshake carried this build's version and protocol.
    let hello = desktop.state.hellos.lock().unwrap()[0].clone();
    assert_eq!((hello.protocol_major, hello.app_version.as_str()), (1, "0.1.0"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_command_off_the_allowlist_never_leaves_the_phone() {
    let desktop = Desktop::start().await;
    let phone = phone();
    paired_and_online(&desktop, &phone).await;
    for cmd in ["files_write", "terminal_write", "settings_save", "tracker_resync"] {
        assert_eq!(phone.core.invoke(cmd, json!({})).await, Err(CallError::Forbidden), "{cmd}");
    }
    assert!(desktop.state.calls.lock().unwrap().is_empty());
    assert_eq!(phone.core.listen("terminal:output"), Err(CallError::Forbidden));
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_is_sent_while_the_link_is_down() {
    let phone = phone();
    assert_eq!(phone.core.invoke("tracker_snapshot", json!({})).await, Err(CallError::Offline));
}

#[tokio::test(flavor = "multi_thread")]
async fn events_arrive_and_listening_survives_a_reconnect() {
    let desktop = Desktop::start().await;
    desktop.state.push_events.lock().unwrap().push(("tracker:delta".into(), json!({ "generation": 2 })));
    let phone = phone();
    phone.core.listen("tracker:delta").unwrap();
    paired_and_online(&desktop, &phone).await;
    until("the event", || phone.events().len() == 1).await;
    assert_eq!(phone.events()[0], ("tracker:delta".to_owned(), json!({ "generation": 2 })));

    desktop.drop_connections();
    until("unreachable", || matches!(phone.core.status(), Status::Unreachable { .. })).await;
    until("online again", || phone.core.status() == Status::Online).await;
    // Listen was sent again on the new connection, so the event is delivered again.
    until("the event again", || phone.events().len() == 2).await;
    assert_eq!(desktop.state.listens.lock().unwrap().len(), 2);

    let seen = phone.statuses();
    let ups = seen.iter().filter(|s| **s == Status::Online).count();
    assert_eq!(ups, 2, "online, then online again: {seen:?}");
    assert!(seen.iter().any(|s| matches!(s, Status::Unreachable { last_seen: Some(_) })), "{seen:?}");
    // A call works on the new link.
    assert!(phone.core.invoke("tracker_snapshot", json!({})).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_desktop_that_goes_away_is_unreachable_with_a_last_seen_time() {
    let desktop = Desktop::start().await;
    let phone = phone();
    paired_and_online(&desktop, &phone).await;
    let before = now_ms();
    desktop.endpoint.close().await;
    until("unreachable", || matches!(phone.core.status(), Status::Unreachable { .. })).await;
    match phone.core.status() {
        Status::Unreachable { last_seen: Some(seen) } => assert!(seen >= before - 5_000 && seen <= now_ms()),
        other => panic!("{other:?}"),
    }
    assert_eq!(phone.core.invoke("tracker_snapshot", json!({})).await, Err(CallError::Offline));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_minimum_version_above_this_build_shows_the_update_screen_and_calls_nothing() {
    let desktop = Desktop::start().await;
    let phone = phone();
    phone.core.pair(&desktop.ticket()).await.unwrap();
    *desktop.state.min_version.lock().unwrap() = "0.2.0".into();
    phone.core.connect().unwrap();
    until("update-required", || matches!(phone.core.status(), Status::UpdateRequired { .. })).await;
    assert_eq!(phone.core.status(), Status::UpdateRequired { store_url: "https://store.example/app".into() });
    assert_eq!(phone.core.invoke("tracker_snapshot", json!({})).await, Err(CallError::Offline));
    assert!(desktop.state.calls.lock().unwrap().is_empty());
    assert!(phone.statuses().iter().all(|s| *s != Status::Online), "never online");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_recommended_version_above_this_build_is_online_with_a_banner() {
    let desktop = Desktop::start().await;
    *desktop.state.recommended.lock().unwrap() = "0.3.0".into();
    let phone = phone();
    paired_and_online(&desktop, &phone).await;
    assert_eq!(
        phone.core.status(),
        Status::UpdateRecommended { store_url: "https://store.example/app".into(), recommended: "0.3.0".into() }
    );
    assert!(phone.core.invoke("tracker_snapshot", json!({})).await.is_ok());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_revoked_phone_stops_trying() {
    let desktop = Desktop::start().await;
    let phone = phone();
    paired_and_online(&desktop, &phone).await;
    desktop.state.paired.lock().unwrap().clear();
    desktop.close_all_with(CLOSE_REVOKED);
    until("revoked", || phone.core.status() == Status::Revoked).await;
    let hellos = desktop.state.hellos.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(desktop.state.hellos.lock().unwrap().len(), hellos, "no further attempts");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_token_that_appears_after_pairing_is_reported_and_repeated_at_each_handshake() {
    let desktop = Desktop::start().await;
    let phone = phone();
    paired_and_online(&desktop, &phone).await;
    assert!(desktop.state.tokens.lock().unwrap().is_empty(), "no token yet, nothing to say");

    phone.core.set_push_token("fcm-1");
    until("the token", || desktop.state.tokens.lock().unwrap().len() == 1).await;
    phone.core.set_push_token("fcm-2");
    until("the second token", || desktop.state.tokens.lock().unwrap().len() == 2).await;
    assert_eq!(desktop.state.tokens.lock().unwrap().as_slice(), ["fcm-1", "fcm-2"]);

    // Every handshake carries the token again: the phone keeps no record of what the desktop
    // was told, because such a record could say "told" for a message that never arrived.
    desktop.drop_connections();
    until("unreachable", || matches!(phone.core.status(), Status::Unreachable { .. })).await;
    until("online again", || phone.core.status() == Status::Online).await;
    until("the token again", || desktop.state.tokens.lock().unwrap().len() == 3).await;
    assert_eq!(desktop.state.tokens.lock().unwrap().last().map(String::as_str), Some("fcm-2"));
}

#[tokio::test(flavor = "multi_thread")]
async fn a_token_waits_when_the_desktop_predates_the_message() {
    let desktop = Desktop::start().await;
    *desktop.state.minor.lock().unwrap() = 0;
    let phone = phone();
    paired_and_online(&desktop, &phone).await;
    phone.core.set_push_token("fcm-1");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(desktop.state.tokens.lock().unwrap().is_empty(), "an older desktop would close on the unknown message");
}

#[tokio::test(flavor = "multi_thread")]
async fn unpairing_forgets_the_desktop_and_stops_the_link() {
    let desktop = Desktop::start().await;
    let phone = phone();
    paired_and_online(&desktop, &phone).await;
    phone.core.unpair();
    assert!(phone.core.pairing().is_none());
    assert_eq!(phone.core.invoke("tracker_snapshot", json!({})).await, Err(CallError::Offline));
    assert!(phone.core.connect().is_err());
}

#[tokio::test(flavor = "multi_thread")]
async fn a_close_with_protocol_during_pairing_is_a_failure_and_stores_nothing() {
    let desktop = Desktop::start().await;
    desktop.state.close_pairing_with_protocol.store(true, Ordering::SeqCst);
    let phone = phone();
    let err = phone.core.pair(&desktop.ticket()).await.unwrap_err();
    assert!(err.contains("try again"), "{err}");
    assert!(phone.core.pairing().is_none(), "the ambiguous close is not a pairing");
}

#[tokio::test(flavor = "multi_thread")]
async fn a_retry_from_unreachable_shows_connecting_and_then_the_real_status_again() {
    let desktop = Desktop::start().await;
    let phone = phone();
    paired_and_online(&desktop, &phone).await;
    desktop.endpoint.close().await;
    until("unreachable", || matches!(phone.core.status(), Status::Unreachable { .. })).await;
    let before = phone.statuses().len();

    phone.core.connect().unwrap();
    // The retry is announced, and its failure is announced too, though the value after it
    // equals the value before it.
    until("connecting then unreachable again", || {
        let seen = phone.statuses();
        let after = &seen[before..];
        after.iter().position(|s| *s == Status::Connecting).is_some_and(|i| {
            after[i..].iter().any(|s| matches!(s, Status::Unreachable { .. }))
        })
    })
    .await;
}
