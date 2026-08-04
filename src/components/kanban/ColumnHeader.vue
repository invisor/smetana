<script setup>
import { computed, nextTick } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import StatusDot from '../status/StatusDot.vue'
import { statusColors } from '../status/status.js'

const props = defineProps({
  status: { type: String, required: true },
  count: { type: Number, default: 0 },
  wipLimit: { type: Number, default: null },
  /* Not every column accepts new issues: in bd an issue is born in one status
     only, and the "+" stands where pressing it actually works. */
  addable: { type: Boolean, default: true },
  /* The header doubles as the column's drag handle. It only reports the
     gesture — where the column ends up is the board's to decide, since the
     board is what holds the list. */
  movable: { type: Boolean, default: false },
  moving: { type: Boolean, default: false }
})

const emit = defineEmits(['add', 'grab', 'move'])

const c = computed(() => statusColors(props.status))
const over = computed(() => props.wipLimit != null && props.count > props.wipLimit)
const label = computed(() => c.value.key.replace(/-/g, ' '))

/* A pointerdown on the "+" is a press of that button and nothing else. Without
   this the button still works — a click survives a drag that never passed its
   threshold — but the column follows the pointer while somebody aims at it. */
const onPointerdown = (event) => {
  if (!props.movable || event.button !== 0) return
  if (event.target.closest('button')) return
  emit('grab', event)
}

/* Alt, not the bare arrow: a board is scrolled with arrow keys, and taking
   them would mean a focused header could no longer be moved past by keyboard
   without dragging the column along.

   The refocus afterwards is not a nicety. Columns are keyed by status, so Vue
   moves this very element rather than rebuilding it — but moving a node is a
   removal and an insertion, and the browser blurs what it removes. Without this
   the first alt+arrow works, focus lands on the body, and the second one
   silently does nothing. `focus` also brings the column back into view, which
   is the other half of what a keyboard move owes a board wider than its pane. */
const onKeydown = (event) => {
  if (!props.movable || !event.altKey) return
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
  event.preventDefault()
  const el = event.currentTarget
  emit('move', event.key === 'ArrowRight' ? 1 : -1)
  nextTick(() => el.focus())
}

const style = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--row-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-3) 0 var(--space-4)',
  borderBottom: `var(--border-w-strong) solid ${c.value.border}`,
  marginBottom: 'var(--space-4)',
  /* Being dragged is a surface step up, the same as every other interaction in
     this system — never a colour change and never a transform, so a column
     under the pointer cannot jump away from it. */
  background: props.moving ? 'var(--surface-active)' : 'transparent',
  borderRadius: 'var(--radius-2) var(--radius-2) 0 0',
  cursor: props.movable ? (props.moving ? 'grabbing' : 'grab') : 'default',
  /* Without this a touch drag scrolls the board instead of moving the column,
     and the pointer capture never sees the moves. */
  touchAction: props.movable ? 'none' : 'auto'
}))

const nameStyle = computed(() => ({
  font: 'var(--weight-semibold) var(--text-xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: c.value.fg,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}))

/* Only a movable header takes focus, and then it owes the keyboard a name that
   says what focusing it is for — the label is the entire discoverability of
   alt+arrow. A header that cannot move stays out of the tab order: a stop that
   does nothing is worse than no stop. */
const moveLabel = computed(() => `Column ${label.value}. Alt with left or right arrow moves it.`)

const wipStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '2px',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  color: over.value ? 'var(--status-failed-fg)' : 'var(--text-muted)'
}))
</script>

<template>
  <div
    :style="style"
    :tabindex="movable ? 0 : undefined"
    :aria-label="movable ? moveLabel : undefined"
    @pointerdown="onPointerdown"
    @keydown="onKeydown"
  >
    <StatusDot :status="status" :size="8" />
    <span :style="nameStyle">{{ label }}</span>
    <span :style="{ font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }">
      {{ count }}
    </span>
    <span v-if="wipLimit != null" :title="`WIP limit ${wipLimit}`" :style="wipStyle">
      <Icon :name="over ? 'triangle-alert' : 'gauge'" :size="10" :stroke-width="2" />/{{ wipLimit }}
    </span>
    <span :style="{ flex: 1 }" />
    <slot name="actions">
      <IconButton v-if="addable" icon="plus" :label="`Add task to ${c.key}`" size="sm" @click="$emit('add')" />
    </slot>
  </div>
</template>
