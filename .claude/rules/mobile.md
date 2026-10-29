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

**A TestFlight build is one command: `cd mobile && npm run testflight`** (`scripts/ios-testflight.sh`). It builds,
signs for distribution and uploads to TestFlight's internal testing. The App Store Connect API key is outside the
repository, in `~/Desktop/Projects/smetana-secrets/` (or `$SMETANA_SECRETS`): `asc-api.p8`, and `asc-api.txt` with the
Key ID on line 1 and the Issuer ID on line 2; a missing file stops the script by name. **The key's role must be
Admin**: an App Manager key is refused cloud-managed distribution certificates (403 at export), and there is no local
Apple Distribution certificate - the cloud one is the whole of the signing, and nobody signs in to Xcode. **Both App IDs
must already exist** in the developer portal (`com.invisor.smetana.companion` and `...companion.NotificationService`,
the second with no capabilities): the export creates profiles but cannot register a bundle identifier. The team is not a
secret and is committed: `bundle.iOS.developmentTeam` in `tauri.conf.json`, and `DEVELOPMENT_TEAM` on both targets in
`project.yml`, because tauri writes it onto the app target only and the extension needs it as well.

tauri builds the archive (`tauri ios build --archive-only`, with `APPLE_API_KEY`, `APPLE_API_ISSUER` and
`APPLE_API_KEY_PATH` set, which makes it archive unsigned) and the script does the export itself. tauri's own export
puts the entitlements back on the app's binary alone before handing over, and the export takes each bundle's
distribution entitlements from what it finds in the archive, so the extension came out without its keychain group and
could not have read the app's secrets. The script signs both bundles ad hoc with their own entitlements files
(extension first), then runs `xcodebuild -exportArchive` (`app-store-connect`, `-allowProvisioningUpdates` with the key)
into `gen/apple/build/testflight/`. That export is what turns `aps-environment` into `production`; the committed
entitlements stay `development` for a device build. The upload is `xcrun altool --upload-app`. Check a build with
`codesign -d --entitlements - <app>` on both the app and `PlugIns/NotificationService.appex`.

