# smetana

Desktop app for supervising autonomous AI coding agents. This repository holds the
Vue 3 front end, imported from the **Smetana Design System**
(`claude.ai/design`, project `5da5ca35`).

## Run

```sh
npm install
npm run dev      # http://localhost:5173
npm run build
```

Query parameters, matching the design template's two props:

| parameter | values | default |
|---|---|---|
| `theme` | `dark`, `light` | `dark` |
| `density` | `comfortable`, `compact` | `comfortable` |
| `view` | `gallery`, `settings` | the app |

`?view=gallery` renders every component once — a dev harness for spotting a broken
component before it reaches the product. It is code-split and never lands in the
app bundle. `?view=settings` is the settings window, which in the desktop app is a
second OS window loading that same query string.

## Releases

A release is cut with one command, from `main`:

```sh
npm run release            # 0.1.0 → 0.1.1
npm run release -- minor   # 0.1.0 → 0.2.0
npm run release -- major   # 0.1.0 → 1.0.0
```

The step is `patch` when no argument is given, which is what most releases are.
An exact version number is not accepted as an argument at all, and that absence is
the point rather than a gap: the number is computed from `version` in
`src-tauri/tauri.conf.json`, so the only way to get it wrong — naming it yourself —
is not available.

`scripts/release.mjs` raises that one field, runs all three gates (`npm test`,
`npm run build`, `cargo test`), then commits, pushes `main`, tags `v<x.y.z>` and
pushes the tag. That is the same sequence a person used to run by hand, and it is
still exactly what happens — what the script adds is that it cannot misremember the
number and will not tag a commit no gate has seen. A red gate stops the release
before the tag exists and puts `tauri.conf.json` back at the version it started
from. It refuses outright, having changed nothing, on a dirty working tree, on any
branch but `main`, or when `main` and `origin/main` have moved apart, and says
which of the three it was. Nothing after the pushed tag is the script's business:
it never touches the GitHub API, because the build and the signature belong to a
runner that holds the secrets and no laptop does.

Pushing that tag is what the rest hangs on. `.github/workflows/release.yml` listens
on `v*` and on nothing else. `version` in `src-tauri/tauri.conf.json` is the single
source of the app's version: it is what the app reports about itself and the number
to quote in a bug report, while `package.json` and `src-tauri/Cargo.toml` carry
their own and mean nothing. The workflow compares the tag against it and fails,
naming both, when they disagree — otherwise `latest.json` would announce a version
the bundle does not carry. It then builds a macOS arm64 bundle and publishes a
GitHub release holding the `.dmg`, the `.app.tar.gz` an updater installs, its
`.sig`, and `latest.json`, which is what `plugins.updater.endpoints` points at —
`https://github.com/invisor/smetana/releases/latest/download/latest.json`, reachable
with no token because this repository is public.

The last three of those now have something to read them. `tauri-plugin-updater` is
a dependency, `lib.rs` registers it, and `src-tauri/src/updates.rs` owns everything
that follows: a check a minute after start and once a day after that while
`updates.autoCheck` is on, a download that happens by itself, and an install that
never does — the app holds unsaved editor buffers and live terminals, so a relaunch
nobody asked for is a relaunch that loses somebody's work. That switch is on by
default, and the General tab's Startup group is where somebody turns it off; off
stops the schedule and nothing else, so an update already downloaded stays
downloaded and a check asked for by hand is still made. An install is also refused
outright while a run is going in any project, since restarting would kill the agent
processes those sessions started. The capability file grants the plugin nothing, and
that is the decision rather than an omission: the front end calls that module's own
commands, so a grant would be required by nothing while publishing
`download_and_install` to the webview.

What is missing is the part drawn on screen. Nothing yet shows what a check found
or offers the press that installs it — the About rows are a separate task — so a
released copy today checks, downloads and waits with nothing on screen saying so.

Three things had to be done by a person before the first release could work, and all
three are in place, done once by the repository owner. They are written out here
as what happened rather than as instructions, because each of them carries a detail
that is expensive to rediscover and there is nowhere else it is written down.

1. `npm run tauri signer generate` produced the minisign key pair, and it was given a
   password. The command prints both halves to the terminal — two base64 blobs, under
   `Private:` and `Public:` — and writes nothing to disk unless you pass `-w <path>`.
   Tauri's update signature is mandatory and cannot be turned off.
2. The public half is committed, as `plugins.updater.pubkey` in
   `src-tauri/tauri.conf.json`. The `check` job in the workflow refuses to build while
   that field is empty, rather than publish a release nobody can install as an update.
   It now guards against somebody emptying it rather than against the first release
   going out unsigned.
3. `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` are set as
   repository secrets, under Settings → Secrets and variables → Actions. The first
   holds the blob printed under `Private:`; a pair written out with `-w` goes in as
   that file's **contents** and never as a path to it — the CLI accepts a path when it
   runs on your own machine, which is the tempting thing to paste, and on a runner a
   path resolves to nothing. The second holds the password from step 1, and forgetting
   it fails the build with `incorrect updater private key password`, which reads like a
   wrong password rather than a missing one and costs somebody the debugging twice.

The private half and that password live in the repository owner's password manager.
Neither has ever been in this tree, and neither belongs in a run log.

**Losing the private key means every already-installed copy can never be updated
again.** There is no way back from it: a copy already out there accepts only an
update signed by the key whose public half it was built with, so generating a fresh
pair is not a repair — it abandons everybody already running the app. Keep it
somewhere that outlives the machine it was generated on.

