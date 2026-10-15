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

/// The longest blob text accepted, in characters of base64. Apple caps a whole payload at 4096
/// bytes and Google caps `data` at 4 KiB; the wrapper around the blob is under 100 bytes in either,
/// so this leaves room for it. A longer blob would be rejected by the provider anyway, after the
/// gateway had spent a request on it.
pub const MAX_BLOB: usize = 3500;

/// How many distinct clients the limiter remembers at once.
pub const DEFAULT_MAX_CLIENTS: usize = 100_000;

/// The smallest gap between two sweeps forced by a full table.
const MIN_FORCED_SWEEP: Duration = Duration::from_secs(1);

/// What a client is counted as: an IPv4 address in full, an IPv6 address by its /64, because a
/// single subscriber is routinely handed a whole /64 and could otherwise mint a fresh budget with
/// every address in it.
fn client_key(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => {
            let mut octets = v6.octets();
            octets[8..].fill(0);
            IpAddr::V6(octets.into())
        }
    }
}

struct Table {
    seen: HashMap<IpAddr, (Instant, u32)>,
    last_sweep: Instant,
}

impl Table {
    fn sweep(&mut self, now: Instant, window: Duration) {
        self.seen
            .retain(|_, (start, _)| now.duration_since(*start) < window);
        self.last_sweep = now;
    }
}

/// Fixed-window counter per client, in memory only.
///
/// The table is bounded: once it holds `max_clients` live entries a client it has not seen is
/// refused, which fails closed (a flood of spoofed or rotating sources can lock out newcomers for
/// the length of a window, but cannot grow the process). Expired entries are swept at most once
/// per window, or when a full table is hit, at most once a second, never on every request.
pub struct RateLimiter {
    limit: u32,
    window: Duration,
    max_clients: usize,
    table: Mutex<Table>,
}

impl RateLimiter {
    pub fn new(limit: u32, window: Duration) -> RateLimiter {
        RateLimiter::with_capacity(limit, window, DEFAULT_MAX_CLIENTS)
    }

    pub fn with_capacity(limit: u32, window: Duration, max_clients: usize) -> RateLimiter {
        RateLimiter {
            limit,
            window,
            max_clients,
            table: Mutex::new(Table {
                seen: HashMap::new(),
                last_sweep: Instant::now(),
            }),
        }
    }

    /// Counts one request from `ip`; false when it is over the limit or the table is full.
    pub fn allow(&self, ip: IpAddr) -> bool {
        let key = client_key(ip);
        let now = Instant::now();
        let Ok(mut table) = self.table.lock() else {
            return false;
        };
        if now.duration_since(table.last_sweep) >= self.window {
            table.sweep(now, self.window);
        }
        if !table.seen.contains_key(&key) && table.seen.len() >= self.max_clients {
            if now.duration_since(table.last_sweep) >= MIN_FORCED_SWEEP {
                table.sweep(now, self.window);
            }
            if table.seen.len() >= self.max_clients {
                return false;
            }
        }
        let entry = table.seen.entry(key).or_insert((now, 0));
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
    /// How long a client gets to deliver its body before the request is dropped with 408.
    pub body_timeout: Duration,
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
    let read = tokio::time::timeout(
        state.body_timeout,
        axum::body::to_bytes(request.into_body(), MAX_BODY),
    )
    .await;
    let Ok(read) = read else {
        return reply(StatusCode::REQUEST_TIMEOUT, "unknown");
    };
    let Ok(bytes) = read else {
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
        || body.blob.len() > MAX_BLOB
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
