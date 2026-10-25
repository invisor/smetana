# One server, many clients — splitting Smetana into `smetanad` and its clients

Today everything this app supervises lives inside the desktop process: the tracker worker, the
bd sidecar, PTY and driven sessions, runs, worktrees, git. The phone companion
(`2026-10-15-mobile-companion-design.md`) already treats that process as a server and itself as a
client. This design finishes the thought: the part that does the work becomes a headless process,
`smetanad`, and **every** user interface — the desktop app included — becomes a client of it.

## Why

- **Isolation.** Some people do not want autonomous agents running on their own machine. With a
  headless server they install `smetanad` on a VPS and the agents, the code and the credentials the
  agents use live there; the laptop only draws.
- **One code path.** The desktop talks to the server the same way locally and remotely, and the same
  way the phone does. A feature built once works in every placement.
- **Work outlives the window.** Closing the desktop app today kills agents and runs. With a server,
  closing a client closes a view.

## Decisions taken

- The split is **total**: the desktop app is a client even on one machine. It does not keep an
  in-process copy of the core for the local case.
- **The whole current desktop feature set** works through the server: board, inspector, runs,
  conversation panel, terminals (read, write, resize, create shells), file tree and editor, Git panel,
  branch compare, settings, attachments, reports.
- **One owner, N devices.** A server belongs to one person. Every client — desktops and phones — is a
  device of that person, paired the way phones are paired today. There are no users, roles or
  per-device permissions beyond what a device *kind* is allowed (see "Capabilities").

## Goals

1. `smetanad`: a headless binary for macOS, Linux and Windows that runs the core with no Tauri, no
   webview and no display, configured and paired from a command line.
2. The desktop app reaches all of its back-end functionality through a connection to a `smetanad`,
   local or remote, chosen in its settings.
3. Locally, installing the desktop app is still all a person does: the app ships `smetanad`, starts
   it when it is not running, and pairs with it without asking anything.
4. Remotely, a person installs `smetanad` on a server, pairs their desktop and phone with it, and gets
   the same app.
5. The phone keeps working unchanged in behaviour, now talking to `smetanad` rather than to a desktop.

## Non-goals

- Several people on one server; accounts; roles; audit by person.
- One client talking to several servers at once. A client is connected to one server; switching is a
  setting.
- A web client served by `smetanad`.
- Moving the mobile app off iroh, or changing its feature set.
- High availability, clustering, or a server that survives its own machine.

## Anti-goals — it would be a failure even if it worked

- **A local install that makes the person manage a service.** If a desktop-only user ever has to
  start, pair or update the local server by hand, the split has made the common case worse.
- **Two back ends.** A "local fast path" that bypasses the connection would let the two placements
  drift until a feature works only on one of them.
- **Anything readable in transit or on a relay.** The mobile design's guarantee — end-to-end
  encryption, nothing stored on our servers — extends unchanged to desktop clients.
- **A remote server that is easier to take over than the laptop it replaces.** A VPS full of agents
  and credentials is a better target than a laptop; the only way in is a paired device key.

## Architecture

```
 desktop client (src-tauri shell + src/)  ─┐
                                           ├─ iroh/QUIC, E2E ─  smetanad (core + endpoint)
 phone (mobile/)                          ─┘    (direct, or via relay)        │
                                                                              ├─ bd, git, PTYs, agents
                                                                              └─ push gateway (as today)
```

### Units