The build number (`CFBundleVersion`) is the UTC minute, `YYYYMMDDHHMM`, set per run through `bundle.iOS.bundleVersion`
in a `--config` override: App Store Connect wants each upload above the last, and a clock needs no file to remember it.
Two uploads in the same minute collide, so wait a minute. Not `--build-number`: tauri appends it to the version as a
fourth component and parses it as `u32`, which a date does not fit. The short version is `tauri.conf.json`'s, and the
script copies it into the extension's `Info.plist` too, since App Store Connect refuses an extension whose version
differs from its app's. The build rewrites tracked files under `gen/apple` (tauri merges `Info.plist` and the signing
settings into the project; `agvtool new-version -all` stamps the build number into both `Info.plist`s), so the script
snapshots every tracked file there first and puts it back on exit, success or not. The script finds Xcode itself when
`xcode-select` points at the Command Line Tools, and puts an `xcodebuild` wrapper first on `PATH`, because tauri runs
xcodebuild with an explicit environment that drops `DEVELOPER_DIR`. Export compliance is answered in the app's
`Info.plist` (and in `project.yml`, which xcodegen rebuilds it from): `ITSAppUsesNonExemptEncryption` is `false`, so App
Store Connect stops asking on every upload. It is a legal statement and the owner made it (2026-10-29): the app uses
standard algorithms only (iroh's QUIC/TLS through rustls, the push seal), the mass-market exemption. The key also skips
the questionnaire's France question; a French encryption declaration (or leaving France out of the storefronts) is still
owed before the app reaches the public App Store. Do not flip the value without the owner.

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

`screens/Workspace.vue` is what the app is once the desktop answers: four tabs (Kanban, Agents, Projects, Settings) in
`Shell.vue`, and screens opened over them (`TaskScreen`, `ConversationScreen`, `NewTaskScreen`, `RunScreen`). Where the
person is is one route, `nav.js` (pure; `route.project` is always null; `routeForTarget` is what a tapped notification
becomes: `run:<project path>` -> the Agents tab, where the run's row is, `session:<id>` -> the driven conversation, `terminal:<id>` -> the read-only terminal; the two id spaces
are separate counters, so the kind travels with the id; anything else leaves the app where it is). The glue that
starts the stores and reads a project is `workspace.js`, called from `session.js` on every (re)connect. **A run is started from the board, as on the
desktop, and stopped from Agents.** The Ready column's header carries a play (`BoardScreen`, `click.stop` so it does not
fold the column), shown only when `runOffered` of the run configuration and grey with `scopeBusyReason` while a run over
the queue is going; `boardRun.js` (pure, tested) is that rule. It opens `RunScreen` with `{ kind: 'queue' }`, the
desktop's "Run the queue" form open at once (it carries by hand the opening values and wording of `RunModal.vue`, with
`runStart.js`), and the screen reads the project's branches itself, the one thing `loadProject` does not load. Cancel
goes back; a started run lands on Agents. There the project's runs sit above "New agent", one `RunBar` each without
"Run anyway" (the bridge has no `run_release`); Stop calls `stopRun(token)` and a failure is one phrase under the bars.
The run stores simply follow the active project through `loadProject`; nothing pins them to another one.
**`SettingsScreen`** is the fourth tab: for now the About group on `SettingsGroup`/`SettingsRow` (the companion's version,
the desktop's protocol minor or "Not connected", and Unpair). Unpairing is two taps, `unpairStep.js` being the rule
(confirm only from the asked step); Confirm calls `unpairAndForget()` and `App.vue` then shows the pairing screen. Above
About sit Appearance (Theme, Density, Font size) and Kanban (Columns, Show tasks from): the phone's own overrides of the
desktop's settings. **`phoneSettings.js`** (pure, tested) is the shape and the rule: every choice but the font size
defaults to `'desktop'` ("Same as desktop") and a phone's choice wins over the desktop's, decided in one place,
`applyDesktopSettings(target, stored, { minor, phone })`. The choices live in `localStorage` under
`smetana.companion.settings`, are held reactive as `phone` in `workspace.js`, and `setPhoneSetting` saves and applies
again over the last `settings_load` answer, so going back to "Same as desktop" needs no read. Nothing is sent to the
desktop (`settings_save` stays off the list), and `unpairAndForget` keeps them, since they are about this phone. `uiFontSize`
is the phone's alone: the desktop's is never read, and `App.vue` paints it with `paintRoot` (`--ui-scale`). Agent and model
are not offered (a phone's `session_start` carries neither), nor the `alwaysShow`/`unlimited` column lists.
**The board is
one column under another, not a swipe**: every column's name and count is on screen at once, the gesture is the
thumb's own, and a header folds its column (`board.js` is `orderColumns` then `visibleColumns`, the desktop's rules, plus
a 20-card preview). `NewTaskScreen` folds its Type, Priority and three stage selects under an "Advanced settings" `Disclosure`, the same `core/Disclosure.vue` the desktop's New task window uses, closed on every open and after every successful start. `env(safe-area-inset-*)` is in `screens/safeArea.js` only.

What the phone must not call is kept out of the stores' reach rather than hoped about:

- **`allowed.js`** is the phone's copy of the epic's two lists; `tests/allowed.test.js` holds it equal to
  `crates/remote-protocol/src/bridge.rs` and scans `src/` for any literal `invoke('...')` off the list.
  `tests/workspace.test.js` runs every screen's reads and writes through the real stores and asserts the same of what
  actually left, and that `settings_save`, files, git and terminal writes did not.
- **`bridge/core.js` `LOCAL`** answers three questions a store asks about the desktop's machine without sending them
  (`tracker_access_repair`, `terminal_marks`, `terminal_restorable`); **`bridge/event.js`** does not ask the core to
  subscribe to a name off the list. Do not use `loadSettings` (it starts the write-back watch): `desktopSettings.js`
  copies the few fields the screens read into the same `settings` object, and silences the stores' `chime`. It copies
  the desktop's `appearance.theme` and `density` too, but only from a desktop at protocol 1.14 (`LOOK_MINOR`; below it
  the server answers its defaults and the phone keeps `system` and `comfortable`), and never `uiFontSize`. **`look.js`**
  is the pure rule `App.vue` binds `data-theme`/`data-density` to: `system` means the phone's own system theme,
  unknown values read as `system`/`comfortable`, and `?theme=`/`?density=` win (browser only). Until the first
  `settings_load` the theme is `system` (`wireStores()` sets it; the store's default is `dark`), and the last look is
  cached with the summary (`look` in `summary.js`, dropped by `forgetSummary`) so a launch with the desktop away opens in it.
  The phone listens to `settings:changed` (1.14+) and answers by calling `refreshFromDesktop()`, never by folding the payload.
- **`vite.config.js` `phoneStores`** answers `stores/app.js` and `stores/windowDrops.js` with `src/phone/` files (the
  conversation panel imports three names from the first; the second would ask `drag_drop_space`). `Composer` and
  `ConversationView` take `attachable`, `TaskCard` takes `actionable`, `RunBar` takes `canRelease`; all default on, and
  the phone turns them off: paperclip and "Run anyway" are writes the bridge does not carry, and the card's
  three-dot button is off because the tap on the card is the way into its menu (the sheet below), so a button would be
  a second door to the same place.

**A tap on a board card opens a bottom sheet, not the task** (smetana-wi7h). `BoardScreen` emits `sheet`,
`Workspace.vue` draws `screens/TaskSheet.vue` (inline styles, tokens only, `--z-modal`, bottom padding under
`safeArea.js`; it closes on the backdrop, Escape and the system back, which is a history entry it pushes while it is
up) and carries out its `act` events. The sheet decides nothing about its rows. **`taskSheet.js`** (pure, tested) is
the rule: `sheetRows` is "View task" and then the desktop's own `taskMenuItems` called with the same five inputs
`DesktopApp.vue` computes (`runnable` is `runnableTask`, `cardScope` the desktop's, `runBlockedReason` is
`scopeBusyReason` over that scope, `parentBlocked` comes from `lockCascade.js`), so a card offers on the phone what it
offers on the desktop and a change to the menu reaches both. **`taskActions.js`** is the writes over the real stores:
`setStatus` (the patch is `kanban/statusPatch.js`, shared with the desktop's `setTaskStatus`, so the `promoted:` note
cannot differ), `toggleLock` (the `lockIds`/`unlockIds` cascade, one `updateIssue` per id, every id in `writingIds`
before the first write), `remove`, `startOn` (`editTask`, `resolveTask`, `fixTask`). Each answers whether it landed;
a refusal is the tracker's own sentence (`refusal()`), shown under the rows because a phone has no toasts, and the
sheet stays open. An issue that has gone (`issueById` null) makes every action a no-op and closes the sheet. Move to...
is a second step in the same sheet, with Back on top; Delete and a parked task's move to Ready go through the
desktop's `DeleteTaskModal` and `ReadyTaskModal`; Run this opens the run form as a stack screen (`{ screen: 'run',
scope, title }`) with `RunScreen`'s `scope` prop, and `runPayload` carries that scope; Follow-up task opens New task
with a `parent` (`{ id, title }`) and `newTaskIntent` puts its id in `draft.parent`.

**The three write rows are gated on the desktop's protocol minor.** `TRACKER_WRITES_MINOR = 13` in `taskSheet.js`
(`canWriteTracker`, read from `bridge.status` as `canAttach` is): a desktop older than 1.13 does not carry
`tracker_update` and `tracker_delete`, so Block/Unblock, Move to... and Delete are grey, not absent (absent rows read as a
missing feature), the separator stays, and `sheetNote` puts one caption under the rows (`OLD_SERVER_NOTE`: the
desktop's server needs a restart); a grey Move to... does not open. The paperclip (`canAttach`) is still absent. The number moves with the protocol crate, not on its own.

**`session:state` and the Agents list come from the server** (protocol 1.12, smetana-ahp9). The bridge carries
`session_list`, `session:state`, `session:removed` and `terminal:restorable`, and the shared stores mirror them, so the
list holds what the desktop started and offline rows arrive too; the phone's own synthesis of the state
(`bridge/sessionState.js`, and the local `terminal_restorable` answer) is gone. **Known gaps, all of the bridge's shape
and not the screens'**: the Agents screen does not draw offline rows yet, resume them or close a row (the commands are
allowed; the screen is a separate task), and a conversation screen open on a session another device closed stays open. There is no event for
the desktop switching project, so the phone reads `settings_load` again on reconnect and when the page becomes visible.
A terminal session's output is the one `terminal_attach` snapshot (`terminalText.js`), refreshed on `terminal:state`
and by a button; `terminal:output` is not on the list.

`npm run dev` (port 5174) now shows the working screens over the desktop's `mockBackend.js` through `devBackend.js`
(dev only; it refuses an off-list command as the core would): `?theme=`, `?density=` and `?target=session:1` (a tap on a
notification) are read, and `window.__phone.emit(name, payload)` delivers an event.

Not checked by any runner: every `.vue` file here. They are small, inline-style, token-only, and verified by eye in
`npm run dev` (port 5174), where the shell shows its pairing screen.
