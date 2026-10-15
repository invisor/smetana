//! Configuration, from environment variables only. Nothing is read from or written to a database.

use std::net::SocketAddr;
use std::path::PathBuf;

pub struct ApnsConfig {
    pub key_pem: Vec<u8>,
    pub key_id: String,
    pub team_id: String,
    pub bundle_id: String,
    pub sandbox: bool,
}

pub struct FcmConfig {
    pub service_account_json: Vec<u8>,
}

pub struct Config {
    pub bind: SocketAddr,
    pub rate_limit_per_minute: u32,
    pub trust_forwarded_for: bool,
    pub apns: Option<ApnsConfig>,
    pub fcm: Option<FcmConfig>,
}

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

fn required(name: &str) -> Result<String, String> {
    var(name).ok_or_else(|| format!("{name} is required when APNS_KEY_PATH is set"))
}

fn flag(name: &str) -> bool {
    matches!(var(name).as_deref(), Some("1" | "true" | "yes"))
}

impl Config {
    pub fn from_env() -> Result<Config, String> {
        let bind = var("PUSH_BIND")
            .unwrap_or_else(|| "0.0.0.0:8080".into())
            .parse()
            .map_err(|_| "PUSH_BIND is not a socket address".to_string())?;
        let rate_limit_per_minute = match var("PUSH_RATE_LIMIT_PER_MINUTE") {
            Some(v) => v
                .parse()
                .map_err(|_| "PUSH_RATE_LIMIT_PER_MINUTE is not a number".to_string())?,
            None => 30,
        };

        let apns = match var("APNS_KEY_PATH") {
            None => None,
            Some(path) => Some(ApnsConfig {
                key_pem: read(&PathBuf::from(path), "APNS_KEY_PATH")?,
                key_id: required("APNS_KEY_ID")?,
                team_id: required("APNS_TEAM_ID")?,
                bundle_id: required("APNS_BUNDLE_ID")?,
                sandbox: flag("APNS_SANDBOX"),
            }),
        };
        let fcm = match var("FCM_SERVICE_ACCOUNT_PATH") {
            None => None,
            Some(path) => Some(FcmConfig {
                service_account_json: read(&PathBuf::from(path), "FCM_SERVICE_ACCOUNT_PATH")?,
            }),
        };

        Ok(Config {
            bind,
            rate_limit_per_minute,
            trust_forwarded_for: flag("PUSH_TRUST_FORWARDED_FOR"),
            apns,
            fcm,
        })
    }
}

fn read(path: &std::path::Path, name: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("cannot read the file named by {name}: {}", e.kind()))
}
