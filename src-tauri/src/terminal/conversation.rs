//! The conversation id this app gives an agent, so that it knows afterwards
//! what to resume.
//!
//! The app spawns the harness and the harness invents the id, naming its
//! transcript after it. Matching a live session to a transcript by directory
//! and mtime is a guess, and it is wrong exactly where two agents run in one
//! directory — so the app chooses the id instead and passes it at the spawn
//! (`Profile::session_id_args`).
//!
//! Sixteen bytes off `/dev/urandom` rather than a crate: this is the whole of
//! what a uuid dependency would be used for, and every target this app is built
//! for is a unix. A machine that will not answer gets `None`, and the session is
//! simply started without an id of ours — one that cannot be offered back after
//! a restart, which is what happened to every session before this existed.

use std::io::Read;

/// A version 4 UUID in the canonical 8-4-4-4-12 lowercase hex form, or `None`
/// where the machine would not give sixteen random bytes.
///
/// The form is not decoration: `claude --session-id` takes a UUID and refuses
/// anything else, so a shorter or upper-cased string here would be a session
/// that fails to start rather than one that cannot be resumed.
pub fn new_id() -> Option<String> {
    let mut bytes = [0u8; 16];
    let mut source = std::fs::File::open("/dev/urandom").ok()?;
    source.read_exact(&mut bytes).ok()?;
    // Version 4 in the high nibble of byte 6, and the RFC 4122 variant in the
    // top two bits of byte 8.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    Some(format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_is_a_canonical_version_4_uuid() {
        let id = new_id().expect("this machine has /dev/urandom");
        let parts: Vec<&str> = id.split('-').collect();
        assert_eq!(parts.len(), 5, "{id}");
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12], "{id}");
        assert!(
            id.chars().all(|c| c == '-' || (c.is_ascii_hexdigit() && !c.is_ascii_uppercase())),
            "{id}"
        );
        assert!(parts[2].starts_with('4'), "version 4: {id}");
        assert!(
            matches!(parts[3].chars().next(), Some('8' | '9' | 'a' | 'b')),
            "the RFC 4122 variant: {id}"
        );
    }

    #[test]
    fn two_ids_are_not_the_same_id() {
        assert_ne!(new_id().expect("one"), new_id().expect("another"));
    }
}
