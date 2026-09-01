<script setup>
import { computed, nextTick, onUnmounted, ref, useId, watch } from 'vue'
import Icon from '../core/Icon.vue'
import StatusDot from '../status/StatusDot.vue'
import {
  counterLabel,
  filterIssues,
  relationOf,
  sectionLabel,
  stepIndex,
  waitingLabel
} from './commandPalette.js'

/* Finding a task, from the middle of the window rather than from a corner of the
   bar. The panel is wide enough for a whole title, a relation and a status, and
   that width is the whole reason it is a modal: everything the old field got
   wrong — the saturated focus ring, the dropdown that did not line up with it,
   forty-pixel rows, a heading arguing with an empty state — was a fault of the
   container rather than of the search.

   Two questions, one at a time. Typing filters the snapshot the store already
   holds, by id and title and nothing else. `⌘⏎` asks an agent the same question
   by meaning, and that call has a ninety-second deadline — which is why the
   heading and the rows here follow `answered` and never the mode. */
const props = defineProps({
  open: { type: Boolean, default: false },
  /* Every task in the project, merge lock already excluded and statuses already
     in this system's vocabulary — both are the caller's, because both rules live
     in a store that imports Tauri. */
  issues: { type: Array, default: () => [] },
  /* The store's own dependency maps, `{blockedBy, blocking}`. Read rather than
     the issue's `dependencies` and `dependent_count`, because these carry the
     rule those do not: a blocker that is closed no longer blocks. */
  edges: { type: Object, default: () => ({ blockedBy: new Map(), blocking: new Map() }) },
  /* Ids of the last few tasks somebody opened, newest first — what an empty
     query has to show, since a palette with nothing in it says nothing. */
  recent: { type: Array, default: () => [] },
  /* The semantic question is out. Drawn twice: the spinner where the counter
     is, and the waiting row where the heading stands. The rows themselves must
     not move until there is an answer to move them to. */
  pending: { type: Boolean, default: false },
  /* Why it was refused, already a sentence — `OneshotError` names six ways and
     every one of them is something a person can act on. */
  error: { type: String, default: '' },
  /* What the agent answered, as ids. Rows are drawn from `issues` rather than
     from anything the agent said, so an id it invented cannot reach the screen. */
  semanticIds: { type: Array, default: () => [] },
  /* Whether an answer has come back at all. `semanticIds` being empty is two
     different facts — nothing asked, and asked with nothing named — and they are
     two different screens. */
  answered: { type: Boolean, default: false }
})

const emit = defineEmits(['close', 'select', 'semantic', 'reset'])

/* Unique per instance, because `aria-activedescendant` points at a row by id and
   the gallery draws more than one palette on one page.

   `useId` and not a counter of this file's own: the body of `<script setup>` is
   the setup function, so it runs once per instance and a `let` beside it is
   re-initialised every time — a counter written there hands out 1 to everybody.
   The gallery caught this exactly as predicted: five palettes, five comboboxes,
   one id between them, and four of them pointing `aria-activedescendant` into
   another dialog's rows. */
const uid = `sm-palette-${useId()}`
const listId = `${uid}-list`
const rowId = (index) => `${uid}-row-${index}`

const query = ref('')
/* What is being *asked*, which is not the same as what is being *shown*: the
   heading and the rows follow `answered` below. */
const mode = ref('text')
const sel = ref(0)
const field = ref(null)
const scroller = ref(null)

const byId = computed(() => new Map(props.issues.map((issue) => [issue.id, issue])))

const row = (issue) => ({ id: issue.id, title: issue.title, status: issue.status })

/* An id resolved against the board, or nothing. Both the recents and the agent's
   answer go through it: a task that has since been deleted, and an id the agent
   made up, are the same problem seen from two sides, and neither belongs on
   screen. */
const resolve = (ids) => ids.map((id) => byId.value.get(id)).filter(Boolean).map(row)

/* The relation is worked out once per row here rather than three times in the
   template — the icon, the label and the decision whether to draw it at all are
   one answer, and asking for it three times is three chances to draw two thirds
   of one. */
