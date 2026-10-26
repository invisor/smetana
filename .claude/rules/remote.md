---
paths:
  - "crates/remote-protocol/**"
  - "src-tauri/src/remote/**"
  - "crates/remote-server/**"
  - "crates/smetanad/**"
  - "crates/core/src/remote/**"
  - "crates/core/src/health.rs"
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
- `crates/core/` — package `smetana-core`, module `smetana_core`, a path dependency of `src-tauri` on the
  same terms (no root workspace; `cd crates/core && cargo test`). It holds every worker and has no
  `tauri` in its manifest, directly or by a feature. Its `remote/` is only the push half the workers
  reach for — `push.rs` (the messages, `waiting_began`, `run_ended`, the `notify_*` calls) and
  `DEFAULT_RELAY_URL`, which `settings/model.rs` needs; the shell re-exports the constant.
- `crates/remote-server/` — package `remote-server`, module `remote_server`, a path dependency of `src-tauri` and of
  `smetanad` on the same terms (no root workspace; `cd crates/remote-server && cargo test`; no `tauri` in its manifest).
  It was `src-tauri/src/remote/` until the headless server needed it: `lib.rs` (the `Remote` service, constants),
  `endpoint.rs` (binding, accept loop, the three kinds of connection), `pairing.rs` (secrets), `devices.rs`
  (`remote-devices.json`), `commands.rs` (`plain`, the bodies of the device commands), `bridge.rs` (the per-connection
  loop, the event hub, `emit`), `dispatch.rs` (the server commands called without Tauri), `gateway.rs` (the restart
  gateway), `push.rs` (sealing and the POST), `contract.rs` (the contract test), `tests.rs` and `tests/` (two real iroh
  endpoints on loopback).
- **Where the constants are.** `PROTOCOL_MAJOR`, `PROTOCOL_MINOR`, `MIN_MOBILE_VERSION` and `RECOMMENDED_MOBILE_VERSION` are in
  `crates/remote-server/src/lib.rs` (they were in `src-tauri/src/remote/mod.rs`); `mobile/src-tauri/tests/versions.rs` reads that
  file. `.smetana/project.toml` (untracked) still names `src-tauri/src/remote/mod.rs` for them in its hazards; read that as this file.
