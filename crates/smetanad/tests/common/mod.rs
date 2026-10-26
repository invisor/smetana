//! Helpers the integration tests share.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::time::Duration;

use iroh::endpoint::presets;
use iroh::{Endpoint, EndpointAddr, RelayMode, SecretKey};
use remote_protocol::{
    read_frame, write_frame, PairReply, PairRequest, PairingTicket, Platform, ALPN, MAX_FRAME,
};
use smetanad::{client, local};

/// The bd the desktop bundle ships, as `npm run fetch-bd` lays it out. A test that needs a tracker
/// fails loudly without it: a silent skip is a gate that stays green with the tracker missing.
pub fn bd() -> PathBuf {
    if let Some(path) = std::env::var_os("SMETANAD_TEST_BD") {
        return PathBuf::from(path);
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/binaries");
    let found = std::fs::read_dir(&dir).ok().and_then(|entries| {
        entries
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.file_name().is_some_and(|name| name.to_string_lossy().starts_with("bd-")))
    });
    found.unwrap_or_else(|| {
        panic!("no bd sidecar in {}: run `npm run fetch-bd`, or set SMETANAD_TEST_BD", dir.display())
    })
}

/// One runtime for every test in the process. The workers spawn onto a process-wide runtime handle
/// that the first server to start installs (`smetana_core::context::install_runtime`), so a runtime
/// per `#[tokio::test]` would leave the second test's workers on one that has already been dropped.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    static RUNTIME: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RUNTIME
        .get_or_init(|| tokio::runtime::Builder::new_multi_thread().enable_all().build().expect("a runtime"))
        .block_on(future)
}

pub async fn wait_for(what: &str, mut done: impl FnMut() -> bool) {
    for _ in 0..600 {
        if done() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("timed out waiting for {what}");
}

/// A device that is not the local client: its own key, dialling the server's loopback address.
pub struct Device {
    pub key: SecretKey,
    pub endpoint: Endpoint,
    pub addr: EndpointAddr,
}

impl Device {
    pub async fn new(data_dir: &Path) -> Device {
        let key = SecretKey::generate();
        let endpoint = Endpoint::builder(presets::Minimal)
            .secret_key(key.clone())
            .relay_mode(RelayMode::Disabled)
            .clear_ip_transports()
            .bind_addr("127.0.0.1:0")
            .unwrap()
            .bind()
            .await
            .unwrap();
        let addr = client::address(&local::read(data_dir).unwrap());
        Device { key, endpoint, addr }
    }

    /// Spends the ticket as a phone would, except that it dials the loopback address and not a relay.
    pub async fn pair_with(&self, ticket: &str, name: &str) -> PairReply {
        let (ticket, _kind) = PairingTicket::decode_any(ticket).expect("a ticket");
        let conn = self.endpoint.connect(self.addr.clone(), ALPN).await.expect("connects");
        let (mut send, mut recv) = conn.open_bi().await.unwrap();
        let request = PairRequest {
            secret: ticket.secret,
            device_name: name.into(),
            platform: Platform::Android,
            push_token: None,
            push_key: [5; 32],
            app_version: "1.0.0".into(),
        };
        write_frame(&mut send, &request).await.unwrap();
        let reply = read_frame::<PairReply>(&mut recv, MAX_FRAME).await.expect("a pairing answer");
        conn.close(0u32.into(), b"done");
        reply
    }
}
