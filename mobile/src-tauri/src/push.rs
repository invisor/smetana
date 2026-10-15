//! Opening a push blob on the phone.
//!
//! The desktop seals a short JSON message to the phone's X25519 push key
//! (`remote_protocol::push::seal`); the gateway carries it as `base64(postcard(blob))`
//! inside an APNs or FCM payload. Two code paths open it:
//!
//! - **Android**: the messaging service (`PushService.kt`) calls `open_wire` through JNI
//!   (`android.rs`), with the secret it read from the Keystore-backed vault.
//! - **iOS**: the notification service extension (`gen/apple/NotificationService/`)
//!   cannot afford the app's whole Rust core in its memory budget, so it carries a
//!   Swift implementation of the same algorithm (CryptoKit plus a small BLAKE3).
//!   That one is checked against the vector in `test-vectors/push.json`, which
//!   this module's tests also open: both sides read the same bytes.

use remote_protocol::push::{open, PushBlob};

/// What a push carries once opened.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct PushMessage {
    pub title: String,
    pub body: String,
    /// Where tapping the notification goes. The screens are routed by a later stage;
    /// this stage only hands it to the front end.
    #[serde(default)]
    pub target: Option<String>,
}

/// The plaintext of a blob, or `None` when it does not open (wrong key, damaged,
/// not a blob at all). One answer on purpose, as `remote_protocol::push` gives one.
pub fn open_wire(secret: &[u8; 32], wire: &str) -> Option<Vec<u8>> {
    let blob = PushBlob::from_wire(wire.trim()).ok()?;
    open(secret, &blob).ok()
}

pub fn open_message(secret: &[u8; 32], wire: &str) -> Option<PushMessage> {
    serde_json::from_slice(&open_wire(secret, wire)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use remote_protocol::push::{public_key, seal};

    const VECTOR: &str = include_str!("../../test-vectors/push.json");

    fn vector() -> serde_json::Value {
        serde_json::from_str(VECTOR).unwrap()
    }

    #[test]
    fn a_blob_sealed_by_the_desktops_code_opens_here() {
        let secret = [11u8; 32];
        let blob = seal(&public_key(&secret), br#"{"title":"Run finished","body":"5 tasks","target":"run/7"}"#);
        let message = open_message(&secret, &blob.to_wire()).unwrap();
        assert_eq!(message.title, "Run finished");
        assert_eq!(message.target.as_deref(), Some("run/7"));
    }

    #[test]
    fn the_committed_vector_opens_with_the_android_path_and_with_the_desktops_open() {
        let v = vector();
        let secret: [u8; 32] = data_encoding::HEXLOWER.decode(v["secret"].as_str().unwrap().as_bytes()).unwrap().try_into().unwrap();
        let wire = v["wire"].as_str().unwrap();
        let expected = v["plaintext"].as_str().unwrap().as_bytes();
        assert_eq!(public_key(&secret).to_vec(), data_encoding::HEXLOWER.decode(v["public"].as_str().unwrap().as_bytes()).unwrap());
        assert_eq!(open_wire(&secret, wire).as_deref(), Some(expected));
        let blob = PushBlob::from_wire(wire).unwrap();
        assert_eq!(open(&secret, &blob).unwrap(), expected);
    }

    #[test]
    fn a_wrong_key_or_a_damaged_blob_is_none() {
        let secret = [11u8; 32];
        let wire = seal(&public_key(&secret), b"{}").to_wire();
        assert!(open_wire(&[12u8; 32], &wire).is_none());
        assert!(open_wire(&secret, "!!!not base64").is_none());
        assert!(open_wire(&secret, &wire[..wire.len() - 8]).is_none());
        assert!(open_wire(&secret, "").is_none());
    }

    /// Writes the vector the Swift side is checked against. Run with
    /// `SMETANA_WRITE_VECTOR=1 cargo test write_the_push_vector`; the committed
    /// file is not rewritten otherwise, because a vector that changes with every
    /// run proves nothing about compatibility.
    #[test]
    fn write_the_push_vector() {
        if std::env::var("SMETANA_WRITE_VECTOR").is_err() {
            return;
        }
        let secret = [7u8; 32];
        let plaintext = r#"{"title":"Run in smetana finished","body":"5 tasks, 1 waiting for you","target":"run/12"}"#;
        let blob = seal(&public_key(&secret), plaintext.as_bytes());
        let json = serde_json::json!({
            "secret": data_encoding::HEXLOWER.encode(&secret),
            "public": data_encoding::HEXLOWER.encode(&public_key(&secret)),
            "plaintext": plaintext,
            "wire": blob.to_wire(),
        });
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../test-vectors/push.json");
        std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_string_pretty(&json).unwrap() + "\n").unwrap();
    }
}
