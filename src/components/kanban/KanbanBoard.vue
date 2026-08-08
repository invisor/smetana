<script setup>
import { computed, onBeforeUnmount, ref } from 'vue'
import KanbanColumn from './KanbanColumn.vue'
import EmptyState from '../core/EmptyState.vue'
import { moveColumn, orderColumns } from './columnOrder.js'

/* Columns come from the tracker's status set, which users extend — the board
   never assumes four fixed columns. */
const props = defineProps({
  columns: { type: Array, default: () => [] },
  selectedId: { type: String, default: undefined },
  /* The status of the one column that accepts new issues — it alone has a "+".
     null: the board creates nothing. Which status that is, is the product's
     decision: there is no fixed set of columns here and there cannot be. */
  addTo: { type: String, default: null },
  /* The status of the one column whose header offers a run — `ready` in
     practice, and a prop for the same reason `addTo` is one: there is no fixed
     set of columns here. Null means runs are not offered at all, which is what
     an unconfigured project gets. */
  runFrom: { type: String, default: null },
  /* Why a run cannot be started just now, in words; empty means it can. Every
     play on the board — the header's and every card's — goes inactive on it and
     says this instead. Not a boolean: a control that is off owes a person the
     reason, and the board is not the place that knows it.

     A lowercase fragment, and this is the prop a caller actually binds — the
     plays interpolate it into "Run this — …" and "Run the queue — …", so a
     capital arriving here renders as two sentences joined by a dash, silently
     and only inside a tooltip. */
  runBlockedReason: { type: String, default: '' },
  /* The status of the one column whose whole contents can be moved into the
     queue in one press — `deferred` in practice, and a third prop of the same
     kind as `addTo` and `runFrom` for the same reason. Null means the board
     offers nothing of the sort.

     Deliberately not folded into `runFrom`: a run and this press look alike
     from here — one button on one column — and are opposites. A run takes work
     out of the queue and does it; this only puts work into the queue and stops
     there. */
  promoteFrom: { type: String, default: null },
  reorderable: { type: Boolean, default: true }
})

/* `reorder` carries the full list of statuses in their new order, not a
   from/to pair: the board is not the owner of the order and has no business
   describing a change to a list it does not keep. Whoever stores it applies the
   answer wholesale and hands it back through `columns`. */
const emit = defineEmits(['select', 'add', 'reorder', 'run', 'run-task', 'promote'])

const strip = ref(null)
/* The order under the pointer, and only while the pointer holds it. Idle, this
   is null and the board draws exactly what it was given — the drawn order is
   the parent's, the same way a panel's drawn width is not the width it stored.
   The draft is applied through orderColumns, so a column bd grows mid-drag
   appears at the end instead of vanishing. */
const draft = ref(null)
const held = ref(null)

const view = computed(() =>
  draft.value ? orderColumns(props.columns, draft.value) : props.columns
)

const movable = computed(() => props.reorderable && props.columns.length > 1)

/* The page-wide guards a drag needs, the same pair Resizer takes: without them
   the pointer sweeping the board selects every column title it crosses, and the
   cursor flickers between grabbing and whatever it passes over. */
let guarded = null
const guard = () => {
  if (guarded) return
  guarded = { userSelect: document.body.style.userSelect, cursor: document.body.style.cursor }
  document.body.style.userSelect = 'none'
  document.body.style.cursor = 'grabbing'
}
const unguard = () => {
  if (!guarded) return
  document.body.style.userSelect = guarded.userSelect
  document.body.style.cursor = guarded.cursor
  guarded = null
}

let capture = null
let moved = false

/* Which column the pointer is over, read from the columns' own boxes rather
   than computed from a width — the board scrolls, and a fixed 212px is a fact
   about the stylesheet, not about where a column is on screen right now. */
const columnAt = (x) => {
  const cells = strip.value ? [...strip.value.children] : []
  for (let i = 0; i < cells.length; i += 1) {
    if (x < cells[i].getBoundingClientRect().right) return i
  }
  return cells.length - 1
}

/* Escape abandons the drag. It has to be on the window: the pointer is captured
   and focus is wherever it was, so a handler on the board would never see it. */
