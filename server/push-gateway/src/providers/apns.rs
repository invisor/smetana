//! APNs over HTTP/2 with token-based authentication (a `.p8` key).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde_json::json;

use super::{Provider, ProviderError, PushMessage, SendFuture};
use crate::config::ApnsConfig;

/// Apple rejects a provider token older than an hour and one refreshed more often than every
/// twenty minutes; forty is in the middle.
const TOKEN_LIFETIME: Duration = Duration::from_secs(40 * 60);

pub struct Apns {
    http: reqwest::Client,
    key: Option<EncodingKey>,
    key_id: String,
    team_id: String,
    bundle_id: String,
    host: String,
    cached: Mutex<Option<(Instant, String)>>,
}

impl Apns {
    pub fn new(http: reqwest::Client, config: ApnsConfig) -> Apns {
        let host = if config.sandbox {
            "https://api.sandbox.push.apple.com"
        } else {
            "https://api.push.apple.com"
        };
        Apns::with_base_url(http, config, host.to_string())
    }

    /// As `new`, against another server (a stand-in, in tests). `base_url` has no trailing slash.
    pub fn with_base_url(http: reqwest::Client, config: ApnsConfig, base_url: String) -> Apns {
        let key = EncodingKey::from_ec_pem(&config.key_pem).ok();
        if key.is_none() {
            tracing::error!("the APNs key could not be parsed; iOS pushes will fail");
        }
        Apns {
            http,
            key,
            key_id: config.key_id,
            team_id: config.team_id,
            bundle_id: config.bundle_id,
            host: base_url,
            cached: Mutex::new(None),
        }
    }

    fn bearer(&self) -> Option<String> {
        let mut cached = self.cached.lock().ok()?;
        if let Some((at, jwt)) = cached.as_ref() {
            if at.elapsed() < TOKEN_LIFETIME {
                return Some(jwt.clone());
            }
        }
        let mut header = Header::new(Algorithm::ES256);
        header.kid = Some(self.key_id.clone());
        let iat = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        let jwt = jsonwebtoken::encode(
            &header,
            &json!({ "iss": self.team_id, "iat": iat }),
            self.key.as_ref()?,
        )
        .ok()?;
        *cached = Some((Instant::now(), jwt.clone()));
        Some(jwt)
    }
}

/// The notification body. `alert` is neutral; the phone's Notification Service Extension decrypts
/// the blob and replaces the text, which `mutable-content` is what allows.
pub fn payload(blob: &str) -> serde_json::Value {
    json!({
        "aps": { "alert": { "title": "Smetana" }, "mutable-content": 1 },
        "blob": blob,
    })
}

impl Provider for Apns {
    fn send<'a>(&'a self, message: &'a PushMessage) -> SendFuture<'a> {
        Box::pin(async move {
            let jwt = self.bearer().ok_or(ProviderError::Unavailable)?;
            let url = format!("{}/3/device/{}", self.host, message.token);
            let response = self
                .http
                .post(url)
                .bearer_auth(jwt)
                .header("apns-topic", &self.bundle_id)
                .header("apns-push-type", "alert")
                .header("apns-priority", "10")
                .json(&payload(&message.blob))
                .send()
                .await
                .map_err(|_| ProviderError::Unavailable)?;
            match response.status().as_u16() {
                200 => Ok(()),
                410 => Err(ProviderError::TokenGone),
                _ => Err(ProviderError::Unavailable),
            }
        })
    }
}
