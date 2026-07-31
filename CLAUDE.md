# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```sh
npm install          # postinstall fetches the bd sidecar; it warns and continues without one
npm run dev          # http://localhost:5173 — front end alone, backed by the mock
npm run build
npm run preview      # serve the production build
npm run tauri dev    # the actual desktop app: Rust worker, real bd, live board
npm run fetch-bd     # download the bd binary explicitly (fails hard, unlike postinstall)
cd src-tauri && cargo test
```

`cargo test` is the only test runner in this repository, and it covers the Rust side only. There is
no front-end test runner, linter or formatter — do not invent one, and do not claim a change is
"tested" on the basis of a build succeeding.

The way to verify a front-end change is to open it in the dev server with the query parameters the
app reads (`src/App.vue`):

| parameter | values | default |
|---|---|---|
| `theme` | `dark`, `light` | `dark` |
| `density` | `comfortable`, `compact` | `comfortable` |
| `view` | `gallery` | the app |

`?view=gallery` renders every exported component once (`src/views/Gallery.vue`) — the harness for
catching a broken component. Check any component change in all four theme × density combinations,
since both switches only swap CSS custom properties and a component that hardcodes a value will
look correct in exactly one of them.

## Architecture

A desktop app for supervising autonomous AI coding agents: a Tauri 2 shell (`src-tauri/`, Rust)
around a Vue 3 front end (`src/`). The front end is ported from the **Smetana Design System**
(`claude.ai/design`, project `5da5ca35`). Tokens are copied from the design system verbatim;
components are ports of its React sources, keeping prop names, computed styles and behaviour. React
`value`/`onChange` became `v-model` (`modelValue`); React `children` props became named slots. When
something looks odd, the design system is the source of truth — match it rather than "fixing" it.

`src/main.js` → `src/App.vue` → either `views/DesktopApp.vue` (the three-column shell: worktree
files + agents, tab bar over the kanban, task inspector with live log) or `views/Gallery.vue`
(code-split, never in the app bundle). The board is live tracker data. Everything else on the
screen — the file tree, the agents, the log — is still fixture state in `views/desktopAppData.js`.

### The tracker bridge

The board shows the **bd** issue tracker of the directory the app was opened in, and follows it as
it changes, no matter who changed it: this window, an agent, or a person in a terminal. bd has no
daemon and no API — its CLI is the API, and one call costs about two seconds. Hence the shape of
`src-tauri/src/tracker/`:

| file | what it does |
|---|---|
| `model.rs` | `Issue`, `ColumnDef`, `Delta`, `Health`, `TrackerError` — the vocabulary the front end sees |
| `bd.rs` | the only file that knows bd's CLI: arguments, spawning, parsing |
| `store.rs` | the in-memory snapshot and the delta computation |
| `watcher.rs` | `notify` on `.beads/`, path filter, failure reporting |
| `service.rs` | the worker: one owner of mutable state, a request queue, deltas and health events |
| `commands.rs` | thin `#[tauri::command]`s that put a request on the queue and await the reply |

`service.rs` is a single tokio task holding the snapshot; commands, watcher ticks and the 60-second
safety sweep meet in one `select!`. Nothing shares mutable state with it — that is what keeps
~2-second bd calls from blocking each other unpredictably. Writes go straight into the snapshot and
out as a delta without waiting for the watcher. `generation` advances by exactly one per emitted
delta; the front end resyncs when it sees a gap. `store.rs` and the argument builders in `bd.rs` are
pure and carry the unit tests.

Health (`ok`, `not-a-beads-repo`, `bd-version-mismatch`, `error`) is both an event and a command:
the event fires microseconds after start, before the webview can subscribe, so the worker also
answers `tracker_health`. `DesktopApp.vue` renders it where the board would be — quietly, since the
loud budget belongs to the card that needs a human.

`src/stores/tracker.js` and `src/stores/settings.js` are the **only** files in `src/` that know Tauri
exists — components see reactive stores and nothing else. `tracker.js` also owns the two
translations: bd's statuses to the design system's (`open → ready`, `in_progress → running`,
`closed → done`; everything else, including custom statuses, passes through to `normalizeStatus` and
gets a hash colour with a 2-letter code), and Rust's diagnostics to short English messages, with the
raw text left in the console.

In a browser there is no back end, so `src/stores/mockBackend.js` installs the official `mockIPC`
with the old fixtures: read commands answer, and writes to the tracker reject loudly — a "write"
that looked like it worked would be worse than none. `settings_save` is the one exception: it is
accepted and dropped, because a browser has nowhere to keep it and failing every debounce tick
would only fill the console. That is what keeps `npm run dev` and `?view=gallery` working with no
branching in components.

### The bd sidecar

bd ships inside the bundle (`bundle.externalBin`), so the app is self-contained and the version is
fixed. The binary is 128 MB and is **not** committed: `scripts/fetch-bd.mjs` downloads the pinned
release, verifies it against the sha256 digests committed next to `BD_VERSION` (the release's own
`checksums.txt` is only a cross-check), and lays it out as `src-tauri/binaries/bd-<target-triple>`.

`postinstall` runs it with `--optional` and only warns on failure — a contributor who wants the
front end alone should not need a Rust toolchain and a 43 MB download. `npm run fetch-bd` and CI
fail hard instead. `EXPECTED_BD_VERSION` in `service.rs` must stay in step with `BD_VERSION` in the
script; a mismatch surfaces as `bd-version-mismatch` in health, not as a crash.

### Settings

What the app remembers between runs lives in one JSON file in `app_config_dir()`
(`~/Library/Application Support/com.invisor.smetana/settings.json` on macOS).
`src-tauri/src/settings/` owns it: `model.rs` is the schema, the validation and the merge — pure,
and where the tests are; `file.rs` is the disk (atomic write through a per-call temp file that is
`sync_all`ed and renamed, a `.bak` copy of anything unparseable or too new); `commands.rs` is two
thin commands.

