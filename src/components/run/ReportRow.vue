<script setup>
/* One row of the Reports tab's list: a run's own document, read back into the
   fields `reports.rs`'s `parse_head` could find. The whole row is a button —
   there is nothing else on it to click — and a press opens the document
   itself through `showReport`, the same permanent tab the bell's card already
   uses.

   Every count is `null` for the reason `runs::reports::Head` carries it: an
   unread board and a genuinely finished-at-zero run are different facts, and
   this row must never turn the first into the second. `cellText` is the one
   place that decision is made, and it draws a dash rather than a blank — a
   blank cell in a row of numbers reads as a rendering fault, a dash reads as
   "nothing to say".

   The column that used to draw the report *kind* — "Run report", "Task
   report" — now draws `summary`: `reports::parse_head`'s plain-language
   sentence for the run (a lead's own `summary` batch field, or, for an older
   document with no such section, the closed tasks' titles). Which queue or
   task it covers is already visible in Scope, so naming the report kind a
   second time spent a column on nothing.

   Parked is the only colour in this table (`--status-blocked-fg` above
   zero) — the design handoff's own budget for attention here — and a zero in
   Closed, Parked or Batches is dimmed to `--attn-quiet`/`--attn-quiet-opacity`
   rather than drawn at full weight, so a quiet row does not compete with a
   loud one for a glance. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import { useInteractive } from '../core/interactive.js'
import { COLUMNS, barPercent, formatStamp } from './reportsPage.js'

const props = defineProps({
  row: { type: Object, required: true },
  selected: { type: Boolean, default: false },
  maxSeconds: { type: Number, required: true }
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
  position: 'relative',
  background: props.selected ? 'var(--surface-selected)' : hover.value ? 'var(--surface-hover)' : 'transparent',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

const markerStyle = {
  position: 'absolute',
  left: 0,
  top: 0,
  bottom: 0,
  width: 'var(--border-w-strong)',
  background: 'var(--focus-ring)'
}

const whenStyle = {
  display: 'flex',
  alignItems: 'baseline',
  gap: 'var(--space-3)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)',
  fontVariantNumeric: 'tabular-nums',
  whiteSpace: 'nowrap'
}
const dateStyle = { color: 'var(--text-muted)' }
const timeStyle = { color: 'var(--text-secondary)' }

const summaryStyle = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const summaryEmptyStyle = { ...summaryStyle, color: 'var(--text-muted)' }

const scopeStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  minWidth: 0,
  color: 'var(--text-muted)'
}
const queueTextStyle = {
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-sans)',
  color: 'var(--text-muted)',
  whiteSpace: 'nowrap'
}
const taskTextStyle = {
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

const baseCountStyle = {
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)',
  fontVariantNumeric: 'tabular-nums',
  textAlign: 'right',
  paddingRight: 'var(--space-2)',
  whiteSpace: 'nowrap'
}
const quietStyle = { color: 'var(--attn-quiet)', opacity: 'var(--attn-quiet-opacity)' }
const dashStyle = { color: 'var(--text-muted)' }

function countStyleFor(value, colourWhenPositive) {
  if (value === null || value === undefined) return { ...baseCountStyle, ...dashStyle }
  if (value === 0) return { ...baseCountStyle, ...quietStyle }
  return { ...baseCountStyle, color: colourWhenPositive }
}

const totalStyle = {
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'flex-end',
  gap: 'var(--space-2)'
}
const totalLabelStyle = {
  font: 'var(--weight-medium) var(--text-sm)/1 var(--font-mono)',
  fontVariantNumeric: 'tabular-nums',
  whiteSpace: 'nowrap'
}
const barTrackStyle = {
  width: 'calc(var(--space-9) * 2)',
  height: 'calc(var(--border-w) * 2)',
  background: 'var(--border-subtle)',
  display: 'flex',
  justifyContent: 'flex-end'
}
const barFillStyle = (pct) => ({ width: `${pct}%`, height: '100%', background: 'var(--border-strong)' })

/* `null`/`undefined` is a dash, never a blank and never a zero — the one rule
   this whole row exists to keep. `0` is a real count and is drawn as `0`. */
const cellText = (value) => (value === null || value === undefined ? '—' : String(value))

const when = computed(() => formatStamp(props.row.stamp))
const summary = computed(() => props.row.summary ?? null)
const isQueue = computed(() => props.row.scope === 'the queue')
const scope = computed(() => props.row.scope ?? null)
const bar = computed(() => barPercent(props.row.seconds, props.maxSeconds))
</script>

<template>
  <div
    role="button"
    tabindex="0"
    :style="rowStyle"
    v-bind="handlers"
    @click="emit('open', row.path)"
    @keydown.enter="emit('open', row.path)"
    @keydown.space.prevent="emit('open', row.path)"
  >
    <span v-if="selected" :style="markerStyle" />
    <span :style="whenStyle">
      <span :style="dateStyle">{{ when.date }}</span>
      <span :style="timeStyle">{{ when.time }}</span>
    </span>
    <span :style="summary === null ? summaryEmptyStyle : summaryStyle" :title="summary ?? undefined">
      {{ summary === null ? '—' : summary }}
    </span>
    <span :style="scopeStyle">
      <template v-if="scope === null">
        <span :style="dashStyle">—</span>
      </template>
      <template v-else-if="isQueue">
        <Icon name="layers" :size="12" />
        <span :style="queueTextStyle">the queue</span>
      </template>
      <template v-else>
        <Icon name="hash" :size="12" />
        <span :style="taskTextStyle" :title="scope">{{ scope }}</span>
      </template>
    </span>
    <span :style="countStyleFor(row.closed, 'var(--text-primary)')">{{ cellText(row.closed) }}</span>
    <span :style="countStyleFor(row.parked, row.parked > 0 ? 'var(--status-blocked-fg)' : 'var(--text-primary)')">
      {{ cellText(row.parked) }}
    </span>
    <span :style="countStyleFor(row.batches, 'var(--text-secondary)')">{{ cellText(row.batches) }}</span>
    <span :style="totalStyle">
      <span :style="totalLabelStyle">{{ cellText(row.total) }}</span>
      <span v-if="bar !== null" :style="barTrackStyle">
        <span :style="barFillStyle(bar)" />
      </span>
    </span>
  </div>
</template>
