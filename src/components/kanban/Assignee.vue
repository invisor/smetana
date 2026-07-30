<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'

const props = defineProps({
  kind: { type: String, default: 'unassigned' },
  name: { type: String, default: '' }
})

const icon = computed(() =>
  props.kind === 'agent' ? 'bot' : props.kind === 'human' ? 'user' : 'circle-dashed'
)

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  color: props.kind === 'unassigned' ? 'var(--text-muted)' : 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)'
}))
</script>

<template>
  <span :style="style">
    <Icon :name="icon" :size="11" />{{ name || 'unassigned' }}
  </span>
</template>