const withRelation = (list) =>
  list.map((hit) => {
    const issue = byId.value.get(hit.id)
    return { ...hit, relation: issue ? relationOf(issue, props.edges) : null }
  })

/* Nothing typed: the last few tasks somebody looked at. Otherwise the agent's
   answer once it has actually landed, and the text matches until then.

   The middle branch keys off `answered` rather than off `mode`, and that is the
   one decision this component is built around: between `⌘⏎` and the answer the
   rows below are still text matches, and a list following the mode would relabel
   them for a minute and a half. */
const rows = computed(() => {
  if (!query.value.trim()) return withRelation(resolve(props.recent))
  if (props.answered) return withRelation(resolve(props.semanticIds))
  return withRelation(filterIssues(props.issues, query.value))
})

const heading = computed(() => sectionLabel({ query: query.value, answered: props.answered }))

/* How long the question has been out, in whole seconds, and the tick that moves
   it. A timer is DOM rather than a rule, which is why only the number lives
   here and the sentence around it is `waitingLabel`.

   Measured against the moment the question went out rather than counted up one
   per tick: an interval is not a clock — a busy main thread, or a laptop shut
   halfway through a ninety-second wait, drops ticks — and a counter that lost
   them would understate exactly the wait somebody is watching it for.

   The clock runs only while the panel is both waiting and open. Closing the
   palette does not withdraw the question, so `pending` can stand for another
   minute over nothing anybody can see, and a second of that is a second of an
   interval nobody is reading. Reopening picks the true elapsed time back up,
   since `askedAt` survives the close and only a fresh question moves it. */
const waited = ref(0)
let askedAt = 0
let ticker = null

const tick = () => {
  waited.value = Math.floor((Date.now() - askedAt) / 1000)
}

const stopTicking = () => {
  if (ticker === null) return
  clearInterval(ticker)
  ticker = null
}

/* `immediate`, because the gallery mounts this component with `pending` already
   true: a clock started only on the transition would sit at nought there for
   good, in the one place any of this is ever looked at. */
watch(
  [() => props.pending, () => props.open],
  ([pending, open], was) => {
    stopTicking()
    if (!pending) return
    if (!was?.[0]) askedAt = Date.now()
    if (!open) return
    tick()
    ticker = setInterval(tick, 1000)
  },
  { immediate: true }
)

/* The component outlives the panel — `open` is a prop and the dialog is a
   `v-if` inside it — so unmounting is the last of the three ways this stops,
   not the usual one. */
onUnmounted(stopTicking)

const waitLabel = computed(() => waitingLabel(waited.value))

/* Silent while nothing is typed: the rows under an empty query are the recents,
   and counting those against the whole project answers a question nobody asked. */
const counter = computed(() =>
  query.value.trim() ? counterLabel(rows.value.length, props.issues.length) : ''
)

/* What a screen reader is told, and it is a separate thing from what is drawn
   for the same reason the counter is silent under an empty query: the eye reads
   the heading and the count as two parts of one panel it can see at once, and
   speech gets one sentence or nothing.

   It is rendered unconditionally, which is the whole point. A live region
   inserted into the document together with its text is announced by no major
   screen reader — the region has to be there first, and only the text may
   change. Both halves of this were mounted per state before: the counter was
   the `v-else` of the spinner, and the heading lived inside the list's own
   `v-if`, so between them they announced nothing at all. */
const announcement = computed(() => {
  /* The seconds are deliberately left out of it, and this is the one place the
     spoken text and the drawn one part company on purpose: a value changing
     once a second inside `aria-live` is an announcement once a second, which is
     a screen reader talking over itself for the whole of the wait. The eye
     reads the count; speech gets the fact, once. */
  if (props.pending) return 'Asking the agent'
  if (!rows.value.length) return query.value.trim() ? 'Nothing matched' : ''
  return [heading.value, counter.value].filter(Boolean).join(', ')
})

