<script setup>
/* One Claude Code session, read off disk: the right column's Sessions tab.

   Deliberately not `AgentList`, and the two must not be merged later either.
   That list is the live agents of this run of the app — a state, a timer, a
   remove button and the work a run has claimed — and it is drawn in the left
   column, where it stays. This is a conversation that is over: a title, what
   was last said in it, how long it was and when. The only thing the two have in
   common is that both are lists of rows, which is not enough to share a
   component over.

   A row rather than a list, unlike `AgentList`: there is nothing here to keep
   between rows — no selection, no per-row hover cache keyed by id — so the
   `v-for` belongs to whoever is drawing the column and this file draws one
   session. The rule behind every string on it lives in `sessionRow.js` and
   `sessionMenu.js`, beside this file and outside it, because a `.vue` file is
   the one thing no test in this repository can reach.

   `now` is a prop rather than a clock of this component's own. The label under
   the title is relative — `18h ago` — and a relative label computed once turns
   into a lie the moment the app is left open overnight; the store ticks a
   single `now` for the whole list, exactly as `terminals.js` does for the
   agents' elapsed times, so one interval serves the column and every row of it
   moves at the same moment. Passing it in also keeps the gallery honest: a
   fixed `now` there draws fixed labels.

   The card opens, and the menu on it acts on the transcript file — but this
   component neither opens anything nor deletes anything. It raises `toggle` and
   `action`, exactly as `TaskCard` raises `action` over `taskMenu.js`'s kinds,
   because the stores live in the view: one file under `src/components/` imports
   a store and it is `TerminalView`, and this is not going to be the second. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import MenuButton from '../overlays/MenuButton.vue'
import { useInteractive } from '../core/interactive.js'
import {
  FIRST_PROMPT_HEADING,
  NO_FIRST_PROMPT,
  firstPrompt,
  lastMessageLine,
  sessionDetails,
  sessionMeta,
  sessionTitle
} from './sessionRow.js'
import {
  SESSION_MENU_W,
  menuButtonIcon,
  menuButtonLabel,
  sessionMenuItems
} from './sessionMenu.js'

const props = defineProps({
  session: { type: Object, required: true },
  /* Milliseconds, the store's ticking clock. Its own default is what keeps a
     row drawable in isolation — a gallery entry, a future one-off — without
     the label going blank. */
  now: { type: Number, default: () => Date.now() },
  /* Whether this row carries the rule that separates it from the one above.

     The container decides, and it has to: an inline-style component has no
     `:last-child` to suppress anything with, so a rule drawn unconditionally is
     drawn under the last row too — which in the app left a full-width
     separator with four hundred pixels of empty panel under it, separating
     nothing, and in the gallery landed on the frame's own border as a two-pixel
     band of two different colours.

     Drawn **above** rather than below for the same reason, and it is the whole
     of the fix: a rule above every row but the first cannot end a list, whereas
     a rule below every row but the last needs the container to know where the
     end is — one more thing for two callers to agree about, and they would come
     to disagree. Off by default, so a row drawn alone anywhere is a row with no
     rule on it. */
  separated: { type: Boolean, default: false },
  /* Whether this card is open, and the container's business rather than this
     component's.

     Held outside for two reasons that point the same way. Several cards may be
     open at once — that is the design, since comparing two sessions is what
     somebody opens a second one for — so the state is a set and a set belongs
     to whoever holds the list. And a gallery cannot show an opened card at all
     if the flag is a `ref` in here, which would leave the one verification this
     project has unable to see half of this component.

     Not remembered anywhere. Opening a card is a gesture inside one look at a
     list, not a preference worth surviving a restart, and `settings.json` is
     where the things that do survive live. */
  expanded: { type: Boolean, default: false },
  /* True while something destructive is in flight against this session, which
     greys every row of the menu. One flag rather than a verb, because the only
     thing that can be in flight is the delete: everything else here either
     lands on the clipboard at once or is handed to the desktop and forgotten.

     `busy` and not `deleting` for the reason `taskMenu.js`'s is called `busy`:
     what the menu does with it is freeze, and a name for the cause would have
     to be renamed the day a second cause arrives. */
  busy: { type: Boolean, default: false },
  /* What the trigger says right now, and it is the confirmation that a copy
     happened: `''`, `'copied'` or `'failed'`, the three-value vocabulary
     `kanban/copyId.js` set for a task's id. See `menuButtonLabel` for why the
     answer lands on this button rather than in a toast or on the menu row that
     was pressed — the menu closes on the way out, and this button is what is
     left of it.

     A prop and not state of this component's, for the reason `TaskCard`'s
     `copyState` is a prop: this component knows nothing about a clipboard, so
     it cannot know whether anything reached one. */
  copyState: { type: String, default: '' },
  /* And which of the three copying verbs it was, so the sentence can name it.
     Two props rather than one object: whoever draws this holds two refs, and a
     composite would be rebuilt on every unrelated render of the list. */
  copyNoun: { type: String, default: '' }
})

