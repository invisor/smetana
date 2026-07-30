<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import StatusBadge from '../status/StatusBadge.vue'
import DependencyBand from '../status/DependencyBand.vue'
import DependencyMark from '../status/DependencyMark.vue'
import Assignee from './Assignee.vue'
import { attentionLevel } from '../status/status.js'

const props = defineProps({
  id: { type: String, required: true },
  title: { type: String, required: true },
  status: { type: String, default: 'ready' },
  assignee: { type: Object, default: null },
  blockedBy: { type: Number, default: 0 },
  blocks: { type: Number, default: 0 },
  spawnedFrom: { type: String, default: undefined },
  needsResponse: { type: Boolean, default: false },
  state: { type: String, default: 'default' },
  changedBy: { type: String, default: undefined },
  selected: { type: Boolean, default: false }
})

defineEmits(['click'])

const hover = ref(false)
const level = computed(() => attentionLevel(props.status))
const dragging = computed(() => props.state === 'dragging')
const drop = computed(() => props.state === 'drop-target')
const changed = computed(() => props.state === 'changed')
/* An agent waiting on an answer is the one thing allowed to shout. */
const loud = computed(() => props.needsResponse || level.value === 'loud')

const borderColor = computed(() => {
  if (props.selected) return 'var(--focus-ring)'
  if (drop.value) return 'var(--border-strong)'
  if (loud.value) return 'var(--attn-loud)'
  return 'var(--border)'
})

const style = computed(() => ({
  display: 'flex',
  gap: 'var(--space-4)',
  alignItems: 'stretch',
  padding: 'var(--card-pad)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  background: drop.value ? 'var(--surface-sunken)' : 'var(--surface-raised)',
  border: `var(--border-w) solid ${borderColor.value}`,
  borderStyle: drop.value ? 'dashed' : 'solid',
  borderRadius: 'var(--radius-3)',
  boxShadow: dragging.value
    ? 'var(--shadow-drag)'
    : props.selected
      ? '0 0 0 1px var(--focus-ring)'
      : 'var(--shadow-raised)',
  opacity: level.value === 'quiet' && !hover.value ? 'var(--attn-quiet-opacity)' : dragging.value ? 0.9 : 1,
  transform: dragging.value ? 'rotate(-.4deg)' : 'none',
  // "this changed while you weren't looking" — twice, then gone
  animation: changed.value ? 'sm-flash var(--dur-flash) var(--ease-out) 2' : undefined,
  cursor: 'default',
  transition: 'var(--transition-control)',
  outline: hover.value && !props.selected ? '1px solid var(--border-strong)' : 'none',
  outlineOffset: '-1px'
}))

const idStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  letterSpacing: 'var(--tracking-tight)'
}
const askStyle = {
  display: 'inline-flex', alignItems: 'center', gap: '3px', padding: '0 5px', height: '15px',
  background: 'var(--attn-loud)', color: 'var(--attn-loud-contrast)', borderRadius: 'var(--radius-2)',
  font: 'var(--weight-semibold) var(--text-2xs)/1 var(--font-mono)', letterSpacing: 'var(--tracking-caps)'
}
const newStyle = {
  display: 'inline-flex', alignItems: 'center', gap: '3px', color: 'var(--attn-loud)',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)'
}
const titleStyle = {
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-snug)',
  color: 'var(--text-primary)',
  textWrap: 'pretty'
}
</script>

<template>
  <div
    tabindex="0"
    :data-attention="level"
    :style="style"
    @click="$emit('click')"
    @mouseenter="hover = true"
    @mouseleave="hover = false"
  >
    <DependencyBand :blocked-by="blockedBy" :blocks="blocks" />
    <div :style="{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }">
      <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }">
        <span :style="idStyle">{{ id }}</span>
        <span :style="{ flex: 1 }" />
        <span v-if="needsResponse" title="Agent is waiting for your answer" :style="askStyle">
          <Icon name="message-circle-question-mark" :size="9" :stroke-width="2.5" />ASK
        </span>
        <span v-if="changedBy" :title="`Changed by ${changedBy} since you last looked`" :style="newStyle">
          <Icon name="dot" :size="12" :stroke-width="3" />new
        </span>
      </div>
      <div :style="titleStyle">{{ title }}</div>
      <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-5)', flexWrap: 'wrap' }">
        <StatusBadge :status="status" size="sm" />
        <span :style="{ flex: 1 }" />
        <DependencyMark :blocked-by="blockedBy" :blocks="blocks" :spawned-from="spawnedFrom" size="sm" />
        <Assignee v-if="assignee" :kind="assignee.kind" :name="assignee.name" />
      </div>
    </div>
  </div>
</template>