/* The empty state and the heading are never on screen together — one block
   saying one thing is the whole point of the redraw. The error takes this same
   slot rather than adding a third.

   A question still in flight silences it too, and that is a correctness fix
   rather than tidiness: "Nothing matched" is an answer, and the agent has not
   given one yet. The error cannot collide with the wait — `searchSemantic`
   clears it before it sends. */
const isEmpty = computed(
  () => !props.pending && !props.error && !!query.value.trim() && rows.value.length === 0
)

/* Two different sentences, because they are two different facts. Reusing the
   text one for a meaning answer would state something about substrings nobody
   checked. */
const emptyHint = computed(() =>
  props.answered
    ? 'The agent looked and named no task.'
    : 'No task title or id contains this text.'
)

const offerMeaning = computed(() => !!query.value.trim())

const meaningLabel = computed(() =>
  mode.value === 'meaning' ? 'Back to text search' : 'Search by meaning'
)

/* Keeps the selected row visible without moving the page: the palette is a modal
   over a scrolling app, and `scrollIntoView` on its own would take the board
   underneath along with it. */
const reveal = () => {
  nextTick(() => {
    const options = scroller.value?.querySelectorAll('[role="option"]') ?? []
    options[sel.value]?.scrollIntoView({ block: 'nearest' })
  })
}

const step = (by) => {
  sel.value = stepIndex(sel.value, by, rows.value.length)
  reveal()
}

/* A list that changed under the selection — an answer landing, a task
   disappearing — takes it back to the top rather than leaving it past the end,
   where ⏎ would open nothing at all. */
watch(
  () => rows.value.length,
  (length) => {
    if (sel.value >= length) sel.value = 0
  }
)

const choose = () => {
  const hit = rows.value[sel.value]
  if (!hit) return
  emit('select', hit.id)
  emit('close')
}

const openRow = (id) => {
  emit('select', id)
  emit('close')
}

/* On turns the question into the agent's; off withdraws it, which is what makes
   an answer already in flight stop mattering — `clearSemantic` nulls the query
   the store guards on. */
const toggleMode = () => {
  sel.value = 0
  if (mode.value === 'meaning') {
    mode.value = 'text'
    emit('reset')
    return
  }
  if (!query.value.trim()) return
  mode.value = 'meaning'
  emit('semantic', query.value.trim())
}

/* Typing is always a text question. It withdraws whatever the agent said, since
   leaving those ids under a different query would attribute them to it. */
watch(query, () => {
  mode.value = 'text'
  sel.value = 0
  emit('reset')
})

/* `⇧⏎` is deliberately absent: it means "open in a new tab" in every palette
   that has tabs, and this app's tabs are agents, terminals and files — a task
   goes to the right column instead. A key with nowhere to aim is left unbound
   rather than pointed at the nearest thing. */
const onKeydown = (event) => {
  if (event.key === 'Escape') return emit('close')
  if (event.key === 'ArrowDown') return (event.preventDefault(), step(1))
  if (event.key === 'ArrowUp') return (event.preventDefault(), step(-1))
  if (event.key !== 'Enter') return
  if (event.metaKey || event.ctrlKey) return (event.preventDefault(), toggleMode())
  if (event.shiftKey) return
  event.preventDefault()
  choose()
}

/* The caret lands at the end of whatever was typed last time, because the query
   survives a close: somebody reopening after a glance at the board is usually
   carrying on rather than starting again.

   It watches the transition and is deliberately not `immediate`. A palette that
   grabbed the keyboard on mount would, in the gallery, scroll the page to itself
   the moment it loaded — and the gallery is the only verification this project
   has of every other component on that page. */
watch(
  () => props.open,
  (open) => {
    if (!open) return
    sel.value = 0
    nextTick(() => {
      const el = field.value
      if (!el) return
      el.focus()
      el.setSelectionRange(el.value.length, el.value.length)
    })
  }
)

const scrimStyle = {
  position: 'absolute',
  inset: 0,
  zIndex: 'var(--z-modal)',
  background: 'var(--overlay-scrim)',
  display: 'flex',
  alignItems: 'flex-start',
  justifyContent: 'center',
  /* `Modal.vue`'s own offset rather than a number of this component's: two
     overlays standing at two different heights would read as two systems. */
  paddingTop: '8vh'
}

