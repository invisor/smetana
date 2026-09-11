# Handoff: agent conversation panel (rendered markdown)

## Overview

The conversation panel is the narrow, tall column in the three-column shell where
a person talks to a running coding agent and reads what it does. It is read far
more often than typed into and a session runs to hundreds of turns, so this
design is built for density and scannability, not decoration.

The agent's and the person's text arrive as **markdown and are rendered to
HTML**. The elements are generated, so nothing can carry a hand-written style.
The design is therefore delivered as **one stylesheet over a fixed markup
contract**, not as a set of components:

| file | what it is | ship it? |
|---|---|---|
| `sm-prose.css` | the stylesheet. Every rule scoped under `.sm-prose`, every value a `var(--token)` | **yes, as-is** |
| `sm-prose-tokens.css` | the tokens this design needs that the system does not have yet | **yes** — or fold into `tokens/` |
| `markup-contract.md` | the exact HTML the renderer must emit, case by case, with the reasoning behind each decision | read first |
| `sm-prose-turns.css` | three turn treatments not taken, plus the previous `rail` default | reference only |
| `reference.html` | the rendered result, standalone. Open it in a browser; switch theme, density and turn style in the top bar | reference only |
| `ds/` | the smetana token set, copied so `reference.html` runs offline | already in the app |
| `assets/*.png` | two stand-in charts, drawn for this design to demonstrate the illustration mat | throwaway |

## About the design files

`reference.html` is a **design reference created in HTML** — a prototype showing
the intended look and behaviour. Its page chrome (top bar, panel frame, the
inline `<script>`) is scaffolding, not production code.

`sm-prose.css` and `sm-prose-tokens.css` are the opposite: they **are** the
deliverable and are meant to be dropped into the app unchanged. The work in the
codebase is (a) making the markdown renderer emit exactly the markup in
`markup-contract.md`, and (b) wiring the four behaviours listed under
*Interactions* below in the app's existing environment (Vue 3 + Tauri 2, per the
design system) using its established patterns.

## Fidelity

**High fidelity.** Final colours, type, spacing, radii, states and motion, all
resolved from smetana tokens — no raw values anywhere. Recreate it exactly.
There are no invented colours to approve: everything comes from
`tokens/color-surfaces.css`, `tokens/color-status.css`,
`tokens/color-editor.css`, `tokens/typography.css`, `tokens/space.css`,
`tokens/shape.css`, `tokens/motion.css`, plus the 16 proposed tokens named in
`sm-prose-tokens.css`.

## The one view

**Conversation panel** — a single scrolling column, designed at **420px** and
correct from ~320px up. There is no second screen: every case below lives in the
same column.

