//! The gateway against stand-in providers: what reaches Apple and Google, what is refused before
//! it gets there, and what the log is allowed to say.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use push_gateway::providers::{Provider, ProviderError, PushMessage, SendFuture};
use push_gateway::{app, RateLimiter, State};
use tower::ServiceExt;

#[derive(Default)]
struct Recorder {
    got: Mutex<Vec<PushMessage>>,
    fail: Option<ProviderError>,
}

impl Provider for Recorder {
    fn send<'a>(&'a self, message: &'a PushMessage) -> SendFuture<'a> {
        Box::pin(async move {
            self.got.lock().unwrap().push(message.clone());
            match self.fail {
                Some(e) => Err(e),
                None => Ok(()),
            }
        })
    }
}

struct Rig {
    apns: Arc<Recorder>,
    fcm: Arc<Recorder>,
    state: State,
}

fn rig(limit: u32) -> Rig {
    rig_with(limit, Recorder::default())
}

fn rig_with(limit: u32, apns: Recorder) -> Rig {
    let apns = Arc::new(apns);
    let fcm = Arc::new(Recorder::default());
    let state = State {
        apns: Some(apns.clone()),
        fcm: Some(fcm.clone()),
        limiter: Arc::new(RateLimiter::new(limit, Duration::from_secs(60))),
        trust_forwarded_for: false,
        body_timeout: Duration::from_secs(5),
    };
    Rig { apns, fcm, state }
}

async fn post(state: &State, ip: &str, body: impl Into<Body>) -> StatusCode {
    let mut request = Request::post("/v1/push").body(body.into()).unwrap();
    let addr: SocketAddr = format!("{ip}:4000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(addr));
    app(state.clone()).oneshot(request).await.unwrap().status()
}

fn json(platform: &str, token: &str, blob: &str) -> String {
    serde_json::json!({ "platform": platform, "token": token, "blob": blob }).to_string()
}

const IOS_TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

#[tokio::test]
async fn blob_reaches_the_ios_provider_byte_for_byte() {
    let rig = rig(100);
    let bytes: Vec<u8> = (0..=255u8).cycle().take(900).collect();
    let blob = STANDARD.encode(&bytes);
    assert_eq!(post(&rig.state, "10.0.0.1", json("ios", IOS_TOKEN, &blob)).await, StatusCode::OK);

    let got = rig.apns.got.lock().unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].token, IOS_TOKEN);
    assert_eq!(got[0].blob, blob);
    assert_eq!(STANDARD.decode(&got[0].blob).unwrap(), bytes);
    assert!(rig.fcm.got.lock().unwrap().is_empty());
}

#[tokio::test]
async fn blob_reaches_the_android_provider_byte_for_byte() {
    let rig = rig(100);
    let bytes: Vec<u8> = (0..=255u8).rev().cycle().take(1500).collect();
    let blob = STANDARD.encode(&bytes);
    let token = "fcm-token_1:APA91b.xyz";
    assert_eq!(post(&rig.state, "10.0.0.1", json("android", token, &blob)).await, StatusCode::OK);

    let got = rig.fcm.got.lock().unwrap();
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].token, token);
    assert_eq!(STANDARD.decode(&got[0].blob).unwrap(), bytes);
    assert!(rig.apns.got.lock().unwrap().is_empty());
}

