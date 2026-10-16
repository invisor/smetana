---
paths:
  - "crates/remote-protocol/**"
  - "src-tauri/src/remote/**"
  - "src/stores/remote.js"
  - "src/components/settings/DevicesSettings.vue"
  - "src/components/settings/devices.js"
---

# The phone companion: protocol, pairing, devices

The design is `docs/superpowers/specs/2026-10-15-mobile-companion-design.md`. Stage 1 of epic
smetana-032e is the shared crate, the desktop's endpoint, pairing by QR code and the Devices tab; stage 2
(smetana-zri2) is the command bridge: a paired phone connects, says `Hello`, and then calls allowlisted
commands and listens to allowlisted events on the same stream.

## Where things are

- `crates/remote-protocol/` — package `remote-protocol`, module `remote_protocol`, a path dependency of
  `src-tauri`. **No root workspace**, on purpose: `cd crates/remote-protocol && cargo test` is its own
  gate, and a workspace at the root would change where every `cargo` command in the repository finds its
  manifest. It holds wire types only (`PairingTicket`, `PairRequest`, `PairReply`, `Hello`,
  `HelloReply`, `Platform`), the bridge (`ClientMsg`/`ServerMsg`/`BridgeError`, `ALLOWED_COMMANDS`,
  `ALLOWED_EVENTS`, `capabilities()`), the frame format (postcard behind a `u32` big-endian length,
  checked against `MAX_FRAME` — 16 MiB, since a board snapshot passes a megabyte — before the body is
  read) and `ALPN = b"smetana/remote/1"`. It has no clock and no policy. `contract/` holds the pinned
  shape of every allowlisted command and event.
- `src-tauri/src/remote/` — `mod.rs` (the `Remote` service, constants), `endpoint.rs` (binding,
  accept loop, the three kinds of connection), `pairing.rs` (secrets), `devices.rs`
  (`remote-devices.json`), `commands.rs`, `bridge.rs` (the per-connection loop, the event hub, `emit`),
  `dispatch.rs` (the allowlisted commands called without Tauri), `contract.rs` (the contract test),
  `tests.rs` and `tests/phone_bridge.rs` (two real iroh endpoints on loopback).
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
`Hello` from a known key); the mobile client does **not** treat that as "already paired": the desktop sends the same code when the first
frame times out, so it is reported as a pairing failure the person can retry. At most `MAX_PAIRING_CONNECTIONS` (4) unknown-key connections are served at once; the rest get
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

**`DEFAULT_RELAY_URL` is ours** (`https://relay.smetana.io`), and `GATEWAY_URL` in `remote/push.rs`
is `https://push.smetana.io/v1/push`: both run from `server/` on the project's VPS, behind one Caddy
(`server/README.md`, "Our deployment"). The front end imports one copy from
`devices.js`, and `tests/components/settings/devices.test.js` reads `remote/mod.rs` to keep the pair equal.

**The QR is drawn from the module matrix by the component**, not from the library's SVG string: that
string carries colours as attributes, which cannot be `var(--token)`, and the only way to place it is
`v-html`. Its two colours are `--qr-ink` and `--qr-paper` (`tokens/qr.css`), the same in both themes,
because a scanner wants dark on light.

**Tests** use `RelayMode::Disabled` and a loopback `bind_addr`; no relay process is needed.

## The bridge (stage 2)

**Wire.** `serde_json::Value` cannot go through postcard (it deserializes with `deserialize_any`), so
every `Value` in `ClientMsg`/`ServerMsg`/`BridgeError::Handler` travels as the text of its JSON; under a
human-readable format (`serde_json`) it is the value itself. The Rust types and the logical shape are
exactly the epic's. `capabilities` in `HelloReply` is the allowed commands, then the allowed events; a
command name never contains `:` and an event name always does.

**The allowlist is checked in the bridge, before the dispatcher.** `Forbidden` means the desktop has a
command of that name (`DESKTOP_COMMANDS` in `bridge.rs`, held equal to `lib.rs`'s `generate_handler!` by
a test, so a new command is `Forbidden` and not `UnknownCommand` the day it is added); `UnknownCommand`
means it has none. A refused command never reaches `Dispatch`. `session_send` is allowlisted but a
non-empty `attachments` is `Forbidden`: those are paths on the desktop, and the spec keeps attachments
from disk off the phone. `terminal_attach` is allowlisted and is served by the worker's read-only `Request::Peek`, never `Attach`: an attach moves the single `active` seat and `flush` drops the output of every session that is not active, so a phone would freeze the desktop's tab. `terminal_write`/`resize` are not allowlisted.

