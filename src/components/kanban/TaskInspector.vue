<script setup>
/* Everything the tracker knows about one issue.

   Nothing in this panel is editable, and nothing in it acts. Rewriting a title
   or a description is an agent's job, and the four things a person can do to an
   issue — run it, hand it to an agent, move it between columns, delete it —
   live in the card's own menu on the board, so that they reach the card under
   the pointer rather than only the selected one. A panel that cannot be typed
   into cannot silently overwrite what an agent wrote while it sat open. */
import { computed } from 'vue'
import StatusBadge from '../status/StatusBadge.vue'
import TypeBadge from './TypeBadge.vue'
import { priorityLabel } from './issueType.js'

const props = defineProps({
  /* The issue in bd's own shape, straight out of the tracker store. */
  issue: { type: Object, required: true },
  /* Statuses translated to the design system's vocabulary, for the badge. */
  uiStatus: { type: String, required: true }
})

/* bd hands dates over as RFC 3339 in UTC. The panel is narrow, so the year is
   worth the four characters only because an issue can be old — the alternative,
   a relative "3 days ago", turns into a lie the moment the app is left open
   overnight. Anything unparseable is shown as it arrived rather than as
   "Invalid Date": bd's own text is more use to whoever has to explain it. */
const FORMAT = new Intl.DateTimeFormat('en-GB', {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit'
})

function formatDate(value) {
  if (!value) return null
  const parsed = Date.parse(value)
  return Number.isNaN(parsed) ? value : FORMAT.format(parsed)
}

/* bd gives an issue only its outgoing edges, so "blocks" cannot be counted
   here — that sum needs every issue on the board and lives in the tracker
   store. What is countable from one issue is what blocks it. Parentage travels
   as a parent-child edge and is excluded: every subtask would otherwise read as
   blocked by its own parent. */
const blockedBy = computed(() =>
  (props.issue.dependencies ?? []).filter((d) => d.type === 'blocks').map((d) => d.depends_on_id)
)

/* Only the rows the issue actually has. A fixed list with blanks in it would
   read as a form waiting to be filled in, and this panel is not one. */
const rows = computed(() => {
  const issue = props.issue
  const entries = [
    /* Type is not here: it is a badge in the header, beside the status. */
    /* null when there is none, which the filter below drops: this panel shows
       only the rows an issue actually has. */
    ['Priority', priorityLabel(issue.priority), false],
    ['Owner', issue.owner, true],
    ['Labels', issue.labels?.length ? issue.labels.join(', ') : null, false],
    ['Parent', issue.parent, true],
    /* The ids, not how many: this panel has the room, and the number alone
       leaves a person with nowhere to go next. Mono, like every other
       identifier here. */
    ['Blocked by', blockedBy.value.join(', ') || null, true],
    ['Comments', issue.comment_count || null, false],
    ['Created', formatDate(issue.created_at), true],
    ['Created by', issue.created_by, true],
    ['Started', formatDate(issue.started_at), true],
    ['Updated', formatDate(issue.updated_at), true],
    ['Closed', formatDate(issue.closed_at), true]
  ]
  return entries
    .filter(([, value]) => value !== null && value !== undefined && value !== '')
    .map(([label, value, mono]) => ({ label, value: String(value), mono }))
})

const body = { display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }

const header = { display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }

const idStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}

const titleStyle = {
  font: `var(--weight-medium) var(--text-md)/var(--leading-snug) var(--font-sans)`,
  color: 'var(--text-primary)',
  textWrap: 'pretty'
}

const descriptionStyle = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)',
  whiteSpace: 'pre-wrap',
  textWrap: 'pretty'
}

/* Two columns, the label column sized to its longest word and no wider. */
const grid = {
  display: 'grid',
  gridTemplateColumns: 'max-content minmax(0, 1fr)',
  columnGap: 'var(--space-5)',
  rowGap: 'var(--space-3)',
  alignItems: 'baseline'
}

const rowLabel = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}

const rowValue = (mono) => ({
  font: mono
    ? 'var(--weight-regular) var(--text-xs)/var(--leading-snug) var(--font-mono)'
    : 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
})

/* One of bd's prose fields under its heading: acceptance criteria, design,
   notes. Prose like the description, not a row in the table — a note is a
   paragraph and may be several, since every `bd note` appends another. */
const proseSection = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)'
}

const closeReasonBox = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)'
}

const divider = {
  height: 'var(--border-w)',
  background: 'var(--border-subtle)'
}
</script>

<template>
  <div :style="body">
    <div :style="header">
      <span :style="idStyle">{{ issue.id }}</span>
      <StatusBadge :status="uiStatus" size="sm" />
      <!-- The panel keeps the status badge the card gave up: nothing here says
           which column the issue is in. The type joins it rather than staying a
           row of text below, so the same badge means the same thing in both
           places. -->
      <TypeBadge v-if="issue.issue_type" :type="issue.issue_type" size="sm" />
    </div>

    <div :style="titleStyle">{{ issue.title }}</div>

    <div v-if="issue.description" :style="descriptionStyle">{{ issue.description }}</div>

    <!-- bd's other prose, in a fixed order: the two that are the spec first,
         the log that grows last. Read-only like the description — rewriting
         any of them is an agent's job. Absent fields draw nothing at all, so
         an issue without them looks exactly as it did before they existed. -->
    <div v-if="issue.acceptance_criteria" :style="proseSection">
      <span :style="rowLabel">Acceptance criteria</span>
      <span :style="descriptionStyle">{{ issue.acceptance_criteria }}</span>
    </div>

    <div v-if="issue.design" :style="proseSection">
      <span :style="rowLabel">Design</span>
      <span :style="descriptionStyle">{{ issue.design }}</span>
    </div>

    <div v-if="issue.notes" :style="proseSection">
      <span :style="rowLabel">Notes</span>
      <span :style="descriptionStyle">{{ issue.notes }}</span>
    </div>

    <!-- Only when there is a record to separate: an issue carrying neither
         fields nor a close reason would otherwise draw a rule under the
         description with nothing at all beneath it. -->
    <div v-if="rows.length || issue.close_reason" :style="divider" />

    <div v-if="rows.length" :style="grid">
      <template v-for="row in rows" :key="row.label">
        <span :style="rowLabel">{{ row.label }}</span>
        <span :style="rowValue(row.mono)">{{ row.value }}</span>
      </template>
    </div>

    <div v-if="issue.close_reason" :style="closeReasonBox">
      <span :style="rowLabel">Close reason</span>
      <span :style="descriptionStyle">{{ issue.close_reason }}</span>
    </div>
  </div>
</template>
