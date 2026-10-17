---
paths:
  - "mobile/**"
---

# The mobile companion app (stage 5 of epic smetana-032e)

The design is `docs/superpowers/specs/2026-10-15-mobile-companion-design.md`; the protocol and the desktop's half are
in `remote.md`. `mobile/` is a second Tauri 2 app (iOS and Android, `com.invisor.smetana.companion`) with its own
`package.json`, Vite config and `src-tauri/`. **The root depends on none of it**: no workspace at the root, root
`vitest.config.js` includes only `tests/`, and the root `cargo` never sees `mobile/src-tauri`. Its gates are
`cd mobile && npm test` and `cd mobile/src-tauri && cargo test`.

## The seam: stores over the bridge

`mobile/vite.config.js` answers `@tauri-apps/api/core` and `@tauri-apps/api/event` with `src/bridge/core.js` and
`event.js` **only when the importer is under `../src/stores/`** (`bridgeTransport`). The bridge modules import the real
modules themselves, so a rule that swapped every importer would send them back to themselves. `invoke(cmd, args)`
becomes `bridge_invoke`; `listen(name)` registers locally, tells the core with `bridge_listen`, and is fed by the one
Tauri event `bridge:event`. A name the core refuses (not on the allowlist) is treated as a local event. Stores are not
edited. `tracker_resync` is aliased to `tracker_snapshot` (the allowlist has no resync, and a snapshot is what a
reconnected phone wants). `initTracker` asks for `tracker_access_repair`, which is off the allowlist; it rejects and the
store already tolerates that.

`packagesFromHere` resolves a bare import made by a file under `../src` as if it were written in `mobile/`, so the
build needs only `mobile/node_modules` and there is one copy of Vue. Anything a store imports transitively must be in
`mobile/package.json` (that is why `qrcode` and the dialog and opener plugins are there). Check with the root
`node_modules` moved aside.

After a reconnect the front end reads its snapshots again (`session.js`: `initTracker` the first time the link is
usable, `resync` on every later one). More stores will register in the same place.

## The Rust core (`mobile/src-tauri/src/`)

`core.rs` is Tauri-free: pairing, a supervisor that keeps one iroh connection (dial, `Hello`, serve, drop, back off
1 s doubling to 60 s), `invoke`/`listen`, and the push-token report. `lib.rs` is the Tauri boundary: commands `pair`,
`connect`, `unpair`, `bridge_state`, `bridge_invoke`, `bridge_listen`, `bridge_unlisten`, `take_push_target`; events
`bridge:event`, `bridge:status`, `push:target`. Statuses: `connecting`, `online`, `unreachable{last_seen}`,
`update-required{store_url}`, `update-recommended{store_url, recommended}` (online with a banner), and `revoked`
(an addition: `CLOSE_REVOKED`/`CLOSE_UNKNOWN` on a paired phone means retrying cannot help).

A desktop whose `min_mobile_version` is above this build's, or whose protocol major differs, gets the connection closed
straight after `HelloReply`: **no call ever leaves**. A command or event off `remote_protocol`'s allowlist fails on the
phone without being sent. `ClientMsg::UpdatePushToken` (protocol 1.1) is sent only to a desktop whose reply says
`protocol_minor >= 1`: an older desktop would read an unknown variant as a broken frame and close.

Secrets (iroh key, push secret, desktop key, relay URL, push token) go through `SecretStore`: iOS Keychain
(`keychain.rs`, `AfterFirstUnlock`, no explicit access group - the app and the extension carry the same single
`keychain-access-groups` entry) and Android Keystore (`SecretVault.kt`, reached over JNI from `android.rs`). A desktop
`cargo tauri dev` run uses a file in the temp directory and is not a configuration anyone ships.

## Tests

`core_tests.rs` runs the core against a stand-in desktop built from `remote-protocol` (the real endpoint lives in the
desktop crate and cannot be linked) through iroh's own test relay; `Config::insecure_tls` exists only under
`cfg(test)`. `tests/versions.rs` keeps iroh's locked version, `PROTOCOL_MAJOR`, the app version and
`MIN_MOBILE_VERSION` equal to the rest of the repository. The push vector `test-vectors/push.json` was sealed by
`remote_protocol::push::seal`; `push.rs` opens it with the Android path (`open_wire`) and `ios-push/check.sh` opens it
with the Swift the iOS extension uses (`SMETANA_WRITE_VECTOR=1 cargo test write_the_push_vector` rewrites it - do that
only on purpose).

## Building

`tauri ios build` / `tauri android build` need Xcode (selected, or `DEVELOPER_DIR`) and the NDK. The iroh dependency
tree does not compile for iOS as published, hence two pins that must stay: `libc = 0.2.189` in `Cargo.lock` (0.2.190
drops the routing-socket constants netwatch uses) and the patched `vendor/netdev` (`[patch.crates-io]`; `PATCHED.md`
there). Do not run a blanket `cargo update`. `gen/apple` and `gen/android` are committed and edited by hand: the
notification service extension target is in `gen/apple/project.yml` (re-run `xcodegen generate` there after editing it),
`PushRegistration.m` registers for APNs by adding a method to tao's delegate class, and the Android Kotlin sources
sit beside `MainActivity.kt`. Push on Android needs a Firebase project: `google-services.json` in
`gen/android/app/` (not committed); without it the build works and push is off.