One local consequence, now that `pubkey` is no longer empty: `npm run tauri build`
refuses to bundle with "A public key has been found, but no private key", the bundler
looking for something to sign the updater archive with and finding nothing in the
environment. That is the intended state rather than a fault: the private key lives in
the repository secrets, and a build that signs an updater archive is the workflow's job
and not a laptop's. `npm run tauri build -- --no-sign` remains the way past it — it
skips the updater signature along with the code signing — for that reason now rather
than because the key was missing from the conf.
`npm run tauri dev` is unaffected, and so is everything `npm test`, `npm run build`
and `cargo test` do — none of them bundles.

## First launch

The app is not signed with an Apple Developer ID and is not notarized — the bundle
is ad-hoc signed, which is a deliberate choice and not an oversight. macOS therefore
refuses to open a freshly downloaded copy on a double-click, and the way past that
depends on the version:

- Right-click `Smetana.app` and choose Open. On macOS 14 and earlier the dialog that
  appears carries an Open button, and pressing it is the whole of it.
- From macOS 15 Sequoia on, that button is gone — the dialog offers only Move to
  Trash and Done. Dismiss it, then go to System Settings → Privacy & Security,
  scroll down to the message naming Smetana, and press Open Anyway. That asks for
  Touch ID or your password, and then confirms once more before the app opens.

Either way it is once per machine, not once per launch. It is **not** once per
version, though, because there is no in-app updater: a new version means downloading
the `.dmg` again and going through the same step for it. See Releases above.

A dialog saying **"smetana" is damaged and can't be opened** is a different fault and
not this one, and the two are easy to confuse because both follow a download. That one
means the bundle reached the release carrying no signature of its own: the ad-hoc
signature is `bundle.macOS.signingIdentity` in `src-tauri/tauri.conf.json`, and without
that field `tauri-action` never runs `codesign`, leaving only what the linker put on the
arm64 executable by itself — `codesign -dv` on such a copy says `adhoc, linker-signed`
with `Sealed Resources=none`. Gatekeeper reads a broken signature rather than an
unknown developer, so the dialog offers no Open button and Privacy & Security stays
empty: nothing above works, and the only way in is `xattr -dr com.apple.quarantine` on
a copy moved off the read-only disk image. v0.1.1 shipped that way and is the only
release that did.

## Layout

```
src/
  styles/
    styles.css          entry point — @import list only
    tokens/             fonts, colour (surfaces / status / generated / ansi / editor),
                        typography, space, shape, motion, base
  components/
    index.js            the library's public surface
    core/               buttons, inputs, Dropdown, Tooltip, EmptyState …
                        (+ icons.js, interactive.js)
    status/             status badges, dots and dependency marks
                        (+ status.js: the colour algorithm)
    shell/              AppShell, Panel, Resizer, ScopeIndicator, TabBar, project list
    kanban/             the board, the task inspector, the new-task dialog
    agent/              the agent list, the log view and its parts
    files/              the file tree and the CodeMirror editor
    terminal/           the xterm.js pane
    run/                the run bar, the run dialog, the report view
    notifications/      the bell's panel and cards
    settings/           the settings window's tabs
    overlays/           Modal, Toast, ContextMenu, MenuButton
  views/
    DesktopApp.vue      the three-column shell — the imported template
    SettingsWindow.vue  the second window, under ?view=settings
    Gallery.vue         dev-only component harness
    desktopAppData.js   what is left of the sample state
```

Each group's directory is the list; naming its files here only invites drift.
`CLAUDE.md` carries the architecture and the decisions behind it.

Tokens are copied from the design system verbatim; components are ported from its
React sources to Vue SFCs, keeping prop names, computed styles and behaviour.
Form controls use `v-model` (`modelValue`) instead of the React `value`/`onChange`
pair, and React `children` props become named slots.

## Rules that are load-bearing

Read the design system's own README for the full rationale. The three that break
the product if ignored:

- **The attention ladder.** `loud` (needs you) is budgeted at 1–2 per screen,
  `live` is calm, `quiet` drops to `--attn-quiet-opacity`. `attentionLevel(status)`
  decides; components set `data-attention`. If everything shouts, the design failed.
- **Status is never colour alone.** Reserved statuses have a distinct silhouette and
  glyph; user-defined ones are hashed (FNV-1a → one of 12 hues that avoid every
  reserved hue) and render a 2-letter code. Never add a fixed column set.
- **Colour means state.** The primary button is ink on paper with no brand hue,
  because the whole saturated range belongs to status.

No gradients, no images, no glass, no blur, no emoji — partly taste, partly the
WebKitGTK constraint. Sentence case everywhere; identifiers in mono, prose in sans.

## Icons

Lucide (ISC), registered explicitly in `src/components/core/icons.js` so the build
tree-shakes to the glyphs actually used — that file is the list. Adding a glyph to
the UI means adding it there first; `Icon` warns in dev when a name is not
registered. To swap in a different icon set, replace that file — nothing else
references Lucide.

Two notes carried over from the import:

- The design system asks for `message-circle-question-mark`; lucide 0.469 still
  exports it as `MessageCircleQuestion` (renamed upstream later). The DS name is
  kept as the key and mapped in `icons.js`.
- `LogToolbar`'s search field is `max-width: 180px` rather than a fixed `180px`, so
  the follow-tail button is not clipped inside the 340px inspector panel.

## Fonts

`tokens/fonts.css` pulls IBM Plex Mono from Google Fonts. For an offline Tauri
build, vendor the latin subset locally and replace the `@import`.