1. **`crates/core/`** (name is the builder's call) — everything under `src-tauri/src/` that does work
   today and does not need a window: `tracker`, `runs`, `session`, `sessions`, `terminal`, `files`,
   `vcs`, `git`, `agents`, `attachments`, `settings` (the server half, below), `project`,
   `shell_env`, `rlimit`, the push half of `remote`. No `tauri` dependency. Where code reaches for
   `AppHandle` today it gets an explicit context instead: data directory, an event sink, the sidecar
   path. `remote/dispatch.rs` already calls these modules without Tauri and is the model.
2. **`crates/remote-protocol/`** — unchanged in role; it grows to carry the full command and event
   surface, capabilities per device kind, and the terminal stream framing below.
3. **`smetanad`** (a new binary crate) — the core, the iroh endpoint (today's `remote/endpoint.rs`,
   `pairing.rs`, `devices.rs`, `bridge.rs`, moved), a dispatcher for every server command (today's
   `dispatch.rs`, grown from the phone allowlist to everything), and a small CLI: `run`, `pair`,
   `devices`, `revoke`, `status`.
4. **`src-tauri/`** — the desktop client shell. It keeps what belongs to the machine the person sits
   at: windows (`window.rs`), the native dialogs, clipboard, opener, the client's own updater, its
   login item, appearance. It gains an iroh **client** connection to one server and forwards to it.
5. **`src/`** — unchanged in shape. Stores remain the only files that know Tauri exists; components do
   not change.
6. **`mobile/`** — pairs with and connects to `smetanad` instead of the desktop.

### How the webview reaches the server

The webview does not open network connections. It keeps calling `invoke` and `listen`; the Rust shell
owns the iroh connection and forwards:

- A server command from a store becomes a frame on the connection; the reply resolves the `invoke`.
  The builder chooses between one generic forwarding command and keeping per-command Tauri wrappers
  that forward; the store-facing names (`tracker_snapshot`, `terminal_write`, …) do not change either
  way, so the contract files and `mockBackend.js` keep matching.
- A server event received on the connection is re-emitted to the webview under its existing name
  (`tracker:delta`, `terminal:output`, …).
- Client-local commands (`window_*`, dialogs, clipboard, opener, client updates, login item, app
  version) are answered by the shell itself and never cross the connection.

This keeps the IPC transport the one mock boundary for front-end tests, and keeps network code,
keys and reconnect logic out of JavaScript.

### Connection state is a first-class thing in the UI

Today the back end cannot be "away". After the split it can, locally (the server is starting or
crashed) and remotely (network). The shell exposes a connection state — connecting, connected,
reconnecting, incompatible, unpaired — through one store, and the app draws it: views keep their last
data and show that it is stale, writes are refused with a visible reason rather than queued. On
reconnect each store resynchronises from a snapshot, as `tracker_snapshot` already does after a
project switch. No write is ever replayed automatically.

### Local placement

- The desktop bundle ships `smetanad` beside `bd` (`bundle.externalBin`).
- At launch the shell looks for a local server (its data directory has a lock/pid and its endpoint
  address). If none answers, it spawns one, **detached**, so closing the app leaves it running.
- **Local pairing needs no person.** `smetanad` writes, mode 0600, a local-pairing credential in its
  data directory; a desktop running as the same OS user reads it and pairs on first connect. Anyone who
  can read that file can already read the server's key and every repository, so this adds no exposure.
- The local connection is iroh by direct address on loopback with no relay — the same protocol as
  remote, not a second transport.
- **The local server inherits today's data** (at switch-over; the Next build starts empty — see
  "Transition"). On first start it takes over the desktop's existing
  data directory: the runs registry, transcripts, attachments, `remote-key` and `remote-devices.json`.
  Phones already paired with the desktop therefore keep working against the local server with no
  re-pairing — the key they trust is the same key. **My call**, from Goal 5.
- **Version lock-step locally.** The local server is the one in the bundle. After the app updates, an
  older local server that is still running is replaced by the new one. A restart of `smetanad` ends
  every child it owns, so the gate is not "no run" but **no live work of any kind**: runs, agent
  sessions, driven sessions, plain shells. If there is any, the person is shown the list and chooses
  between restarting now and restarting when it is all finished; nothing is ever killed silently.
  **My call.** While the old server keeps running the new client talks to it under the versioning rule
  below — same major, capabilities hide what the old one lacks — so the board does not go dark. Only a
  major bump makes the client wait, with that list on screen.
- **Switching the desktop to a remote server** leaves the local server running and stops the shell
  from respawning it. If the local server has live work at the moment of the switch, the switch says
  so and offers to stop it after that work ends. **My call.**
- Whether the local server starts at login on its own is a setting, off by default; the existing
  `autostart.rs` reasoning (the OS is the truth, no ACL grant) applies to it.

### Remote placement

- `smetanad` ships as a standalone archive per platform and, for Linux, a systemd unit. Install
  documentation lives next to `server/README.md`.
- First pairing has no screen: `smetanad pair` prints a pairing ticket as text and as a QR code in
  the terminal; the desktop accepts the ticket pasted, the phone scans the QR. Confirmation happens in
  the terminal that ran `pair`. Later pairings can also be approved from an already paired desktop,
  as the Devices tab does today.
