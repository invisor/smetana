# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

It is tracked, and that is a decision. A blanket `CLAUDE.md` line in `.gitignore` kept this document
out of every worktree cut for a task, so nothing working in one could correct it when the code moved
underneath, and it drifted until it named six stores against a tree holding eight. A document that is
wrong is a thing to fix in a diff, and an untracked file has no diff.

`AGENTS.md` beside it is the other half, for agents rather than for this architecture: bd's quick
reference, and the rule that `cp`, `mv` and `rm` may be aliased to `-i` on a person's machine, so
every such call needs `-f`/`-rf` or it hangs for good on a prompt nobody can answer.

## Language

**All comments are written in English, without exception.** This holds for every language in the
tree — Rust `//`, `///` and `//!`, JavaScript and Vue `//` and `/* */`, HTML comments in templates,
CSS, shell and config files. Russian was the earlier convention; the whole tree was swept and there
is no Cyrillic left in it, so a Russian comment in a diff is a regression, not a leftover.

The rule reaches past comments to everything else written for a reader inside the code: test names
(`describe`/`it` titles, Rust `#[test]` function names), assertion messages, `panic!`/`expect`
strings, `thiserror` messages, log lines, `console.*` text and fixture strings. Commit messages stay
in Russian, matching the whole history. UI copy is English, sentence case (see Constraints).

## Commands

```sh
npm install          # postinstall fetches the bd sidecar; it warns and continues without one
npm run dev          # http://localhost:5173 — front end alone, backed by the mock
npm run build
npm run preview      # serve the production build
npm run tauri dev    # the actual desktop app: Rust worker, real bd, live board
npm run fetch-bd     # download the bd binary explicitly (fails hard, unlike postinstall)
node scripts/fetch-icon-associations.mjs   # re-vendor the file-type icons; output is committed
npm test             # front-end tests (vitest), single run
npm run test:watch   # the same, in watch mode
cd src-tauri && cargo test
```

One file, or one test, rather than the lot:

```sh
npm test -- tests/stores/tracker.test.js   # positional argument: a regex over the file paths
npm test -- -t "stale response"            # by the text of an it()/describe()
cd src-tauri && cargo test parse_commondir # by test name
cd src-tauri && cargo test settings::model # by module path
```

Two test runners: `npm test` covers the front end's pure logic — the plain modules and the stores —
and `cargo test` covers the Rust side. That used to say "the nine plain modules" and had been wrong
for some time before anybody noticed; `tests/` mirrors `src/`, so the directory is the count and it
cannot drift the way a number written once does — name where a list lives, not how long it was on the
day somebody looked. Neither runner covers components: there is no component test runner and no
linter or formatter, so do not invent one, and do not claim a change is "tested" on the basis of a
build succeeding.

Front-end tests live in `tests/`, never next to the source. They mock exactly one thing — the IPC
transport — through the official `mockIPC`, and rebuild the store module graph per test;
`tests/support/stores.js` explains why, and `tests/support/setup.js` is the `setupFiles` every one of
them passes through.

A component change is still verified by eye, in the dev server, with the query parameters the app
reads (`src/App.vue`):

| parameter | values | default |
|---|---|---|
| `theme` | `dark`, `light` | `dark` |
| `density` | `comfortable`, `compact` | `comfortable` |
| `view` | `gallery`, `settings`, `compare`, `dialog` | the app |

`?view=gallery` renders every exported component once (`src/views/Gallery.vue`) — the harness for
catching a broken component. Check any component change in all four theme × density combinations,
since both switches only swap CSS custom properties and a component that hardcodes a value will
look correct in exactly one of them.

`?view=settings` is the settings window (`src/views/SettingsWindow.vue`), which in the app is a
second OS window loading that same query string. It paints itself from what the app window tells it,
and in a browser from `settings_load` — which is what makes the settings screen checkable without
Tauri; the one thing it cannot do there is change anything for good. `?theme=`, `?density=` and
`?tab=` are passed in as props rather than read there, so `App.vue` stays the one place that knows
about the query string, and without that this window's own chrome could not be seen in compact or in
the other theme at all. `?view=compare` is the branch-compare window, a third OS window built and
checked the same way; what it is for is in `.claude/rules/vcs-panel.md`.
`?view=dialog&kind=<name>` is the fourth, one window kind for every dialog of the app rather than one
each — the closed list of kinds is `src/views/dialogRegistry.js` and the host is
`src/views/DialogWindow.vue`, whose own headers carry the reasoning.

