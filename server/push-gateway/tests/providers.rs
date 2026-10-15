//! The real APNs and FCM clients against local stand-in servers: what actually goes over the wire.
//!
//! The APNs stand-in is spoken to over HTTP/2 with prior knowledge (cleartext h2c), which is the
//! protocol production uses over TLS with ALPN, so the version is asserted too. The keys are
//! generated here, per run; nothing in the repository is a credential.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request};
use axum::http::{HeaderMap, StatusCode, Version};
use axum::routing::any;
use axum::Router;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use push_gateway::config::{ApnsConfig, FcmConfig};
use push_gateway::providers::apns::Apns;
use push_gateway::providers::fcm::Fcm;
use push_gateway::{app, RateLimiter, State, MAX_BLOB, MAX_BODY};
use rcgen::KeyPair;
use serde_json::Value;
use tower::ServiceExt;

struct Seen {
    path: String,
    version: Version,
    headers: HeaderMap,
    body: Vec<u8>,
}

type Log = Arc<Mutex<Vec<Seen>>>;

/// Starts a server that records every request except `/token` (the OAuth exchange, answered with
/// a fixed access token and recorded too) and answers the rest with `status`.
async fn fake(status: u16) -> (String, Log) {
    let log: Log = Arc::default();
    let seen = log.clone();
    let router = Router::new().fallback(any(move |request: Request| {
        let seen = seen.clone();
        async move {
            let (parts, body) = request.into_parts();
            let body = axum::body::to_bytes(body, 1 << 20).await.unwrap().to_vec();
            let path = parts.uri.path().to_string();
            let is_token = path == "/token";
            seen.lock().unwrap().push(Seen {
                path,
                version: parts.version,
                headers: parts.headers,
                body,
            });
            if is_token {
                (
                    StatusCode::OK,
                    r#"{"access_token":"test-access-token","expires_in":3600}"#.to_string(),
                )
            } else {
                (StatusCode::from_u16(status).unwrap(), String::new())
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (base, log)
}

fn h2c_client() -> reqwest::Client {
    reqwest::Client::builder()
        .http2_prior_knowledge()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn ec_pair() -> (String, String) {
    let pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).unwrap();
    (pair.serialize_pem(), pair.public_key_pem())
}

fn rsa_pair() -> (String, String) {
    let pair = KeyPair::generate_rsa_for(&rcgen::PKCS_RSA_SHA256, rcgen::RsaKeySize::_2048).unwrap();
    (pair.serialize_pem(), pair.public_key_pem())
}

const IOS_TOKEN: &str = "00aa11bb22cc33dd44ee55ff66778899aabbccddeeff00112233445566778899";

fn apns_state(base: &str, key_pem: String) -> State {
    let apns = Apns::with_base_url(
        h2c_client(),
        ApnsConfig {
            key_pem: key_pem.into_bytes(),
            key_id: "KEYID12345".into(),
            team_id: "TEAMID6789".into(),
            bundle_id: "org.example.smetana".into(),
            sandbox: false,
        },
        base.to_string(),
    );
    State {
        apns: Some(Arc::new(apns)),
        fcm: None,
        limiter: Arc::new(RateLimiter::new(100, Duration::from_secs(60))),
        trust_forwarded_for: false,
        body_timeout: Duration::from_secs(5),
    }
}

fn fcm_state(base: &str, private_key: String) -> State {
    let account = serde_json::json!({
        "project_id": "proj-1",
        "client_email": "sender@proj-1.iam.example",
        "private_key": private_key,
        "token_uri": format!("{base}/token"),
    });
    let fcm = Fcm::with_base_url(
        h2c_client(),
        FcmConfig {
            service_account_json: account.to_string().into_bytes(),
        },
        base.to_string(),
    );
    State {
        apns: None,
        fcm: Some(Arc::new(fcm)),
        limiter: Arc::new(RateLimiter::new(100, Duration::from_secs(60))),
        trust_forwarded_for: false,
        body_timeout: Duration::from_secs(5),
    }
}

async fn post(state: &State, platform: &str, token: &str, blob: &str) -> StatusCode {
    let body = serde_json::json!({ "platform": platform, "token": token, "blob": blob });
    let mut request = Request::post("/v1/push")
        .body(Body::from(body.to_string()))
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo("10.0.0.1:1".parse::<SocketAddr>().unwrap()));
    app(state.clone()).oneshot(request).await.unwrap().status()
}

#[tokio::test]
async fn apns_request_carries_the_blob_the_neutral_alert_and_a_signed_es256_token() {
    let (base, log) = fake(200).await;
    let (private, public) = ec_pair();
    let state = apns_state(&base, private);
    let blob = STANDARD.encode((0..=255u8).cycle().take(700).collect::<Vec<_>>());

    assert_eq!(post(&state, "ios", IOS_TOKEN, &blob).await, StatusCode::OK);

    let log = log.lock().unwrap();
    assert_eq!(log.len(), 1);
    let seen = &log[0];
    assert_eq!(seen.path, format!("/3/device/{IOS_TOKEN}"));
    assert_eq!(seen.version, Version::HTTP_2);
    assert_eq!(seen.headers["apns-topic"], "org.example.smetana");
    assert_eq!(seen.headers["apns-push-type"], "alert");

    let body: Value = serde_json::from_slice(&seen.body).unwrap();
    assert_eq!(body["blob"], blob.as_str(), "the blob is forwarded as received");
    assert_eq!(body["aps"]["mutable-content"], 1);
    assert_eq!(body["aps"]["alert"]["title"], "Smetana");

    let auth = seen.headers["authorization"].to_str().unwrap();
    let jwt = auth.strip_prefix("bearer ").or_else(|| auth.strip_prefix("Bearer ")).unwrap();
    let header = jsonwebtoken::decode_header(jwt).unwrap();
    assert_eq!(header.alg, Algorithm::ES256);
    assert_eq!(header.kid.as_deref(), Some("KEYID12345"));
    let mut validation = Validation::new(Algorithm::ES256);
    validation.required_spec_claims.clear();
    validation.validate_exp = false;
    let claims = jsonwebtoken::decode::<Value>(
        jwt,
        &DecodingKey::from_ec_pem(public.as_bytes()).unwrap(),
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(claims["iss"], "TEAMID6789");
    assert!(claims["iat"].as_u64().unwrap() > 1_700_000_000);
}

#[tokio::test]
async fn apns_410_becomes_410_and_other_failures_502() {
    let (base, _) = fake(410).await;
    let (private, _) = ec_pair();
    let state = apns_state(&base, private.clone());
    assert_eq!(post(&state, "ios", IOS_TOKEN, "aGk=").await, StatusCode::GONE);

    let (base, _) = fake(500).await;
    let state = apns_state(&base, private);
    assert_eq!(post(&state, "ios", IOS_TOKEN, "aGk=").await, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn fcm_exchanges_a_jwt_bearer_grant_then_sends_the_blob_as_data() {
    let (base, log) = fake(200).await;
    let (private, public) = rsa_pair();
    let state = fcm_state(&base, private);
    let blob = STANDARD.encode((0..=255u8).rev().cycle().take(900).collect::<Vec<_>>());

    assert_eq!(post(&state, "android", "dev-token_1:APA.x", &blob).await, StatusCode::OK);
    // A second push reuses the cached access token: no second exchange.
    assert_eq!(post(&state, "android", "dev-token_1:APA.x", &blob).await, StatusCode::OK);

    let log = log.lock().unwrap();
    let paths: Vec<_> = log.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(
        paths,
        ["/token", "/v1/projects/proj-1/messages:send", "/v1/projects/proj-1/messages:send"]
    );

    let form: Vec<(String, String)> = std::str::from_utf8(&log[0].body)
        .unwrap()
        .split('&')
        .map(|kv| {
            let (k, v) = kv.split_once('=').unwrap();
            (k.to_string(), v.to_string())
        })
        .collect();
    let grant = form.iter().find(|(k, _)| k == "grant_type").unwrap();
    assert_eq!(grant.1, "urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Ajwt-bearer");
    let assertion = &form.iter().find(|(k, _)| k == "assertion").unwrap().1;
    assert_eq!(jsonwebtoken::decode_header(assertion).unwrap().alg, Algorithm::RS256);
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[format!("{base}/token")]);
    let claims = jsonwebtoken::decode::<Value>(
        assertion,
        &DecodingKey::from_rsa_pem(public.as_bytes()).unwrap(),
        &validation,
    )
    .unwrap()
    .claims;
    assert_eq!(claims["iss"], "sender@proj-1.iam.example");
    assert_eq!(claims["scope"], "https://www.googleapis.com/auth/firebase.messaging");

    let send = &log[1];
    assert_eq!(send.headers["authorization"], "Bearer test-access-token");
    let body: Value = serde_json::from_slice(&send.body).unwrap();
    assert_eq!(body["message"]["token"], "dev-token_1:APA.x");
    assert_eq!(body["message"]["data"]["blob"], blob.as_str());
}

#[tokio::test]
async fn fcm_404_becomes_410() {
    let (base, _) = fake(404).await;
    let (private, _) = rsa_pair();
    let state = fcm_state(&base, private);
    assert_eq!(post(&state, "android", "tok", "aGk=").await, StatusCode::GONE);
}

#[tokio::test]
async fn the_longest_accepted_blob_fits_both_providers_limit() {
    let blob = "A".repeat(MAX_BLOB);
    let apns = push_gateway::providers::apns::payload(&blob).to_string();
    // Google's 4 KiB limit is on the `data` map, not on the token around it.
    let fcm = push_gateway::providers::fcm::payload("t", &blob)["message"]["data"].to_string();
    assert!(apns.len() <= 4096, "apns payload is {} bytes", apns.len());
    assert!(fcm.len() <= 4096, "fcm payload is {} bytes", fcm.len());
    assert!(MAX_BLOB < MAX_BODY);
}
