//! The delivery half of push: sealing a message to each paired phone's key and posting it to the
//! gateway. What is said, and when, is `smetana_core::remote::push`; this is the part that needs the
//! paired devices (`devices.rs`) and the companion's own `Remote`, and so stays with the endpoint.
//!
//! The gateway (`server/push-gateway`) is the only party in between and sees a device token, the time
//! and the size of a blob — never the text. Delivery is best effort and says so: one failed request is
//! one log line without the token or the content, with no retry and no queue.

use std::sync::Arc;
use std::time::Duration;

use remote_protocol::push::seal;
use remote_protocol::Platform;
use serde::Serialize;
use smetana_core::remote::push::Message;

use super::devices::Device;
use super::Remote;

/// The gateway's endpoint. Ours and not configurable, unlike the relay: the gateway holds the APNs and
/// FCM credentials of the published apps, so it cannot be self-hosted by someone else.
/// It runs beside the relay, behind the same Caddy (`server/README.md`, "Our deployment").
pub const GATEWAY_URL: &str = "https://push.smetana.io/v1/push";

/// How long one request to the gateway may take. A push that is late by more than this is not worth
/// holding a task for.
const GATEWAY_TIMEOUT: Duration = Duration::from_secs(10);

/// What the gateway accepts: `server/push-gateway/src/lib.rs`, `PushRequest`.
#[derive(Serialize)]
struct Request<'a> {
    platform: &'static str,
    token: &'a str,
    blob: String,
}

fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Ios => "ios",
        Platform::Android => "android",
        Platform::Desktop => "desktop",
    }
}

fn blob_for(device: &Device, plaintext: &[u8]) -> Option<String> {
    let key: [u8; 32] = data_encoding::HEXLOWER.decode(device.push_key.as_bytes()).ok()?.try_into().ok()?;
    Some(seal(&key, plaintext).to_wire())
}

/// Sends `message` to every device that has a push token, one request each. Never fails: a refusal or an
/// unreachable gateway is a log line saying which device (by name) and what kind of failure, with no
/// token, no content and no error text — a `reqwest` error prints the URL it was made for.
pub async fn deliver(client: &reqwest::Client, gateway: &str, devices: &[Device], message: &Message) {
    let Ok(plaintext) = serde_json::to_vec(message) else { return };
    for device in devices {
        // The gateway knows two platforms and answers 400 to anything else; a desktop is woken by
        // its own connection, not by a push.
        if device.platform == Platform::Desktop {
            continue;
        }
        let Some(token) = device.push_token.as_deref().filter(|token| !token.is_empty()) else {
            continue;
        };
        let Some(blob) = blob_for(device, &plaintext) else {
            log::warn!("remote: push skipped for {}: its push key is unusable", device.name);
            continue;
        };
        let body = Request { platform: platform_name(device.platform), token, blob };
        let Ok(body) = serde_json::to_string(&body) else { continue };
        let sent = client
            .post(gateway)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(body)
            .send()
            .await;
        match sent {
            Ok(response) if response.status().is_success() => {}
            Ok(response) => {
                log::warn!("remote: push gateway answered {} for {}", response.status().as_u16(), device.name)
            }
            Err(err) => {
                let why = if err.is_timeout() {
                    "timed out"
                } else if err.is_connect() {
                    "unreachable"
                } else {
                    "failed"
                };
                log::warn!("remote: push gateway {why} for {}", device.name)
            }
        }
    }
}

fn client() -> Option<reqwest::Client> {
    // The updater installs this lazily on its first check, and a push can come first.
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder().timeout(GATEWAY_TIMEOUT).build().ok()
}

