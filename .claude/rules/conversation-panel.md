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
The markup contract itself — every element, every `data-` attribute — is
`docs/design_handoff_conversation_panel/markup-contract.md`; this file does not repeat it, only the
reasoning behind the choices in it that are not otherwise written down anywhere in the tree.

## The fourth inline-style exception

`src/styles/sm-prose.css`, scoped entirely under `.sm-prose`, is the **fourth** declared exception to
"inline style objects, never CSS classes" (CLAUDE.md, Styling), beside `files/editor/theme.js`,
`terminal/theme.js` and `runs/report.rs`. The reason is mechanical, the same shape as the other three:
`::marker`, `::-webkit-scrollbar`, `:focus-visible`, `:hover` and `@media (prefers-reduced-motion)`
have no computed-style-object equivalent, and a design that needs a custom list marker, a themed
scrollbar thumb, a focus ring that only shows for a keyboard, a hover state or reduced motion cannot
ship without one of them. The exception is narrowed the same way the other three are, not lifted:
every value inside is still a `var(--token)` reference, backed by `tokens/prose.css` for the handful
this design needed and the system did not already have, with no `#hex`, no `px` and no gradient
anywhere in the file — and it licenses only those five selector shapes, on that one root class.

**It does not license `v-html`.** The markup this file paints is still emitted by Vue's own
templates — `Markdown.vue`'s tree of `<p>`, `<ul>`, `<blockquote>` and the rest, one element per
block node, emitted by one `v-else-if` chain, the same as every other view in this tree. A
stylesheet answers "how does this element look"; it says nothing about how the element got there,
and nothing about that changed.

## Why `v-html` never appeared

The markup contract describes finished HTML — a `<ul data-task>`, a `figure[data-code]` — which reads
as an argument for building a string of markup and injecting it. `Markdown.vue`'s own header states
the decision made instead, in so many words: **no `v-html`, deliberately and permanently**. The tree
is drawn as Vue nodes, one element per block node, emitted by one `v-else-if` chain, so an issue's
text — bd's own prose fields, an agent's, a person's — can never become markup, because it is never
parsed as markup in the first place past `parseMarkdown`'s own AST; there is no sanitiser anywhere in
this family of files because there is nothing for one to guard. The cost is real and paid on purpose:
the renderer is a `Markdown`/`MarkdownInline` pair recursing over a parsed tree rather than one
`innerHTML` assignment, and every new block type is a new branch of that `v-else-if` chain rather
than a new string template. The bubble's own "tail" is the same decision at the level of one glyph:
the person's turn gets a squared corner (`border-radius`, section 1 of the contract) rather than a
drawn speech-bubble triangle, because a triangle is either an SVG or a `::before` shape hack, and
either way is one more thing an HTML-injection path would have had to be trusted to allow. Side and
shape carry the distinction instead, with nothing for a sanitiser to admit or refuse.

## The bubble, and why `rail` was dropped

The person's turn sits on its own side — raised surface, bounded, held off the right edge — and the
agent's turn is the plain ground of the panel: no container, no avatar, framing every one of them
would turn the column into a stack of boxes, and a driven conversation is mostly the agent talking.
Side and shape carry the distinction on purpose, not hue: saturated colour is reserved for status
(CLAUDE.md, `status/status.js`), and a coloured bubble would put a second vocabulary next to the one
that already means something specific in this app.

The turn-treatment exploration tried four answers to "how does a person know whose sentence they are
reading" — `docs/design_handoff_conversation_panel/sm-prose-turns.css`, kept as a record of what was
compared; the design project's own `Turn variants` page walked the same four side by side, but that
page lives in the design project, not this repository. The one before the bubble — `rail`, a raised
surface with a neutral bar down its left edge — was the previous default, and `sm-prose-turns.css`'s
own `rail` block carries the reason it was dropped: **a left bar is what a quotation uses**, and this
panel's own `blockquote` is two steps away, inside the very same turn. A person's message framed with the
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
it quietly. Striping is also the first thing to go muddy at compact density, where rows sit 4px
apart — a stripe that thin reads as noise rather than as a row boundary. A hairline
`border-bottom` on every cell plus a weighted header does the same job — telling one row from the
next — with one border instead of a second fill colour, and it survives compact density unchanged.

