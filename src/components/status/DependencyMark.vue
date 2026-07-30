<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'

const props = defineProps({
  blockedBy: { type: Number, default: 0 },
  blocks: { type: Number, default: 0 },
  spawnedFrom: { type: String, default: undefined },
  size: { type: String, default: 'md' }
})

const sm = computed(() => props.size === 'sm')
const empty = computed(() => props.blockedBy <= 0 && props.blocks <= 0 && !props.spawnedFrom)

const chipStyle = (tone) => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '2px',
  color: tone,
  font: `var(--weight-medium) ${sm.value ? 'var(--text-2xs)' : 'var(--text-xs)'}/1 var(--font-mono)`
})

const spawnStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '2px',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)'
}))

const wrapStyle = { display: 'inline-flex', alignItems: 'center', gap: 'var(--space-4)' }
const glyphSize = computed(() => (sm.value ? 10 : 11))
</script>

<template>
  <span v-if="!empty" :style="wrapStyle">
    <span
      v-if="blockedBy > 0"
      :title="`${blockedBy} task(s) block this one`"
      :style="chipStyle('var(--status-blocked-fg)')"
    >
      <Icon name="lock" :size="glyphSize" :stroke-width="2" />{{ blockedBy }}
    </span>
    <span
      v-if="blocks > 0"
      :title="`blocks ${blocks} downstream task(s)`"
      :style="chipStyle('var(--text-secondary)')"
    >
      <Icon name="git-fork" :size="glyphSize" :stroke-width="2" />{{ blocks }}
    </span>
    <span v-if="spawnedFrom" :title="`spawned from ${spawnedFrom}`" :style="spawnStyle">
      <Icon name="corner-down-right" :size="10" :stroke-width="2" />{{ spawnedFrom }}
    </span>
  </span>
</template>