/// Hands `message` to every paired phone without waiting for it. Does nothing when no phone has a token.
///
/// Called with the companion's own `Remote` by whoever owns it: the context's sink finds it, so the
/// workers that want a push sent say so through `notify_*` below and never hold the `Remote`.
pub fn dispatch(remote: &Arc<Remote>, message: Message) {
    let devices: Vec<Device> =
        remote.devices.all().into_iter()
        .filter(|device| device.push_token.is_some() && device.platform != Platform::Desktop)
        .collect();
    if devices.is_empty() {
        return;
    }
    smetana_core::context::spawn(async move {
        let Some(client) = client() else {
            log::warn!("remote: push not sent: no HTTP client");
            return;
        };
        deliver(&client, GATEWAY_URL, &devices, &message).await;
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::{Json, Router};
    use remote_protocol::push::{open, PushBlob};
    use smetana_core::remote::push::run_finished;
    use smetana_core::runs::summary::{RunSummary, TaskLine, Tasks};

    use super::*;

    /// A stand-in gateway on loopback: records every body it is sent and answers with `status`.
    struct Gateway {
        url: String,
        seen: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    async fn gateway(status: StatusCode) -> Gateway {
        type Shared = (Arc<Mutex<Vec<serde_json::Value>>>, StatusCode);
        async fn handler(State((seen, status)): State<Shared>, Json(body): Json<serde_json::Value>) -> StatusCode {
            seen.lock().unwrap().push(body);
            status
        }
        let seen = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new().route("/v1/push", post(handler)).with_state((seen.clone(), status));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v1/push", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Gateway { url, seen }
    }

    fn http() -> reqwest::Client {
        client().expect("a client")
    }

    fn device(name: &str, platform: Platform, token: Option<&str>, secret: &[u8; 32]) -> Device {
        let public = x25519_public(secret);
        Device {
            key: format!("key-{name}"),
            name: name.into(),
            platform,
            push_token: token.map(str::to_owned),
            push_key: data_encoding::HEXLOWER.encode(&public),
            paired_at: 1,
            last_contact: None,
            kind: remote_protocol::DeviceKind::Phone,
        }
    }

    fn x25519_public(secret: &[u8; 32]) -> [u8; 32] {
        // The same derivation the phone uses; reached through the protocol crate's own seal/open so the
        // test does not take a second dependency on the curve library.
        remote_protocol::push::public_key(secret)
    }

    fn summary(closed: usize, parked: usize) -> RunSummary {
        let line = |n: usize| TaskLine { id: format!("t-{n}"), title: format!("task {n}") };
        RunSummary {
            seconds: 60,
            tasks: Some(Tasks { closed: (0..closed).map(line).collect(), parked: (0..parked).map(line).collect() }),
            report: None,
        }
    }

    fn decrypt(secret: &[u8; 32], request: &serde_json::Value) -> serde_json::Value {
        let blob = PushBlob::from_wire(request["blob"].as_str().unwrap()).unwrap();
        serde_json::from_slice(&open(secret, &blob).unwrap()).unwrap()
    }

    #[tokio::test]
    async fn a_finished_run_makes_one_request_per_device_with_a_token() {
        let gateway = gateway(StatusCode::OK).await;
        let (a, b, c) = ([1u8; 32], [2u8; 32], [3u8; 32]);
        let devices = vec![
            device("ios phone", Platform::Ios, Some("abcdef0123"), &a),
            device("android phone", Platform::Android, Some("tok_en:1"), &b),
            device("no token", Platform::Android, None, &c),
        ];
        let message = run_finished("/work/smetana", Some(&summary(5, 1)));
        deliver(&http(), &gateway.url, &devices, &message).await;

        let seen = gateway.seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "one request each for the two devices with a token, none for the third");
        assert_eq!(seen[0]["platform"], "ios");
        assert_eq!(seen[0]["token"], "abcdef0123");
        assert_eq!(seen[1]["platform"], "android");
        assert_eq!(seen[1]["token"], "tok_en:1");
        for request in seen.iter() {
            let keys: Vec<_> = request.as_object().unwrap().keys().cloned().collect();
            assert_eq!(keys.len(), 3, "exactly platform, token and blob: {keys:?}");
        }
        let plain = decrypt(&a, &seen[0]);
        assert_eq!(plain["kind"], "run-finished");
        assert_eq!(plain["project"], "smetana");
        assert_eq!(plain["title"], "Run finished");
        assert_eq!(plain["body"], "smetana: 5 tasks closed, 1 waiting for you");
        assert_eq!(plain["target"], "run:/work/smetana");
        assert_eq!(decrypt(&b, &seen[1])["kind"], "run-finished");
    }

    #[tokio::test]
    async fn a_desktop_with_a_token_is_not_sent_to_the_gateway() {
        let gateway = gateway(StatusCode::OK).await;
        let devices = vec![
            device("desktop", Platform::Desktop, Some("zz"), &[3; 32]),
            device("phone", Platform::Ios, Some("ab12"), &[1; 32]),
        ];
        deliver(&http(), &gateway.url, &devices, &run_finished("/p", None)).await;
        let seen = gateway.seen.lock().unwrap();
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0]["platform"], "ios");
    }

    #[tokio::test]
    async fn a_blob_opens_only_with_the_devices_own_secret() {
        let gateway = gateway(StatusCode::OK).await;
        let (a, other) = ([1u8; 32], [9u8; 32]);
        let devices = vec![device("phone", Platform::Ios, Some("ab12"), &a)];
        deliver(&http(), &gateway.url, &devices, &run_finished("/p", None)).await;
        let seen = gateway.seen.lock().unwrap();
        let blob = PushBlob::from_wire(seen[0]["blob"].as_str().unwrap()).unwrap();
        assert!(open(&other, &blob).is_err());
        assert!(open(&a, &blob).is_ok());
    }

    #[tokio::test]
    async fn a_gateway_error_is_not_retried_and_does_not_panic() {
        let gateway = gateway(StatusCode::BAD_GATEWAY).await;
        let devices = vec![device("phone", Platform::Ios, Some("ab12"), &[1; 32])];
        deliver(&http(), &gateway.url, &devices, &run_finished("/p", None)).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(gateway.seen.lock().unwrap().len(), 1, "one attempt and no second");
    }

    #[tokio::test]
    async fn an_unreachable_gateway_returns_normally() {
        let closed = {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            format!("http://{}/v1/push", listener.local_addr().unwrap())
        };
        let devices = vec![device("phone", Platform::Ios, Some("ab12"), &[1; 32])];
        deliver(&http(), &closed, &devices, &run_finished("/p", None)).await;
    }

    #[tokio::test]
    async fn an_unusable_push_key_skips_that_device_only() {
        let gateway = gateway(StatusCode::OK).await;
        let mut broken = device("broken", Platform::Ios, Some("ab12"), &[1; 32]);
        broken.push_key = "not hex".into();
        let devices = vec![broken, device("fine", Platform::Android, Some("tok"), &[2; 32])];
        deliver(&http(), &gateway.url, &devices, &run_finished("/p", None)).await;
        assert_eq!(gateway.seen.lock().unwrap().len(), 1);
    }
}
