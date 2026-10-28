---
paths:
  - "crates/remote-protocol/**"
  - "crates/remote-server/**"
  - "crates/smetanad/**"
  - "crates/core/src/remote/**"
  - "crates/core/src/health.rs"
  - "src/stores/remote.js"
  - "src/stores/connection.js"
  - "src/components/shell/ConnectionBanner.vue"
  - "src/components/shell/connectionNotice.js"
  - "src-tauri/src/client/**"
  - "src-tauri/src/connection.rs"
  - "scripts/build-smetanad.mjs"
  - "src/components/settings/DevicesSettings.vue"
  - "src/components/settings/devices.js"
  - "crates/core/src/placement.rs"
  - "src/stores/serverFiles.js"
  - "src-tauri/src/commands/client_files.rs"
  - "src/components/settings/ServerSettings.vue"
  - "src/components/settings/serverChoice.js"
  - "src/components/projects/**"
  - "src/components/overlays/clientActions.js"
  - "server/smetanad/**"
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
- `src-tauri/src/client/` — the desktop as a client of one `smetanad`, see "The desktop is a client" below. The shell
  serves no phones and runs no endpoint of its own any more; `src-tauri/src/remote/` is gone.
- `crates/smetanad/` — the headless server, see "smetanad" below. A phone pairs with it: the local one on the desktop's
  machine, or one on another machine.
- `src/stores/remote.js`, `components/settings/DevicesSettings.vue`, `components/settings/devices.js`.

## Decisions worth knowing before changing them

**The endpoint starts late.** Binding one contacts a relay, and the server does not open a socket by itself
for a feature nobody has used. It starts on `remote_pair_begin`, and at the server's start only when some
device on the list was paired by a ticket (a phone inherited from the release is one). A changed relay URL takes effect at the next `remote_pair_begin` (the endpoint is rebound).

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

**The device list is its own file**, `remote-devices.json` in the server's data directory (for the local server,
`<app data dir>/server`), written atomically and
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

## The desktop is a client

Steps 5 and 8 of `docs/superpowers/specs/2026-10-25-headless-server-design.md`. **There is one build and one back end.** The
shell starts none of the workers and serves no phones; `src-tauri/src/client/` holds one iroh connection to one `smetanad`,
local or remote. The cargo feature `split`, the `tauri.next.conf.json` overlay ("Smetana Next", its own identifier and data
directory) and the in-process path were the transition and were deleted at switch-over (step 8): `grep -n split
src-tauri/Cargo.toml` is empty, and `smetanad` is an ordinary dependency (for its client half) and a sidecar (the binary).

- **Forwarding is generic.** `client::handler` wraps the `invoke_handler`: a name in `forward::LOCAL_COMMANDS` (the client's own:
  windows, dialogs, clipboard, opener, updater, login item, `connection_*`, the device settings, the server choice, the
  `client_*` files, `tracker_access_reset` — see `tracker.md`) goes to the Tauri command registered for it; a client command
  the shell does not register would be rejected rather than sent (a test keeps that set empty); **every other name is
  sent to the server** as an `Invoke` and the server's reply resolves the `invoke`. Arguments pass untouched (the dispatcher
  already reads the webview's camelCase). The `generate_handler!` in `lib.rs` is held equal to `LOCAL_COMMANDS` by a test, and
  `LOCAL_COMMANDS` to the server's `CLIENT_COMMANDS`.
- **Events** the server advertises (`HelloReply.capabilities`, names with a colon) are `Listen`ed to and re-emitted with
  `app.emit` under the same names; `terminal:output` and `terminal:log` come with `terminal_attach`. `terminals.js` applies the
  server's coalesced and `reset` payloads (`applyOutput`) and holds output that arrives while an attach is outstanding.
