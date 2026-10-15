//! The wire types shared by the desktop, the mobile app and the servers.
//!
//! Only what both ends of a connection must agree on lives here: the pairing
//! ticket a QR code carries, the pairing exchange, the handshake, and the frame
//! format everything travels in. Nothing in this crate decides anything — it
//! has no clock, no device list and no policy — so a change to it is a change
//! to a contract and is read as one.
//!
//! The command bridge (`ClientMsg`, `ServerMsg`, `BridgeError`) is the next
//! stage's. It will travel as the same length-prefixed postcard frames, on one
//! bi-directional stream per connection, and is deliberately absent here.

mod frame;
mod ticket;

pub use frame::{decode_frame, encode_frame, read_frame, write_frame, FrameError, MAX_FRAME};
pub use iroh::PublicKey;
pub use ticket::{PairingTicket, TicketError, TICKET_PREFIX};

use serde::{Deserialize, Serialize};

/// The ALPN both ends present. The trailing `1` is the transport's generation,
/// not the protocol major: the major is negotiated inside the connection, by
/// `Hello`, so that a desktop can still answer an older phone politely.
pub const ALPN: &[u8] = b"smetana/remote/1";

/// Application close codes, sent as the error code of a QUIC connection close.
/// They are part of the protocol: a phone reads them to tell a refused pairing
/// from a revoked device from an unsupported version.
///
/// A phone whose key is already on the device list and which sends a
/// `PairRequest` is answered `CLOSE_PROTOCOL`, because the desktop expects a
/// `Hello` from a known key. It should then connect again and send `Hello`.
pub const CLOSE_DONE: u32 = 0;
/// The key is not on the list and no pairing is open (or the desktop is busy).
pub const CLOSE_UNKNOWN: u32 = 1;
/// The device was removed from the list while connected.
pub const CLOSE_REVOKED: u32 = 2;
/// The `Hello` named a protocol major this desktop does not serve.
pub const CLOSE_VERSION: u32 = 3;
/// The first frame was not the message the connection's state expects.
pub const CLOSE_PROTOCOL: u32 = 4;

/// Length of the one-time pairing secret and of the push key, in bytes.
pub const SECRET_LEN: usize = 32;

/// What the phone is, as far as the desktop's device list is concerned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Ios,
    Android,
}

/// The first thing a phone says on a pairing connection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairRequest {
    pub secret: [u8; SECRET_LEN],
    pub device_name: String,
    pub platform: Platform,
    pub push_token: Option<String>,
    /// The X25519 public key push blobs are encrypted to. The iroh key signs
    /// and is not reused for encryption.
    pub push_key: [u8; SECRET_LEN],
    pub app_version: String,
}

/// The desktop's answer to a `PairRequest`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PairReply {
    Accepted,
    Rejected { reason: String },
}

/// The first message of an ordinary connection, sent by the phone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub app_version: String,
    pub capabilities: Vec<String>,
}

/// The desktop's answer to a `Hello`. It carries the two versions of its own
/// release that let a desktop push an update onto a phone without a major.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloReply {
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub app_version: String,
    pub capabilities: Vec<String>,
    pub min_mobile_version: String,
    pub recommended_mobile_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_is_lowercase_in_json_and_survives_postcard() {
        assert_eq!(serde_json::to_string(&Platform::Ios).unwrap(), "\"ios\"");
        assert_eq!(serde_json::from_str::<Platform>("\"android\"").unwrap(), Platform::Android);
        let bytes = postcard::to_stdvec(&Platform::Android).unwrap();
        assert_eq!(postcard::from_bytes::<Platform>(&bytes).unwrap(), Platform::Android);
    }

    #[test]
    fn a_pair_request_round_trips_through_a_frame() {
        let request = PairRequest {
            secret: [7; SECRET_LEN],
            device_name: "Pixel 8".into(),
            platform: Platform::Android,
            push_token: Some("token".into()),
            push_key: [9; SECRET_LEN],
            app_version: "1.0.0".into(),
        };
        let frame = encode_frame(&request).unwrap();
        let (decoded, rest): (PairRequest, _) = decode_frame(&frame).unwrap();
        assert_eq!(decoded, request);
        assert!(rest.is_empty());
    }

    #[test]
    fn close_codes_are_distinct() {
        let codes = [CLOSE_DONE, CLOSE_UNKNOWN, CLOSE_REVOKED, CLOSE_VERSION, CLOSE_PROTOCOL];
        for (i, a) in codes.iter().enumerate() {
            for b in &codes[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn replies_round_trip() {
        for reply in [PairReply::Accepted, PairReply::Rejected { reason: "no".into() }] {
            let frame = encode_frame(&reply).unwrap();
            assert_eq!(decode_frame::<PairReply>(&frame).unwrap().0, reply);
        }
        let hello = Hello {
            protocol_major: 1,
            protocol_minor: 0,
            app_version: "1.0.0".into(),
            capabilities: vec!["board".into()],
        };
        let frame = encode_frame(&hello).unwrap();
        assert_eq!(decode_frame::<Hello>(&frame).unwrap().0, hello);
    }
}
