//! FCM HTTP v1 with a service account: a signed JWT is exchanged for an OAuth access token,
//! which is cached in memory until shortly before it expires.

use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, EncodingKey, Header};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use super::{Provider, ProviderError, PushMessage, SendFuture};
use crate::config::FcmConfig;

const SCOPE: &str = "https://www.googleapis.com/auth/firebase.messaging";

#[derive(Deserialize)]
struct ServiceAccount {
    project_id: String,
    client_email: String,
    private_key: String,
    token_uri: String,
}

#[derive(Deserialize)]
struct TokenReply {
    access_token: String,
    expires_in: u64,
}

pub struct Fcm {
    http: reqwest::Client,
    account: Option<ServiceAccount>,
    base_url: String,
    cached: Mutex<Option<(Instant, String)>>,
}

impl Fcm {
    pub fn new(http: reqwest::Client, config: FcmConfig) -> Fcm {
        Fcm::with_base_url(http, config, "https://fcm.googleapis.com".to_string())
    }

    /// As `new`, sending to another server (a stand-in, in tests). The OAuth exchange goes to the
    /// service account's own `token_uri`. `base_url` has no trailing slash.
    pub fn with_base_url(http: reqwest::Client, config: FcmConfig, base_url: String) -> Fcm {
        let account = serde_json::from_slice::<ServiceAccount>(&config.service_account_json).ok();
        if account.is_none() {
            tracing::error!("the FCM service account could not be parsed; Android pushes will fail");
        }
        Fcm {
            http,
            account,
            base_url,
            cached: Mutex::new(None),
        }
    }

    async fn access_token(&self, account: &ServiceAccount) -> Option<String> {
        let mut cached = self.cached.lock().await;
        if let Some((expires, token)) = cached.as_ref() {
            if Instant::now() < *expires {
                return Some(token.clone());
            }
        }
        let key = EncodingKey::from_rsa_pem(account.private_key.as_bytes()).ok()?;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()?
            .as_secs();
        let assertion = jsonwebtoken::encode(
            &Header::new(Algorithm::RS256),
            &json!({
                "iss": account.client_email,
                "scope": SCOPE,
                "aud": account.token_uri,
                "iat": now,
                "exp": now + 3600,
            }),
            &key,
        )
        .ok()?;
        let reply: TokenReply = self
            .http
            .post(&account.token_uri)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", assertion.as_str()),
            ])
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()?;
        let expires = Instant::now() + Duration::from_secs(reply.expires_in.saturating_sub(120));
        *cached = Some((expires, reply.access_token.clone()));
        Some(reply.access_token)
    }
}

/// A data-only message: the phone's app wakes, decrypts the blob and raises its own notification.
pub fn payload(token: &str, blob: &str) -> serde_json::Value {
    json!({
        "message": {
            "token": token,
            "data": { "blob": blob },
            "android": { "priority": "high" },
        }
    })
}

impl Provider for Fcm {
    fn send<'a>(&'a self, message: &'a PushMessage) -> SendFuture<'a> {
        Box::pin(async move {
            let account = self.account.as_ref().ok_or(ProviderError::Unavailable)?;
            let access = self
                .access_token(account)
                .await
                .ok_or(ProviderError::Unavailable)?;
            let url = format!(
                "{}/v1/projects/{}/messages:send",
                self.base_url, account.project_id
            );
            let response = self
                .http
                .post(url)
                .bearer_auth(access)
                .json(&payload(&message.token, &message.blob))
                .send()
                .await
                .map_err(|_| ProviderError::Unavailable)?;
            match response.status().as_u16() {
                200 => Ok(()),
                404 => Err(ProviderError::TokenGone),
                _ => Err(ProviderError::Unavailable),
            }
        })
    }
}