#[tokio::test]
async fn refused_requests_are_400_and_never_reach_a_provider() {
    let rig = rig(1000);
    let good = STANDARD.encode(b"ciphertext");
    let oversized = json("ios", IOS_TOKEN, &STANDARD.encode(vec![7u8; 5000]));
    assert!(oversized.len() > 4096);

    let cases: Vec<(&str, String)> = vec![
        ("unknown platform", json("windows", IOS_TOKEN, &good)),
        ("blob is not base64", json("ios", IOS_TOKEN, "not base64 !!")),
        ("empty blob", json("ios", IOS_TOKEN, "")),
        ("body over 4 KiB", oversized),
        ("not json", "{".to_string()),
        ("missing field", r#"{"platform":"ios"}"#.to_string()),
        ("ios token is not hex", json("ios", "../../v1/evil", &good)),
        ("empty token", json("android", "", &good)),
    ];
    for (name, body) in cases {
        assert_eq!(post(&rig.state, "10.0.0.1", body).await, StatusCode::BAD_REQUEST, "{name}");
    }
    assert!(rig.apns.got.lock().unwrap().is_empty());
    assert!(rig.fcm.got.lock().unwrap().is_empty());
}

#[tokio::test]
async fn body_exactly_at_the_limit_is_accepted_and_one_over_is_not() {
    let rig = rig(1000);
    let blob = STANDARD.encode(b"x");
    // Whitespace padding keeps the JSON valid while the size gate is what is being tested.
    let padded = format!("{}{}", json("android", "tok", &blob), " ".repeat(4096 - json("android", "tok", &blob).len()));
    assert_eq!(padded.len(), 4096);
    assert_eq!(post(&rig.state, "10.0.0.1", padded).await, StatusCode::OK);
    let over = format!("{}{}", json("android", "tok", &blob), " ".repeat(4097 - json("android", "tok", &blob).len()));
    assert_eq!(over.len(), 4097);
    assert_eq!(post(&rig.state, "10.0.0.1", over).await, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn over_the_rate_limit_from_one_ip_is_429_and_another_ip_is_unaffected() {
    let rig = rig(3);
    let body = json("ios", IOS_TOKEN, &STANDARD.encode(b"b"));
    for _ in 0..3 {
        assert_eq!(post(&rig.state, "10.0.0.1", body.clone()).await, StatusCode::OK);
    }
    assert_eq!(post(&rig.state, "10.0.0.1", body.clone()).await, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(post(&rig.state, "10.0.0.2", body).await, StatusCode::OK);
    assert_eq!(rig.apns.got.lock().unwrap().len(), 4);
}

#[tokio::test]
async fn provider_outcomes_map_to_distinct_statuses() {
    let gone = rig_with(100, Recorder { fail: Some(ProviderError::TokenGone), ..Default::default() });
    let down = rig_with(100, Recorder { fail: Some(ProviderError::Unavailable), ..Default::default() });
    let body = json("ios", IOS_TOKEN, &STANDARD.encode(b"b"));
    assert_eq!(post(&gone.state, "10.0.0.1", body.clone()).await, StatusCode::GONE);
    assert_eq!(post(&down.state, "10.0.0.1", body.clone()).await, StatusCode::BAD_GATEWAY);

    let mut unconfigured = rig(100).state;
    unconfigured.apns = None;
    assert_eq!(post(&unconfigured, "10.0.0.1", body).await, StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn forwarded_for_is_honoured_only_when_trusted() {
    let mut rig = rig(1);
    let body = json("ios", IOS_TOKEN, &STANDARD.encode(b"b"));
    let send = |state: State, xff: &'static str| {
        let body = body.clone();
        async move {
            let mut request = Request::post("/v1/push")
                .header("x-forwarded-for", xff)
                .body(Body::from(body))
                .unwrap();
            request.extensions_mut().insert(ConnectInfo("10.9.9.9:1".parse::<SocketAddr>().unwrap()));
            app(state).oneshot(request).await.unwrap().status()
        }
    };
    // Untrusted: both requests are the same peer, the header is ignored.
    assert_eq!(send(rig.state.clone(), "1.1.1.1").await, StatusCode::OK);
    assert_eq!(send(rig.state.clone(), "2.2.2.2").await, StatusCode::TOO_MANY_REQUESTS);
    // Trusted: the last hop distinguishes clients behind the proxy.
    rig.state.trust_forwarded_for = true;
    rig.state.limiter = Arc::new(RateLimiter::new(1, Duration::from_secs(60)));
    assert_eq!(send(rig.state.clone(), "9.9.9.9, 1.1.1.1").await, StatusCode::OK);
    assert_eq!(send(rig.state.clone(), "9.9.9.9, 2.2.2.2").await, StatusCode::OK);
    assert_eq!(send(rig.state.clone(), "9.9.9.9, 2.2.2.2").await, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn ipv6_clients_share_a_budget_per_slash_64_and_ipv4_per_address() {
    let limiter = RateLimiter::new(2, Duration::from_secs(60));
    let ip = |s: &str| s.parse::<std::net::IpAddr>().unwrap();
    assert!(limiter.allow(ip("2001:db8:1:2::1")));
    assert!(limiter.allow(ip("2001:db8:1:2:ffff:ffff:ffff:ffff")));
    // A third address in the same /64 is over the shared budget of two.
    assert!(!limiter.allow(ip("2001:db8:1:2:aaaa::7")));
    // The neighbouring /64 has a budget of its own.
    assert!(limiter.allow(ip("2001:db8:1:3::1")));
    // IPv4 is counted per full address.
    assert!(limiter.allow(ip("192.0.2.1")));
    assert!(limiter.allow(ip("192.0.2.1")));
    assert!(!limiter.allow(ip("192.0.2.1")));
    assert!(limiter.allow(ip("192.0.2.2")));
}

#[tokio::test]
async fn a_full_table_refuses_new_clients_but_still_serves_known_ones() {
    let limiter = RateLimiter::with_capacity(5, Duration::from_secs(60), 2);
    let ip = |s: &str| s.parse::<std::net::IpAddr>().unwrap();
    assert!(limiter.allow(ip("192.0.2.1")));
    assert!(limiter.allow(ip("192.0.2.2")));
    assert!(!limiter.allow(ip("192.0.2.3")), "a new client is refused when the table is full");
    assert!(limiter.allow(ip("192.0.2.1")), "a known client is still counted normally");
}

#[tokio::test]
async fn expired_entries_make_room_again() {
    let limiter = RateLimiter::with_capacity(5, Duration::from_millis(50), 1);
    let ip = |s: &str| s.parse::<std::net::IpAddr>().unwrap();
    assert!(limiter.allow(ip("192.0.2.1")));
    assert!(!limiter.allow(ip("192.0.2.2")));
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(limiter.allow(ip("192.0.2.2")));
}

#[tokio::test]
async fn blob_over_the_cap_is_400_and_the_cap_itself_is_accepted() {
    let rig = rig(100);
    let at = "A".repeat(push_gateway::MAX_BLOB);
    let over = "A".repeat(push_gateway::MAX_BLOB + 4);
    assert_eq!(post(&rig.state, "10.0.0.1", json("android", "tok", &at)).await, StatusCode::OK);
    assert_eq!(post(&rig.state, "10.0.0.1", json("android", "tok", &over)).await, StatusCode::BAD_REQUEST);
    assert_eq!(rig.fcm.got.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_body_that_never_finishes_is_408() {
    let mut rig = rig(100);
    rig.state.body_timeout = Duration::from_millis(50);
    // The sender is kept alive and never sends: the body stays pending.
    let (_sender, channel) = http_body_util::channel::Channel::<axum::body::Bytes, std::convert::Infallible>::new(1);
    let mut request = Request::post("/v1/push").body(Body::new(channel)).unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("10.0.0.1:1".parse::<SocketAddr>().unwrap()));
    let status = app(rig.state.clone()).oneshot(request).await.unwrap().status();
    assert_eq!(status, StatusCode::REQUEST_TIMEOUT);
    assert!(rig.apns.got.lock().unwrap().is_empty() && rig.fcm.got.lock().unwrap().is_empty());
}
