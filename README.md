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
