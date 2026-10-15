//! The desktop's half of push: when a run ends or an agent starts waiting for a person, every paired
//! phone that registered a push token is sent a short message, encrypted to the key it paired with.
//!
//! The gateway (`server/push-gateway`) is the only party in between and sees a device token, the time
//! and the size of a blob — never the text. Delivery is best effort and says so: one failed request is
//! one log line without the token or the content, with no retry and no queue. The run report is still
//! on the desktop and the phone shows it when it next connects, so a lost push costs a nudge and
//! nothing else.
//!
//! Sending is spawned off the path that produced the event. A run's worker and a session's worker are
//! the places this is called from, and neither may wait on a network that is down.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use remote_protocol::push::seal;
use remote_protocol::Platform;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use super::devices::Device;
use super::Remote;
use crate::runs::summary::RunSummary;
use crate::session::model::SessionState;

/// The gateway's endpoint. Ours and not configurable, unlike the relay: the gateway holds the APNs and
/// FCM credentials of the published apps, so it cannot be self-hosted by someone else.
///
/// **A placeholder**, like `DEFAULT_RELAY_URL`: `.example` never resolves, so a build that ships it
/// fails to reach a gateway rather than reaching somebody else's. Replace it before the first release
/// that includes the companion.
pub const GATEWAY_URL: &str = "https://push.smetana.example/v1/push";

/// How long one request to the gateway may take. A push that is late by more than this is not worth
/// holding a task for.
const GATEWAY_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    RunFinished,
    NeedsYou,
}

/// The plaintext of a blob: what the phone decrypts and shows. `target` is the string the phone opens a
/// screen by, `run:<project>` or `session:<id>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub kind: Kind,
    pub project: String,
    pub title: String,
    pub body: String,
    pub target: String,
}

/// What the gateway accepts: `server/push-gateway/src/lib.rs`, `PushRequest`.
#[derive(Serialize)]
struct Request<'a> {
    platform: &'static str,
    token: &'a str,
    blob: String,
}

fn project_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The message for a run that has just ended. `tasks` being `None` means the board could not be diffed,
/// which is not the same as nothing moving, so the body does not claim a count.
pub fn run_finished(project_path: &str, summary: Option<&RunSummary>) -> Message {
    let project = project_name(project_path);
    let body = match summary.and_then(|summary| summary.tasks.as_ref()) {
        Some(tasks) => format!(
            "{project}: {} closed, {} waiting for you",
            count(tasks.closed.len(), "task"),
            tasks.parked.len()
        ),
        None => format!("{project}: the run has ended"),
    };
    Message {
        kind: Kind::RunFinished,
        title: "Run finished".into(),
        body,
        target: format!("run:{project_path}"),
        project,
    }
}

/// The message for a session that has started waiting. The body names the agent, and the session's title
/// when it has one.
pub fn needs_you(project_path: &str, session: u64, agent: &str, title: Option<&str>) -> Message {
    let project = project_name(project_path);
    let body = match title.filter(|title| !title.is_empty()) {
        Some(title) => format!("{project}: {agent} on \"{title}\""),
        None => format!("{project}: {agent}"),
    };
    Message {
        kind: Kind::NeedsYou,
        title: "An agent is waiting for you".into(),
        body,
        target: format!("session:{session}"),
        project,
    }
}

/// Whether this change of state is the start of a wait. One push per transition, not per event: while a
/// session stays in `NeedsYou` — a second permission queued behind the first, a text delta, a journal
/// event of any kind — the state does not change and this is false.
pub fn waiting_began(before: SessionState, after: SessionState) -> bool {
    after == SessionState::NeedsYou && before != SessionState::NeedsYou
}

/// Whether a run report moved the run into its ended state. `was_over` is what the worker held before
/// the report, `None` when it held nothing under that token. A stop with nothing in flight ends a run on
/// the worker's own side before any report, and is the person's own act: it is not announced.
pub fn run_ended(was_over: Option<bool>, is_over: bool) -> bool {
    is_over && was_over == Some(false)
}