## The figure's mat, and the inline `<svg>` that gets none

This landed in `smetana-je5v`, immediately before this task; the reasoning is `MarkdownFigure.vue`'s and
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
is drawn at rest — muted text, no visible border — rather than appearing on hover. Its border is not
absent, only invisible: `border:var(--border-w) solid transparent`, load-bearing rather than
decorative, since it reserves the same box the hover state's real border fills, and without it the
control would shift by a border's width the moment a pointer found it. `sm-prose.css` section 9
gives the resting-versus-hover reason itself: this panel is read by keyboard as much as by pointer,
and a control that only appears on hover is a control that does not exist for someone who has just
tabbed to it. Its resting weight keeps the budget close to nothing when nobody is using it, but "not
there until proven wanted" was rejected in favour of "there, quietly, until pressed."

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

## Streaming, and the one harness it actually reaches

`sm-prose.css` section 11's four moments and its `span[data-edge]` rule were cut ahead of the wire
that would use them; `smetana-6we6` is what wired them. `session/model.rs`'s `EventKind` grew
`TextDelta`, one incremental piece of a reply, and `journal.js`'s fold — `streamingRow` in its own
header — stitches a run of them into one growing `agent` row (`streaming: true`) and closes it on the
existing whole `Text` event, replacing the stitched text with that event's own authoritative copy
rather than trusting the concatenation. `TurnResult.vue` gained the fourth `data-activity` word,
`streaming`, sharing `waiting`'s own clock — a reply arriving is the same wait resolving, not a second
thing starting. `Markdown.vue` gained a `streaming` prop and draws `span[data-edge]` as the last
child of the last block while one is true, propagated through a blockquote or a list to whichever
nested block is actually last; a table, a definition list, a rule, or a run of images as the literal
last block draws no caret, recorded as a narrow gap in that file's own header rather than solved.

**This reaches Claude Code alone, and that is not a phase one of two.** `session::service::driver_for`
answers `"claude" => ClaudeDriver, _ => None` and `ClaudeDriver` is the only `impl Driver` in the
tree — a Codex session never reaches `journal.js`, `EventKind`, or anything else this file is about; it
runs the PTY road `.claude/rules/terminal.md` describes, start to finish. The original wording of this
paragraph asked for "text deltas on the wire from both harnesses this app drives", which rested on a
belief about the tree rather than anything true of it — there was no second driver for a delta to
travel down before this task and there still is not one after it, and building one is a subsystem of
its own, not a line item a streaming task picks up in passing. Claude Code's own half of the wire is
`--include-partial-messages` (`agents/claude_driver.rs`'s own header carries the CLI flag and the
shape it was verified against), read only for `content_block_delta`/`text_delta` — reasoning and a
tool call's arguments still arrive whole, from the same consolidated `assistant` event this driver
already produced, because streaming either of those was out of this task's scope rather than out of
reach. A resumed session never replays a stray delta either, on two guarantees rather than one:
Claude Code's own persisted transcript holds only the consolidated records this driver always read,
never a raw `stream_event` line, which was checked against the installed CLI rather than assumed —
and `session::history::is_past` refuses the event kind a second time regardless, the deliberate
filter both `model.rs` and `history.rs` carry as the reason not to rest on the first fact alone.

## `AskUserQuestion` answers through the permission channel, and draws its own card

`AskUserQuestion` is a tool like any other on the wire `permission.rs` serves — Claude Code routes
every tool call through `--permission-prompt-tool` alike, this one included — and before smetana-63kn
that meant it got `PermissionRequest.vue` and nothing else: a yellow card reading "AskUserQuestion
needs your permission" with Allow and Deny, no question, no options, because
`agents::claude::tool_detail` has one line to give a tool's whole call and this one needed a form.
Whatever a person pressed answered a call the agent never actually receives an answer to, and the
conversation went on as if it had not been asked.