## Architecture

A desktop app for supervising autonomous AI coding agents: a Tauri 2 shell (`src-tauri/`, Rust)
around a Vue 3 front end (`src/`). The front end is ported from the **Smetana Design System**
(`claude.ai/design`, project `5da5ca35`). Tokens are copied from the design system verbatim;
components are ports of its React sources, keeping prop names, computed styles and behaviour. React
`value`/`onChange` became `v-model` (`modelValue`); React `children` props became named slots. When
something looks odd, the design system is the source of truth — match it rather than "fixing" it.

`src/main.js` → `src/App.vue` → either `views/DesktopApp.vue` (the three-column shell: worktree
files + agents, tab bar over the kanban, task inspector) or `views/Gallery.vue`
(code-split, never in the app bundle). The board is live tracker data, and so are the file tree, the
file tabs, the agents, the branch in the scope bar and the sidebar's Git tab. What is left on the
screen — the scope bar's dirty-file and agent counters among it — is still fixture state in
`views/desktopAppData.js`. The right column used to end in a log pane fed from that same file; it is
gone, because invented output under a real issue claimed the app knew something it did not, and a
session's actual output is the terminal tab. `LogView` itself stays in the library and in the
gallery — the component is fine, the fixture in the product was not.

That right column draws one of four things, and which one is **derived rather than stored**
(`rightPanel` in `DesktopApp.vue`). `DraftInspector` shows a task still being filed — the person's
own words, read-only, with no issue behind them. `ClaimedTasks` shows what a run has taken, with the
card for whichever of them is picked, the list staying above the card because the choice between them
is the point. Otherwise it is `TaskInspector` on the selected issue, and with nothing picked an
`EmptyState` saying so — the slot used to hold a fixture issue instead, so a newly added project
opened by announcing that somebody had filed a task needing a human (smetana-agh). That empty state
is deliberately not drawn under a run's claimed list, where the list is the content. Deriving the
choice is what stops the halves drifting: `selectedTask` is remembered per project in
`settings.json`, so a panel choice that wrote to it would turn a glance at an agent into an edit of a
preference, and a stored version had the run case highlighting a card the inspector refused to draw.

`src/stores/tracker.js`, `src/stores/settings.js`, `src/stores/projects.js`, `src/stores/files.js`,
`src/stores/terminals.js`, `src/stores/git.js`, `src/stores/vcs.js`, `src/stores/runs.js`,
`src/stores/attachments.js`, `src/stores/updates.js`, `src/stores/compare.js`
and `src/stores/app.js` are the **only** files in `src/` that know Tauri exists — components see
reactive stores and nothing else. `mockBackend.js` below is the last and the exception that proves
it: it imports Tauri in order to stand in for the absence of one. `app.js` is the odd one, and it is
a store for exactly this reason rather than for holding state: it has none. It is what the app knows
about itself and asks the desktop for — open the settings window, read this build's version, open a
link in the person's own browser — and every one of those would otherwise be an `@tauri-apps/api`
import inside a component. Several of those files open by counting themselves into this list, which
is a habit worth knowing about before trusting one: an ordinal is written once and the list keeps
growing under it. The list here is the one to check against the tree.

In a browser there is no back end, so `src/stores/mockBackend.js` installs the official `mockIPC`
with the old fixtures: read commands answer, and writes to the tracker reject loudly — a "write"
that looked like it worked would be worse than none. `settings_save` is the one exception, accepted
and dropped, because a browser has nowhere to keep it and failing every debounce tick would only
fill the console. That is what keeps `npm run dev` and `?view=gallery` working with no branching in
components.

One family of modules runs through the whole front end and is named here rather than under any one
part of it: the pure rules, each pulled out of a component because a `.vue` file is the one thing no
test in this repository can reach, so the whole of a rule lives outside the component that draws it.
The place to see them all is `tests/` — for
the reason the note under Commands gives, a list written out here is wrong by the time somebody
trusts it. What they have in common is the shape rather than the count: each is the whole of one
rule, pure, with no Vue and no DOM in it, and each lives under the directory of the part of the
interface it is a rule about.