Structure, outside `.sm-prose` (already the app's own chrome):

```
panel (flex column, --canvas, 1px --border, --radius-4)
├── header   --tab-h,  StatusBadge + agent name + model · project · branch
├── scroll   flex:1; min-height:0; overflow-y:auto; overflow-x:hidden
│   └── .sm-prose            ← everything this handoff covers
└── composer --border-top, input + Button variant="primary" size="sm"
```

`.sm-prose` is a **flex column** with `gap: --space-6` (`--space-5` at compact)
and `padding: --panel-pad`. It must stay flex: the person's turn sits on the
right of the column by `align-self`.

### The two sides

- **Person's turn** — `article[data-turn="person"]`: a bubble on its own side.
  `--surface-raised`, 1px `--border-subtle`, `--card-pad` inset,
  `max-width:88%`, `align-self:flex-end`, `--shadow-raised` (light theme only —
  the token collapses to `none` in dark). Corners are `--radius-4` except the
  **bottom-right, which is `--radius-1`**: the squared corner is the tail and it
  points back at the sender. No drawn triangle, nothing extra for the sanitiser.
  Side and shape carry the distinction — there is no hue in it, because
  saturated colour belongs to status.
- **Agent's turn** — `article[data-turn="agent"]`: no container, no avatar, no
  caption. It is the plain ground of the panel and most of the column.
- May end in an **attachment strip**: `ul[data-attachments]` of mono chips
  carrying a file name (never a thumbnail), truncating with an ellipsis.

Three treatments that were explored and not taken (`bubble-left`,
`sliver-right`, `ledger`) and the previous `rail` default are in
`sm-prose-turns.css` behind `data-turn-style` on the prose root. Do not ship
them; keep them if the question reopens.

## Everything else, case by case

`markup-contract.md` is the specification — it carries the exact markup and the
justification for each decision. The short version:

| case | markup | the decision to know |
|---|---|---|
| prose | `p strong em del small kbd code ul ol li dl dt dd blockquote hr h1–h6`, no attributes | headings do not scale like a document: h1 18px + rule, h2 15px + hairline, h3/h4 body size at two weights, h5 12px, h6 the 10px mono caps micro-header. In a 420px column weight and rules separate levels, not size |
| inline code | `:not(pre) > code` | recessed (`--surface-sunken`, hairline, `--radius-2`) so it never reads as a button |
| lists | nested `ul`/`ol`, `ol` inside `ul` | disc → en dash → circle, so two levels never look like one list. `ol` markers are mono and muted |
| task lists | `ul[data-task] > li[data-checked]` | the box is **drawn in CSS**, not `<input type="checkbox" disabled>` — a native control is three different pieces of platform chrome across WebKitGTK / WKWebView / WebView2, cannot be coloured from the token set, and reads as operable when it is not |
| tables | `div[data-table-scroll] > table`, `data-align="left\|center\|right"`, `data-wide` | alignment is an **attribute, not `style=`** — one thing for the sanitiser to allow, and right-aligned cells then get mono tabular figures. **Ruled, not zebra**: stripes lay a second rhythm over a column that already alternates bubbles against plain ground, and go muddy at compact density |
| wide tables | `data-wide` on the table | the wrapper scrolls, the panel never grows. The affordance is an **always-drawn trough**; overlay scrollbars would hide the only signal that the block continues, and a fade would need a gradient |
| code blocks | `figure[data-code][data-lang] > figcaption + button[data-copy] + pre > code` | a 2-column grid: header band (row 1), code (row 2, spanning). The copy control lives **in the band**, so it can never overlap a long first line. `white-space:pre` — code never wraps |
| copy control | `data-state="idle\|copied"` | **always visible, never hover-revealed** — a control that appears on hover is not there when you tab to it. Confirmation is the control changing (tick glyph + "Copied"), not a toast and not a colour flash |
| links | `a[href^="https://"]` vs `a[data-path][data-kind="file\|dir"]` | external: sans, `--text-link`, `↗`. Local: mono with a permanent hairline underline, because it sits in text already full of mono that does nothing. The distinction is the **attribute, never the href shape** |
| long paths | `a[data-path] > span[data-head] + span[data-tail]` | middle truncation is done in the **markup**, because CSS cannot: the head truncates, the tail (file name + line) never does |
| working states | `div[data-activity="waiting\|streaming\|done\|failed"]` | one strip, four moments, same element in the same place. **Waiting does not spin**: the elapsed clock is the liveness signal, and the mark fades on `--dur-pulse` rather than blinking |
| live edge | `span[data-edge]` as the last child of the last block | a `--prose-caret-w` bar in `--attn-live`. Nothing else about a partial message differs from a finished one, so nothing reflows when it completes |
| reasoning | `details[data-reasoning] > summary` | folded by default, dimmed by **colour and size, not opacity**, so it still clears the contrast floor when open. Our own disclosure marker (two borders, rotated); the platform triangle is hidden |
| illustrations | `figure[data-figure] > (img \| svg \| div[data-placeholder]) + figcaption + button[data-expand]` | see below |

### The illustration rule (the dark-theme failure case)

A picture carrying its own light ground inside a dark panel is solved by making
the two accepted forms mean different things:

- **`<img>` is always matted.** It sits on `--prose-figure-mat` — deliberately
  the *same* paper value in both themes — inside a border. A white-ground PNG
  then reads as a pinned print, intentional and bordered, instead of a hole cut
  in the panel; and a transparent PNG drawn in dark ink stays legible.
- **Inline `<svg>` is the preferred form** and is accepted **only** when it
  paints with `currentColor` and `var()` tokens. It gets no mat: it is part of
  the panel and follows the theme. Text inside it is styled by us (mono,
  `--text-2xs`, `fill:currentColor`).

The renderer enforces that rule so the stylesheet never has to guess. Loading
(`data-state="loading"`, skeleton at `--prose-figure-loading-h`) and failure
(`data-state="error"`, mono "Failed to render" + source and reason) keep the
frame and caption, so nothing jumps when the picture resolves. Two figures in
one turn go in `div[data-figures]`, a wrapping flex row that wraps rather than
crushing below `--prose-figure-min`.

## Interactions & behaviour (what the app must implement)

1. **Copy.** Click `button[data-copy]` → copy `pre` text content → set
   `data-state="copied"` and the label to "Copied" → revert after **1600 ms**.
   Keyboard reachable, focus ring inset (`outline-offset:-2px`) because the
   figure clips.
2. **Expand.** Click `button[data-expand]` → open the illustration full size in
   a window of its own (Tauri window or the app's Modal). The control is in the
   caption row and is always present, including when there is no caption.
3. **Local links.** Intercept clicks on `a[data-path]`; `data-kind="file"` opens
   in the editor, `data-kind="dir"` reveals in the file manager. `href` stays
   for native focus, middle-click and copy-link. External links open in the
   person's browser.
4. **Activity strip.** Mount `div[data-activity="waiting"]` the moment the
   message is sent, with a ticking elapsed clock; switch to `"streaming"` when
   the first token arrives and append `span[data-edge]` to the last block,
   moving it as text arrives; on completion replace both with
   `div[data-activity="done"]` carrying the turn's receipt (`in · out · $ · s`).
   On failure, `"failed"` — the one place the strip takes a saturated colour,
   because failed is a status.

Motion is the system's: `--dur-fast` for state changes, `--dur-pulse` for the
activity mark and the caret, and `prefers-reduced-motion:reduce` kills all of it
(the clock still ticks). No entrance animation anywhere.

## State

Per turn: `role` (person/agent), blocks, attachments, `activity`
(`waiting|streaming|done|failed`), elapsed seconds, receipt (tokens in/out, cost,
duration), reasoning text + open/closed. Per code block: `copied` (transient,
1600 ms). Per figure: `loading|ok|error` + error reason. Panel-wide: nothing —
**theme and density are attributes on the document root** (`data-theme="dark"`,
`data-density="compact"`) and the stylesheet never branches on anything else.

## Design tokens

No raw values. Existing smetana tokens carry everything except the 16 in
`sm-prose-tokens.css`, each with its role in a comment:

`--prose-figure-mat` `--prose-figure-min` `--prose-figure-loading-h`
`--prose-table-wide-min` `--prose-box` `--prose-tick-w` `--prose-tick-h`
`--prose-chevron` `--prose-dot` `--prose-caret-w` `--prose-icon`
`--prose-scrollbar-h` `--prose-scroll-track` `--prose-scroll-thumb`
`--prose-external-glyph` `--prose-dir-glyph` `--prose-meta-sep`

Only `--prose-figure-mat` is a colour, and it is intentionally theme-independent.
Everything else is a small length or a content string.

Aliases (`--prose-block-gap`, `--prose-rule`, `--prose-code-bg`, …) are declared
**on `.sm-prose` itself**, not on `:root`, and must stay there: a custom property
substitutes `var()` at the element where it is declared, so a density-dependent
alias written on `:root` would freeze at the root's density and compact would
stop working inside the panel.

## Assets

- `assets/fig-latency.png`, `assets/fig-bundles.png` — stand-in charts drawn for
  this design to demonstrate the illustration mat in dark theme. Throwaway; real
  pictures come from the agent.
- Icons are **Lucide** (`copy`, `check`, `maximize-2`) behind the design
  system's `Icon` component, as inline SVG inside the buttons. The status badge
  in the panel header is the system's `StatusBadge`. No new icons were drawn.
- Fonts: the platform sans stack and **IBM Plex Mono**, already the app's only
  webfont.

## Build target

`es2021` / `chrome100` / `safari15`. No `:has()`, no container queries, no
nesting; `:where()` carries the zero-specificity base. Three properties are
newer than the target and used as progressive enhancement only —
`text-wrap:pretty|balance`, `::marker{font-family}`, `scrollbar-color` — all
falling back to the browser default with no layout consequence.

## Acceptance checks

- Four combinations (dark/light × comfortable/compact) at 420px, and again at
  320px: no horizontal overflow of the panel anywhere.
- A 40-character hash in a table cell does not widen the panel; a 5-column table
  scrolls inside its wrapper with the trough visible.
- A code block whose first line is 140 characters long: the copy button is
  reachable, legible and does not overlap the code.
- Tab through a turn: local link, external link, reasoning summary, copy,
  expand — all take a visible 2px `--focus-ring`.
- A white-ground PNG in dark theme sits on the mat inside a border, and does not
  read as a hole in the panel.
- First and last child of every turn have no stray margin.
- With `prefers-reduced-motion: reduce`, nothing moves and the elapsed clock
  still ticks.