const emit = defineEmits(['toggle', 'action'])

/* Hover only, and no press: the surface step up is what says the row can be
   interacted with, and it is never a colour and never a transform, so a pointer
   crossing a dense column cannot make anything move. That reading has not
   changed now that the row opens — `AgentList` and `ClaimedTasks` take it of
   their own rows, and a card that grew a pressed state would be the only one
   here that jumps. */
const { hover, handlers } = useInteractive()

/* Three lines with room between them, so the row is padded rather than held at
   `--row-h`: that token is the height of a single-line row and this is not one.
   The rule separates a session from the one above it; without it two wrapped
   last-messages read as one paragraph. See `separated` for why it is above and
   why it is the container's decision. */
const rowStyle = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4) var(--space-5)',
  borderTop: props.separated ? 'var(--border-w) solid var(--border-subtle)' : undefined,
  background: hover.value ? 'var(--surface-hover)' : 'transparent',
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

/* The three lines and the two controls, side by side. The controls are
   `flex: none` and the lines take the rest: without that the title's ellipsis
   would never fire, since a flex item at its automatic minimum refuses to
   shrink below its content and it is the buttons that would be pushed off
   instead. */
const headStyle = { display: 'flex', alignItems: 'flex-start', gap: 'var(--space-4)' }
const linesStyle = {
  flex: 1,
  minWidth: 0,
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)'
}
const controlsStyle = { flex: 'none', display: 'flex', alignItems: 'center', gap: 'var(--space-1)' }

/* The title is prose — the person's own first sentence — so it is sans, and one
   line with an ellipsis: a conversation opened with a paragraph as readily as
   with a question, and a list whose rows are each as tall as their opening
   remark cannot be scanned.

   It keeps that one line when the card is open, which is deliberate rather than
   an omission. The opened card carries the whole of the first prompt in a block
   of its own below, so letting this line wrap as well would be the same words
   twice; and holding it at one line is what stops the row's top edge — and
   every row above it — from moving when somebody opens a card halfway down the
   column. */
const titleStyle = {
  font: 'var(--weight-medium) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

/* The last thing said, over about two lines. The clamp is the only way to hold
   a box at a number of lines rather than a height, and a height in tokens would
   be wrong in one of the two densities anyway — `2` here is a count of lines,
   not a measurement, of a piece with the unitless `flex` and `opacity` values
   elsewhere in this system.

   This box must never be a flex item, or the clamp is blockified away. The
   wrapper that keeps it from being one, and the whole of why, are in the
   template beside it. */
const lastStyle = {
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)',
  display: '-webkit-box',
  WebkitBoxOrient: 'vertical',
  WebkitLineClamp: 2,
  overflow: 'hidden'
}

/* The meta line wraps rather than ellipsising: every piece of it is short and
   whole, and a branch name cut in half says less than one on a second line. The
   gap is what puts air around the separators, so nothing here is a space
   character somebody has to keep. */
const metaStyle = {
  display: 'flex',
  flexWrap: 'wrap',
  alignItems: 'baseline',
  gap: 'var(--space-2)',
  color: 'var(--text-muted)'
}
const sansPart = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)' }
/* Identifiers — the model id, the branch — in mono, the prose about them in
   sans. The project's rule, and the reason the pieces arrive tagged rather
   than joined into one string. */
const monoPart = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)' }

