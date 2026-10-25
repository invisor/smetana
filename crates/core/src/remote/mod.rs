//! The part of the mobile companion that the workers themselves reach for: what is pushed to a
//! phone (`push.rs`) and the one constant the settings schema needs. The endpoint, the pairing and the
//! device list are the desktop shell's `remote/` until the server gets them.

pub mod push;

/// The relay a desktop uses until its owner names another, and the one put in
/// every ticket it issues.
///
/// Ours: the relay in `server/`, behind Caddy on the project's own VPS
/// (`server/README.md`, "Our deployment").
/// It is written out twice in the tree, here and as `DEFAULT_RELAY_URL` in
/// `components/settings/devices.js`, which every front-end default imports;
/// `tests/components/settings/devices.test.js` holds the two equal.
pub const DEFAULT_RELAY_URL: &str = "https://relay.smetana.io";
