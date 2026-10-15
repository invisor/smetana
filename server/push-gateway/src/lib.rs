//! A push gateway that stores nothing and reads nothing.
//!
//! One route, `POST /v1/push`. The body names a platform, a device token and an encrypted blob;
//! the blob is handed to APNs or FCM exactly as it arrived. The gateway holds no key that could
//! open it. Per-IP rate limiting lives in memory and dies with the process.
//!
//! Logging is a decision, not a default: a line carries a status and a platform and nothing else.
//! No token, no blob, no address, and no provider error text (a `reqwest` error prints its URL,
//! and an APNs URL contains the device token). There is deliberately no request-tracing layer.

pub mod config;
pub mod providers;

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State as AxumState};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::Router;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Deserialize;

use providers::{Provider, ProviderError, PushMessage};

/// The largest request body, in bytes.
pub const MAX_BODY: usize = 4 * 1024;

/// Fixed-window counter per address, in memory. A window is dropped when it expires, so the map
/// holds only addresses seen within the last window.
pub struct RateLimiter {
    limit: u32,
    window: Duration,
    seen: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

impl RateLimiter {
    pub fn new(limit: u32, window: Duration) -> RateLimiter {
        RateLimiter {
            limit,
            window,
            seen: Mutex::new(HashMap::new()),
        }
    }

    /// Counts one request from `ip`; false when it is over the limit.
    pub fn allow(&self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let Ok(mut seen) = self.seen.lock() else {
            return false;
        };
        if seen.len() > 10_000 {
            seen.retain(|_, (start, _)| now.duration_since(*start) < self.window);
        }
        let entry = seen.entry(ip).or_insert((now, 0));
        if now.duration_since(entry.0) >= self.window {
            *entry = (now, 0);
        }
        entry.1 += 1;
        entry.1 <= self.limit
    }
}

#[derive(Clone)]
pub struct State {
    pub apns: Option<Arc<dyn Provider>>,
    pub fcm: Option<Arc<dyn Provider>>,
    pub limiter: Arc<RateLimiter>,
    /// Behind a reverse proxy the socket peer is the proxy; the real client is the last hop it
    /// appended to `X-Forwarded-For`. Only honoured when the operator says a proxy is there.
    pub trust_forwarded_for: bool,
}

pub fn app(state: State) -> Router {
    Router::new()
        .route("/v1/push", post(push))
        .with_state(state)
}

#[derive(Deserialize)]
struct PushRequest {
    platform: String,
    token: String,
    blob: String,
}

fn client_ip(state: &State, headers: &HeaderMap, peer: IpAddr) -> IpAddr {
    if state.trust_forwarded_for {
        if let Some(ip) = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.rsplit(',').next())
            .and_then(|v| v.trim().parse().ok())
        {
            return ip;
        }
    }
    peer
}

/// Device tokens go into a URL path (APNs), so only the characters real tokens use are accepted.
fn token_ok(platform: &str, token: &str) -> bool {
    if token.is_empty() || token.len() > 1024 {
        return false;
    }
    match platform {
        "ios" => token.bytes().all(|b| b.is_ascii_hexdigit()),
        _ => token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b':' | b'.')),
    }
}

fn reply(status: StatusCode, platform: &str) -> StatusCode {
    tracing::info!(status = status.as_u16(), platform, "push");
    status
}

async fn push(
    AxumState(state): AxumState<State>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    request: Request<Body>,
) -> StatusCode {
    let ip = client_ip(&state, request.headers(), peer.ip());
    if !state.limiter.allow(ip) {
        return reply(StatusCode::TOO_MANY_REQUESTS, "unknown");
    }

    // An oversized body is a 400, as specified, and it is cut off at the limit rather than
    // buffered: to_bytes stops reading one byte past it.
    let Ok(bytes) = axum::body::to_bytes(request.into_body(), MAX_BODY).await else {
        return reply(StatusCode::BAD_REQUEST, "unknown");
    };
    let Ok(body) = serde_json::from_slice::<PushRequest>(&bytes) else {
        return reply(StatusCode::BAD_REQUEST, "unknown");
    };
    let platform = match body.platform.as_str() {
        "ios" => "ios",
        "android" => "android",
        _ => return reply(StatusCode::BAD_REQUEST, "unknown"),
    };
    if !token_ok(platform, &body.token)
        || body.blob.is_empty()
        || STANDARD.decode(&body.blob).is_err()
    {
        return reply(StatusCode::BAD_REQUEST, platform);
    }

    let provider = if platform == "ios" { &state.apns } else { &state.fcm };
    let Some(provider) = provider else {
        return reply(StatusCode::SERVICE_UNAVAILABLE, platform);
    };
    let message = PushMessage {
        token: body.token,
        blob: body.blob,
    };
    match provider.send(&message).await {
        Ok(()) => reply(StatusCode::OK, platform),
        Err(ProviderError::TokenGone) => reply(StatusCode::GONE, platform),
        Err(ProviderError::Unavailable) => reply(StatusCode::BAD_GATEWAY, platform),
    }
}
