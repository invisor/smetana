<script setup>
import { computed, nextTick, ref, watch } from 'vue'
import Icon from '../core/Icon.vue'
import { searchIssues } from './taskSearch.js'

/* Finding a task from the top of the window.

   Two tiers, and the slow one is never taken by itself: typing gives instant
   hits off the snapshot the store already holds, and the last row of the list
   asks the agent the same question by meaning. The instant hits stay where they
   are when the slow answer arrives — the person is comparing two answers, and
   replacing one with the other hides what they were comparing against. */
const props = defineProps({
  /* The project's issues, merge lock already excluded by the caller. */
  issues: { type: Array, default: () => [] },
  /* The semantic question is out. Drawn as a spinner in the last row. */
  pending: { type: Boolean, default: false },
  /* Why it failed, already a sentence — `OneshotError` names six ways and each
     one is something a person can act on, so it is drawn rather than swallowed. */
  error: { type: String, default: '' },
  /* What the agent answered, as ids. Rows are drawn from `issues` rather than
     from anything the agent said, so an id it invented cannot reach the screen. */
  semanticIds: { type: Array, default: () => [] }
})

const emit = defineEmits(['select', 'semantic', 'reset'])

const query = ref('')
const open = ref(false)
const active = ref(0)
const field = ref(null)

defineExpose({
  focus: () => {
    open.value = true
    nextTick(() => field.value?.focus())
  }
})

const hits = computed(() => (query.value.trim() ? searchIssues(props.issues, query.value) : []))

/* The agent's ids, resolved against the store and in the order it gave them.
   An id nothing matches drops out here rather than drawing an empty row. */
const semanticHits = computed(() => {
  const byId = new Map(props.issues.map((issue) => [issue.id, issue]))
  return props.semanticIds
    .map((id) => byId.get(id))
    .filter(Boolean)
    .map((issue) => ({ id: issue.id, title: issue.title, type: issue.issue_type ?? 'task' }))
})

/* One flat list for the keyboard, so that ↑ and ↓ walk the two groups and the
   ask-the-agent row without knowing there are groups at all. The divider in the
   template is drawn *inside* this one loop rather than by splitting it, which
   is what keeps the running index the arrows move through and the index a click
   opens from being two different numbers. */
const rows = computed(() => [
  ...hits.value.map((hit) => ({ kind: 'hit', hit })),
  ...semanticHits.value.map((hit) => ({ kind: 'semantic', hit })),
  ...(query.value.trim() ? [{ kind: 'ask' }] : [])
])

/* Where the divider goes: above the first row of the agent's own group. */
const firstSemantic = computed(() => hits.value.length)

const showList = computed(() => open.value && rows.value.length > 0)

/* A new query invalidates the old answer: leaving the agent's ids under a
   different question would attribute them to it. Reopening is conditional on
   there being something to open over — `close()` below empties the field, and
   an unconditional reopen here would undo the very close that emptied it. */
watch(query, (value) => {
  active.value = 0
  if (value.trim()) open.value = true
  emit('reset')
})

const close = () => {
  open.value = false
  query.value = ''
  active.value = 0
  emit('reset')
}

const choose = (row) => {
  if (!row) return
  if (row.kind === 'ask') {
    if (!props.pending) emit('semantic', query.value.trim())
    return
  }
  emit('select', row.hit.id)
  close()
}

const step = (by) => {
  if (!rows.value.length) return
  active.value = (active.value + by + rows.value.length) % rows.value.length
}

const onKeydown = (event) => {
  if (event.key === 'Escape') return close()
  if (event.key === 'ArrowDown') return (event.preventDefault(), step(1))
  if (event.key === 'ArrowUp') return (event.preventDefault(), step(-1))
  if (event.key !== 'Enter') return
  event.preventDefault()
  if (event.altKey) {
    if (query.value.trim() && !props.pending) emit('semantic', query.value.trim())
    return
  }
  choose(rows.value[active.value])
}

/* Leaving the field puts the list away but keeps what was typed. Closing hard,
   the way Esc does, would throw a half-typed query away for the sake of a click
   somewhere else on the window — and the rows themselves cancel their own
   mousedown, so a press on one of them never reaches this at all. */
const onBlur = () => {
  open.value = false
}

/* What a prose hit matched, in this system's own words rather than bd's field
   names: sentence case English, and `acceptance_criteria` is neither. */
const FIELD_LABEL = {
  description: 'Description',
  acceptanceCriteria: 'Acceptance criteria',
  design: 'Design',
  notes: 'Notes',
  labels: 'Labels'
}

/* Narrow at rest, wide while it is being used. The bar's headline is the one
   segment already written to give way when the window is narrow, and a field
   permanently at its open width would take that width from it on every screen
   rather than only while somebody is looking for something.

   The width is transitioned beside `--transition-control` rather than by it:
   that token names background, border and colour, which is every property a
   control in this system usually moves, and this is the one that also changes
   size. The duration and the easing are still the token's own, so the widening
   and the border lighting up finish together — and both stop dead under
   `prefers-reduced-motion`, which zeroes `--dur-fast` at the root. */
