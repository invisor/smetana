# A phone for the workstation — the mobile companion

Everything this app supervises lives inside the desktop process: the PTY and driven sessions,
the bd sidecar, the runs, the worktrees. A person who walks away from the workstation loses
sight of all of it until they come back.

This adds a companion app for iOS and Android that is a **remote client** of a desktop it has
been paired with. It shows the board and the runs, starts agents and talks to them through
the conversation panel ("Smetana UI"), files tasks through an agent, starts and stops runs,
and is woken by a push notification when a run ends or an agent is waiting. The phone
executes nothing: agents, bd and git stay on the workstation.

It has to work **from anywhere**, not only on the workstation's Wi-Fi, and **for any user**,
with no networking set up by hand.

## Constraints that shaped it

- **No mesh VPN.** A phone holds one active VPN at a time, and the people this is for already
  have a commercial VPN on, on both devices. Tailscale and its kind are therefore out.
- **Nothing readable on any server.** Every byte that crosses a server is end-to-end
  encrypted, and no server stores anything. This has to be demonstrable, not promised: the
  product must not be open to the charge of reading somebody's code.
- **Self-hostable.** Anybody can run their own relay and point the desktop at it.
- **Independent releases.** The desktop ships often and mostly changes things the phone
  never sees; the mobile app is released only when something on the phone changes.

## Architecture

Three parties, four units of code, one repository.

```
 phone (mobile/)  ──iroh/QUIC, E2E──  relay (server/, ours or self-hosted)  ──  desktop (src-tauri/src/remote/)
       ▲                                                                            │
       └──────── APNs / FCM ◄──── push gateway (server/, always ours) ◄─────────────┘
                                   token + encrypted blob only
```

### Transport: iroh