- The server's environment is its own: agent CLIs (`claude`, `codex`, …) are installed and logged in
  on the server, as are git credentials. `smetanad status` reports what is missing (bd, git, each
  agent profile's binary and whether it answers), and the same report reaches the client, which shows
  it where `tracker_health` is shown today.
- Remote updates are the operator's: the server is a binary they replace. The client tells them when
  the server is older than it needs (see "Versioning").

### Capabilities per device kind

`HelloReply` already carries a capability list. The server decides it by device kind, and **the kind
is fixed when the device is paired**, never taken from what the device claims in `Hello`: a stolen
phone key must not be able to call itself a desktop and get a writable shell. A pairing ticket names
the kind it admits (`smetanad pair --kind desktop|phone`, and the Devices tab picks one), the kind is
stored in the devices file, and `Hello`'s platform field becomes informational. **My call.**

- **desktop** — every server command and event.
- **phone** — today's `ALLOWED_COMMANDS` / `ALLOWED_EVENTS`, unchanged. A terminal stays read-only on a
  phone.

This is not a permission system: it is the existing allowlist, keyed by kind. All devices belong to
the one owner.

### Settings: what is the server's and what is the device's

`settings.json` splits in two.

- **Server** — the project list, per-project configuration, agent profiles, run settings, relay URL,
  anything an agent or a run reads. Every client sees the same values.
- **Device** — theme, density, font size, panel widths, window geometry, the selected task per
  project, which server to connect to. Each desktop keeps its own.

The builder draws the exact line by one rule: if the server would read it, it is the server's. A
migration reads an existing `settings.json` on first launch of the new desktop and moves each half to
its place.

With several clients the server half can no longer be saved as a whole document on a debounce: two
desktops would silently undo each other. The server owns the writes — a client sends a patch of the
fields it changed, the server applies it and broadcasts the result as an event every client follows.
The device half keeps today's debounced whole-file save, locally. **My call.**

### The active project

Today the tracker worker holds one active project and a phone's switch moves the desktop window with
it (`remote:project-switched`). That stays: the server has one active project and every client follows
it. **My call** — with one owner, the person is on one project at a time, and per-client projects
would need a tracker worker per client.

### Things that name a path on the client

On one machine a path picked in a dialog or dropped on a window is a path the server can open. On a
remote server it is not.

- **Adding a project.** The native folder dialog stays for a local server. For a remote server the
  client offers a server-side folder browser (a new server command listing directories under a root),
  and "clone a repository" as an alternative to picking.
- **Attachments and dropped files.** `.claude/rules/attachments.md` says the path travels, never the
  bytes. That holds locally. Remotely the bytes travel: the client uploads through
  `attachment_write` (already used by the phone), and what is stored is the server's path. The image
  window and figures are fetched from the server by the same path rule.
- **Run reports.** A report is a self-contained HTML file opened in the person's own browser. With a
  remote server the client fetches its bytes over the connection (one server command reading a file
  under the reports directory), writes them to a temporary file and opens that. The same command
  serves the image window. **My call** — reports are in the feature set, so they are fetched, not
  hidden.
- **"Reveal in Finder", "open in editor" and similar** act on the client's machine. With a remote
  server they are hidden, not drawn disabled with a puzzle.

## Terminals over the connection

The model is already client and server inside one process: `pty.rs` owns the process, every chunk goes
into `ring.rs` and `screen.rs`, and a `TerminalView` attaches by repainting from the ring and then
following `terminal:output`. Over a network four things are added.

1. **Sequence numbers.** Every output chunk carries a per-session sequence number; `terminal_attach`
   answers the ring together with the sequence it ends at. A client that reattaches after a drop asks
   for what follows its last sequence, and gets the ring again if that has been trimmed away. No byte
   is shown twice or lost between the snapshot and the stream.
2. **Backpressure.** The server coalesces output per session and per client, and a client that falls
   behind by more than a bound is sent a fresh ring instead of an ever-growing queue. A slow phone
   never slows the PTY or another client.
3. **Size with several viewers.** The PTY takes the size of the client that last wrote to it or last
   resized it, as tmux does. A read-only phone never resizes.
4. **Driven sessions** (harness protocol, `session:events` / `crew:events`) are typed events already
   and need only the same resume-after-sequence rule, which `session_since` already provides.

Typing latency to a remote server is the network's, as with ssh. No local echo is attempted.

## Versioning

- The protocol carries a major version in the ALPN (`smetana/remote/1`) and a capability list in
  `HelloReply`, as today. A client hides what the server does not advertise.
- A client and server of different majors refuse each other with a message saying which side to
  update. Locally this never happens: the shell respawns the bundled server.
- The contract directory pins the shape of every command and event, now the full surface rather than
  the phone's subset.

## Security

- The only way in is a paired device key; unknown keys are closed before a byte is read, as today.
- Pairing tickets are single-use and short-lived, as today. `smetanad pair` is the only way to create
  the first one.
- Local auto-pairing relies on the OS user boundary of the data directory, as described above.
- `smetanad` refuses to run as root.
- Revoking a device (`smetanad revoke`, or the Devices tab) closes its connection at once.
- The relay sees ciphertext only. The push gateway sees a token and an encrypted blob only.

## Errors and degradation

- Server unreachable: views stay with stale marking; writes refused with a reason; a reconnect loop
  with backoff; the status footer says what is happening.
- Server environment incomplete (no bd, an agent not logged in): reported through the health surface,
  not as failures of individual actions.
- Incompatible version: one screen saying which side to update; nothing else is attempted.

## Transition: a separate build until the new one works

The released app keeps its current in-process back end until the client-server build has been used
and works. Until then the two live side by side, and the "two back ends" anti-goal is suspended for
exactly that period, with its end written into the plan as the last step.

- **A cargo feature, `split`, in `src-tauri`,** selects the client shell. Without it the app builds
  as today, in-process. Everything the split needs that is not the client shell itself (the core
  crate, the protocol, `smetanad`) is built unconditionally and lands on `main` as it is finished.
- **A separate build, "Smetana Next".** A Tauri config overlay (`tauri.next.conf.json`, merged with
  `--config`) gives it its own `identifier` (`com.invisor.smetana.next`), its own product name and
  therefore its own data directory, and turns `split` on. The installed release and the Next build
  can run at the same time on one machine and share nothing: not settings, not the device key, not
  runs. The Next build's `smetanad` gets its own data directory too (passed by the shell; the server
  never guesses it).