const wrapStyle = computed(() => ({
  position: 'relative',
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  width: open.value ? '260px' : '120px',
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-3)',
  background: 'var(--surface-raised)',
  border: `var(--border-w) solid ${open.value ? 'var(--focus-ring)' : 'var(--border)'}`,
  borderRadius: 'var(--radius-3)',
  transition: 'var(--transition-control), width var(--dur-fast) var(--ease-out)'
}))

const inputStyle = {
  flex: 1,
  minWidth: 0,
  border: 'none',
  outline: 'none',
  background: 'transparent',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}

/* Fixed, not absolute, and for the reason `DesktopApp.vue` records over the
   notification panel: the bar is a flex item in a column that clips, so a list
   positioned inside it would be cut off at its own first row.

   The surface, the border and the shadow are the bell's panel's, since the two
   hang from the same edge of the same bar and a second idiom for a floating
   list would be visible as one. */
const listStyle = {
  position: 'fixed',
  top: 'calc(var(--scope-bar-h) + var(--space-2))',
  right: 'var(--space-5)',
  width: 'min(360px, calc(100vw - var(--space-6) * 2))',
  maxHeight: 'min(60vh, 520px)',
  overflowY: 'auto',
  padding: 'var(--space-2)',
  background: 'var(--surface-overlay)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3)',
  boxShadow: 'var(--shadow-overlay)',
  zIndex: 'var(--z-popover)'
}

const rowStyle = (on) => ({
  display: 'flex',
  alignItems: 'baseline',
  gap: 'var(--space-3)',
  width: '100%',
  padding: 'var(--space-2) var(--space-3)',
  border: 'none',
  borderRadius: 'var(--radius-2)',
  background: on ? 'var(--surface-hover)' : 'transparent',
  color: 'var(--text-primary)',
  textAlign: 'left',
  cursor: 'pointer',
  font: 'var(--weight-regular) var(--text-xs)/1.4 var(--font-sans)'
})

const idStyle = {
  flex: '0 0 auto',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1.4 var(--font-mono)'
}

/* The title and the snippet share the row, and the basis is what makes them
   share it rather than one of them winning. A snippet is ninety characters and
   a title is rarely half that, so with either of them sized by its own content
   the long one takes the whole row and the short one is squeezed to nothing —
   which in the first draft of this component meant a hit whose match was in the
   prose drew its id and its snippet and no title at all. Equal bases, both
   shrinking, so each keeps about half and ellipsises inside it; a row with no
   snippet gives the whole width back to the title. */
const titleStyle = {
  flex: '1 1 45%',
  minWidth: 0,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}

const snippetStyle = {
  flex: '1 1 45%',
  minWidth: 0,
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1.4 var(--font-sans)',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}

const dividerStyle = {
  padding: 'var(--space-2) var(--space-3)',
  color: 'var(--text-muted)',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-sans)',
  borderTop: 'var(--border-w) solid var(--border)',
  marginTop: 'var(--space-2)'
}

const errorStyle = {
  padding: 'var(--space-2) var(--space-3)',
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-2xs)/1.4 var(--font-sans)'
}

/* The same spinner every waiting control in this system draws — see
   `git/CommitBox.vue`, whose suggest button asks the very same mechanism the
   row below asks. */
const spinStyle = {
  color: 'var(--attn-live)',
  animation: 'sm-spin var(--dur-pulse) linear infinite'
}
</script>

<template>
  <span :style="wrapStyle">
    <Icon name="search" :size="12" :style="{ color: 'var(--text-muted)' }" />
    <input
      ref="field"
      v-model="query"
      :style="inputStyle"
      type="text"
      placeholder="Search tasks"
      aria-label="Search tasks"
      @focus="open = true"
      @blur="onBlur"
      @keydown="onKeydown"
    />

    <div v-if="showList" :style="listStyle">
      <template v-for="(row, index) in rows" :key="row.kind === 'ask' ? 'ask' : `${row.kind}-${row.hit.id}`">
        <!-- The agent's answer is its own group rather than mixed into the
             instant hits: the person is comparing two answers, and a list that
             does not say which is which hides the comparison. -->
        <p v-if="row.kind === 'semantic' && index === firstSemantic" :style="dividerStyle">
          By meaning
        </p>
        <button
          :style="rowStyle(index === active)"
          type="button"
          @mousedown.prevent
          @click="choose(row)"
        >
          <template v-if="row.kind === 'ask'">
            <Icon
              v-if="pending"
              name="loader-circle"
              :size="12"
              :style="spinStyle"
              title="Asking the agent"
            />
            <Icon v-else name="sparkles" :size="12" />
            <span :style="titleStyle">
              {{ pending ? 'Asking the agent…' : 'Search by meaning' }}
            </span>
          </template>
          <template v-else>
            <span :style="idStyle">{{ row.hit.id }}</span>
            <span :style="titleStyle">{{ row.hit.title }}</span>
            <span v-if="row.kind === 'hit' && row.hit.snippet" :style="snippetStyle">
              {{ FIELD_LABEL[row.hit.field] }}: {{ row.hit.snippet }}
            </span>
          </template>
        </button>
      </template>

      <p v-if="error" :style="errorStyle">{{ error }}</p>
    </div>
  </span>
</template>