/* One piece of the meta line **with the separator that precedes it inside the
   same box**, which is what stops a middot being left at the end of a wrapped
   line with nothing after it. The wrapping happens between these boxes and
   never inside one, so a separator always arrives with the piece it announces.
   `sessionRow.js` decides which pieces have one; this only draws it.

   A flex box rather than a nowrap inline one: the two children are laid out
   with the same gap the line uses, and a branch name too long for the column
   still wraps inside its own piece instead of pushing the line sideways. */
const partBox = { display: 'flex', alignItems: 'baseline', gap: 'var(--space-2)', minWidth: 0 }
/* How the separator is set, which is the only part of it that is this file's:
   the glyph, the precedents for it and the rule that it travels with the piece
   after it are `META_SEPARATOR`'s, a file over. Sans and muted like the counts
   beside it, deliberately not a mark of its own — the gap on either side is
   what makes it read as punctuation. */
const dotStyle = sansPart

/* ---- the opened card ----------------------------------------------------- */

/* Set off from the three lines by the space above it and by the framed block
   inside it, never by a second background: a card inside a row inside a panel
   would be three surfaces deep, and this system's depth is carried by borders.
   The top rule is what makes it read as a section of this row rather than as a
   fourth line of it. */
const bodyStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  marginTop: 'var(--space-2)',
  paddingTop: 'var(--space-4)',
  borderTop: 'var(--border-w) solid var(--border-subtle)'
}

/* The caption over the first prompt. The small-caps idiom every other caption
   in this system uses — `ContextMenu`'s labels, the inspector's field headings
   — with the glyph in front of it doing what the glyph on Orca's own block
   does: saying that what follows is a message rather than a field. */
const captionStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  color: 'var(--text-muted)',
  fontSize: 'var(--text-2xs)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase'
}

/* The prompt itself, framed. It wraps to whatever it takes: the card was opened
   on purpose, so there is nothing to save by clipping it, and the worker has
   already cut the text to 240 characters on the way over (`model.rs`'s `CLIP`).

   `pre-wrap` and not `pre`: the text arrives with its whitespace already
   collapsed, so there is nothing to preserve, and `pre` would refuse to wrap a
   long path at all and push the panel sideways. What it does buy is honesty
   about the one thing that is left — a run of spaces somebody typed. */
const promptStyle = {
  padding: 'var(--space-4)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  background: 'var(--surface-sunken)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)',
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere'
}

/* The two paths under it: a sans label and a mono value, one under the other
   rather than side by side. Side by side is what a 240px-minimum panel cannot
   do with an absolute path — the value would take a two-character column — and
   `overflowWrap` is what lets a path break where it has to instead of widening
   the panel. */
const detailsStyle = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }
const detailLabelStyle = {
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-sans)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
const detailValueStyle = {
  marginTop: 'var(--space-1)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-secondary)',
  overflowWrap: 'anywhere'
}

const title = computed(() => sessionTitle(props.session))
const last = computed(() => lastMessageLine(props.session))
const meta = computed(() => sessionMeta(props.session, props.now))
const prompt = computed(() => firstPrompt(props.session))
const details = computed(() => sessionDetails(props.session))
const items = computed(() =>
  sessionMenuItems({ busy: props.busy, userAgent: navigator.userAgent })
)

/* `navigator.userAgent` is read here rather than in `sessionMenu.js`, which is
   the one thing in that file that would have made it impure. It is the same
   split `FileTree.vue` makes over `fileMenuItems`, and the rule about what the
   platform calls its file manager stays testable because the reading is an
   argument. */

const triggerIcon = computed(() => menuButtonIcon(props.copyState))
const triggerLabel = computed(() => menuButtonLabel(props.copyState, props.copyNoun))

/* The whole row toggles, and the chevron is what says so. Both, rather than one
   of the two: the chevron is the affordance somebody looks for, and a card
   whose body did nothing when clicked would be the only expandable thing in
   this app that has to be hit on a 20px target.

   Nothing else on the row is a target, which is why the controls stop the
   click: a menu press that also toggled the card would move everything under
   the pointer at the moment the panel opened. */
const toggle = () => emit('toggle', props.session.id)
</script>