fn platform_name(platform: Platform) -> &'static str {
    match platform {
        Platform::Ios => "ios",
        Platform::Android => "android",
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

/// Hands `message` to every paired phone without waiting for it. Does nothing when the companion is
/// off or no phone has a token.
fn dispatch(app: &AppHandle, message: Message) {
    let Some(remote) = app.try_state::<Arc<Remote>>() else { return };
    let devices: Vec<Device> =
        remote.devices.all().into_iter().filter(|device| device.push_token.is_some()).collect();
    if devices.is_empty() {
        return;
    }
    tauri::async_runtime::spawn(async move {
        let Some(client) = client() else {
            log::warn!("remote: push not sent: no HTTP client");
            return;
        };
        deliver(&client, GATEWAY_URL, &devices, &message).await;
    });
}

pub fn notify_run_finished(app: &AppHandle, run: &crate::runs::model::Run) {
    dispatch(app, run_finished(&run.project, run.summary.as_ref()));
}

pub fn notify_needs_you(app: &AppHandle, session: u64, project: &str, agent: &str, title: Option<&str>) {
    dispatch(app, needs_you(project, session, agent, title));
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::extract::State;
    use axum::http::StatusCode;
    use axum::routing::post;
    use axum::{Json, Router};
    use remote_protocol::push::{open, PushBlob};

    use super::*;
    use crate::runs::summary::{TaskLine, Tasks};
    use crate::session::model::{state_of, Event, EventKind};

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

    #[test]
    fn the_run_body_counts_and_does_not_invent_a_count_it_lacks() {
        assert_eq!(run_finished("/a/b", Some(&summary(1, 0))).body, "b: 1 task closed, 0 waiting for you");
        assert_eq!(run_finished("/a/b", None).body, "b: the run has ended");
        let unknown = RunSummary { seconds: 1, tasks: None, report: None };
        assert_eq!(run_finished("/a/b", Some(&unknown)).body, "b: the run has ended");
    }

    #[test]
    fn a_run_is_announced_once_when_it_becomes_over() {
        assert!(run_ended(Some(false), true));
        assert!(!run_ended(Some(true), true), "a second report of an ended run");
        assert!(!run_ended(Some(false), false));
        assert!(!run_ended(None, true), "a report for a run the worker no longer holds");
    }

    fn ev(seq: u64, kind: EventKind) -> Event {
        Event { seq, at: "2026-10-15T12:00:00Z".into(), kind }
    }

    fn permission(id: &str) -> EventKind {
        EventKind::Permission {
            id: id.into(),
            tool: "Bash".into(),
            detail: "ls".into(),
            options: vec![],
            input: serde_json::Value::Null,
        }
    }

    #[test]
    fn a_wait_makes_one_needs_you_however_many_events_follow_it() {
        let journal = vec![
            ev(1, EventKind::TurnStart { by: crate::session::model::Actor::Person }),
            ev(2, permission("p1")),
            ev(3, EventKind::TextDelta { text: "still thinking".into() }),
            ev(4, permission("p2")),
            ev(5, EventKind::TextDelta { text: "more".into() }),
        ];
        let mut state = SessionState::Starting;
        let mut pushes = 0;
        for upto in 1..=journal.len() {
            let next = state_of(&journal[..upto], true);
            if waiting_began(state, next) {
                pushes += 1;
            }
            state = next;
        }
        assert_eq!(state, SessionState::NeedsYou);
        assert_eq!(pushes, 1, "the second permission and the deltas are the same wait");
    }

    #[test]
    fn a_wait_after_an_answer_is_a_new_one() {
        let journal = vec![
            ev(1, EventKind::TurnStart { by: crate::session::model::Actor::Person }),
            ev(2, permission("p1")),
            ev(3, EventKind::PermissionAnswered {
                id: "p1".into(),
                decision: crate::session::model::Decision::Allow,
                answers: None,
            }),
            ev(4, permission("p2")),
        ];
        let mut state = SessionState::Starting;
        let mut pushes = 0;
        for upto in 1..=journal.len() {
            let next = state_of(&journal[..upto], true);
            if waiting_began(state, next) {
                pushes += 1;
            }
            state = next;
        }
        assert_eq!(pushes, 2);
    }

    #[test]
    fn the_needs_you_message_names_the_project_and_the_agent() {
        let message = needs_you("/work/smetana", 7, "claude", Some("fix the board"));
        assert_eq!(message.kind, Kind::NeedsYou);
        assert_eq!(message.title, "An agent is waiting for you");
        assert_eq!(message.body, "smetana: claude on \"fix the board\"");
        assert_eq!(message.target, "session:7");
        assert_eq!(needs_you("/work/smetana", 7, "claude", None).body, "smetana: claude");
    }
}
