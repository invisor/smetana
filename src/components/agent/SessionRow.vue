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
   session. The rule behind every string on it lives in `sessionRow.js`, beside
   this file and outside it, because a `.vue` file is the one thing no test in
   this repository can reach.

   `now` is a prop rather than a clock of this component's own. The label under
   the title is relative — `18h ago` — and a relative label computed once turns
   into a lie the moment the app is left open overnight; the store ticks a
   single `now` for the whole list, exactly as `terminals.js` does for the
   agents' elapsed times, so one interval serves the column and every row of it
   moves at the same moment. Passing it in also keeps the gallery honest: a
   fixed `now` there draws fixed labels. */
import { computed } from 'vue'
import { useInteractive } from '../core/interactive.js'
import { lastMessageLine, sessionMeta, sessionTitle } from './sessionRow.js'

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
  separated: { type: Boolean, default: false }
})

/* Hover only, and no press: a row is a place rather than a button, the same
   reading `AgentList` and `ClaimedTasks` take of their own rows. It is a
   surface step up and nothing else — never a colour, never a transform — so a
   pointer crossing a dense column cannot make it move. */
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

/* The title is prose — the person's own first sentence — so it is sans, and one
   line with an ellipsis: a conversation opened with a paragraph as readily as
   with a question, and a list whose rows are each as tall as their opening
   remark cannot be scanned. */
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

const title = computed(() => sessionTitle(props.session))
const last = computed(() => lastMessageLine(props.session))
const meta = computed(() => sessionMeta(props.session, props.now))
</script>

<template>
  <!-- The working directory the session ran in, which is the one fact about it
       the three lines do not carry: a session out of a worktree and one out of
       the project root are both this project's, and only this tells them
       apart. A hover string rather than a fourth line, since it is a question
       somebody asks about one row rather than something they scan for. -->
  <div :style="rowStyle" v-bind="handlers" :title="session.cwd">
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
         the row — the box must never be a direct child of the column in
         `rowStyle`. The result: a long last message stands exactly two lines
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
</template>