- `src-tauri/src/remote/` — what is left of it: `mod.rs` makes a `Remote` out of an `AppHandle` (`start`, `shutdown`,
  `cancel_pairing`, the dispatcher's `Services` from managed state) and `commands.rs` holds the `#[tauri::command]`
  wrappers over `remote_server::commands::plain`.
- `crates/smetanad/` — the headless server, see "smetanad" below.
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
`devices.js`, and `tests/components/settings/devices.test.js` reads `crates/core/src/remote/mod.rs` to keep the pair equal.

**The QR is drawn from the module matrix by the component**, not from the library's SVG string: that
string carries colours as attributes, which cannot be `var(--token)`, and the only way to place it is
`v-html`. Its two colours are `--qr-ink` and `--qr-paper` (`tokens/qr.css`), the same in both themes,
because a scanner wants dark on light.

**Tests** use `RelayMode::Disabled` and a loopback `bind_addr`; no relay process is needed. `remote_contract_*` read the contract
files, the stores and the workers by path from `crates/remote-server`, and `phone_bridge.rs` reads the shell's `lib.rs`.

## The bridge (stage 2)

**Wire.** `serde_json::Value` cannot go through postcard (it deserializes with `deserialize_any`), so
every `Value` in `ClientMsg`/`ServerMsg`/`BridgeError::Handler` travels as the text of its JSON; under a
human-readable format (`serde_json`) it is the value itself. The Rust types and the logical shape are
exactly the epic's. `capabilities` in `HelloReply` is the allowed commands, then the allowed events; a
command name never contains `:` and an event name always does.

**The allowlist is checked in the bridge, before the dispatcher.** `Forbidden` means the desktop has a
command of that name (`DESKTOP_COMMANDS` in `bridge.rs`, held equal to the shell's `lib.rs` `generate_handler!` by
a test, so a new command is `Forbidden` and not `UnknownCommand` the day it is added); `UnknownCommand`
means it has none. A refused command never reaches `Dispatch`. `session_send` is allowlisted but a
non-empty `attachments` is `Forbidden`: those are paths on the desktop, and the spec keeps attachments
from disk off the phone. `terminal_attach` is allowlisted and is served by the worker's read-only `Request::Peek`, never `Attach`: an attach moves the single `active` seat and `flush` drops the output of every session that is not active, so a phone would freeze the desktop's tab. `terminal_write`/`resize` are not allowlisted.

**One function per command.** A command that takes `State` or `AppHandle` is split into a plain function
over the worker's handle (`tracker::commands::snapshot`, `session::commands::plain::send`, …) that the
`#[tauri::command]` wrapper and `dispatch.rs` both call. Only the allowlisted ones were split. A handler's
error is `serde_json::to_value(err)`, which is how Tauri hands it to the webview, and travels as
`BridgeError::Handler`.

**One emission point.** Every event a worker sends leaves through `Context::emit(name, &payload)`
(`crates/core/src/context.rs`), whose Tauri sink (`host.rs`) emits to the windows as `app.emit` did and then
offers the payload to the phones listening (`Hub::publish`), which drops a name that is not on the
allowlist. So there is one road for every event and the seven allowlisted ones reach the phones by it;
nothing in a worker holds an `AppHandle` any more. A phone whose
queue fills (512 messages) is closed with `CLOSE_SLOW` and reads its snapshots again, as after any dropped
connection. Calls run concurrently (32 per phone); replies carry the phone's id and come in any order.

**The contract.** `crates/remote-protocol/contract/commands/<cmd>.json` holds `args` (hand-written
examples, every parameter present) and `reply` (examples of the real value, one of every enum variant);
`events/<name>.json` holds `payload`; `versions.json` maps each protocol version to the digest of the whole contract at that version. `remote/contract.rs`
builds the real values from the handlers' types, compares them field by field and fails naming the command
or event and the path. A removed or renamed field needs `PROTOCOL_MAJOR`; an added one needs the file
updated and `PROTOCOL_MINOR` raised. `SMETANA_UPDATE_CONTRACT=1 cargo test remote_contract` (in `crates/remote-server`) rewrites the
replies and payloads from the server's values — run it once, not twice, since the tests share the files and a write can land under a read; arguments are edited by hand. `settings_load` pins the
whole of `ResolvedSettings`, so adding a setting touches the contract — that is the price of the policy.

**A regenerated contract does not pass by itself.** `SMETANA_UPDATE_CONTRACT` never writes `versions.json`.
The test `remote_contract_changes_only_with_a_protocol_version` fails when the files hash to something other
than the digest recorded for the current `PROTOCOL_MAJOR.PROTOCOL_MINOR`, and says which bump is needed:
MINOR for additions, MAJOR for removals and renames. The way forward is a new entry under a higher version,
added by hand together with the constants; an existing entry is a frozen contract and is not edited.

## Protocol 1.9: the server's own commands

Step 4 of `docs/superpowers/specs/2026-10-25-headless-server-design.md`. Additions only; a phone sees no difference.

`SERVER_ONLY_COMMANDS` (`remote-protocol`) are server commands the desktop app has no Tauri command for, because they are
about the machine the server runs on. They are the end of `SERVER_COMMANDS`, a phone calling one is `Forbidden` (not
`UnknownCommand`), and the contract test accepts a server command that is not in `DESKTOP_COMMANDS` only when it is declared
here. Two exist:

- **`server_health {}`** answers `smetana_core::health::Report`: `ok`, `problems`, `bd` and `bdExpected`, `git`, and one
  `agents` row per profile, each a `Tool` (`name`, `found`, `path`, `answers`, `version`, `problem`) plus the `PATH` the
  agents inherit. "Answers" is `--version` exiting successfully inside eight seconds; it cannot say an agent is logged in.
  It is separate from `tracker_health`, which moves with the open project. `smetanad status` prints the same report.
- **`server_restart { mode, confirmed }`**, the restart gateway (`gateway.rs`). `mode` is `now`, `when-idle` or `cancel`;
  `confirmed` is optional: `null`, or the list of work the person was shown. A restart ends every child the server owns, so
  the rule is **no live work of any kind** (runs, agent sessions, driven sessions, shells) and nothing is ended silently:
  `now` with nothing alive restarts; with work alive it answers `needs-confirmation` with the list and does nothing. The
  person's answer is the same request with `confirmed` set to **that list**, and it covers only the work it names: if
  anything else is alive the server answers `needs-confirmation` again with the fresh list. A run is named by its **token**
  (`id`), so confirming one run does not cover another on the same project; a batch of a run (its recorded session, in
  whichever worker its transport says) carries `owner: <token>` and is covered with that run, including a batch the run starts
  after the confirmation.
  A restart is a task of its own that `cancel` stops, wherever it has got to: after the 300 ms that let the reply leave it
  takes the list, then **refuses the work-creating commands** (`WORK_COMMANDS`: `terminal_create`, `terminal_shell`,
  `session_start`, `run_start`; a handler error saying the server is restarting) and takes the list once more, so the last
  look is the one the workers are ended on. If work not covered has appeared, nothing is ended: the refusal is lifted, the
  restart turns into a `when-idle` one (logged), and every desktop device hears **`server:restart-deferred`** with
  `{ work }`, what it now waits for — the answer it already had said `restarting`, and this is how that is taken back. A
  restart is **one piece of state under one lock** (`Phase`: `Idle`, `Frozen(epoch)`, `Committed`, with the cancel epoch beside
  it): a task may freeze, commit, re-arm or announce only while its epoch is the current one, a cancel clears the freeze and
  moves the epoch in the same step, and a task's freeze is lowered by a drop guard however it ends (an aborted task included).
  Once `restart()` has been called the state is `Committed`: a cancel then answers **`already-restarting`** (not
  `nothing-scheduled`), the freeze stays until the process is gone, and another `now` just answers `restarting`. The set of
  refused commands is `WORK_COMMANDS` in `gateway.rs`, **which pairs with the `calls!` table in `dispatch.rs`: a new command that
  starts a long-lived child (a shell, a session, a run, a PTY) must be added to it**, or the freeze lets it through to be ended
  unlisted. A host with no gateway advertises neither `server_restart` nor `server:restart-deferred`. The 1.9 digest in `versions.json`
  was recomputed for these changes because 1.9 had not left the branch.
  `when-idle` never ends anything: it arms a watch (polled every five seconds) and restarts when the list is empty, however
  long that takes, and answers `scheduled` with what it waits for. `cancel` disarms it (`cancelled` or `nothing-scheduled`).
  The reply is `{ outcome, scheduled, work: [{ kind, id, project, label, owner }] }` with `outcome` one of `restarting`, `scheduled`,
  `needs-confirmation`, `cancelled`, `nothing-scheduled`, `already-restarting` and `kind` one of `run`, `agent`, `session`, `shell`. The restart
  itself runs 300 ms after the reply, so it leaves first. A run's own agent is listed beside the run. On the desktop, which has
  no gateway, the command is a handler error and **is left out of the capabilities** in `HelloReply`
  (`Dispatch::serves`, `Hub::capabilities_for`), so a client does not draw a control the host cannot honour.

## smetanad

`crates/smetanad/`: a binary and a library, `cd crates/smetanad && cargo test`; **`cargo tree | grep tauri` is empty and
stays so**. It is `smetana-core`, `remote-server` and `remote-protocol` behind a command line. Every subcommand takes
`--data-dir`, which is never guessed (no environment variable, no platform default).

- `run` — the server, in the foreground. Refuses to run as root (`server::refuse_root`, before anything is created). Takes
  `server.lock` in the data directory with `File::try_lock`, so a second server on the directory is refused naming the pid.
  Everything is in the data directory: `settings.json` (it is the config directory too), attachments, transcripts, the device
  key `remote-key` and list `remote-devices.json` (the same names the desktop uses, so a copy of a desktop's directory is a
  server's), and the files below. The initial project is the last one in `settings.json`, never the working directory. `bd` is
  `--bd`, else beside the executable, else on `PATH`. SIGTERM or SIGINT stops it cleanly (children ended, devices told, the
  credential removed); a gateway restart does the same and then `exec`s itself with the same arguments (exit code 75 where
  that is not possible, for a service manager to act on).
- **Two endpoints, one protocol.** The loopback endpoint (`Remote::serve_local`) is always up: `127.0.0.1`, no relay, the
  system's port, the same key. The public endpoint, which contacts a relay, starts late as the desktop's does: at `pair`, and at
  start only if some device was paired by a ticket (`Device.local` is false). A local client never wakes it.
- **Local pairing.** `local-pairing.json` (mode 0600, written to a temporary file with that mode and renamed) holds
  `{ version, node, addrs, secret }` and is rewritten at every start and removed at a clean stop. The secret is
  `Pairing::set_local`: not a ticket, no term, not burned, **accepted as a desktop's pairing code with no confirmation**
  (`Spent::Local`), and the only thing a request from an unknown key needs when no ticket is out (`accepts_requests`). **It
  counts on the loopback endpoint alone** (`endpoint::Door`): the public endpoint, which anybody who knows the node id can dial,
  closes an unknown key before reading a byte unless a ticket is out, and never takes the local secret, so a leaked copy of the
  data directory pairs nothing from off the machine. It is good for the life of the process. Whoever can read the file can already read the key and every repository, so it adds no
  exposure; a device revoked from the list can pair itself again through it, which is the point for a local client and is why
  revoking one is cosmetic. `smetanad revoke` on one says so in plain words. A device paired this way has `local: true` in the
  list (and in `DeviceView`, where it is omitted when false).
- **The other subcommands are clients.** `client.rs` is a desktop-kind device: it pairs with the credential on first use (its
  own key `local-client-key` and `local-client-paired` marker beside the server's, named "smetanad (this machine)"), says
  `Hello` and calls the ordinary commands. `pair` calls `remote_pair_begin` and listens to `remote:pair-request`, asking the
  terminal; `devices` is `remote_devices`; `revoke` is `remote_device_revoke` (an id is a prefix of at least four characters of
  the key that names one device) and the server closes the live connection with `CLOSE_REVOKED` before it answers; `restart` is
  `server_restart` (`--when-idle`, `--cancel`, `--yes`; `--yes` still prints the list it confirms and sends it back); `status` asks the running server (`server_health`, the report under the service's own environment) and says so, and falls back to
  a probe in the CLI's own process, saying so, only when no server answers — and it says which of two: "no server is running"
  (no credential file) or "the server did not answer" (a credential file is there: stopped uncleanly, stuck, older build).
  There is no second control channel.
- **Pairing in the terminal.** `smetanad pair --kind desktop|phone` prints the ticket as text and as a QR code
  (`qrcode`, half-block characters) and the confirmation is asked in that terminal; a closed stdin or anything but `y` is no.
  The ticket is the existing single-use, two-minute one, `smetana1:` for a phone and `smetana2:` for a desktop.
- **Tests.** `tests/server.rs` runs a `Server` in the test process (one shared runtime, because the workers spawn on a
  process-wide handle the first server installs); `tests/binary.rs` runs the executable. Both need the bd sidecar
  (`npm run fetch-bd`, or `SMETANAD_TEST_BD`) and fail loudly without it. `ops::pair` and `ops::restart` take their terminal as
  a closure and a writer, so the tests drive the code the command line runs.

## Protocol 1.8: the full command set, and device kinds

Step 3 of `docs/superpowers/specs/2026-10-25-headless-server-design.md`. Additions only; the phone sees no difference.

**Kinds.** `DeviceKind` (`phone` | `desktop`) is fixed at pairing: the ticket names the kind it admits (a phone's keeps the
`smetana1:` layout byte for byte, a desktop's is `smetana2:`), `Pairing` remembers it, and `remote-devices.json` stores it
(`kind`, absent means phone). `Hello`'s platform is informational and is not even a field of `Hello`: a client that has one
sends `HelloInfo` in the same frame after the `Hello` (`encode_hello`/`decode_hello`), which a reader of the plain `Hello`
ignores. `Platform::Desktop` was added at the end. `capabilities_for(kind)`, `is_command_allowed_for` and
`is_event_allowed_for` decide everything: a phone gets `ALLOWED_*` unchanged, a desktop gets `SERVER_COMMANDS` and
`SERVER_EVENTS` (the phone's lists first, then the rest). The list is checked per call against the kind the device list holds.

**The server surface** is every command in `DESKTOP_COMMANDS` that is not in `CLIENT_COMMANDS` (windows, dialogs,
clipboard, opener, updater, login item, `tracker_access_reset`, which ends in an app restart). The contract tests fail for a
command or event with neither a pinned example nor a client declaration, and scan the stores and the workers for event names.
`dispatch.rs` declares each command once in the `calls!` table. Where the kind changes behaviour: a phone's `session_send`
may carry no attachments and its `session_start` is confined to the attachment store; a desktop's is not.

**Terminal stream.** Chunks are committed by the worker for every session each tick, numbered, kept in a bounded history
(`terminal/stream.rs`) and offered to the sink (`Sink::terminal_chunk`). `terminal_attach` takes an optional `after`; the
reply gains `resumedAfter` only when `data` is the continuation after that number, and otherwise `data` is the ring as of
`seq`. A desktop device that attaches is registered for the session (`Outbox` in `bridge.rs`): output is coalesced per
session and per device, a device that falls behind its ceiling (`OUTBOX_CAP`) is sent the ring again as `terminal:output`
with `reset: true`, and the worker is never made to wait. The PTY has the size of the client that last wrote or resized it
(`Viewers`); the in-process window is client 0.

**How a client follows a session.** Attaching is the whole subscription: `terminal_attach` (a desktop device only) registers
the follow before the worker is asked and arms it once the reply is on its way, so no `Listen` is needed and the reply
always precedes the stream; `Unlisten terminal:output` ends every follow of that connection, and `terminal_detach` ends one.
Attaching again to a session on a live connection is supported and starts the follow afresh (pass `after` to resume).
Every bridged `terminal:output` is `{ id, from, seq, data }` plus `reset: true` when it is a fresh ring: the data is chunks
`from..=seq` joined. A client holding chunk `last` (the reply's `seq`, or the previous event's) applies an event only when
`from == last + 1`; anything else is a gap and it attaches again. A `reset` has `from == seq`, replaces the terminal's
contents instead of continuing them, and is sent in place of the chunks a device that fell behind its ceiling did not get;
chunks after it continue from its `seq`. A reset is applied whatever `last` is, and sets `last = seq` (outside an outstanding
attach, below). The window's own events have no `from`.

**While an attach is outstanding.** From sending `terminal_attach` for a session until its reply arrives, the client discards
that session's `terminal:output`; the reply alone sets `last`. Events already written when the attach was sent precede the
reply on the stream and are covered by it (the server drops its buffer for the session when the attach arrives, and the
follow is armed only after the reply), so applying them would repeat chunks the reply re-sends.

## Protocol 1.7: a tool result's denial

`EventKind::ToolResult` gained `denial`: `null`, or the reason Claude Code's auto-mode classifier gave when it refused the
call (`conversation-panel.md`, the block card). It rides every `session:events` and `crew:events` payload and the replies
that replay a journal. Additions only. The phone imports the same `ConversationView` and `journal.js`, so a phone built from this tree draws the card too; only an older phone build ignores the field and draws a plain failed row. The mobile
build's own `PROTOCOL_MINOR` stays where it was.

## Protocol 1.6: files beside pictures

`TaskDraft` (the `newTask` intent in `session_start`) gained `files`, a list of attachment-store paths for attached
files that are not pictures, and `attachment_write`'s reply (`Attachment`) has a nullable `data`: `null` for a file
that is not a picture. Additions only. `confine_new_task` checks `files` exactly as it checks `images` (any path outside
the store is `Forbidden`). The phone's `ATTACHMENTS_MINOR` stays 3 and its picker stays `image/*`; `newTaskIntent`
splits the list by `url`. A desktop older than 1.6 ignores `files` (accepted). The mobile build's own `PROTOCOL_MINOR`
stays where it was.

## Protocol 1.5: the session's model

`session_attach`'s reply (`Attached`) gained an optional `model`: the model the session was actually started with,
present only when one was passed, absent otherwise. The desktop's panel names the harness and model from it
(`conversation-panel.md`, "Who the header names"); a phone that ignores it keeps working. The contract pins it on the
first fixture element only, so both shapes are held. The mobile build's own `PROTOCOL_MINOR` is the newest minor it
*knows* and stays where it was, as it did for 1.4.

## Protocol 1.4: the batch log

`Session` gained `timeline: bool` and `terminal_attach`'s reply gained `log` and `logTrimmed`, for the desktop's own
Agent tab (see `terminal.md`, "A batch's log as a timeline"). Additions only, so a phone that ignores them keeps
working; a phone reads the ring snapshot as before and draws xterm.

## Protocol 1.1: `UpdatePushToken`

`ClientMsg::UpdatePushToken { token }` is the last variant of `ClientMsg`, so postcard's numbering of the others is
unchanged. A phone sends it when its push token appears or changes after pairing; the desktop stores it with
`Devices::set_push_token` (an empty token or one over `TOKEN_LIMIT` is ignored, the connection stays). An older desktop
would read the variant as a broken frame, so a phone sends it only when `HelloReply.protocol_minor >= 1`. The contract
files did not change, so `versions.json` has `1.1` with the same digest as `1.0`.

## Protocol 1.3: `attachment_write`

`attachment_write { name, data }` (base64 bytes, `name` may be `null`) joins the allowlist as its last entry and
answers an `Attachment` whose `path` is absolute on the desktop; its body is `attachments::plain::write`, so a phone's
picture is judged by the rules a paste is. The phone gates its Images block on `protocol_minor >= 3` (`desktopMinor`
of the status it is given), because an older desktop answers `Forbidden`. The other half is the confinement in
`dispatch.rs`: a phone's `session_start` with `Intent::NewTask` is `Forbidden` unless every `draft.images` path is
`attachments::cleanup::in_store` the store (`Services.attachments_root`), since the agent that files the task opens
whatever path it is given.

## Push

`remote/push.rs` (the shell's half: the sealing and the POST; what is said, and when, is
`crates/core/src/remote/push.rs`) sends a run's ending and a session's start of waiting to every device with a `push_token`.
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
