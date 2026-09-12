---
paths:
  - "src/components/conversation/**"
  - "src/components/markdown/**"
  - "src/stores/conversation.js"
  - "src/styles/sm-prose.css"
---

# The conversation panel: what it renders, and why

`.claude/rules/terminal.md` is where a driven session's *identity* is decided — which harness, which
id, which tab it is drawn under. This file is the other half: what the panel that shows one actually
puts on screen, and what the epic that built it (`smetana-qix9`) decided against on the way there.
The markup contract itself — every element, every `data-` attribute, the four rendered combinations —
is `docs/design_handoff_conversation_panel/markup-contract.md`; this file does not repeat it, only
the reasoning behind the choices in it that are not otherwise written down anywhere in the tree.

## The fourth inline-style exception

`src/styles/sm-prose.css`, scoped entirely under `.sm-prose`, is the **fourth** declared exception to
"inline style objects, never CSS classes" (CLAUDE.md, Constraints), beside `files/editor/theme.js`,
`terminal/theme.js` and `runs/report.rs`. The reason is mechanical, the same shape as the other three:
`::marker`, `::-webkit-scrollbar`, `:focus-visible`, `:hover` and `@media (prefers-reduced-motion)`
have no computed-style-object equivalent, and a design that needs a custom list marker, a themed
scrollbar thumb, a focus ring that only shows for a keyboard, a hover state or reduced motion cannot
ship without one of them. The exception is narrowed the same way the other three are, not lifted:
every value inside is still a `var(--token)` reference, backed by `tokens/prose.css` for the handful
this design needed and the system did not already have, with no `#hex`, no `px` and no gradient
anywhere in the file — and it licenses only those five selector shapes, on that one root class.

**It does not license `v-html`.** The markup this file paints is still emitted by Vue's own
templates — `Markdown.vue`'s tree of `<p>`, `<ul>`, `<blockquote>` and the rest, one component per
element, the same as every other view in this tree. A stylesheet answers "how does this element
look"; it says nothing about how the element got there, and nothing about that changed.

## Why `v-html` never appeared

The markup contract describes finished HTML — a `<ul data-task>`, a `figure[data-code]` — which reads
as an argument for building a string of markup and injecting it. `Markdown.vue`'s own header states
the decision made instead, in so many words: **no `v-html`, deliberately and permanently**. The tree
is drawn as Vue nodes, one component per block and per inline span, so an issue's text — bd's own
prose fields, an agent's, a person's — can never become markup, because it is never parsed as markup
in the first place past `parseMarkdown`'s own AST; there is no sanitiser anywhere in this family of
files because there is nothing for one to guard. The cost is real and paid on purpose: the renderer is
a `Markdown`/`MarkdownInline` pair recursing over a parsed tree rather than one `innerHTML` assignment,
and every new block type is a new branch of a `<template v-else-if>` rather than a new string
template. The bubble's own "tail" is the same decision at the level of one glyph: the person's turn
gets a squared corner (`border-radius`, section 2 of the contract) rather than a drawn speech-bubble
triangle, because a triangle is either an SVG or a `::before` shape hack, and either way is one more
thing an HTML-injection path would have had to be trusted to allow. Side and shape carry the
distinction instead, with nothing for a sanitiser to admit or refuse.

## The bubble, and why `rail` was dropped

The person's turn sits on its own side — raised surface, bounded, held off the right edge — and the
agent's turn is the plain ground of the panel: no container, no avatar, framing every one of them
would turn the column into a stack of boxes, and a driven conversation is mostly the agent talking.
Side and shape carry the distinction on purpose, not hue: saturated colour is reserved for status
(CLAUDE.md, `status/status.js`), and a coloured bubble would put a second vocabulary next to the one
that already means something specific in this app.

The turn-treatment exploration tried four answers to "how does a person know whose sentence they are
reading" (`docs/design_handoff_conversation_panel/sm-prose-turns.css`, kept as a record of what was
compared, `Turn variants.dc.html`). The one before the bubble — `rail`, a raised surface with a
neutral bar down its left edge — was the previous default and was dropped for a specific reason
written into that file's own header: **a left bar is what a quotation uses**, and this panel's own
`blockquote` is two steps away, inside the very same turn. A person's message framed with the
quotation's own device would read as the panel quoting the person rather than the person speaking,
exactly backwards from what a chat turn is supposed to say. The bubble's side-and-corner treatment
shares no device with `blockquote` at all, which is why it, not `rail`, is what shipped.