<template>
  <!-- The working directory the session ran in, which is the one fact about it
       the three lines do not carry: a session out of a worktree and one out of
       the project root are both this project's, and only this tells them
       apart. A hover string rather than a fourth line, since it is a question
       somebody asks about one row rather than something they scan for — and it
       stays now that the opened card names it in full, because a hover string
       is for a row somebody is pointing at rather than one they have opened. -->
  <div :style="rowStyle" v-bind="handlers" :title="session.cwd">
    <div :style="headStyle">
      <div :style="linesStyle" @click="toggle">
        <div :style="titleStyle">{{ title }}</div>
        <!-- The wrapper the clamped box hangs in, and it is load-bearing rather
             than a spare div. It carries no styles at all, deliberately: what it is
             for is being the flex item, so that the box inside it is not one.

             `display: -webkit-box` is blockified when the box is a flex item, and a
             blockified box is no longer a `-webkit-box`. Chromium honours
             `-webkit-line-clamp` on one anyway, so a dev browser draws two lines
             and hides the defect completely; WebKit has historically applied the
             clamp only to a real `-webkit-box`, and this app ships in WKWebView,
             WebKitGTK and WebView2 with `safari15` as the build target. Where the
             clamp is dropped nothing is clipped either — the height is auto, so
             `overflow: hidden` has nothing to cut — and the row grows to whatever
             the last message was, on every screen except the one anybody here can
             check.

             Do not simplify the wrapper away, and do not try to check it by reading
             the computed `display`. That check was written here first and it does
             not work: Chromium reports `flow-root` for any element carrying
             `-webkit-line-clamp` at all, flex item or not, and it does not report
             flex blockification in computed style either. So the reading is
             `flow-root` while the fix is in place, and somebody trusting it would
             take a correct row for a regression and put the box back where it
             started.

             What can be checked is the shape and the result. The shape: the clamped
             element's parent is this wrapper, and the wrapper is the flex child of
             the lines column — the box must never be a direct child of that
             column. The result: a long last message stands exactly two lines
             tall and ends in a real ellipsis, measured at 33.0px against a 16.5px
             line — the number for the shipped size, not a constant to assert. -->
        <div v-if="last">
          <div :style="lastStyle">{{ last }}</div>
        </div>
        <div :style="metaStyle">
          <div v-for="(part, index) in meta" :key="index" :style="partBox">
            <span v-if="part.lead" :style="dotStyle">{{ part.lead }}</span>
            <span :style="part.mono ? monoPart : sansPart">{{ part.text }}</span>
          </div>
        </div>
      </div>

      <!-- The two controls, and `click.stop` on the box rather than on each of
           them: the menu's panel is teleported to the body and is therefore not
           inside this element at all, so what is being stopped here is the
           trigger's own press and the chevron's, which is exactly the pair that
           must not also toggle the card. -->
      <div :style="controlsStyle" @click.stop>
        <IconButton
          :icon="expanded ? 'chevron-down' : 'chevron-right'"
          :label="expanded ? 'Collapse this session' : 'Open this session'"
          size="sm"
          :aria-expanded="expanded"
          @click="toggle"
        />
        <MenuButton
          :items="items"
          :icon="triggerIcon"
          :label="triggerLabel"
          size="sm"
          :width="SESSION_MENU_W"
          @select="(item) => emit('action', { kind: item.kind, session })"
        />
      </div>
    </div>

    <!-- What the card holds once it is open: the whole of the first prompt, and
         the two paths. Nothing else — an opened card is not a transcript
         viewer, and reading the conversation is what Open log is for. -->
    <div v-if="expanded" :style="bodyStyle">
      <div>
        <div :style="captionStyle">
          <Icon name="message-square" :size="12" />
          <span>{{ FIRST_PROMPT_HEADING }}</span>
        </div>
        <!-- The sentence for a transcript with no human message in it, in the
             same frame rather than instead of it: an empty box under a caption
             reads as a block that failed to draw. It is set in the same voice
             as the prompt would have been, since it is standing in for one. -->
        <div :style="{ ...promptStyle, marginTop: 'var(--space-2)' }">
          {{ prompt ?? NO_FIRST_PROMPT }}
        </div>
      </div>
      <div :style="detailsStyle">
        <div v-for="detail in details" :key="detail.label">
          <div :style="detailLabelStyle">{{ detail.label }}</div>
          <div :style="detailValueStyle">{{ detail.value }}</div>
        </div>
      </div>
    </div>
  </div>
</template>