const panelStyle = {
  display: 'flex',
  flexDirection: 'column',
  width: '620px',
  maxWidth: 'calc(100% - var(--space-6) * 2)',
  overflow: 'hidden',
  background: 'var(--surface-overlay)',
  color: 'var(--text-primary)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-4)',
  boxShadow: 'var(--shadow-modal)',
  font: 'var(--weight-regular) var(--text-body-size)/var(--leading-normal) var(--font-sans)'
}

const inputRowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  padding: 'var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}

/* **No focus ring, and that is the point.** In this system colour means state,
   and a saturated border around the one field on screen was the first of the six
   faults this redraw exists to fix. The panel's own border is the affordance. */
const inputStyle = {
  flex: 1,
  minWidth: 0,
  border: 'none',
  outline: 'none',
  background: 'transparent',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-lg)/var(--leading-snug) var(--font-mono)'
}

/* The one thing on the panel wearing the action surface: it is the mode itself,
   sitting where the mode is chosen, and its `x` is the way back out. */
const chipStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  flex: '0 0 auto',
  padding: 'var(--space-1) var(--space-2)',
  border: 'none',
  borderRadius: 'var(--radius-2)',
  background: 'var(--action-primary-bg)',
  color: 'var(--text-inverse)',
  cursor: 'pointer',
  font: 'var(--weight-regular) var(--text-xs)/1.4 var(--font-mono)'
}

const counterStyle = {
  flex: '0 0 auto',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}

/* The same spinner every waiting control in this system draws, and it is worn
   twice over: once here at the end of the field, and once in the waiting row
   below, which is the one a person actually looks at after `⌘⏎`. */
const spinStyle = {
  flex: '0 0 auto',
  color: 'var(--attn-live)',
  animation: 'sm-spin var(--dur-pulse) linear infinite'
}

/* Off screen, in the one way that leaves an element in the accessibility tree:
   a size of one pixel and clipped, rather than hidden or `display: none`. The
   values are lengths and not spacing, which is why there is no token for them
   and none is wanted. */
const liveStyle = {
  position: 'absolute',
  width: '1px',
  height: '1px',
  overflow: 'hidden',
  clipPath: 'inset(50%)',
  whiteSpace: 'nowrap'
}

const scrollStyle = {
  display: 'flex',
  flexDirection: 'column',
  maxHeight: '320px',
  overflowY: 'auto'
}

/* Outside the scroll area rather than sticky inside it, and that is a fix
   rather than a simplification: an opaque heading stuck to the top of the
   scrollport is exactly where `scrollIntoView({block: 'nearest'})` puts the row
   it is revealing, so arrowing up through a long list slid the selected row
   under the heading — 24px of a 28px row covered, and `⏎` opening a task nobody
   could see. There is only ever one heading here, so nothing needs to stick to
   anything. */
const headStyle = {
  flex: 'none',
  padding: 'var(--space-2) var(--space-5)',
  background: 'var(--surface)',
  color: 'var(--text-muted)',
  textTransform: 'uppercase',
  letterSpacing: 'var(--tracking-caps)',
  font: 'var(--weight-regular) var(--text-2xs)/1.6 var(--font-mono)'
}

/* The wait, in the heading's own style and in the heading's own place.

   Thirteen pixels of spinner at the end of the field was the whole of the
   signal before, and it is not where anybody looks after pressing `⌘⏎` — they
   look at the list. So the wait is said where the list is labelled, at the same
   height and in the same type, and nothing below it moves: the text matches
   stand until an answer lands, and `⏎` opens one without waiting for the agent
   at all. The spinner alone would say no more than the corner did; the seconds
   beside it are what tell a long answer from a hung one, against a deadline of
   ninety of them. */