The app icon is the desktop's artwork, built for a phone. `mobile/app-icon.png` is the master: 1024 px, opaque, the
bowl filling the whole square on the artwork's own ground (7, 10, 13), made by `scripts/make-app-icon.py --square`. It is
not the desktop master (`app-icon.png` at the root), which carries transparent margins and a squircle because macOS does
not round icons; iOS and Android do, so those would be drawn twice and the bowl would come out a third too small.
`mobile/app-icon.json` is the `tauri icon` manifest beside it: `app-icon-bg.png` is the Android adaptive background (a flat
ground) and `app-icon-fg.png` the foreground, the same artwork at 0.66 (`--square --scale 0.66`) so the bowl sits inside
the middle 66 dp of 108 that a launcher mask leaves (measured: farthest bowl pixel 125 px from the centre of the 432 px
layer, limit 132). The manifest's `android_fg_scale` is not used: tauri-cli 2.12.3 reads it and ignores it. To regenerate,
run `cd mobile && npm run icons`, which runs `tauri icon app-icon.json` into `gen/apple/.../AppIcon.appiconset` and
`gen/android/.../res`, then drops the alpha channel the CLI leaves on the iOS PNGs (`scripts/flatten-ios-icons.py`; App
Store Connect rejects an icon that has one). The desktop-format files it also writes into `src-tauri/icons/` ship in
nothing, and `icon.icns` comes out with its entries in a different order every run, so `git checkout` it after a regeneration
that did not change the artwork.

## The working screens (stage 6, smetana-xvbo)

`screens/Workspace.vue` is what the app is once the desktop answers: four tabs (Board, Agents, Runs, Projects) in
`Shell.vue`, and screens opened over them (`TaskScreen`, `ConversationScreen`, `NewTaskScreen`). Where the person is
is one route, `nav.js` (pure; `routeForTarget` is what a tapped notification becomes: `run:<project path>` -> Runs of
that project, `session:<id>` -> the driven conversation, `terminal:<id>` -> the read-only terminal; the two id spaces
are separate counters, so the kind travels with the id; anything else leaves the app where it is). The glue that
starts the stores and reads a project is `workspace.js`, called from `session.js` on every (re)connect. **The board is
one column under another, not a swipe**: every column's name and count is on screen at once, the gesture is the
thumb's own, and a header folds its column (`board.js` is `orderColumns` then `visibleColumns`, the desktop's rules, plus
a 20-card preview). `env(safe-area-inset-*)` is in `screens/safeArea.js` only.

What the phone must not call is kept out of the stores' reach rather than hoped about:

- **`allowed.js`** is the phone's copy of the epic's two lists; `tests/allowed.test.js` holds it equal to
  `crates/remote-protocol/src/bridge.rs` and scans `src/` for any literal `invoke('...')` off the list.
  `tests/workspace.test.js` runs every screen's reads and writes through the real stores and asserts the same of what
  actually left, and that `settings_save`, files, git and terminal writes did not.
- **`bridge/core.js` `LOCAL`** answers three questions a store asks about the desktop's machine without sending them
  (`tracker_access_repair`, `terminal_marks`, `terminal_restorable`); **`bridge/event.js`** does not ask the core to
  subscribe to a name off the list. Do not use `loadSettings` (it starts the write-back watch): `desktopSettings.js`
  copies the few fields the screens read into the same `settings` object, and silences the stores' `chime`.
- **`vite.config.js` `phoneStores`** answers `stores/app.js` and `stores/windowDrops.js` with `src/phone/` files (the
  conversation panel imports three names from the first; the second would ask `drag_drop_space`). `Composer` and
  `ConversationView` take `attachable`, `TaskCard` takes `actionable`, `RunBar` takes `canRelease`; all default on, and
  the phone turns them off because paperclip, card menu and "Run anyway" are writes the bridge does not carry.

**There is no `session:state` on the bridge**, and a conversation's state decides Send versus Stop and the Agents
list's needs-you. `bridge/sessionState.js` folds the journal exactly as `session::model::state_of` does (except it
cannot see the child die: a session read as ended stays ended, one that dies later reads as before until the next
snapshot) and `bridge/event.js` dispatches the result locally as `session:state` after each `session:events` batch;
`bridge/core.js` seeds it from every `session_attach` reply. **Known gaps, all of the bridge's shape and not the
screens'**: no command lists the desktop's driven sessions, so the Agents list holds the PTY sessions
(`terminal_list`), the sessions the phone started and the ones a run drives (adopted from `run_state`); a conversation
started on the desktop is reachable from a `session:<id>` notification and not from the list. There is no event for
the desktop switching project, so the phone reads `settings_load` again on reconnect and when the page becomes visible.
A terminal session's output is the one `terminal_attach` snapshot (`terminalText.js`), refreshed on `terminal:state`
and by a button; `terminal:output` is not on the list.

`npm run dev` (port 5174) now shows the working screens over the desktop's `mockBackend.js` through `devBackend.js`
(dev only; it refuses an off-list command as the core would): `?theme=`, `?density=` and `?target=session:1` (a tap on a
notification) are read, and `window.__phone.emit(name, payload)` delivers an event.

Not checked by any runner: every `.vue` file here. They are small, inline-style, token-only, and verified by eye in
`npm run dev` (port 5174), where the shell shows its pairing screen.