`src/paths.js` is the one that breaks the second half of that, and it is worth saying why, because
its location is the one thing about it a reader cannot work out for themselves. `basename` — what a
path is called, for a project row, a file tab, a dialog's sentence — is not a rule about any one part
of the interface: two stores and a component module want it at once, so there is no "under" to put it
and it sits at the top of `src/` instead. It had been written out three times over, and the three
disagreed — the newest answered `''` for a root path where the other two answer the path itself,
which would have drawn an empty gap in the middle of a tooltip's sentence. Borrowing a store's copy
instead of lifting it out would have pulled Vue and Tauri into a family defined by having neither.

`src/appearance.js` sits beside it for the same reason: what a stored theme means right now, and what
factor a chosen font size comes to, are wanted by two windows at once — the app and the settings
window — so there is no one part of the interface to file them under. It is deliberately small: the
sizes themselves are the stylesheet's, and this file only says by how much. Its DOM half is split off
into `views/useAppearance.js`, which is what keeps the rules themselves reachable by a test.

`src/catppuccinIcon.js` is the third of them, and the reason is the same shape: what a file's name is
drawn as is wanted by the tree's rows, by the document tabs' store and by the Git panel's change
list, so it belongs to none of the three. It is the **second icon source in the tree**, beside
`core/icons.js`, and the split is by question: lucide answers "what does this control mean", in one
colour, from a hand-kept list; this answers "what kind of file is this", from 656 icons named after
languages and tools. A vocabulary that size cannot be a list somebody maintains by hand, which is
also why it is not tree-shaken — the whole vendored table ships, and the bundle grew by a third for
it.

**It is the one place in the front end that draws colours this design system did not choose**, and
the exception is bought rather than overlooked. `scripts/fetch-icon-associations.mjs` vendors
Catppuccin's own build (MIT), whose SVGs are written against colour *names* rather than hexes, so the
bodies are stored once and a palette is substituted per theme — Latte on light, Macchiato on dark.
Taking a compiled flavour instead is the version that was thrown away: one dark palette measured
1.38:1 against `--surface`, and most of the tree's icons were not on screen in the light theme. What
is still paid: sixteen foreign hues sit near the status colours they are not allowed to be confused
with, and in the change list a modified `.js` puts the status letter and the icon within one degree
of hue of each other. The file's own header carries the measurements; do not re-open the question
without them.

### Where the rest of this document went

This file used to hold a section per subsystem and had grown past 168 000
characters, which is loaded into the context window of every session whatever
that session is about. Rewriting it shorter was tried and bought three days:
the prose grows at the rate subsystems are finished, so the only fix that
holds is one that changes where the growth lands. Each subsystem's section now
lives in `.claude/rules/`, scoped with `paths:` frontmatter to the code it is
about, and is loaded when — and only when — a file under one of those paths is
read. What is left here is what is true in every session regardless of subject.

Two consequences to know before trusting this. A rule is not re-injected after
`/compact`; it comes back the next time a file it covers is read. And a session
that reasons about a subsystem without opening any of its files starts without
its prose, which is what the table below is for — open the file by hand.

**Prose about one subsystem belongs in that subsystem's rule file, never here.**
`tests/docs/claudeMd.test.js` fails when this file passes its budget, which is
the mechanical half of that rule; the other half is judgement, and the test
cannot make it.

| subsystem | rule file |
|---|---|
| the bd tracker, the board, the project list | `.claude/rules/tracker.md` |
| the file tree, the tabs, the CodeMirror editor | `.claude/rules/files-and-editor.md` |
| the branch in the scope bar, the branch list | `.claude/rules/git-head.md` |
| the Git panel: status, merge, rebase, conflicts | `.claude/rules/vcs-panel.md` |
| PTY sessions, the output ring, attention detection | `.claude/rules/terminal.md` |
| CLI agent profiles, intents, prompts, the skill library | `.claude/rules/agents.md` |
| a parked task and the way back from one | `.claude/rules/parked-tasks.md` |
| images on a task, and the one thing that deletes | `.claude/rules/attachments.md` |
| the bell, and the three deliveries of a report | `.claude/rules/notifications.md` |
| runs: the loop, the registry, the report | `.claude/rules/runs.md` |
| how wide a side panel may be | `.claude/rules/panel-widths.md` |
| the column order, and what of the board is drawn | `.claude/rules/kanban-board.md` |
| `settings.json`, and the settings window | `.claude/rules/settings.md` |
| in-app updates, and the release that feeds them | `.claude/rules/updates.md` |