- **State** (`client/state.rs`, `connection:state` event, `connection_state` command, `mode: 'client'`): `connecting`, `connected`,
  `reconnecting`, `incompatible`, `unpaired` and `failed` (no data directory to keep a server in; nothing retries), with an `epoch` that counts connections. `Link` is the one holder; `supervise` is its
  one writer, with backoff 0.5 s doubling to 10 s, and a slow poll (15 s) for the two states a person must fix.
  **No call is ever retried.** A call with no connection is refused with a reason that says nothing was changed; one in flight
  when the connection ends is answered "never sent" or "may or may not have been made". The one exception: a **read** (the allowlist `READS`
  in `forward.rs`) made while the **first** connection of the run is still being made waits for it, up to 45 s, because the stores
  read as the window mounts. A write is never held, so a double press while the server starts cannot start two of anything.
- **Front end.** `stores/connection.js` holds the state; a store that must read its snapshot again registers with
  `onReconnect` (tracker, terminals, runs, vcs, sessions history, conversations, and `DesktopApp.vue`'s `catchUp`, which re-lists the tree, reloads tabs left in `error` by a refused read, and re-reads head, config and storage). Each new epoch runs them; an app that mounted
  onto an already-up connection does not run them for it. `DesktopApp.vue` dims the body while stale and draws
  `ConnectionBanner` (words in `connectionNotice.js`). In a browser `mockBackend.js` answers `mode: 'browser'`, always
  `connected`, and nothing is drawn.
- **Local placement.** The server's data directory is `<app data dir>/server` (its own `settings.json` and `remote-key` must not
  meet the shell's files), the shell's own key and paired marker are in `<app data dir>`, and the server's output goes to
  `<app data dir>/smetanad.log`. `spawn::ensure` asks the directory's lock (`smetanad::server::is_owned`), and if nothing owns it
  starts the bundled `smetanad run --data-dir … --inherit-from <app data dir> --bd … --resource-dir …` in its own process group
  (Windows: detached), so closing or updating the app leaves it running. Pairing is `Client::connect_as`, which reads
  `local-pairing.json`, pairs once and re-pairs by itself when the server no longer knows the key. A server killed without a
  goodbye is noticed at once by its lock being free.
- **Version lock-step.** A different `protocol_major` is `incompatible` (the reason names `smetanad restart --data-dir …`, since
  the old server cannot be asked). A server with a *newer* `app_version` is `incompatible` (update the app) and is never restarted. A different, older `app_version` with the same major makes the shell ask `server_restart now`
  without `confirmed`: nothing alive restarts it (once per running version, so a binary that did not change cannot loop);
  otherwise `connected` carries `outdated` with the list, and the banner offers *Restart now* (confirmed with exactly that
  list) or *Restart when finished*.
  The server's `app_version` is `smetanad::VERSION`, baked in by `crates/smetanad/build.rs` from `version` in `src-tauri/tauri.conf.json`
  (the one field a release bumps); the crate manifests stay 0.1.0 and the release does not bump them.
- **Updates replace the client only.** The release feed updates the app as before (`updates.md`); there is no run gate any
  more, because the runs and sessions are the detached server's and outlive the app's restart. The new bundle's `smetanad`
  takes over through the version lock-step above.
- **The bundle.** `tauri.conf.json` lists `binaries/smetanad` beside `binaries/bd` in `externalBin`, and its
  `beforeDevCommand` / `beforeBuildCommand` run `npm run build-smetanad [-- --release]`, which builds it into
  `src-tauri/binaries/smetanad-<triple>` (ignored by git). The release workflow builds it per target before `tauri build`.
- The shell still links `smetana-core` as a library for the device's settings file, the types its commands name and the
  sidecar path; no worker is started.

### Switch-over: the local server inherits the release's data

The release before the switch kept everything in the app's own data directory (`<app data dir>` = `~/Library/Application
Support/com.invisor.smetana` on macOS). On the first start of the new local server two things move, both **copy and never
move**, so an older release reinstalled still finds its data:

- **`smetanad::inherit`** (`--inherit-from`, run by `Server::start` after the lock is held and **before the key is read**):
  `remote-key` is copied (validated first), so the server's node id is the one paired phones already trust and **no phone has
  to pair again**; the devices of `remote-devices.json` are merged into the server's list only together with that key (a list
  bound to another key is left alone); `attachments/` is mirrored (hard links where possible), since issues name the old
  paths and agents go on reading them there. A server that already has its own key keeps it. The old `settings.json`'s
  `remote.relayUrl` is read too (`Report::relay`, kept in the marker): the server binds its public endpoint at start when an
  inherited phone is on the list, before the shell has handed the settings over, and `server::relay_url` takes that relay
  until the server has a `settings.json` of its own, so a phone paired through a person's own relay finds the server there.
  The shell passes the flag on every start, which is what lets the update itself need no step of its own.
- **It never stops the server.** Each failed step is a line in `Report::problems`, logged, and the start goes on: a key that
  does not parse or is not text (useless to the old release too) leaves the server to make its own, and the list bound to it
  stays where it is; a key that cannot be read or written *this time* sets `key_pending`, and the server serves that run
  under a key it does not keep, **with its public side refused** (`Remote::refuse_public`: no public endpoint, no
  `remote_pair_begin`), so the next start can still take the real one and nothing pairs with a node id that will vanish; an
  attachment that cannot be mirrored is skipped and holds nothing back. `inherited.json` is written last unless the key is
  pending or the device merge failed, and every step skips what is already done.
- **The device merge happens once, ever** (`inherited-devices.json`, written as soon as it succeeds, and checked before it
  runs): merging on a later start would bring back a phone the person revoked in the meantime. Tests: `inherit.rs`
  (including a revoke followed by a restart with the completion marker missing), and `tests/server.rs` (a real
  `Server::start` over an old directory keeps the node id, the phone and the relay; a damaged old key still starts; an
  unreadable one starts under an unkept key and refuses to pair).
- **A development build inherits nothing.** `npm run tauri dev` runs under the release's identifier, so Tauri's data
  directory *is* the release's. `src-tauri/src/dirs.rs` moves every directory the shell reads to a sibling,
  `<identifier>.dev`, under `debug_assertions`, and `client::placement` passes no `--inherit-from` and no settings file to
  migrate in that build. The migration is checked on a copy of a real directory, never on the live one (the design's rule;
  until the switch-over the "Smetana Next" identifier kept them apart).
- **Settings**: `client/migrate.rs`, below ("Migration", source one): the server half of the old `settings.json` (projects,
  per-project run settings and agents, profiles, languages, relay) goes to the server by `settings_adopt`; the device half
  stays.
- **Not moved, because it was never in that directory**: the runs' registry is `.smetana/runs.json` in each project and the
  transcripts are Claude Code's under `~/.claude/projects`; the server reads both where they are.

A person who moves to a remote server pairs the phone with that server (`smetanad pair --kind phone`); the local server's
pairing is untouched (`server/smetanad/README.md`).

## Protocol 1.15: the root of `crew:tree` names the lead's conversation (smetana-u0c9)

Additions only. `CrewNode.conversation` (optional, absent until known, only ever on a root) and `crew:tree` joins the
phone's event allowlist, which a phone needs in order to hide the Crew lead's registry record
(see "A Crew lead stays in the registry" in `terminal.md`). A phone on an older desktop is refused the subscription and
reads it as a local name: nothing is shadowed. `crew_list {}` answers `[{ project, root, nodes }]`, every live tree, for a
client that connected mid-run or after a server restart; it replaces the client's trees whole. The mobile build's own
`PROTOCOL_MINOR` stays 3.

## Protocol 1.14: the phone hears settings:changed

Two changes under one minor (smetana-x9r9; 1.13 is smetana-jdw5's). **The phone draws the desktop's theme, density,
kanban rules and column order**, so those moved to the server half of the settings (see the second clause of the rule under
"Settings"): the examples of `settings_patch`, `settings:changed` and `settings_device_half` in `contract/` changed shape
(`appearance { theme, density }`, `kanban`, a project's `columnOrder`), which is what the digest in `versions.json` pins.

- **`settings:changed` joined the phone's `ALLOWED_EVENTS`** (and `mobile/src/allowed.js`), at the end, so the phone's
  list stays the front of the server's and `terminal:restorable` is no longer the last capability. A desktop older than
  1.14 refuses the subscription, which `bridge/event.js` reads as a local name: no live updates, and the re-read on
  reconnect and on showing the page stays.
- **The phone does not fold the payload.** On the event it calls `refreshFromDesktop()` (the same `settings_load` with
  `project: null` a reconnect makes, so a project pinned on the Runs screen stays pinned). `applyDesktopSettings` copies
  `appearance.theme` and `density` only at `minor >= LOOK_MINOR` (14): an older server answers `dark` and `comfortable`
  from its defaults, which the phone cannot tell from a choice. `kanban` and `columnOrder` are copied whatever the
  minor. `uiFontSize` is never copied, by the person's decision. The phone subscribes only when the desktop is at 14.
- **Keep in step:** `PROTOCOL_MINOR`, the `versions.json` entry, `LOOK_MINOR` in `mobile/src/desktopSettings.js` and
  `protocol_minor` in `mobile/src/devBackend.js` all say 14; `SERVER_ROOT`/`SERVER_APPEARANCE`/`SERVER_PROJECT` in
  `split.rs` and `ROOT`/`SERVER_APPEARANCE`/`PROJECT_FIELDS` in `settingsSplit.js`; `ALLOWED_EVENTS` in `bridge.rs` and
  `allowed.js`.

## Protocol 1.12: the agents list (smetana-ahp9)

Additions only. **The list of driven conversations and of offline rows is the server's; every client mirrors it.** Before,
`started` in `stores/conversation.js` was the memory of the window that called `session_start`, so a phone saw only what
it began itself and the desktop drew a conversation the server still led as a dim offline row after the app restarted.

- **"Every session" below means every non-Crew driven session.** A Crew lead is an ordinary `Live` (`crew_lead`) and is
  left out of `session_list` and of `session:state`; Crew panels come from `crew:tree`.
- **`session_list {}`** answers `[{ id, project, agent, state, conversation, title, work, startedAt, cwd }]` across all
  projects, sorted by id, for every session the worker holds that no cross has closed (`removed`), ended or not. It is
  `Request::List`, not a widening of `Request::Live`: that one keeps only a running child, for the restart gateway.
  It is in `forward.rs` `READS` (the store reads at mount, before the local server answers; a refused read would leave
  the list empty on every cold start).
- **`session:state`** also carries `project`, `agent`, `work`, `startedAt`, so a client that never started the session
  draws its row from one event. **`session:removed { id }`** goes out once from `Request::Close` (the second cross says
  nothing) and where a startup that never became a conversation leaves the map. A closed session stays in the map with
  its flag and still emits a last state as its child is reaped; the client keeps a set of ids it was told are gone, or
  that event would draw the row again.
- **`terminal:restorable { project, records }`** is the whole `.smetana/agents.json` of a project after every write to it
  (`restore::announce`, beside each `record`/`drop_record`, and `terminal_forget`, which takes the `Context` for it). A
  list and not a nudge: the file is written several times around a spawn. Where a branch also sends `session:state`, that
  goes first.
- The phone may now call `session_list`, `session_close`, `terminal_restorable`, `terminal_forget` and `terminal_remove`
  and hear the three events; a terminal still cannot be written to from a phone, but any row may be closed from any
  device (the author's answer). The phone's lists stay a prefix of the server's, so those four commands moved out of the
  desktop-only tail.
- An older server answers `Forbidden` to `session_list`; the store keeps its list and logs to the console. After a
  server restart the list is shorter on purpose: a session the server no longer holds is offered by the registry.
- A spawn announces `session:state` and then `terminal:restorable` (`announce_new`), so another device never sees a driven
  conversation as an offline offer; a closed session emits no further state.

## A server on another machine (step 7, protocol 1.11)

`docs/superpowers/specs/2026-10-25-headless-server-design.md`, "Things that name a path on the client" and "Remote placement".

**Which server.** `client/target.rs`: `server-target.json` in the app data directory, `{"kind":"local"}` or `{"kind":"remote","node","relay"}`
(nothing secret: the ticket's secret is spent at pairing). A damaged file is the local server. `Link` holds the target and a
generation (`set_target`); `attempt` branches on it. **Local** is as before (`ensure`, lock watch, version lock-step). **Remote**
(`attempt_remote`, `Client::connect_remote` in `smetanad::client`): no start, no lock watch, no version gate and **no settings
migration** (a fresh VPS must not adopt this machine's project paths); a different major is `incompatible` with a reason that names the
side to update. Choosing another server drops the old connection (the loop notices the generation), leaves the local server running,
and does not respawn it while the choice is remote; going back finds or starts it. `hold` is the shared connected loop. The client
shell commands (all in `CLIENT_COMMANDS`, `LOCAL_COMMANDS` and the `generate_handler!` list):
`server_target`, `server_use_local`, `server_use_remote { ticket, stopLocal }`, `server_local_work`.

**Pairing by ticket.** `Client::pair_remote` reads a `smetana2:` ticket (`check_ticket`: a phone's, an expired or an unusable-relay one is
refused before anything is dialled), binds an endpoint on the ticket's relay with the same device key the local client uses
(`local-client-key`), and waits up to 150 s for the person to confirm in the terminal that ran `smetanad pair`. Pairing is the only step
that can fail a switch; a failure leaves the device where it was. **Not covered by a test:** the dial through a real relay (no relay in
this crate's tests); `check_ticket` and the rest of the switch are.

**Leaving a busy local server.** `server_work {}` (read-only: the gateway's live work) and `server_restart` mode `stop-when-idle`
(like `when-idle`, but the host's `stop` callback runs: `Exit::Stop`, exit 0, which a service manager must not restart; reply outcome
`stopping` when nothing is alive, `scheduled` otherwise; it replaces a waiting restart and `cancel` withdraws it). `server_use_remote`
asks for it only when the person chose "stop it when it is finished"; the settings tab asks first when `server_local_work` is not empty
(`serverChoice.js::nextStep`). Nothing is ended by the switch.

**Four server commands** (`SERVER_ONLY_COMMANDS`, desktop devices only, a phone is `Forbidden`), the logic in `smetana_core::placement`:
- `folders_list { path }`: directories under the server user's home (`HOME`, else `USERPROFILE`), hidden ones left out, symlinks resolved
  and dropped if they leave the root, at most 2000 entries; a path outside is `outside`. Every path the client sends back was listed by
  the server; it never composes one.
- `project_clone { url, parent, name }`: `git clone -- url dest` with `protocol.ext.allow=never` and `protocol.file.allow=never`, no terminal
  prompt, 15 minutes, in its own process group that is killed whole on a timeout or a dropped future, into a folder this call created and a drop guard removes; `valid_clone_url` (mirrored by `validCloneUrl` in `folderBrowser.js`, never more permissive) refuses `-…`, `ext::`,
  `file:` and local paths; `parent` must be under the root and the destination must not exist.
- `server_file_read { path }`: one file, base64, at most `MAX_ATTACHMENT_BYTES`, **only** inside the attachment store or a known project's
  `.smetana/reports` (the projects are read from the server's `settings.json` at each call; a `reports` or `.smetana` that is a symlink
  leaving the project is not a root). The test is on real paths (`canonicalize`) of both sides, plus a refusal of any `..` before the
  filesystem is asked; a missing file outside every root is the same `outside` as an existing one. Tests: `placement.rs` (traversal,
  symlinks, absolute paths, a symlinked reports folder) and `desktop_bridge.rs` (end to end, and a phone is refused).
- `server_work`, above. A refusal travels as `{ kind, message }` (`PlacementError`'s own `Serialize`).

**Front end.** `isRemote` (connection store) is `mode === 'client' && local === false`. `stores/serverFiles.js`: `pathsForServer` (local:
the paths; remote: `client_file_read` then `attachment_write`, the stored paths come back), `openReportInBrowser` (local:
`client_open_document`; remote: `server_file_read` then `client_open_fetched`). The two openers hand only an `.html` to the system
(`commands/client_files.rs`), and the fetched copy is in a private folder under the app's cache directory (not a shared /tmp), swept after a day. The report button is drawn only when `connection.mode` is `client`. Consumers: `attachments.js::importPaths`
(New task window), the composer (`stores/app.js::pathsForServer`, called by `ConversationView` when files are chosen or dropped, so a chip names the server's copy from the start; it answers `{ paths, failures }`, each file on its own, so one refusal does not cost the batch and is shown on the panel's error line; the phone's `app.js` answers with the same list and no failures), `terminals.js::sendDroppedPaths` (a file dropped on a terminal),
`DesktopApp.vue` (report button on a run report's tab; `DocumentModeToggle` gets a second button and `toggleLane(2)`). The image window needs
nothing: `image_read` is already a server command and its bytes cross the connection.
Add project: `onAddProject` opens the `server-folder` dialog window (`FolderBrowserModal`, state in the app window, `projects.js::listServerFolders`
/ `cloneOnServer` / `addProjectAt`) when `isRemote`. **Hidden, not disabled:** `DesktopApp.vue` provides `smHideClientActions` (= `isRemote`),
and `PointerMenu` and `MenuButton` draw `withoutClientActions(items)` (`overlays/clientActions.js`: `reveal`, `reveal-log`, `open-log`,
`open-cwd`, separators cleaned); a new menu row of that sort is added to that list. The Server tab (`ServerSettings.vue`, `serverChoice.js`) exists only
when `connection.mode` is `client`.

**Another server is another set of projects, and the move is guarded.** The Server tab only *requests* a switch (`connection.js::requestServerSwitch`,
events `server:switch-request` / `-ack` / `-result`); the app window, which owns the tabs, runs `projects.js::switchServer`: the unsaved-edits question
(`askUnsaved`), saving to the server still connected, a flush of the pending settings to it, and only then the shell's `server_use_remote` /
`server_use_local`. A cancel answers `{ cancelled: true }`; a refusal by the shell throws; either leaves the device where it was. After the move,
`View.target` (`local` or the remote key) tells the front end the connection is to a different server; `connection.js::onServerChange` hooks run before
the ordinary resync, and `projects.js::followServerChange` takes the new server's list and open project (`settings.js::adoptServerProjects`, which also
voids unsent patches and re-baselines), drops the old project's tabs and tree and moves to the server's open project, or the empty state if it has none.
It assumes nothing is dirty and says so with `console.error` if something is. **No client path reaches another server, structurally:** from the moment
the target changes until `adoptServerProjects` has returned `connection.switching` is true and `settings.js::flushSplit` computes no server patch; a
patch is recorded with the target it was made for and dropped at send time if that no longer matches.

**Install.** `server/smetanad/README.md` and `smetanad.service` (also in every archive); the `smetanad` job in `.github/workflows/release.yml`
builds `crates/smetanad` per platform, packs it with `bd`, `resources/`, the unit and the README, and uploads archive and `.sha256` to the
release the app job published. `tests/docs/smetanadInstall.test.js` holds the unit's flags to the CLI and the three files to each other.

## Settings: the server's half and the device's (protocol 1.10)

`crates/core/src/settings/split.rs` draws the line by a rule of two clauses: a field is the server's if a worker on the
server reads it, **or if a paired phone draws it** (added in 1.14). The server's: `agent`, `model`, `agentRoles`, the
four languages, `agentPrompt`, `conversationPanel`, `subscription`, `runFailover`, `remote.relayUrl`,
`git.removeWorktrees`, `openProjects`, `lastProject`, the whole `kanban` section, `appearance.theme` and
`appearance.density`, and per project `runSettings`, `agents`, `usedAt` and `columnOrder`. Everything else is the device's
(`appearance.uiFontSize`, layout, `editor`, `reports`, `git.autoFetch`, `window`, `updates`, `notifications`, `dialogs`,
`deviceSeeded`, `lookShared`, and a project's tabs, folds, selected task, other orders, names and `storageWarnedMib`);
a field in neither list is the device's, and a test pins the server list. `appearance` and `git` are split by field
(`SERVER_APPEARANCE`, `SERVER_GIT`), and `src/stores/settingsSplit.js` writes the same lists a second time. `usedAt` is
kept on both sides for each file's own project limit. The cost, taken knowingly: one server, one look, so two desktops on
one server share a theme and a board, and the last to migrate wins.

- **Server half: a patch.** `settings_patch { patch }` (server-only command) is a JSON merge patch over the server's keys:
  the server drops anything else, applies it to its `settings.json` under one process-wide lock, validates it as a read does,
  writes, and **inside that lock** emits `settings:changed` (the server half whole) so clients hear states in the order they
  were written. Two clients that changed different fields both keep them; a list (`openProjects`, `runFailover.priority`) is
  replaced whole, so two simultaneous edits of the *same* list lose one.
- **Device half: a file on the machine.** `device_settings_load` / `device_settings_save` are client commands (answered by the
  shell, in `LOCAL_COMMANDS` and `CLIENT_COMMANDS`) over the shell's own `settings.json`;
  `merge_device` writes the device's keys and leaves the server's as that file has them. `window.rs`, the updater and the
  dialog sizes keep reading that file unchanged.
- **Front end** (`stores/settings.js`, pure parts in `stores/settingsSplit.js`): `connection_state` saying `client` turns it
  on; a browser (the mock) takes no branch and saves one document with `settings_save`. The store keeps a `baseline` (the server's half as last agreed); a flush
  saves the device half whole and sends `diff(baseline, now)` as the patch. `settings:changed` and a reconnect fold the
  server's state in with this window's unsent edits laid over it. A refused patch is **not kept or replayed**: the server's
  state is read back and wins. Which project is open stays this window's own (`lastProject` is sent, never adopted).
- **Numbered states.** The answer to a patch and every `settings:changed` carry `rev`, counted under the write lock and
  restarting at 1 with the server. The store ignores a state whose `rev` is not newer than the last it took in (and resets the
  count on each new connection), and ignores `settings:changed` while a patch of its own is unanswered: that state is older
  than something it sent, and the answer to the last patch carries every state before it. The baseline the store compares
  against is the confirmed state **plus the patches still on the way**, so an echo can never undo a later edit.
- **A failed patch.** A refusal by the server (a handler error) takes back that patch's fields alone: the store reads the
  server's state and keeps whatever else was typed since. A call that did not reach the server's handler is told by words
  every such shell rejection carries - `NOT_SENT` ("Nothing was changed") and `MAYBE_SENT` ("may or may not have been made") in
  `src-tauri/src/client/state.rs`, matched by `TRANSPORT_WORDS` in `src/stores/settingsSplit.js`, each side pinned by a test and
  naming the other; keep both when rewording a rejection. Such a failure keeps the change pending; the reconnect reads the server's state and sends what is
  still different. Neither is a replay of a command: it is the state being brought into agreement.
- **`openProjects`.** The store keeps the project open in this window in the list whatever another client did to it; that
  re-append is this window's bookkeeping and is not sent back (`diffViews`), so a client that closed a project does not see it
  return. Two clients editing the list at the same moment still lose one edit (the list is replaced whole).
- **Migration** (`src-tauri/src/client/migrate.rs`), on every connection before `connected`, three sources:
  1. The device's `settings.json` still holds a server half (what every install of a release from before the switch-over
     has on its first start after it): `settings_adopt` offers it (taken only by a server with no `settings.json`), then `retire_server_half` (which sets `deviceSeeded` in the same write as it resets the server's keys, so no crash between two writes can lead to a seeding over the device's own half) keeps a copy of
     the file (`settings.pre-split.json`, never overwritten; a later retire whose file differs writes
     `settings.pre-split.2.json` and on, so a second server half is never lost) and resets the server's keys in it. The device half already
     there stands.
  2. The device file is not marked (`deviceSeeded`) and source one did not fire: an earlier build of the client (step 5)
     forwarded `settings_save`, so a "Smetana Next" install has the whole document in the **server's** file and only `dialogs`
     on the device. `settings_device_half` answers the device half of the server's file (or `null`); `seed_device` writes it
     into the device file with this machine's dialog sizes laid over it and sets `deviceSeeded` in the same write, after a copy
     to `settings.pre-seed.json`. The server's file is left whole (the device keys there are dead data), so nothing is lost and a
     rerun does nothing. A marked file is never seeded again.
  3. (1.14) The device file is not marked `lookShared`: an already split desktop holds its theme, density, `kanban` and
     column orders in its own file, and they are the server's now. After sources one and two, `split::look_patch` builds the
     **whole** look (theme, density, `kanban`, every project's `columnOrder`, defaults and empty orders included, since a
     person who went back to dark chose it), the client sends it with `settings_patch` and `mark_look_shared` sets the
     mark; a failure leaves the file as it is and the next connection tries again. The order is fixed: `settings_adopt`
     takes only a server with no settings and a look patch would give it some. A pre-split release file carries its look
     inside the adopted half (`legacy_server_patch` ignores the look when deciding whether there is anything to hand over,
     or a split desktop would retire its look before sending it); if the server **refuses** the adoption the look is sent
     with `settings_patch` before the retire. `retire_server_half` and the seeding from the server leave the device with no
     look and set `lookShared` in the same write, as does a fresh install (no file), so source three sends nothing for them.
     A desktop meeting a server that has a look overwrites it with its own.
  A server that already had settings keeps them; a second desktop's old server half, or a second one on the same desktop, survives only in its `.pre-split` copy.
  Known edge: source two seeds from the server's file whenever the device file is unmarked and source one did not fire, so a
  device file with a customised device half but no server half, meeting a server file that already holds device keys, takes
  the server's. At switch-over it does not arise: a release file always carries a server half (source one fires and marks the
  file), and the local server's directory is new, with no file of a Next build in it.

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
clipboard, opener, updater, login item, `tracker_access_reset`, which restarts the local server). The contract tests fail for a
command or event with neither a pinned example nor a client declaration, and scan the stores and the workers for event names.
`dispatch.rs` declares each command once in the `calls!` table. Where the kind changes behaviour: a phone's `session_send`
may carry no attachments and its `session_start` is confined to the attachment store; a desktop's is not.

**Terminal stream.** Chunks are committed by the worker for every session each tick, numbered, kept in a bounded history
(`terminal/stream.rs`) and offered to the sink (`Sink::terminal_chunk`). `terminal_attach` takes an optional `after`; the
reply gains `resumedAfter` only when `data` is the continuation after that number, and otherwise `data` is the ring as of
`seq`. A desktop device that attaches is registered for the session (`Outbox` in `bridge.rs`): output is coalesced per
session and per device, a device that falls behind its ceiling (`OUTBOX_CAP`) is sent the ring again as `terminal:output`
with `reset: true`, and the worker is never made to wait. The PTY has the size of the client that last wrote or resized it
(`Viewers`).

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

## Protocol 1.13: tracker writes

Additions only. `tracker_update { id, patch }` and `tracker_delete { id }` join the phone's allowlist as its last two
entries (after the 1.12 names), in `ALLOWED_COMMANDS` and in `mobile/src/allowed.js`, in that order. They were already
`SERVER_COMMANDS` with handlers in `dispatch.rs` and contract examples, so no contract file changed and `versions.json`
has `1.13` with the digest the contract test reports. The server's list moved the two names out of the tracker group
into the phone's prefix, since the phone's lists must stay the front of the server's.

- **Why:** the phone's tap-a-card sheet carries the desktop card menu in full: Block/Unblock and Move to... are a status
  written through `tracker_update`, and Delete is `tracker_delete`.
- **Still forbidden to a phone:** `tracker_close`, `tracker_reopen` and everything else not on the list. The desktop menu
  writes a status through `tracker_update`, so closing and reopening need no door of their own.
- **Gating:** the phone hides these actions on a desktop whose `protocol_minor` (`desktopMinor` of the bridge status) is
  below 13, because an older desktop answers `Forbidden`. It reads the minor, not the `capabilities` list, which the
  phone's front end never sees.

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