## The task box is drawn, not `<input type="checkbox" disabled>`

`sm-prose.css`'s own comment states the reason as fact: a native checkbox renders as platform chrome
in three different webviews (WebKitGTK, WKWebView, WebView2), cannot be coloured from the token set
without `appearance:none` — at which point it is being drawn by hand anyway, just through a stranger
API — and it *reads as operable* when it is not. This is a transcript of a task list an agent wrote,
not a form; the state is already decided by the moment it reaches the panel, and a control that
invites a click it will not honour is worse than no control. `ul[data-task] > li::before`/`::after`
draw the box and the tick as plain shapes off `--border-strong` and `--text-secondary`, and a checked
item drops to `--text-muted` — done is quiet, the same idiom `data-attention="quiet"` uses elsewhere.

## Tables are ruled, not zebra-striped

`sm-prose.css` section 8 states this one directly: zebra striping fills a second alternating rhythm
over a column that already alternates raised person turns against the agent's plain ground, so a
zebra table inside an agent turn would compete with the turn structure itself rather than sit inside
it quietly. Striping is also the first thing to go muddy at compact density, where rows sit four
points apart — a stripe that thin reads as noise rather than as a row boundary. A hairline
`border-bottom` on every cell plus a weighted header does the same job — telling one row from the
next — with one border instead of a second fill colour, and it survives compact density unchanged.

## The figure's mat, and the inline `<svg>` that gets none

This landed an hour before this task, in `smetana-je5v`; the reasoning is `MarkdownFigure.vue`'s and
`figureSource.js`'s own, restated here because it is a decision the next session could otherwise
re-litigate. Two accepted forms answer the same dark-theme failure case differently:

- A raster (`<img>`) always sits on `--prose-figure-mat`, the same paper token in both themes, inside
  a border with padding. A screenshot that carries its own white ground then reads as a print pinned
  to a mat rather than as a hole cut in a dark panel, and a transparent PNG drawn in dark ink stays
  legible against the same mat either way.
- An inline `<svg>` is the *preferred* form and gets no mat at all: it is treated as part of the
  panel rather than as a photograph of one, so it follows the theme like everything else on screen.
  That trust is conditional — `figureSource.js`'s `readInlineSvg` decodes the `data:image/svg+xml`
  payload and walks it element by element against a closed diagram vocabulary, refusing any element,
  attribute or namespace outside it, and refusing any `fill`/`stroke` that is not `currentColor`,
  `none`, `transparent` or a `var(--token)` reference. **The renderer checks the render, not the
  source's shape**: this is not a sanitiser that strips the parts it does not like and shows the
  rest — an SVG that fails any part of the walk fails the *whole* figure and falls back to the same
  placeholder a broken raster load draws, captioned with the specific reason. A gate that still shows
  something is a gate somebody has to trust the surviving parts of; this one refuses outright instead.

The expand control (`button[data-expand]`, opening the picture in `ImageWindow.vue`) is drawn only
for a figure with a real file behind it — resolved through `image_read`, the same command
`.claude/rules/attachments.md` describes for a stored attachment. A `data:` source and a validated
inline `<svg>` are never read off disk, so there is no absolute path to aim that window at, and the
caption row draws without the control rather than with a dead one.

## The copy button is always visible, never hover-revealed

A code block's copy control sits in the header band rather than floating over the first line, and it
is drawn at rest — muted text, no border — rather than appearing on hover. `sm-prose.css` section 9
gives the reason: this panel is read by keyboard as much as by pointer, and a control that only
appears on hover is a control that does not exist for someone who has just tabbed to it. Its resting
weight keeps the budget close to nothing when nobody is using it, but "not there until proven wanted"
was rejected in favour of "there, quietly, until pressed."

## Waiting does not spin — it ticks