[iroh](https://docs.rs/iroh) is a Rust library for QUIC connections addressed by public key,
with hole punching and a relay fallback. It fits because both ends are Rust — the desktop's
`src-tauri` and the mobile app's Tauri core — so one implementation serves both.

- Each device has a persistent `SecretKey`; its public half is its identity.
- A connection is TLS 1.3 between those keys. The relay forwards ciphertext and cannot
  impersonate either end, because the phone learns the desktop's key from the QR code, never
  from the relay.
- The relay is configured with `RelayMode::custom([url])`. The default is ours; a setting on
  the desktop replaces it, and the relay URL travels in the pairing ticket, so the phone
  never asks for it.
- A relay is addressed by an HTTPS URL with a real certificate. A bare IP would need a
  self-signed certificate on both ends and is not supported.

Rejected: WebRTC (a signalling server and a TURN server of our own to run) and a WebSocket
relay of our own (reimplements what iroh does, and every byte always goes through the
server).

### Units

1. **`crates/remote-protocol/`** — shared by all three parties.
   - The pairing ticket: desktop public key, relay URL, one-time pairing secret, expiry.
   - The bridge messages: `Invoke { id, cmd, args }` → `Reply { id, ok | err }`,
     `Listen { event }` → `Event { name, payload }`, and the handshake.
   - **The allowlist** of commands and events the phone may reach — the one place that
     decides it.
   - The contract: the schema of every allowlisted command's arguments and reply and every
     allowlisted event's payload.
   - The push blob format and its encryption.
2. **`src-tauri/src/remote/`** — the desktop's half.
   - An iroh endpoint with a persistent key and the configured relay.
   - Pairing: issue a ticket, show it as a QR code, accept, confirm on the desktop, store the
     phone's key.
   - The device list, with revocation.
   - The bridge: accepts `Invoke` only from a paired key and only for an allowlisted
     command, calls the same handler a Tauri command calls, and forwards allowlisted events.
   - Push: when a run ends or an agent waits, encrypt a short message for each paired phone
     and hand it to the gateway.
3. **`mobile/`** — a second Tauri 2 app, built for iOS and Android.
   - Rust core: the iroh client, the pairing stored in Keychain / Keystore, push token
     registration, push decryption.
   - Front end: screens of its own for a narrow screen — projects → board → task, agents →
     conversation, runs. It imports components, tokens and stores from `src/` at build time
     and swaps the stores' transport for the bridge, through the same seam `mockIPC` uses
     today. What it imports is fixed at the version it was built against.
4. **`server/`** — two stateless processes in one Docker image.
   - `iroh-relay`, iroh's own relay.
   - `push-gateway`: an HTTP endpoint taking a device token and an encrypted blob and
     handing them to APNs or FCM. It cannot be self-hosted: only the holder of the app's own
     APNs/FCM credentials can wake it, which is a rule of Apple's and Google's. This is the
     same arrangement Matrix and ntfy use.

### What changes in existing code

- **Rust commands become callable without Tauri.** A handler that takes `Window` or
  `AppHandle` only to emit or to reach state is split into a plain function the
  `#[tauri::command]` wrapper and the bridge both call. This is the main piece of desktop
  work, and only the allowlisted commands need it.
- **Stores choose their transport.** `invoke` and `listen` are reached through one seam that
  the mobile build points at the bridge.
- **Settings.** A relay URL (default ours) and the paired device list. The device list holds
  public keys and names; it is not a secret, but revoking from it must take effect at once.

## Data flow

### Pairing

1. *Connect a phone* in the desktop's settings creates a one-time secret valid for two
   minutes and shows a QR code carrying the ticket.
2. The phone scans it, connects to the desktop over iroh and presents the secret with its own
   public key, its device name, its push token and a separate X25519 public key that push
   blobs are encrypted to (the iroh key is a signing key and is not reused for encryption).
3. The desktop asks: "Pixel 8 wants to connect". On confirmation the phone's key joins the
   device list and the secret is spent. Without confirmation nothing is stored.
4. From then on the phone connects by key alone. Revoking it on the desktop removes the key,
   closes any open connection, and refuses the next one.

### A session

- On connect, the handshake (below), then the phone reads its snapshots — the board, the
  agents, the runs — with the same commands the desktop front end reads at start-up, and
  subscribes to events. This is exactly what the stores already do.
- On a dropped connection the phone reconnects and reads the snapshots again. Missed deltas
  are not replayed; the snapshot covers them.

### The active project

The tracker worker holds **one** active project (`tracker_set_project`), and the stores are
built on that. For now the phone **follows the desktop**: it shows the project open on the
desktop, and switching on the phone switches the desktop. Nobody is at the desktop while the
phone is in use, so this costs nothing in practice and nothing in the worker. A client with a
project of its own is a later change, if it is ever wanted.

Notifications cover every project either way.

### Push

When a run ends or an agent starts waiting on a person, the desktop encrypts a short message
to each paired phone's push key ("Run in smetana finished: 5 tasks, 1 waiting for you") and sends
the gateway the token and the blob. The gateway passes them to APNs or FCM; the phone
decrypts and shows them. Tapping one opens the matching screen.

## What the phone can do

The allowlist, first version:

- **Read:** board and issues, agents and their conversations, runs, run reports.
- **Act:** file a task, start an agent, send a message to an agent and answer it (including
  permission and `AskUserQuestion` cards), start and stop a run, switch the active project.

Everything else is refused with `forbidden`: writing files, git operations, settings,
attachments from disk, shell sessions, device management.

Agents are started in the **conversation panel**, i.e. as driven sessions; the phone renders
them with the panel's own components. A terminal session (Smetana UI off, or a shell) is
**read-only** on the phone in the first version — a TUI on a narrow screen is not usable.

## Security

- **Desktop:** accepts a connection only from a key on the device list; an unknown key is
  dropped before any message is parsed.
- **Allowlist:** the main control, enforced by the bridge and pinned by a test.
- **Arguments:** validated by the handlers themselves, exactly as for the desktop front end.
  The bridge adds no validation of its own and cannot bypass the existing one.
- **Pairing secret:** single use, two minutes, accepted only together with confirmation on
  the desktop. Whoever photographs the QR still has to get past that dialog.
- **Keys:** the desktop's in the app data directory, the phone's in Keychain / Keystore.
- **Gateway:** one `POST` route, rate-limited per IP, no logging of bodies or tokens.

### What every server can and cannot see

Stated in the server's README and in the app, because silence about it is what would be
held against us:

- **Cannot see:** any content — the board, code, agent output, conversations, push text.
- **Relay sees:** IP addresses, device public keys, timing and volume of traffic.
- **Gateway sees:** the device token, the time of each push and the size of the blob.
- **Apple / Google see:** that a notification was delivered to the device.
- Neither server stores anything. The server code is open and built reproducibly.

## Versioning and independent releases

The bridge forwards desktop commands as they are, so **the shape of every allowlisted command
and event is a public contract**. Renaming a field in a reply used to be free, because the
front end changed in the same commit; once the bridge exists it would silently break phones
already installed.

- **The contract is pinned** in `crates/remote-protocol`: a test compares each allowlisted
  command's and event's schema with what the desktop actually produces, and fails on any
  change until compatibility is restored or the version is raised.
- **Minor changes** — a new field, command or event — are free. The phone ignores what it
  does not know.
- **Breaking changes** — a removed or renamed one — raise the protocol major, and the desktop
  keeps serving the previous major for a period.
- **Handshake:** both ends send their protocol version and capability list. The phone hides
  what the desktop cannot do, and the desktop refuses a major it no longer serves.
- **Forcing a mobile update without a major.** The desktop also sends two constants of its
  own release:
  - `minMobileVersion` — below it, the phone shows one screen, "Update the app", with a
    store link, and calls nothing.
  - `recommendedMobileVersion` — below it, a dismissible banner.

  A store cannot be made to install anything; the phone can only stop working with a build
  that is too old.

The outcome: changes inside the desktop — scripts, prompts, run logic, desktop UI — ship
without touching the phone, and the phone is released when the phone changes.

Rejected: serving the mobile UI from the desktop over the bridge. It removes the version
problem entirely, but ties every phone screen to the desktop's release and its reachability.

## Errors and degradation

- **Desktop off or asleep:** "Workstation unreachable, last seen at …" with the last snapshot
  read-only. Nothing is sent blind.
- **Relay down:** iroh tries a direct path; failing that, the same unreachable state.
- **Gateway down:** the push is lost. The report is still on the desktop and is seen on the
  next connection; the desktop keeps no queue.
- **Version mismatch:** as above — an update screen, not a crash.
- **Sleep:** not managed. A workstation running agents should not sleep in the first place.

## Repository layout

```
crates/remote-protocol/   shared protocol, allowlist, contract
src-tauri/src/remote/     desktop endpoint, pairing, bridge, push sender
mobile/                   Tauri 2 app; gen/apple and gen/android generated inside
server/                   iroh-relay + push-gateway, one Docker image
```

The mobile toolchain (Xcode, Android SDK) is needed only to work on `mobile/`. `npm run
tauri dev` and both test runners stay independent of it.

## Testing

- **`crates/remote-protocol`** (`cargo test`): ticket round trip, secret expiry, push blob
  encryption round trip, handshake rules (major, `minMobileVersion`), and **the contract
  test**.
- **The desktop bridge** (`cargo test`): a non-allowlisted command answers `forbidden`; an
  unknown key is dropped; a revoked device is disconnected; two iroh endpoints in one process
  over a local relay pair and complete a call.
- **Stores over the bridge transport** (`npm test`): snapshot, events, reconnect — the same
  approach as `mockIPC`.
- **The gateway** (`cargo test`): APNs and FCM mocked; the body is passed on unchanged and
  nothing reaches the log.
- **Mobile screens:** checked by eye, as all UI in this project is — no component runner is
  introduced.
- **Live check**, on real iOS and Android devices off the workstation's Wi-Fi (mobile data):
  pair, read the board, talk to an agent, start a run, receive its push when it ends, revoke
  the device. This cannot be automated and is a checklist.

## Out of scope for the first version

- A per-client active project.
- Terminal sessions writable from the phone.
- More than one desktop per phone.
- Offline actions queued for later.