const waitStyle = {
  ...headStyle,
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  /* The two properties the heading is not followed in, and they are the same
     distinction `Panel.vue`'s `railTitleStyle` draws: caps and letter-spacing
     mark the *name* of the group below — `RECENT`, `MATCHING TEXT` — and a name
     is not what this row is. It is a sentence with an ellipsis and a numeral
     that changes once a second, and shouted it reads `12S`, where the unit
     turns into a letter. Everything else the heading gives is kept, because the
     row has to stand at the heading's height and in its place. */
  textTransform: 'none',
  letterSpacing: 'var(--tracking-normal)'
}

/* The row is `--row-h` rather than the handoff's 30 and 24: the token already
   carries both densities and the app-wide font factor, and a number here would
   be a third opinion about how tall a row is. A `done` row is dimmed by the same
   token the done column uses. No shadow and no transform — a list this dense
   cannot have rows that jump. */
const rowStyle = (on, status) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-5)',
  flex: 'none',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  cursor: 'pointer',
  background: on ? 'var(--surface-selected)' : 'transparent',
  opacity: status === 'done' ? 'var(--attn-quiet-opacity)' : 1
})

const idStyle = {
  flex: '0 0 152px',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)'
}

const titleStyle = {
  flex: 1,
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  font: 'var(--weight-regular) var(--text-md)/1 var(--font-sans)'
}

const relationStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  flex: '0 0 auto',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}

/* A quiet name rather than a `StatusBadge`, and that is a deliberate departure
   from the idiom this repository uses everywhere else: a bordered coloured pill
   repeated down twenty rows is exactly the loudness the palette was drawn to
   remove, and `status.js` budgets loud at one or two rows on a screen. Nothing
   is lost — `StatusDot` beside it carries the same status as shape and as its
   own accessible name. A long custom status ellipsises inside its column. */
const statusStyle = {
  flex: '0 0 62px',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  textAlign: 'right',
  textTransform: 'uppercase',
  letterSpacing: 'var(--tracking-caps)',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)'
}

const emptyStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-6) var(--space-5)'
}

const emptyTitleStyle = {
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-md)/1.4 var(--font-sans)'
}

const emptyHintStyle = {
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-sm)/1.4 var(--font-sans)'
}

/* A refusal stands where the empty state would have: it is the answer to the
   same question, and the handoff draws no error state at all — a hole rather
   than a decision, since `OneshotError` writes six sentences a person can act
   on. */
const errorStyle = {
  ...emptyStyle,
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-sm)/1.4 var(--font-sans)'
}

const meaningRowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  flex: 'none',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  cursor: 'pointer',
  /* The row is a button, so the platform's own border has to be taken off
     before the one edge this design wants is put back. Longhand after the
     shorthand, in that order, because the browser applies them in it. */
  border: 'none',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface-selected)',
  color: 'var(--text-primary)'
}

const meaningTextStyle = {
  flex: 1,
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  textAlign: 'left',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-sans)'
}

const keyStyle = {
  flex: '0 0 auto',
  padding: 'var(--space-1) var(--space-2)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-2)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-2xs)/1.4 var(--font-mono)'
}

/* The sixth fault: nothing on the old screen said which key did what. */
const legendStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-6)',
  flex: 'none',
  padding: 'var(--space-3) var(--space-5)',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1.4 var(--font-mono)'
}
</script>

