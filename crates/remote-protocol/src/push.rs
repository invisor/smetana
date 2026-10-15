//! The encrypted body of a push notification.
//!
//! The desktop seals a short JSON message to the phone's X25519 push key; the push gateway carries the
//! result to APNs or FCM without being able to read it, and the phone opens it with its private half.
//! Every blob uses a fresh ephemeral key, so two pushes to one phone share nothing a gateway could link
//! beyond the token it already sees.
//!
//! The scheme: an ephemeral X25519 agreement with the recipient's key, the shared secret and both
//! public keys hashed into a 32-byte key (BLAKE3 `derive_key`, with a context string that names this
//! use), then XChaCha20-Poly1305 under a random 24-byte nonce. The nonce is long enough that drawing it
//! at random is safe, and the key is new every time besides.

use chacha20poly1305::aead::{Aead, AeadCore, KeyInit, OsRng};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey as X25519Public, StaticSecret};

const KDF_CONTEXT: &str = "smetana 2026-10-15 push blob v1";

/// What travels to the gateway, as `base64(postcard(PushBlob))`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PushBlob {
    /// The sender's one-time X25519 public key.
    pub ephemeral: [u8; 32],
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

impl PushBlob {
    /// The form the gateway carries: `base64(postcard(self))`, standard alphabet with padding.
    pub fn to_wire(&self) -> String {
        let bytes = postcard::to_stdvec(self).expect("a blob always serialises");
        data_encoding::BASE64.encode(&bytes)
    }

    /// The inverse of [`to_wire`](Self::to_wire), for the phone and for tests.
    pub fn from_wire(text: &str) -> Result<Self> {
        let bytes = data_encoding::BASE64.decode(text.as_bytes()).map_err(|_| PushError)?;
        postcard::from_bytes(&bytes).map_err(|_| PushError)
    }
}

/// Why a blob would not open. Deliberately one kind: a phone has nothing to do differently for a wrong
/// key and for a damaged blob, and telling them apart would only help someone probing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PushError;

impl std::fmt::Display for PushError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the push blob could not be opened")
    }
}

impl std::error::Error for PushError {}

pub type Result<T> = std::result::Result<T, PushError>;

fn derive_key(shared: &[u8; 32], ephemeral: &[u8; 32], recipient: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new_derive_key(KDF_CONTEXT);
    hasher.update(shared);
    hasher.update(ephemeral);
    hasher.update(recipient);
    *hasher.finalize().as_bytes()
}

/// The public key that goes with a push secret: what a phone sends at pairing as `push_key`.
pub fn public_key(secret: &[u8; 32]) -> [u8; 32] {
    X25519Public::from(&StaticSecret::from(*secret)).to_bytes()
}

/// Encrypts `plaintext` to the X25519 public key `push_key`.
pub fn seal(push_key: &[u8; 32], plaintext: &[u8]) -> PushBlob {
    let secret = StaticSecret::random_from_rng(OsRng);
    let ephemeral = X25519Public::from(&secret).to_bytes();
    let shared = secret.diffie_hellman(&X25519Public::from(*push_key));
    let key = derive_key(shared.as_bytes(), &ephemeral, push_key);
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ciphertext = XChaCha20Poly1305::new(&key.into())
        .encrypt(&nonce, plaintext)
        .expect("encrypting into a Vec cannot fail");
    PushBlob { ephemeral, nonce: nonce.into(), ciphertext }
}

/// Opens a blob with the private half of the key it was sealed to. Fails on a wrong key and on any
/// altered byte of the nonce, the ephemeral key or the ciphertext.
pub fn open(secret: &[u8; 32], blob: &PushBlob) -> Result<Vec<u8>> {
    let secret = StaticSecret::from(*secret);
    let recipient = X25519Public::from(&secret).to_bytes();
    let shared = secret.diffie_hellman(&X25519Public::from(blob.ephemeral));
    let key = derive_key(shared.as_bytes(), &blob.ephemeral, &recipient);
    XChaCha20Poly1305::new(&key.into())
        .decrypt(XNonce::from_slice(&blob.nonce), blob.ciphertext.as_slice())
        .map_err(|_| PushError)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair() -> ([u8; 32], [u8; 32]) {
        let secret = StaticSecret::random_from_rng(OsRng);
        (secret.to_bytes(), X25519Public::from(&secret).to_bytes())
    }

    #[test]
    fn open_of_seal_returns_the_plaintext() {
        let (secret, public) = pair();
        let blob = seal(&public, b"hello phone");
        assert_eq!(open(&secret, &blob).unwrap(), b"hello phone");
    }

    #[test]
    fn public_key_is_the_one_a_secret_was_generated_with() {
        let (secret, public) = pair();
        assert_eq!(public_key(&secret), public);
    }

    #[test]
    fn a_changed_ciphertext_byte_is_an_error() {
        let (secret, public) = pair();
        let mut blob = seal(&public, b"hello phone");
        blob.ciphertext[0] ^= 1;
        assert_eq!(open(&secret, &blob), Err(PushError));
    }

    #[test]
    fn a_changed_nonce_or_ephemeral_byte_is_an_error() {
        let (secret, public) = pair();
        let blob = seal(&public, b"hello phone");
        let mut nonce = blob.clone();
        nonce.nonce[0] ^= 1;
        assert!(open(&secret, &nonce).is_err());
        let mut ephemeral = blob;
        ephemeral.ephemeral[0] ^= 1;
        assert!(open(&secret, &ephemeral).is_err());
    }

    #[test]
    fn another_recipients_key_is_an_error() {
        let (_, public) = pair();
        let (other_secret, _) = pair();
        let blob = seal(&public, b"hello phone");
        assert!(open(&other_secret, &blob).is_err());
    }

    #[test]
    fn two_seals_of_one_message_differ() {
        let (_, public) = pair();
        assert_ne!(seal(&public, b"same"), seal(&public, b"same"));
    }

    #[test]
    fn a_blob_survives_its_wire_form() {
        let (secret, public) = pair();
        let text = seal(&public, b"over the wire").to_wire();
        assert!(text.len() < 3500, "the gateway refuses a longer blob");
        let blob = PushBlob::from_wire(&text).unwrap();
        assert_eq!(open(&secret, &blob).unwrap(), b"over the wire");
        assert!(PushBlob::from_wire("%%%").is_err());
    }

    #[test]
    fn a_blob_survives_postcard() {
        let (secret, public) = pair();
        let bytes = postcard::to_stdvec(&seal(&public, b"over the wire")).unwrap();
        let blob: PushBlob = postcard::from_bytes(&bytes).unwrap();
        assert_eq!(open(&secret, &blob).unwrap(), b"over the wire");
    }
}
