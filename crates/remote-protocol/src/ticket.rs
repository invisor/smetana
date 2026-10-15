//! The pairing ticket: what the QR code on the desktop carries.
//!
//! `"smetana1:"` followed by the postcard bytes of the ticket in base32, lower
//! case, no padding. The prefix is the ticket's own version: a layout change is
//! `smetana2:`, and a phone that does not know it can say so.

use data_encoding::BASE32_NOPAD;
use iroh::PublicKey;
use serde::{Deserialize, Serialize};

use crate::SECRET_LEN;

pub const TICKET_PREFIX: &str = "smetana1:";

/// Everything a phone needs to reach a desktop it has never met. The relay is in
/// here so the phone never has to ask anybody where to go.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingTicket {
    pub desktop: PublicKey,
    pub relay: String,
    pub secret: [u8; SECRET_LEN],
    /// Seconds since the Unix epoch.
    pub expires_at: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TicketError {
    /// The string does not start with `smetana1:`.
    Prefix,
    /// The rest is not base32.
    Encoding,
    /// The bytes are not a ticket, or there are bytes after one.
    Layout,
}

impl std::fmt::Display for TicketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TicketError::Prefix => "not a Smetana pairing code",
            TicketError::Encoding => "the pairing code is damaged",
            TicketError::Layout => "the pairing code is not readable",
        })
    }
}

impl std::error::Error for TicketError {}

impl PairingTicket {
    /// The string for the QR code.
    pub fn encode(&self) -> String {
        // Serialising this struct cannot fail: no map keys, no unsupported types.
        let bytes = postcard::to_stdvec(self).expect("a ticket always serialises");
        format!("{TICKET_PREFIX}{}", BASE32_NOPAD.encode(&bytes).to_lowercase())
    }

    /// The ticket a string carries, or why it is not one.
    pub fn decode(text: &str) -> Result<Self, TicketError> {
        let body = text.trim().strip_prefix(TICKET_PREFIX).ok_or(TicketError::Prefix)?;
        let bytes = BASE32_NOPAD
            .decode(body.to_uppercase().as_bytes())
            .map_err(|_| TicketError::Encoding)?;
        let (ticket, rest) =
            postcard::take_from_bytes::<Self>(&bytes).map_err(|_| TicketError::Layout)?;
        if !rest.is_empty() {
            return Err(TicketError::Layout);
        }
        Ok(ticket)
    }
}

impl std::fmt::Display for PairingTicket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.encode())
    }
}

impl std::str::FromStr for PairingTicket {
    type Err = TicketError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::decode(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroh::SecretKey;

    fn ticket() -> PairingTicket {
        PairingTicket {
            desktop: SecretKey::from_bytes(&[3; 32]).public(),
            relay: "https://relay.example.com".into(),
            secret: [42; SECRET_LEN],
            expires_at: 1_800_000_000,
        }
    }

    #[test]
    fn a_ticket_survives_the_trip_through_its_string() {
        let ticket = ticket();
        let text = ticket.to_string();
        assert!(text.starts_with("smetana1:"));
        assert_eq!(text, text.to_lowercase(), "lower case throughout");
        assert!(!text.contains('='), "no padding");
        assert_eq!(text.parse::<PairingTicket>().unwrap(), ticket);
    }

    #[test]
    fn a_damaged_string_is_an_error_and_not_a_panic() {
        let text = ticket().to_string();
        assert_eq!(PairingTicket::decode(""), Err(TicketError::Prefix));
        assert_eq!(PairingTicket::decode("smetana2:abc"), Err(TicketError::Prefix));
        assert_eq!(PairingTicket::decode("smetana1:!!!"), Err(TicketError::Encoding));
        assert_eq!(PairingTicket::decode("smetana1:"), Err(TicketError::Layout));
        assert!(PairingTicket::decode(&text[..text.len() - 9]).is_err());
        assert_eq!(PairingTicket::decode(&format!("{text}aa")), Err(TicketError::Layout));
        // Every possible truncation must come back as an error, never a panic.
        for end in 0..text.len() {
            let _ = PairingTicket::decode(&text[..end]);
        }
    }
}
