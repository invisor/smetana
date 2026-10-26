//! The wire types shared by the desktop, the mobile app and the servers.
//!
//! Only what both ends of a connection must agree on lives here: the pairing
//! ticket a QR code carries, the pairing exchange, the handshake, and the frame
//! format everything travels in. Nothing in this crate decides anything — it
//! has no clock, no device list and no policy — so a change to it is a change
//! to a contract and is read as one.
//!
//! The command bridge (`ClientMsg`, `ServerMsg`, `BridgeError`), its allowlist
//! and the contract that pins the shape of everything on that list travel as
//! the same length-prefixed postcard frames, on one bi-directional stream per
//! connection.

mod bridge;
mod frame;
pub mod push;
mod ticket;

pub use bridge::{
    capabilities, capabilities_for, command_contract_path, event_contract_path, is_allowed_command,
    is_allowed_event, is_command_allowed_for, is_event_allowed_for, BridgeError, ClientMsg, ServerMsg,
    ALLOWED_COMMANDS, ALLOWED_EVENTS, SERVER_COMMANDS, SERVER_EVENTS, SERVER_ONLY_COMMANDS,
};
pub use frame::{
    decode_frame, encode_frame, read_frame, read_frame_bytes, write_frame, FrameError, MAX_FRAME,
};
pub use iroh::PublicKey;
pub use ticket::{PairingTicket, TicketError, DESKTOP_TICKET_PREFIX, TICKET_PREFIX};

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
/// The phone did not read events as fast as the desktop produced them. It should
/// connect again and read the snapshots again, as after any dropped connection.
pub const CLOSE_SLOW: u32 = 5;

/// Length of the one-time pairing secret and of the push key, in bytes.
pub const SECRET_LEN: usize = 32;

/// What the device is, as far as the server's device list is concerned. Informational:
/// what a device may do is decided by its [`DeviceKind`], never by this.
///
/// `Desktop` is the last variant, so postcard's numbering of the first two is unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Ios,
    Android,
    Desktop,
}

/// What a device is allowed to do. **Fixed when the device is paired** — the pairing ticket
/// names the kind it admits and the server stores it — and never taken from anything the
/// device says afterwards: a stolen phone key must not be able to call itself a desktop and
/// get a writable shell.
///
/// A device list written before kinds existed has none, and every entry in it is a phone.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceKind {
    /// Today's allowlist, unchanged; a terminal is read-only.
    #[default]
    Phone,
    /// Every server command and event.
    Desktop,
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

/// What a client says about itself after its `Hello`, in the same frame. Informational:
/// the server records nothing from it that it acts on.
///
/// It is not a field of [`Hello`] so that a `Hello` written by a phone built before it
/// existed stays valid: postcard writes a struct's fields one after another, so the bytes of
/// a `Hello` followed by a `Platform` are exactly a frame the old reader reads the `Hello`
/// out of and ignores the rest of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloInfo {
    pub platform: Platform,
}

/// A `Hello` as one frame, followed by the client's [`HelloInfo`].
pub fn encode_hello(hello: &Hello, info: &HelloInfo) -> Result<Vec<u8>, FrameError> {
    let mut body = postcard::to_stdvec(hello).map_err(|e| FrameError::Encode(e.to_string()))?;
    body.extend(postcard::to_stdvec(info).map_err(|e| FrameError::Encode(e.to_string()))?);
    if body.len() > MAX_FRAME {
        return Err(FrameError::TooLarge(body.len()));
    }
    let mut out = (body.len() as u32).to_be_bytes().to_vec();
    out.extend(body);
    Ok(out)
}

/// A `Hello` and, if the client sent one, its [`HelloInfo`]. A trailer that is not a
/// `HelloInfo` is ignored rather than an error: it is informational, and a future client's
/// longer one must not lock it out.
pub fn decode_hello(body: &[u8]) -> Result<(Hello, Option<HelloInfo>), FrameError> {
    let (hello, rest) =
        postcard::take_from_bytes::<Hello>(body).map_err(|e| FrameError::Decode(e.to_string()))?;
    let info = postcard::take_from_bytes::<HelloInfo>(rest).ok().map(|(info, _)| info);
    Ok((hello, info))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hello_with_and_without_a_platform_both_read() {
        let hello = Hello {
            protocol_major: 1,
            protocol_minor: 7,
            app_version: "1.0.0".into(),
            capabilities: vec!["a".into()],
        };
        let plain = postcard::to_stdvec(&hello).unwrap();
        assert_eq!(decode_hello(&plain).unwrap(), (hello.clone(), None));
        let frame = encode_hello(&hello, &HelloInfo { platform: Platform::Desktop }).unwrap();
        let (decoded, info) = decode_hello(&frame[4..]).unwrap();
        assert_eq!(decoded, hello);
        assert_eq!(info, Some(HelloInfo { platform: Platform::Desktop }));
        // The reader a phone's build has is the plain one: it reads the same Hello.
        assert_eq!(postcard::from_bytes::<Hello>(&frame[4..]).unwrap(), hello);
    }

    #[test]
    fn a_device_without_a_kind_is_a_phone() {
        assert_eq!(DeviceKind::default(), DeviceKind::Phone);
        assert_eq!(serde_json::to_string(&DeviceKind::Desktop).unwrap(), "\"desktop\"");
    }

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
        let codes =
            [CLOSE_DONE, CLOSE_UNKNOWN, CLOSE_REVOKED, CLOSE_VERSION, CLOSE_PROTOCOL, CLOSE_SLOW];
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
