<script setup>
/* One row of the Reports tab's list: a run's own document, read back into the
   fields `reports.rs`'s `parse_head` could find. The whole row is a button —
   there is nothing else on it to click — and a press opens the document
   itself through `showReport`, the same permanent tab the bell's card already
   uses.

   Every count is `null` for the reason `runs::reports::Head` carries it: an
   unread board and a genuinely finished-at-zero run are different facts, and
   this row must never turn the first into the second. `cellText` is the one
   place that decision is made, and it draws a dash rather than a blank —
   a blank cell in a row of numbers reads as a rendering fault, a dash reads
   as "nothing to say". */
import { computed } from 'vue'
import { useInteractive } from '../core/interactive.js'
import { COLUMNS, formatStamp } from './reportsPage.js'

const props = defineProps({
  row: { type: Object, required: true }
})

const emit = defineEmits(['open'])

const { hover, handlers } = useInteractive()

const rowStyle = computed(() => ({
  display: 'grid',
  gridTemplateColumns: COLUMNS,
  alignItems: 'center',
  gap: 'var(--space-5)',
  minHeight: 'var(--row-h)',
  padding: '0 var(--space-4)',
  background: hover.value ? 'var(--surface-hover)' : 'transparent',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

const stampStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-secondary)',
  whiteSpace: 'nowrap'
}
const titleStyle = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const scopeStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const countStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-secondary)',
  textAlign: 'right',
  whiteSpace: 'nowrap'
}

/* `null`/`undefined` is a dash, never a blank and never a zero — the one rule
   this whole row exists to keep. `0` is a real count and is drawn as `0`. */
const cellText = (value) => (value === null || value === undefined ? '—' : String(value))

const stamp = computed(() => formatStamp(props.row.stamp))
const title = computed(() => props.row.title ?? cellText(null))
const scope = computed(() => props.row.scope ?? cellText(null))
</script>

<template>
  <div
    role="button"
    tabindex="0"
    :style="rowStyle"
    v-bind="handlers"
    @click="emit('open', row.path)"
    @keydown.enter="emit('open', row.path)"
  >
    <span :style="stampStyle">{{ stamp }}</span>
    <span :style="titleStyle">{{ title }}</span>
    <span :style="scopeStyle" :title="scope">{{ scope }}</span>
    <span :style="countStyle">{{ cellText(row.closed) }}</span>
    <span :style="countStyle">{{ cellText(row.parked) }}</span>
    <span :style="countStyle">{{ cellText(row.batches) }}</span>
    <span :style="countStyle">{{ cellText(row.total) }}</span>
  </div>
</template>
