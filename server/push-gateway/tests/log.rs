//! The log is allowed to say a status and a platform, and nothing else.
//!
//! This lives in a file of its own on purpose: tracing caches, per call site, whether any
//! subscriber wants it. In a process shared with other tests, a call site first hit by a test
//! without a subscriber is cached as "never" and this test would read an empty log.

use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::Request;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use push_gateway::providers::{Provider, ProviderError, PushMessage, SendFuture};
use push_gateway::{app, RateLimiter, State};
use tower::ServiceExt;

struct Stub(Option<ProviderError>);

impl Provider for Stub {
    fn send<'a>(&'a self, _: &'a PushMessage) -> SendFuture<'a> {
        Box::pin(async move { self.0.map_or(Ok(()), Err) })
    }
}

fn state(limit: u32, fail: Option<ProviderError>) -> State {
    State {
        apns: Some(Arc::new(Stub(fail))),
        fcm: Some(Arc::new(Stub(fail))),
        limiter: Arc::new(RateLimiter::new(limit, Duration::from_secs(60))),
        trust_forwarded_for: false,
        body_timeout: Duration::from_secs(5),
    }
}

async fn post(state: &State, ip: &str, body: String) {
    let mut request = Request::post("/v1/push").body(Body::from(body)).unwrap();
    let addr: SocketAddr = format!("{ip}:4000").parse().unwrap();
    request.extensions_mut().insert(ConnectInfo(addr));
    app(state.clone()).oneshot(request).await.unwrap();
}

fn json(platform: &str, token: &str, blob: &str) -> String {
    serde_json::json!({ "platform": platform, "token": token, "blob": blob }).to_string()
}

/// A `MakeWriter` that keeps everything written, for reading the log back.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl io::Write for Capture {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Capture;
    fn make_writer(&'a self) -> Capture {
        self.clone()
    }
}

// Single-threaded runtime: the subscriber installed with set_default is thread-local, and every
// task of this test runs on this thread.
#[tokio::test(flavor = "current_thread")]
async fn the_log_holds_no_token_no_blob_and_no_ip() {
    let capture = Capture::default();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(capture.clone())
        .with_ansi(false)
        .with_max_level(tracing::Level::TRACE)
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);

    let token = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    let blob = STANDARD.encode(b"a very secret ciphertext 0123456789");
    let ip = "203.0.113.77";

    let ok = state(100, None);
    let gone = state(100, Some(ProviderError::TokenGone));
    let down = state(100, Some(ProviderError::Unavailable));
    let limited = state(0, None);
    let bad = json("android", "tok-en", "%%%");
    post(&ok, ip, json("ios", token, &blob)).await;
    post(&ok, ip, json("android", "ab:cd_ef", &blob)).await;
    post(&gone, ip, json("ios", token, &blob)).await;
    post(&down, ip, json("ios", token, &blob)).await;
    post(&limited, ip, json("ios", token, &blob)).await;
    post(&ok, ip, bad).await;

    let log = String::from_utf8(capture.0.lock().unwrap().clone()).unwrap();
    assert!(log.contains("status=200"), "the status is logged: {log}");
    assert!(log.contains("platform"), "the platform is logged: {log}");
    for secret in [token, "ab:cd_ef", blob.as_str(), "203.0.113", ip, "tok-en"] {
        assert!(!log.contains(secret), "log leaks {secret:?}: {log}");
    }
}