**There is no second channel, and that is the fix rather than a workaround.** The Agent SDK's own
documented contract serves this tool *inside* the permission flow: the reply is an ordinary allow,
with `updatedInput` carrying `{ questions, answers }` in place of the call's own input echoed back
unchanged. `permission::decision_payload` is the one function that knows the difference — `answers:
None` behaves exactly as it always did, and `answers: Some(map)` is the one branch that builds this
shape instead, echoing `input.questions` back rather than re-deriving it, so a person's answers can
never disagree with the questions that were actually asked.

Three places had to learn to carry a shape richer than a bare `Decision`, and each is a widening
rather than a new vocabulary: `EventKind::Permission` grew an `input` field (`session/model.rs`) —
the tool call's own arguments, untouched, since `detail` remains one line and cannot hold four
questions with their own options; `EventKind::PermissionAnswered` grew `answers: Option<BTreeMap<String,
String>>`, `None` for an ordinary allow/deny and for a decline; and `session_answer` grew a fourth
argument from the front end's own side — the Rust signature counts five, the extra one being the
`State<'_, SessionHandle>` every `#[tauri::command]` takes and no `invoke` call ever sends — optional
and of the same shape, threaded through `Request::Answer` to both the journal and
`PermissionServer::answer`. None of this touches `Decision` itself — `allow`/`allow-always`/`deny`
are still the whole of what a person may answer with, and a decline to answer *is* `Decision::Deny`
with no `answers`, exactly like refusing any other tool.

**`Permission::input` is generic but not universal, and that is a bound rather than an oversight.**
Every asking tool's `Permission` carries the field — a second structured tool needs no second field,
only a second name — but `session::service::question` fills it in for exactly one name today,
`agents::claude::ASK_USER_QUESTION_TOOL`, and writes `Value::Null` for every other tool's event.
That gate exists because the field is not free: this event is appended to a journal that lives for
the life of a session, is cloned whole on every attach and shipped on every `session:events` batch,
and a driven session asks on every `Write`, `Edit`, `MultiEdit` and `Task` — carrying whole file
bodies and whole subagent prompts through it unconditionally would have broken `journal::BUDGET`'s
own promise that each event is small, a promise this file has already had to repair once, for
`TextDelta` (`smetana-6we6`, the section above).

**The front end draws two components off the same `question`, chosen by tool name.**
`ConversationView.vue` computes `isAskUserQuestionCard` from `question.tool` and switches between
`AskUserQuestion.vue` and `PermissionRequest.vue` at the one place either card is drawn — the foot of
the panel, over the composer, exactly where the ordinary card always stood. Every other tool's card
is untouched: `PermissionRequest.vue` still reads `tool`, `detail` and `options` and still draws
Allow/Deny (or Allow/Allow-always/Deny), and nothing about its props or its behaviour moved.
`AskUserQuestion.vue` reads `question.input` instead — the raw arguments, parsed by
`askUserQuestion.js` beside it, pure and tested there for the reason every rule in this family lives
outside its component: a `.vue` file is the one thing no runner in this repository can reach.
`parseQuestions` degrades a malformed field to an empty one rather than throwing, since this is the
one place in the front end reading a tool's own JSON argument rather than a value this app minted.

**A custom answer always wins over a selection, for one question at a time**, per the Design section
of smetana-63kn: a person may always answer in their own words rather than pick from the agent's own
options, and the two are mutually exclusive in the component's own state — choosing an option clears
whatever was typed for that question, and typing clears whatever was chosen. `formatAnswer` in
`askUserQuestion.js` is the one place that resolves the two into the single string the wire wants.

**Which options end up selected is `toggle` in `askUserQuestion.js`, pure and tested there rather than
left inside `AskUserQuestion.vue`** — a `multiSelect` question allows more than one option at once, an
ordinary one allows exactly one, and clicking a chosen option deselects it either way, which is one of
this task's own acceptance criteria and belongs in the one file a test here can reach: an edit that
made single-select accumulate or multi-select replace would ship with both gates green and reach the
agent as one label where four were chosen. `toggle` itself joins nothing — it answers with the array of
whatever now identifies the selected options — and `formatAnswer` is the only join in the file, several
chosen labels with a comma, since a single-select answer is one label and needs none.