const onKeydown = (event) => {
  if (event.key !== 'Escape') return
  event.preventDefault()
  end(false)
}

/* Capture goes on the strip, not on the header that was pressed. It is what
   makes a release outside the window still end the drag — and taking it here
   means every move and release arrives at one element regardless of which
   column the pointer has since crossed into.

   It is also the one call that can throw, on a pointer the engine no longer
   knows. Letting that escape would leave the page unselectable with no drag
   left to release it, so a refused capture costs the drag once the pointer
   leaves the board, and nothing more. */
const onGrab = (status, event) => {
  if (!movable.value || draft.value) return
  held.value = status
  draft.value = view.value.map((column) => column.status)
  moved = false
  guard()
  window.addEventListener('keydown', onKeydown)
  try {
    strip.value.setPointerCapture(event.pointerId)
    capture = event.pointerId
  } catch {
    capture = null
  }
}

/* The order is rebuilt from what is drawn on every move rather than mutated in
   place: `columnAt` answers in the drawn list's indices, and the two would
   disagree the moment a delta added or removed a column mid-drag.

   Swapping when the pointer enters a neighbour's box — not its midpoint — is
   what keeps this from oscillating: once the columns have traded places the
   pointer is over the held column again, so the next move finds it already
   where it belongs and does nothing until the pointer leaves that box entirely. */
const onPointermove = (event) => {
  if (!draft.value) return
  const order = view.value.map((column) => column.status)
  const next = moveColumn(order, order.indexOf(held.value), columnAt(event.clientX))
  if (next === order) return
  draft.value = next
  moved = true
}

const end = (commit = true) => {
  if (!draft.value) return
  const order = draft.value
  const changed = moved
  draft.value = null
  held.value = null
  moved = false
  unguard()
  window.removeEventListener('keydown', onKeydown)
  if (capture != null) {
    /* Releasing a capture the element no longer holds throws in some engines,
       and by here the pointer may already be gone. */
    try {
      strip.value?.releasePointerCapture(capture)
    } catch {
      /* already released — nothing to undo */
    }
    capture = null
  }
  if (commit && changed) emit('reorder', order)
}

/* The keyboard path is a whole drag in miniature — one step, committed at once.
   It works off the drawn order, which is the parent's, so a board whose parent
   ignores `reorder` simply does not move, the same as with the pointer.
   Keeping the moved header focused and in view is the header's own business:
   it is the element that gets moved, and it is the one holding the reference. */
const onMove = (status, step) => {
  if (!movable.value || draft.value) return
  const order = props.columns.map((column) => column.status)
  const from = order.indexOf(status)
  const next = moveColumn(order, from, from + step)
  if (next !== order) emit('reorder', next)
}

// A drag that outlives the component would leave the page unselectable.
onBeforeUnmount(() => {
  unguard()
  window.removeEventListener('keydown', onKeydown)
})

const style = {
  display: 'flex',
  gap: 'var(--space-6)',
  alignItems: 'stretch',
  flex: 1,
  minHeight: 0,
  overflow: 'auto',
  padding: 'var(--panel-pad)'
}
</script>

<template>
  <EmptyState
    v-if="!columns.length"
    icon="columns-3"
    title="No board yet"
    description="Connect a tracker to pull tasks, or create the first task locally."
  />
  <div
    v-else
    ref="strip"
    :style="style"
    @pointermove="onPointermove"
    @pointerup="end()"
    @pointercancel="end(false)"
    @lostpointercapture="end()"
  >
    <KanbanColumn
      v-for="c in view"
      :key="c.status"
      v-bind="c"
      :selected-id="selectedId"
      :addable="c.status === addTo"
      :runnable="runFrom != null && c.status === runFrom"
      :run-blocked-reason="runBlockedReason"
      :promotable="promoteFrom != null && c.status === promoteFrom"
      :movable="movable"
      :moving="c.status === held"
      @select="$emit('select', $event)"
      @add="$emit('add', $event)"
      @run="$emit('run', $event)"
      @promote="$emit('promote', $event)"
      @run-task="$emit('run-task', $event)"
      @grab="onGrab(c.status, $event)"
      @move="onMove(c.status, $event)"
    />
  </div>
</template>