### The bd sidecar

bd ships inside the bundle (`bundle.externalBin`), so the app is self-contained and the version is
fixed. The binary is 128 MB and is **not** committed: `scripts/fetch-bd.mjs` downloads the pinned
release, verifies it against the sha256 digests committed next to `BD_VERSION` (the release's own
`checksums.txt` is only a cross-check), and lays it out as `src-tauri/binaries/bd-<target-triple>`.

`postinstall` runs it with `--optional` and only warns on failure — a contributor who wants the
front end alone should not need a Rust toolchain and a 43 MB download. `npm run fetch-bd` and CI
fail hard instead. `EXPECTED_BD_VERSION` in `service.rs` must stay in step with `BD_VERSION` in the
script; a mismatch surfaces as `bd-version-mismatch` in health, not as a crash.

An agent files a task by running `bd create` itself, and reaches the same binary because
`terminal/pty.rs` puts the sidecar's directory on the front of the `PATH` the agent inherits — see
`.claude/rules/terminal.md` for why in front and not behind.

### Tests

`tests/` mirrors `src/`, and `vitest.config.js` merges the app's Vite config so the alias and the
Vue plugin come along. Two decisions are load-bearing, and both are explained where they live.

The mock boundary is the IPC transport, not the Tauri modules: `listen` and `emit` are themselves
`invoke('plugin:event|…')` calls, so a delta in a test is delivered by a real `emit` through the same
`initTracker` the app runs. `tests/support/ipc.js` also calls `mockWindows`, without which
`getCurrentWindow()` throws, `settings.js` reads that as "we are in a browser" and the window-close
path silently never registers.

Stores are module singletons holding more state than they export, so `tests/support/stores.js`
rebuilds the whole graph per test with `vi.resetModules()`. It hands back `nextTick` from that same
fresh graph: `resetModules` recreates `vue` too, and another instance's `nextTick` drives another
scheduler, so a test awaiting it would wait for a tick that never comes.

Not covered, deliberately: `.vue` files and the CodeMirror wiring (`editor/theme.js`,
`extensions.js`, `compartments.js`), and, for the same reason, `TerminalView.vue` and
`terminal/theme.js` — all of it is DOM, and it is checked by eye through `?view=gallery`.

### Styling: inline style objects, never CSS classes

Components carry no scoped CSS and no utility classes. Every visual value is a computed style object
bound with `:style`, and every value in it is a `var(--token)` reference (see `core/Button.vue`,
`status/StatusBadge.vue`). Two consequences:

- A new component follows the same shape — `computed(() => ({ ... }))` of token references. Do not
  introduce a `<style>` block or a class-based approach.
- Never hardcode a colour, radius, spacing or font value. If a token does not exist for what you
  need, that is a design-system question, not a licence to write `#hex` or `8px`.

Three exceptions, and exactly three. The first is `components/files/editor/theme.js`: CodeMirror
renders its own DOM and the only way to reach it is CSS rules, so this one file is allowed to produce
them through `EditorView.theme()`. The rule is narrowed, not lifted — every value inside is still a
`var(--token)` reference, and no `#hex`, no `px` and no gradient belongs there.
`@codemirror/search`'s own theme paints a `linear-gradient` onto its panel buttons; `theme.js`
suppresses it explicitly (`backgroundImage: 'none'`), because gradients are forbidden everywhere in
this system, including inside a third-party stylesheet that ships its own opinion.

The second is `components/terminal/theme.js`. xterm.js renders its own DOM too, but its API differs
from CodeMirror's in a way that matters: `EditorView.theme()` takes CSS, so `var(--token)` works
there and the browser repaints on its own, while xterm.js takes an `ITheme` of **resolved colour
strings**, so tokens have to be read with `getComputedStyle` and handed over as values. The
consequence has no parallel in the editor — flipping `data-theme` does **not** repaint the terminal
for free, which is why `TerminalView.vue` carries a `MutationObserver` on the root's attributes. The
rule is narrowed the same way: every value still comes from a token.

