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
  now: { type: Number, default: () => Date.now() }
})

/* Hover only, and no press: a row is a place rather than a button, the same
   reading `AgentList` and `ClaimedTasks` take of their own rows. It is a
   surface step up and nothing else — never a colour, never a transform — so a
   pointer crossing a dense column cannot make it move. */
const { hover, handlers } = useInteractive()

/* Three lines with room between them, so the row is padded rather than held at
   `--row-h`: that token is the height of a single-line row and this is not one.
   The rule underneath separates a session from the next; without it two
   wrapped last-messages read as one paragraph. */
const rowStyle = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4) var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
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
   elsewhere in this system. `-webkit-line-clamp` is in every one of the three
   webviews this app is built for. */
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
/* The separator is the one every other list in this app uses for the same job
   (`shell/projectState.js`, `settings/usage.js`). It is a piece of the same
   muted line rather than a mark of its own: the gap around it is what makes it
   read as punctuation. */
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
    <div v-if="last" :style="lastStyle">{{ last }}</div>
    <div :style="metaStyle">
      <template v-for="(part, index) in meta" :key="index">
        <span v-if="index" :style="dotStyle">·</span>
        <span :style="part.mono ? monoPart : sansPart">{{ part.text }}</span>
      </template>
    </div>
  </div>
</template>