<template>
  <!-- A click on the scrim closes; a click inside the panel does not, which is
       why the panel stops the event rather than the scrim testing its target. -->
  <div v-if="open" :style="scrimStyle" @click="$emit('close')">
    <div
      role="dialog"
      aria-modal="true"
      aria-label="Search tasks"
      :style="panelStyle"
      @click.stop
    >
      <div :style="inputRowStyle">
        <Icon name="search" :size="16" :style="{ color: 'var(--text-muted)', flex: '0 0 auto' }" />
        <!-- The mode, drawn where the mode is chosen. Its `x` is the way back to
             text, the same gesture `⌘⏎` performs. -->
        <button
          v-if="mode === 'meaning'"
          type="button"
          :style="chipStyle"
          title="Back to text search"
          @mousedown.prevent
          @click="toggleMode"
        >
          <Icon name="sparkles" :size="11" />
          <span>meaning</span>
          <Icon name="x" :size="11" />
        </button>
        <input
          ref="field"
          v-model="query"
          :style="inputStyle"
          type="text"
          role="combobox"
          spellcheck="false"
          autocomplete="off"
          placeholder="Search tasks by id, title or meaning"
          aria-label="Search tasks by id, title or meaning"
          :aria-expanded="rows.length > 0"
          :aria-controls="listId"
          :aria-activedescendant="rows.length ? rowId(sel) : undefined"
          @keydown="onKeydown"
        />
        <!-- The corner's half of the wait, kept as it was: the field says the
             question is out, and the waiting row below says how long for. The
             rows themselves stay exactly where they were until there is an
             answer to move them to. -->
        <Icon
          v-if="pending"
          name="loader-circle"
          :size="13"
          :style="spinStyle"
          title="Asking the agent"
        />
        <span v-else :style="counterStyle">{{ counter }}</span>
        <!-- The panel's one live region, and it is here rather than around
             anything drawn because it has to exist before it has anything to
             say: a region announced into being with its own text is announced by
             nothing. Off screen and not `display: none`, which would take it out
             of the accessibility tree along with everything it was for. -->
        <span :style="liveStyle" aria-live="polite" aria-atomic="true">{{ announcement }}</span>
      </div>

      <!-- Exactly one line here, ever: the wait while the question is out, the
           heading over rows once it is back, and nothing at all when there are
           no rows to label — a heading above an empty state was two blocks
           competing for one job. The wait is drawn whatever the list is doing,
           including over an empty one, since "Nothing matched" is an answer and
           the agent has not given one. It sits above the scroll area rather
           than inside it, so it cannot be drawn over the row the keyboard just
           scrolled to. -->
      <div v-if="pending" :style="waitStyle">
        <Icon name="loader-circle" :size="12" :style="spinStyle" />
        <span>{{ waitLabel }}</span>
      </div>
      <div v-else-if="rows.length" :style="headStyle">{{ heading }}</div>

      <div v-if="rows.length" ref="scroller" :style="scrollStyle">
        <div :id="listId" role="listbox" :aria-label="heading">
          <div
            v-for="(hit, index) in rows"
            :id="rowId(index)"
            :key="hit.id"
            role="option"
            :aria-selected="index === sel"
            :style="rowStyle(index === sel, hit.status)"
            @mousedown.prevent
            @mouseenter="sel = index"
            @click="openRow(hit.id)"
          >
            <span :style="idStyle">{{ hit.id }}</span>
            <span :style="titleStyle">{{ hit.title }}</span>
            <span v-if="hit.relation" :style="relationStyle">
              <Icon :name="hit.relation.icon" :size="12" />
              <span>{{ hit.relation.label }}</span>
            </span>
            <span :style="statusStyle">{{ hit.status }}</span>
            <StatusDot :status="hit.status" :size="8" />
          </div>
        </div>
      </div>

      <!-- Beside the rows rather than instead of them: a refusal leaves the text
           matches standing, and an error drawn only when the list is empty would
           be a spinner that stopped with nothing anywhere to say why. -->
      <div v-if="error" :style="errorStyle">{{ error }}</div>

      <div v-else-if="isEmpty" :style="emptyStyle">
        <div :style="emptyTitleStyle">Nothing matched</div>
        <div :style="emptyHintStyle">{{ emptyHint }}</div>
      </div>

      <button
        v-if="offerMeaning"
        type="button"
        :style="meaningRowStyle"
        @mousedown.prevent
        @click="toggleMode"
      >
        <Icon name="sparkles" :size="13" />
        <span :style="meaningTextStyle">{{ meaningLabel }}</span>
        <span :style="keyStyle">⌘⏎</span>
      </button>

      <div :style="legendStyle">
        <span>↑↓ move</span>
        <span>⏎ open</span>
        <span>⌘⏎ by meaning</span>
        <span :style="{ flex: 1 }"></span>
        <span>esc close</span>
      </div>
    </div>
  </div>
</template>