The agent's activity strip (`TurnResult.vue`, `sm-prose.css` section 11) has one moving part while a
turn is open, and it is a `<time>` counting seconds, not a spinner. The reason is in `TurnResult.vue`'s
own header: the elapsed time *is* the liveness signal — it is the literal answer to the question a
person watching that strip is asking, "is this still going" — and a spinner would be a second,
decorative answer to the same question repeated next to the real one. The pulsing dot beside it fades
on `--dur-pulse` rather than blinking or rotating, `prefers-reduced-motion` stops the fade globally,
and the clock keeps counting regardless, because ticking seconds are information, not motion for
their own sake. `role="status"` makes the strip an implicit `aria-live="polite"` region, and the
ticking `<time>` inside it carries `aria-hidden="true"` for exactly one reason: without it, a screen
reader would announce "thinking, 5s… 6s… 7s" for the length of the whole turn, drowning the one
sentence the region exists to announce once.

## Streaming: not here yet, and what it is waiting on

`sm-prose.css` section 11 already names four moments for the activity strip — "waiting, streaming,
done, failed" — and carries a rule for `span[data-edge]`, the live caret meant to sit at the end of a
message still arriving. Neither is wired to anything as this task lands: `journal.js`'s fold only
ever produces `waiting`, `done` or `failed`, `session/model.rs`'s `EventKind` has no delta variant,
and nothing under `src/components/conversation/` ever sets `data-activity="streaming"` or renders a
`data-edge` element. The CSS is not dead code so much as a hook cut ahead of the wire that will use
it: text deltas on the wire from both harnesses this app drives, which is exactly what
`smetana-6we6` (in flight in a sibling worktree as this task is written) is building on the
`src-tauri/src/session/` side. **This paragraph is provisional and belongs to whoever lands that
task to correct**: once deltas exist, `journal.js` gains a fourth state, `TurnResult.vue` (or a
sibling) grows a live caret, and this section should describe what shipped rather than what was
merely reserved for it.

## One renderer, shared with the task inspector — and why it was not forked

`Markdown.vue` and `sm-prose.css` are not this panel's alone: `TaskInspector.vue` draws every one of
bd's five prose fields — description, acceptance criteria, design, notes, close reason — through the
same `Markdown` component under the same `class="sm-prose"` root, since a paragraph an agent wrote
into a task's description and one it sent in a message are the same kind of text and end up as
identical markup either way. Forking a second stylesheet or a second component for
"conversation markdown" versus "task-field markdown" was rejected because there is no second set of
rules to write — every element the contract names (a table, a task list, a code block, a figure)
means the same thing and needs the same rule in both places, and a fork would have meant keeping two
copies of that rule in step by hand. What differs between the two hosts is contained to one `:style`
override at the call site: `TaskInspector.vue`'s `flatProse` turns `display:flex` into `display:block`
and zeroes `.sm-prose`'s own `padding`, because the inspector's panel is not a scrolling column of
turns and already has its own inset from `DesktopApp.vue` — without that override, every field in the
inspector would carry the conversation panel's turn gap on top of its own paragraph spacing, and the
description would sit inset twice. The block-level rules (`:where(p,ul,ol,…)` margins,
`:first-child`/`:last-child` trimming) still apply either way, because those are not part of the
"container" half `flatProse` turns off.

## `sm-prose-turns.css` is a record, not a switch

`docs/design_handoff_conversation_panel/sm-prose-turns.css` holds the three turn treatments not
taken plus the retired `rail` default, gated behind `data-turn-style` on the `.sm-prose` root. It
lives under `docs/`, not `src/`, and stays there: it is the exploration the bubble was chosen from,
kept so a reader can see what was compared, not a feature this app ships. `data-turn-style` never
appears anywhere in `src/` — nothing in the front end sets it, and the shipped stylesheet
(`src/styles/sm-prose.css`) needs no `[data-turn-style="bubble-right"]` selector because the bubble is
simply the default. Vendoring the exploration file into the app bundle to keep the other three
treatments "available" was never on the table: a treatment nobody can reach from a setting is a dead
branch with a maintenance cost, not a feature, and the four rendered combinations
(`Conversation panel.dc.html`) already say which one shipped.
