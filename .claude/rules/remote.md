---
paths:
  - "crates/remote-protocol/**"
  - "src-tauri/src/remote/**"
  - "src/stores/remote.js"
  - "src/components/settings/DevicesSettings.vue"
  - "src/components/settings/devices.js"
---

# The phone companion: protocol, pairing, devices

The design is `docs/superpowers/specs/2026-10-15-mobile-companion-design.md`. This is stage 1 of epic
smetana-032e: the shared crate, the desktop's endpoint, pairing by QR code and the Devices tab. There is
no command bridge yet — a paired phone connects, says `Hello`, is answered and is held open.

## Where things are

- `crates/remote-protocol/` — package `remote-protocol`, module `remote_protocol`, a path dependency of
  `src-tauri`. **No root workspace**, on purpose: `cd crates/remote-protocol && cargo test` is its own
  gate, and a workspace at the root would change where every `cargo` command in the repository finds its
  manifest. It holds wire types only (`PairingTicket`, `PairRequest`, `PairReply`, `Hello`,
  `HelloReply`, `Platform`), the frame format (postcard behind a `u32` big-endian length, checked
  against `MAX_FRAME` before the body is read) and `ALPN = b"smetana/remote/1"`. It has no clock and no
  policy. The bridge's `ClientMsg`/`ServerMsg`/`BridgeError` will join it as frames on one bi-stream.
- `src-tauri/src/remote/` — `mod.rs` (the `Remote` service, constants), `endpoint.rs` (binding,
  accept loop, the three kinds of connection), `pairing.rs` (secrets), `devices.rs`
  (`remote-devices.json`), `commands.rs`, `tests.rs` (two real iroh endpoints on loopback).
- `src/stores/remote.js`, `components/settings/DevicesSettings.vue`, `components/settings/devices.js`.

## Decisions worth knowing before changing them

**The endpoint starts late.** Binding one contacts a relay, and this app does not open a socket by itself
for a feature nobody has used. It starts on `remote_pair_begin`, and at launch only when the device list
is not empty. A changed relay URL takes effect at the next `remote_pair_begin` (the endpoint is rebound).

**Three kinds of connection, decided by the key alone.** A key on the list gets `Hello`. An unknown key
while a pairing code is outstanding may send exactly one `PairRequest` — the only time anything is read
from a key the list does not know, bounded by a 16 KiB frame, a timeout and the secret. Any other key is
closed (`CLOSE_UNKNOWN`) before a byte is read. This reads the spec's "closed before any message" with
the pairing window as its one exception, because pairing and ordinary connections share one ALPN and
the first message is the only thing that tells them apart.

**A request that ends is withdrawn.** Whatever ends the wait (decision, timeout, the phone leaving) emits
`remote:pair-ended`, and the window clears the prompt; each wait has an id so a late finish cannot clear a
newer request. `remote_pair_decide` is an error when nothing is waiting, so the screen never shows a
success that did not happen. Closing the settings window cancels the code from Rust (`WindowEvent::Destroyed`
in `window::settings_window_open`), because the page's unmount hooks do not run when the OS window dies.

**The relay is passed, not read.** `remote_pair_begin(relay_url)` takes the address in the field and
refuses one `valid_relay_url` refuses, because an edit reaches `settings.json` only after the app window's
400 ms debounce; start-up alone reads the file.

**A lost `Accepted` is undone.** If the reply cannot be delivered the device is removed again. A phone whose
key is already on the list and which sends a `PairRequest` is answered `CLOSE_PROTOCOL` (the desktop expects
`Hello` from a known key); the mobile client should treat that as "already paired" and connect again with
`Hello`. At most `MAX_PAIRING_CONNECTIONS` (4) unknown-key connections are served at once; the rest get
`CLOSE_UNKNOWN`. The `CLOSE_*` codes live in `remote-protocol` beside the ALPN.

**Webviews never see `push_token` or `push_key`**: `remote_devices` and `remote:devices` carry `DeviceView`.

**A secret is burned when presented, not when confirmed.** A rejected or timed-out request leaves
nothing that could be tried again. A wrong secret does not burn the right one. The wait for the
person is bounded by what is left of the secret's two minutes. Close codes (`CLOSE_*` in `endpoint.rs`)
are part of the protocol a phone reads.

**The device list is its own file**, `remote-devices.json` in `app_data_dir()`, written atomically and
at once on revoke; the key is `remote-key` beside it (hex, mode 0600, created on first use, never
rotated). `settings.json` holds only `remote.relayUrl`, validated by `valid_relay_url` in
`settings/model.rs` (https, a host name, no IP) and mirrored by `validRelayUrl` in `devices.js` so the
field refuses what Rust would silently replace with the default.

**`DEFAULT_RELAY_URL` is a placeholder** (`https://relay.smetana.example`; `.example` never resolves).
The spec says the default is "ours" and names no address. The front end imports one copy from
`devices.js`, and `tests/components/settings/devices.test.js` reads `remote/mod.rs` to keep the pair equal.

**The QR is drawn from the module matrix by the component**, not from the library's SVG string: that
string carries colours as attributes, which cannot be `var(--token)`, and the only way to place it is
`v-html`. Its two colours are `--qr-ink` and `--qr-paper` (`tokens/qr.css`), the same in both themes,
because a scanner wants dark on light.

**Tests** use `RelayMode::Disabled` and a loopback `bind_addr`; no relay process is needed.