The file keeps appearance and panel layout at the root and everything about content under the
project's absolute path. There is still one project, so the map holds one entry — but that is the
shape multi-project will land in. The map never crosses the IPC boundary: `settings_load` returns
the resolved view for the current directory (`{ appearance, layout, project }`) and `settings_save`
puts it back, stamps `usedAt` and trims the map to the 20 most recent projects, dropping the
least recently used and never the current one.

The front end owns the truth here — the opposite of the tracker, where bd owns it.
`src/stores/settings.js` holds a reactive object and writes it back with a 400 ms debounce, one
write in flight at a time; components read and write plain fields. Closing the window does not wait
for the debounce: the store holds the close through `onCloseRequested`, flushes with a two-second
ceiling and then destroys the window itself — the window always closes, a slow back end costs the
last edit rather than the app.

Nothing about settings is reachable from the interface: there is no settings screen and no theme
switch. `?theme=` and `?density=` still override both for one run and are deliberately **not**
written back — one visit to the dev server must not repaint the app forever. `?view=gallery`
neither reads nor writes.

A missing file is the first run, not an error. A broken or too-new file is copied to
`settings.json.bak` and the app starts from defaults, and saving over it afterwards is fine. One
that cannot be read at all — wrong permissions, a directory in its place — has nothing to copy, so
it is logged *and* `settings_save` refuses: overwriting a file nobody could read would destroy it
sight unseen. Damage is contained field by field where it can be: a single field whose *value* is
outside its allowed set loses that field, while a section whose *type* is wrong
(`{"layout": {"leftCollapsed": "yes"}}`) fails to deserialize and loses the whole section to its
defaults — the same holds for one project entry among many.

The side-tab set is a closed list written out twice, in `model.rs` and in `views/DesktopApp.vue`.
Changing one without the other is silent: the value survives the session and comes back as Files.

Window size and position are not in this file: `tauri-plugin-window-state` handles them.

### Styling: inline style objects, never CSS classes

Components carry no scoped CSS and no utility classes. Every visual value is a computed style object
bound with `:style`, and every value in it is a `var(--token)` reference (see `core/Button.vue`,
`status/StatusBadge.vue`). Two consequences:

- A new component follows the same shape — `computed(() => ({ ... }))` of token references. Do not
  introduce a `<style>` block or a class-based approach.
- Never hardcode a colour, radius, spacing or font value. If a token does not exist for what you
  need, that is a design-system question, not a licence to write `#hex` or `8px`.

`styles/styles.css` is an `@import` list only; the tokens live in `styles/tokens/`. `tokens/base.css`
holds element defaults (focus ring, selection, scrollbar) and the only two global classes in the
system (`.sm-mono`, `.sm-hatch-blocked`).

### Theme and density live on the document root

Both are attributes on `document.documentElement` (`data-theme`, `data-density`), set by a
`watchEffect` in each view. Every token is defined against them: `tokens/color-*.css` redefine
colours under `[data-theme="dark"]`, and `tokens/space.css` redefines *only* the space scale and
row/control heights under `[data-density="compact"]` — density never changes colour, radius or type.

### `status/status.js` owns colour and loudness

The single source of truth for what a status looks like and how loud it is:

- `RESERVED` = `blocked, ready, running, needs-you, done, failed`. These get fixed
  `--status-<name>-*` tokens and a distinct glyph from `STATUS_GLYPH`.
- Anything else is user-defined: `normalizeStatus` → FNV-1a hash → one of 12 generated slots
  (`--status-gen-<0-11>-*`), hues chosen to stay outside a guard band around every reserved hue.
  Generated statuses also render a 2-letter `statusCode` — **status is never colour alone**.
- `attentionLevel(status)` returns `loud` / `live` / `quiet`; components set `data-attention` and
  dim `quiet` to `--attn-quiet-opacity`.

Two rules that break the product if ignored: `loud` (needs you) is budgeted at **1–2 per screen** —
if everything shouts, the design failed — and there is no fixed column set, so never hardcode one.

`core/interactive.js` (`useInteractive`) is the shared hover/press tracker: interaction is a surface
step up, never a colour change and never a transform, so controls in a dense list cannot jump.

### Icons

`core/icons.js` is the only file that names Lucide, and it registers glyphs explicitly so the build
tree-shakes to the ~40 actually used. Adding a glyph to the UI
means adding it there first; `Icon` warns in dev for an unregistered name. Swapping icon sets means
replacing that one file. Note `message-circle-question-mark` is kept as the design-system key and
mapped to lucide 0.469's `MessageCircleQuestion`.

### Adding a component

Create it under the matching `src/components/<group>/`, export it from `src/components/index.js`
(the library's public surface), and add it to `views/Gallery.vue` so it stays checkable. Product
code imports from `index.js`; components import their siblings by relative path — the `@` → `src`
alias exists in `vite.config.js` but is currently unused, so prefer relative paths for consistency.

## Constraints

- **No gradients, images, glass, blur or emoji.** Partly taste, partly the WebKitGTK constraint.
- Sentence case everywhere; identifiers in mono (`--font-mono`), prose in sans.
- The primary button is ink on paper with no brand hue — the entire saturated range belongs to
  status.
- The build target (`es2021`, `chrome100`, `safari15`) is set for the system webviews Tauri runs in
  (WebKitGTK / WKWebView / WebView2). Do not raise it, and do not reach for APIs newer than that.
- `tokens/fonts.css` `@import`s IBM Plex Mono from Google Fonts; an offline Tauri build needs the
  latin subset vendored locally instead.
