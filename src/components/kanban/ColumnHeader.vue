<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import StatusDot from '../status/StatusDot.vue'
import { statusColors } from '../status/status.js'

const props = defineProps({
  status: { type: String, required: true },
  count: { type: Number, default: 0 },
  wipLimit: { type: Number, default: null }
})

const c = computed(() => statusColors(props.status))
const over = computed(() => props.wipLimit != null && props.count > props.wipLimit)
const label = computed(() => c.value.key.replace(/-/g, ' '))

const style = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--row-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-3) 0 var(--space-4)',
  borderBottom: `var(--border-w-strong) solid ${c.value.border}`,
  marginBottom: 'var(--space-4)'
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

const wipStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '2px',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  color: over.value ? 'var(--status-failed-fg)' : 'var(--text-muted)'
}))
</script>

<template>
  <div :style="style">
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
      <IconButton icon="plus" :label="`Add task to ${c.key}`" size="sm" />
    </slot>
  </div>
</template>