- Because the data directories are separate, **the Next build starts empty**: projects are added
  again, phones paired again. Inheriting the release's data (see "Local placement") happens only at
  switch-over, and its migration is tested on a copy of a real data directory, never on the live one.
- Phones under test pair with the Next build's server as a second desktop; the release's pairing is
  untouched.
- Signing: a local Next build is signed with the Developer ID like a release, or macOS asks for
  folder access on every rebuild.

## Order of work

Each step leaves `main` releasable, and the release behaves exactly as today until the last one.

1. **Context instead of `AppHandle`.** Inside `src-tauri`, the working modules stop reaching for
   `AppHandle` and take an explicit context (data directory, event sink, sidecar path). No behaviour
   change.
2. **Core crate.** Move those modules into the core crate; the release links it in-process. No
   behaviour change. `cargo test` covers it.
3. **Full dispatcher and contract.** Grow the bridge from the phone allowlist to every server command
   and event, with contract files; capabilities by device kind fixed at pairing; terminal sequence
   numbers and backpressure. The phone sees no difference.
4. **`smetanad`.** The binary, its CLI, pairing from the terminal, local auto-pairing, `--data-dir`,
   the health report, the "no live work" restart gate.
5. **Desktop as a client, behind `split`, and the Next build.** The shell connects instead of linking
   the core; forwarding; connection state in the UI; local spawn and respawn; the overlay config.
6. **Settings split** (in the client build): device half local, server half through patches and a
   broadcast event.
7. **Remote specifics** (in the client build): server-side folder browser and clone, attachment upload
   from the desktop, report and image fetch, hiding client-machine actions for a remote server, install
   docs and the systemd unit.
8. **Switch-over** — only after the Next build has been used and the person says it works: the client
   shell becomes the only one, the in-process path and the `split` feature are deleted, the data
   inheritance runs on first start, and the phone talks to the local `smetanad`. Locally nothing to do
   for a phone: the local server inherited the key. A person who moves to a remote server pairs the
   phone with it; the documentation says so.

## Testing

- Core: the existing `cargo test` suites, moved with their modules.
- Bridge: the existing two-endpoint loopback tests (`tests/phone_bridge.rs`), extended to a desktop
  device kind and to terminal resume and backpressure.
- Contract: every server command and event has a pinned example, checked on both sides, as today.
- Front end: stores keep being tested through `mockIPC`; the connection-state store gets its own
  tests. Components are checked by eye as today.
- One end-to-end check by hand per placement: local (install, launch, close the app with a run going,
  reopen, the run is still there) and remote (a VPS, `smetanad pair`, desktop and phone both attached
  to the same terminal).

## Left to the builder

- Crate names and the exact module boundaries of the core.
- Generic forwarding versus per-command forwarding wrappers in the shell.
- The exact line of the settings split, by the rule given.
- How the local server is found (lock file, socket, address file) and how a respawn waits for runs.
- Bounds for backpressure and reconnect backoff.
- Packaging of `smetanad` for each platform beyond the systemd unit.