**The join is `', '`, a comma and a space, which is a decision rather than the obvious reading of
"joined by commas" in the task's own Design section.** It was kept over a bare `','` because free
text is an accepted answer in this same protocol — a person may always type their own sentence
instead of choosing — so whatever reads `answers` on the far side already has to cope with a string
that is not a machine-parseable list at all, and a comma with nothing after it degrades that reading
to an odd-looking valid answer rather than to an error. Nothing in this repository can test that
reading: the far side is the agent's own model, not code this tree owns, so this is a judgement call
recorded here rather than a behaviour pinned by a test.

**The order of a `multiSelect` answer is click order, not the order the options were offered in** —
`toggle` appends to the end of whatever is already selected, so choosing `Windows` and then `macOS`
sends `"Windows, macOS"` even though the call listed `macOS` first. This was chosen rather than fallen
into: the alternative is sorting the chosen labels back into the options' own order before joining
them, which reads tidier but would silently reorder a person's own emphasis — naming the one they
actually meant first — for no reader on the far side known to care about the difference. `isComplete` gates the card's own
Send button — every question in one call is answered in one reply, never a partial `answers` for a
call that named four, since the agent asked all of them at once and there is nothing to be gained by
making it wait through several short replies for what one round trip already fits.

**The component itself keys a question's selection by option index, never by label**, and `toggle`'s
own parameter is named for that: not `label`, but the identity-agnostic `id`, since the function only
ever compares it for equality and is handed a label in `askUserQuestion.js`'s own tests and an index
by the one real caller. `parseQuestions` defaults a missing `label` to `''`, so two options that both
lost theirs would otherwise be one value as far as `toggle` and the `v-for`'s own `:key` are
concerned — an observation made while fixing this, over a hand-built fixture rather than one in the
tree, and there is nothing under `Gallery.vue` today that shows it: that showcase's one
`AskUserQuestion` fixture has well-formed labels throughout, on purpose, since it is meant to read as
an ordinary call rather than as a test of malformed input. `askUserQuestion.js`'s own tests are what
actually pin the degenerate case now. `selectedLabels(questions, selectedByIndex)`, exported from
that same file rather than left as a `.vue` method, is the one place index and label meet — mapping
the chosen indices back to `question.options[i].label`, `''` for an index past the end of `options`
too — right before `buildAnswers` and `isComplete` are called, since those take the wire's own
vocabulary and index is this component's alone. `setCustom`'s own mutual exclusion — typing clears a
selection — stays in the component, being short enough that moving it out would cost more than it
saves; it is still a rule, and the file's own header says so rather than claiming the component holds
none.

**Drawn at the same `loud` weight as `PermissionRequest.vue`, deliberately**: the harness is holding
the very same tool call open either way, so `AskUserQuestion.vue` reads `statusColors('needs-you')`
and `STATUS_GLYPH` off the identical pair rather than choosing a softer treatment — the two cards read
as one vocabulary for "the session cannot go on without you", and only the shape inside the frame
says which tool is asking. The frame itself — the fill, the border, the outer padding and radius,
the head row's icon and gap — is written out twice, once per component, and kept in step by hand
rather than shared: that duplication is deliberate for now rather than an oversight to fold away, and
nothing fails if only one of the two moves, which is worth knowing before assuming a shared frame
already exists.

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
kept so a reader can see what was compared, not a feature this app ships. Nothing in `src/` sets
`data-turn-style` — the shipped stylesheet names it once, in a comment pointing at the exploration
file, and needs no `[data-turn-style="bubble-right"]` selector because the bubble is simply the
default. Vendoring the exploration file into the app bundle to keep the other three treatments
"available" was never on the table: a treatment nobody can reach from a setting is a dead branch with
a maintenance cost, not a feature.
