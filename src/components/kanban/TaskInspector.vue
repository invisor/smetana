<script setup>
/* Everything the tracker knows about one issue, plus the two things a person
   can do to it here.

   Nothing in this panel is editable except the status. Rewriting a title or a
   description is an agent's job — the "Ask agent to edit" button is what hands
   the issue over — and a field that cannot be typed into cannot silently
   overwrite what an agent wrote while the panel sat open. */
import { computed, ref, watch } from 'vue'
import Button from '../core/Button.vue'
import Dropdown from '../core/Dropdown.vue'
import Modal from '../overlays/Modal.vue'
import StatusBadge from '../status/StatusBadge.vue'
import TypeBadge from './TypeBadge.vue'

const props = defineProps({
  /* The issue in bd's own shape, straight out of the tracker store. */
  issue: { type: Object, required: true },
  /* Statuses translated to the design system's vocabulary, for the badge. */
  uiStatus: { type: String, required: true },
  /* A status write is in flight. */
  busy: { type: Boolean, default: false },
  /* A deletion is in flight. */
  deleting: { type: Boolean, default: false }
})

const emit = defineEmits(['status', 'delete', 'ask-agent'])

/* The three a person is given, and no more. bd has eleven statuses in this
   build and most of them are an agent's business: `in_progress` is claimed by
   whoever starts work, `hooked` says an agent owns the molecule, `deferred`
   and `blocked` are answers to questions this panel is not asking. */
const STATUSES = [
  { value: 'open', label: 'Ready' },
  { value: 'pinned', label: 'Pinned' },
  { value: 'closed', label: 'Done' }
]

/* An issue may well hold a status outside those three — an agent moves it to
   in_progress, bd's own tooling to hooked. With no matching option the select
   would render its first entry and quietly claim the issue is Ready. So the
   status it actually has is appended when it is not already there: choosing it
   again is a no-op, and the panel tells the truth either way. */
const statusOptions = computed(() =>
  STATUSES.some((s) => s.value === props.issue.status)
    ? STATUSES
    : [...STATUSES, { value: props.issue.status, label: props.issue.status }]
)

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
const blockedBy = computed(
  () => (props.issue.dependencies ?? []).filter((d) => d.type === 'blocks').length
)

/* Only the rows the issue actually has. A fixed list with blanks in it would
   read as a form waiting to be filled in, and this panel is not one. */
const rows = computed(() => {
  const issue = props.issue
  const entries = [
    /* Type is not here: it is a badge in the header, beside the status. */
    ['Priority', issue.priority === null || issue.priority === undefined ? null : `P${issue.priority}`, false],
    ['Owner', issue.owner, true],
    ['Labels', issue.labels?.length ? issue.labels.join(', ') : null, false],
    ['Parent', issue.parent, true],
    ['Blocked by', blockedBy.value || null, false],
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

const confirming = ref(false)

/* The dialog closes when the deletion settles, not when it is asked for. On
   success this whole component is gone by then — the issue left the store and
   the panel with it — so what this actually covers is the failure: the write
   came back refused, the card is still on the board, and leaving a modal
   hanging over the toast that explains why would hide the explanation. */
watch(
  () => props.deleting,
  (now, before) => {
    if (before && !now) confirming.value = false
  }
)

const askAgent = () => emit('ask-agent', props.issue)

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

const closeReasonBox = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)'
}

const actions = { display: 'flex', gap: 'var(--space-4)' }

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

    <!-- A Dropdown rather than a Select, and here that is load-bearing rather
         than cosmetic: this panel is inside Panel's scroll container, and a
         list drawn in the flow would be clipped by it. Dropdown's panel leaves
         the document, which is the same move the delete dialog below makes and
         for the same reason. -->
    <Dropdown
      :model-value="issue.status"
      :options="statusOptions"
      :disabled="busy || deleting"
      size="sm"
      @update:model-value="$emit('status', $event)"
    />

    <div :style="actions">
      <Button variant="secondary" size="sm" icon="bot" :disabled="deleting" @click="askAgent">
        Ask agent to edit
      </Button>
      <Button
        variant="ghost"
        size="sm"
        icon="trash-2"
        :disabled="deleting"
        @click="confirming = true"
      >
        Delete
      </Button>
    </div>

    <div :style="divider" />

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

    <!-- Teleported, unlike every other Modal in this app, and for a reason that
         is about where this one lives rather than about what it does. Modal's
         scrim is `position: absolute; inset: 0` with no positioned ancestor
         anywhere in the view, so it sizes itself to the viewport — which is
         what makes the other dialogs full-screen. This one renders inside
         Panel's scroll container (`overflow: auto`), and an overflow box clips
         its absolutely positioned descendants whatever they are positioned
         against: the dialog would be cropped to the right column, a few
         hundred pixels wide, with its buttons off the edge. Teleporting takes
         it out of that box. The tokens still resolve — theme and density are
         attributes on the document root, so anything under <body> inherits
         them. -->
    <Teleport to="body">
      <Modal
        :open="confirming"
        :closable="!deleting"
        :title="`Delete ${issue.id}?`"
        description="bd deletes the issue outright and rewrites references to it in whatever was linked to it. Anything that depended on this issue is left without the dependency. There is no undo."
        @close="confirming = false"
      >
        <div :style="titleStyle">{{ issue.title }}</div>
        <template #footer>
          <Button variant="ghost" :disabled="deleting" @click="confirming = false">Cancel</Button>
          <Button variant="danger" :disabled="deleting" @click="$emit('delete', issue.id)">
            {{ deleting ? 'Deleting…' : 'Delete' }}
          </Button>
        </template>
      </Modal>
    </Teleport>
  </div>
</template>