The third is `src-tauri/src/runs/report.rs`, the widest of the three because its output is not this
app's screen at all: it renders the run report, a self-contained HTML document somebody opens in a
browser with nothing of ours loaded, so there is no stylesheet around it and a token reference would
simply be an unresolved variable. What replaces the rule rather than lifting it: no external
stylesheet, no font off a network, no script and no image — the document reaches nowhere at all,
which is also what makes it safe to hand to a sandboxed frame.

`styles/styles.css` is an `@import` list only; the tokens live in `styles/tokens/`. `tokens/base.css`
holds element defaults (focus ring, selection, scrollbar) and the only three global classes in the
system (`.sm-mono`, `.sm-hatch-blocked`, `.sm-scroll-hidden`).

The first line of `base.css` is `box-sizing: border-box` on everything, and the whole system rests on
it. Components declare a size as a token and add padding and a border on top — `width:100%` with
`padding:0 var(--space-4)`, `height:var(--control-h)` with a border, `width:8px` with a 1.5px ring —
which only comes out right under border-box. The React design system gets it from its own reset; the
port did not carry it over at first, and the cost was `Input` overflowing `Modal` by exactly
`2×--space-4 + 2×--border-w` and `StatusDot` drawing its single `size` prop as three different glyph
sizes. Both vanished with the one line. Do not remove it, and do not "fix" a component by subtracting
its own padding from its width.

### Theme and density live on the document root

Both are attributes on `document.documentElement` (`data-theme`, `data-density`), set by a
`watchEffect` in each view. Every token is defined against them: `tokens/color-*.css` redefine
colours under `[data-theme="dark"]`, and `tokens/space.css` redefines *only* the space scale and
row/control heights under `[data-density="compact"]` — density never changes colour, radius or type.

It carries a third attribute, `data-window-chrome` (`none`, `traffic-lights` or `buttons`, from
`components/shell/windowChrome.js`), and it is a fact about the machine rather than a choice about
the look: the app window's scope bar is its title bar now, so `tokens/space.css` reads it for the
inset macOS's traffic lights need and for the floor under `--scope-bar-h`.

The root carries one more thing in the same spirit, and it is a value rather than a switch:
`--ui-scale`, the app-wide font size as a factor, which the type scale and the row and control
heights are written in terms of. See the settings window above for why it lives in the stylesheet
rather than in JS.

### `status/status.js` owns colour and loudness

The single source of truth for what a status looks like and how loud it is:

- `RESERVED` = `blocked, ready, running, needs-you, done, failed`. These get fixed
  `--status-<name>-*` tokens and a distinct glyph from `STATUS_GLYPH`.
- Anything else is user-defined: `normalizeStatus` → FNV-1a hash → one of 12 generated slots
  (`--status-gen-<0-11>-*`), hues chosen to stay outside a guard band around every reserved hue.
  Generated statuses also render a 2-letter `statusCode` — **status is never colour alone**.
- `attentionLevel(status)` returns `loud` / `live` / `quiet`; components set `data-attention` and
  dim `quiet` to `--attn-quiet-opacity`.

Between the two sits `EXTRA_GLYPH`, and it is a third thing rather than a hole in the first two: a
status that takes a **generated hue and its 2-letter code, but not the generic tag glyph.** Three of
bd's built-in statuses fall outside `RESERVED` and reach the board as user-defined (`deferred`,
`pinned`, `hooked` — clock, pin, anchor), and beside them are the names a project's own way of
working keeps producing: `parked`, `ready-to-merge`, and `human-check`, the column for work that is
merged and closed and still owes somebody a look. `parked` borrows `needs-you`'s triangle
deliberately, since a parked issue is one somebody has to come back to and the two never share a
board. `human-check` deliberately does **not** borrow it: it draws a person with a tick, because the
column is about the person and that has to be visible, while `needs-you`'s loudness is budgeted at
one or two rows on a screen and this column holds a dozen cards at a time — so it stays an ordinary
`live`. A status nobody has heard of is still an ordinary outcome and draws the generic tag.

Two rules that break the product if ignored: `loud` (needs you) is budgeted at **1–2 per screen** —
if everything shouts, the design failed — and there is no fixed column set, so never hardcode one.

`core/interactive.js` (`useInteractive`) is the shared hover/press tracker: interaction is a surface
step up, never a colour change and never a transform, so controls in a dense list cannot jump.

### `kanban/issueType.js` — what a card is, beside what it is doing