**One function per command.** A command that takes `State` or `AppHandle` is split into a plain function
over the worker's handle (`tracker::commands::snapshot`, `session::commands::plain::send`, …) that the
`#[tauri::command]` wrapper and `dispatch.rs` both call. Only the allowlisted ones were split. A handler's
error is `serde_json::to_value(err)`, which is how Tauri hands it to the webview, and travels as
`BridgeError::Handler`.

**One emission point.** The seven allowlisted events leave through `remote::emit(app, name, &payload)`,
which emits to the windows as `app.emit` did and then to the phones listening (`Hub::publish`). A new
emitter of an allowlisted event must use it; an event off the list may still use `app.emit`. A phone whose
queue fills (512 messages) is closed with `CLOSE_SLOW` and reads its snapshots again, as after any dropped
connection. Calls run concurrently (32 per phone); replies carry the phone's id and come in any order.

**The contract.** `crates/remote-protocol/contract/commands/<cmd>.json` holds `args` (hand-written
examples, every parameter present) and `reply` (examples of the real value, one of every enum variant);
`events/<name>.json` holds `payload`; `versions.json` maps each protocol version to the digest of the whole contract at that version. `remote/contract.rs`
builds the real values from the handlers' types, compares them field by field and fails naming the command
or event and the path. A removed or renamed field needs `PROTOCOL_MAJOR`; an added one needs the file
updated and `PROTOCOL_MINOR` raised. `SMETANA_UPDATE_CONTRACT=1 cargo test remote_contract` rewrites the
replies and payloads from the desktop's values; arguments are edited by hand. `settings_load` pins the
whole of `ResolvedSettings`, so adding a setting touches the contract — that is the price of the policy.

**A regenerated contract does not pass by itself.** `SMETANA_UPDATE_CONTRACT` never writes `versions.json`.
The test `remote_contract_changes_only_with_a_protocol_version` fails when the files hash to something other
than the digest recorded for the current `PROTOCOL_MAJOR.PROTOCOL_MINOR`, and says which bump is needed:
MINOR for additions, MAJOR for removals and renames. The way forward is a new entry under a higher version,
added by hand together with the constants; an existing entry is a frozen contract and is not edited.

## Protocol 1.1: `UpdatePushToken`

`ClientMsg::UpdatePushToken { token }` is the last variant of `ClientMsg`, so postcard's numbering of the others is
unchanged. A phone sends it when its push token appears or changes after pairing; the desktop stores it with
`Devices::set_push_token` (an empty token or one over `TOKEN_LIMIT` is ignored, the connection stays). An older desktop
would read the variant as a broken frame, so a phone sends it only when `HelloReply.protocol_minor >= 1`. The contract
files did not change, so `versions.json` has `1.1` with the same digest as `1.0`.

## Push

`remote/push.rs` sends a run's ending and a session's start of waiting to every device with a `push_token`.
The text is sealed in `remote_protocol::push` (X25519 to the device's `push_key`, BLAKE3-derived key,
XChaCha20-Poly1305) and posted to `GATEWAY_URL` as `{ platform, token, blob }` — the body
`server/push-gateway` accepts. The gateway URL is a constant and a **placeholder** (`.example`), not a
setting: the gateway holds the published apps' APNs/FCM credentials.

One push per transition. A session pushes in `refresh_state` when the state *changes into* `NeedsYou`
(`waiting_began`), and a terminal (PTY) session does the same in `reassess` in `terminal/service.rs`
through `announce_wait`. The targets differ: `session:<id>` for a conversation session, `terminal:<id>` for a PTY one,
because the two id counters are separate and both start at 1, so `session:3` from a terminal would open someone
else's conversation; a run pushes in `handle_report` when a loop report moves it to over (`run_ended`),
so a stop pressed with nothing in flight is not announced to one's own phone. Delivery is spawned,
best effort, with no retry or queue; a failure is one log line naming the device and never the token.
`reqwest` is the updater's build of it (`rustls-no-provider`), so `push::client` installs the ring
provider itself in case no update check has run yet.