A card does not draw its status: it is already the column it sits in, and saying it twice spends the
one badge a card has on nothing. It draws bd's **type** instead, through `TypeBadge` — the status
prop stays because it still decides the card's loudness, the border, the flash and the dimming of
anything done.

Three of bd's six types carry a hue (`bug`, `feature`, `epic` — `tokens/color-type.css`); `task`,
`chore`, `decision` and every custom type share the neutral `--type-plain-*` set. That split is the
whole design: `task` is bd's default, so a board where every type is coloured is a board where
nothing stands out. A type this file has never heard of is an ordinary outcome, not an error — it
renders with its own name, the generic `tag` glyph and the neutral colours.

The type hues sit *inside* the reserved status bands, which the generated status palette carefully
avoids, and that is deliberate rather than sloppy: there is no free hue space left, so the two
vocabularies are separated by silhouette instead. A type badge is **sentence-case sans with no
border**; a status badge is **uppercase mono with one**. Both sit side by side in the task
inspector's header, which is where the difference is easiest to check. Keep it — a red `Bug` drawn
in the status idiom is a red `failed` pill to anyone scanning quickly.

### Icons

`core/icons.js` is the only file that names Lucide, and it registers glyphs explicitly so the build
tree-shakes to the ones actually used — the file is the list, and a number written here would be
wrong within a month. Adding a glyph to the UI means adding it there first; `Icon` warns in dev for
an unregistered name. Note `message-circle-question-mark` is kept as the design-system key and mapped
to lucide 0.469's `MessageCircleQuestion`.

It is not the only icon source any more: `src/catppuccinIcon.js` above draws **files and folders by
name**, in colour, from a vendored set of 656. Everything else on screen — every control, every
status, every empty state — is this file's, in one colour, from a list a person can read. Adding a
glyph for a *file type* means the vendoring script; adding one for anything else means here.

### Adding a component

Create it under the matching `src/components/<group>/`, export it from `src/components/index.js`
(the library's public surface), and add it to `views/Gallery.vue` so it stays checkable. Product
code imports from `index.js`; components import their siblings by relative path — the `@` → `src`
alias exists in `vite.config.js` but is currently unused, so prefer relative paths for consistency.

## Constraints

- **No gradients, glass, blur or emoji.** Partly taste, partly the WebKitGTK constraint.
  Two things are drawn as images rather than as tokens, and both are exceptions with a reason. The
  app icon on the About tab (`src/assets/app-icon.png`) is the one **raster**: the exception is the
  artwork rather than the medium, since this picture is the app's identity and a version redrawn from
  tokens would be a second copy to keep in step with the first. The file-type icons
  (`src/catppuccinIcon.js`) are the other, and they are vector inside an `<img>` — which is the part
  that costs: a `data:` URL is opaque to the stylesheet, so nothing in them can be a token and
  nothing repaints on its own. They carry their palette in JS instead, which is why they follow the
  theme at all. Anything else wanting a picture is still a design-system question. The app icon
  carries its own black ground and squircle, which lets one file serve both themes with no border and
  no radius from the component; `scripts/make-app-icon.py` builds it and the bundle icons from one
  source, so the two cannot drift, and `app-icon.png` at the repository root is that 1024 master.
- **No native right-click menu, anywhere.** `src/main.js` refuses every `contextmenu` event in
  the document (`src/nativeMenu.js`), in the app and in the dev server alike, because a check is
  worth having only if it shows what the app does. What the webview offers is the platform's, not
  this product's — Look Up, Translate, Share, Inspect Element — and it opens over any word on the
  screen. A menu this app wants is its own — `overlays/PointerMenu.vue` on the rows that have earned
  one, a project row and a branch row so far — and those keep working: refusing a default stops no
  propagation.
- Sentence case everywhere; identifiers in mono (`--font-mono`), prose in sans.
- The primary button is ink on paper with no brand hue — the entire saturated range belongs to
  status.
- The build target (`es2021`, `chrome100`, `safari15`) is set for the system webviews Tauri runs in
  (WebKitGTK / WKWebView / WebView2). Do not raise it, and do not reach for APIs newer than that.
- `tokens/fonts.css` `@import`s IBM Plex Mono from Google Fonts; an offline Tauri build needs the
  latin subset vendored locally instead.
