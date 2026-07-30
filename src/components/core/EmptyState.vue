<script setup>
import { computed } from 'vue'
import Icon from './Icon.vue'

/* Terse and factual: "Empty" / "Nothing in ready." No apologies, no emoji. */
const props = defineProps({
  icon: { type: String, default: 'inbox' },
  title: { type: String, required: true },
  description: { type: String, default: '' },
  tone: { type: String, default: 'neutral' },
  compact: { type: Boolean, default: false }
})

const err = computed(() => props.tone === 'error')

const style = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 'var(--space-4)',
  textAlign: 'center',
  fontFamily: 'var(--font-sans)',
  padding: props.compact ? 'var(--space-6)' : 'var(--space-9) var(--space-6)',
  color: 'var(--text-muted)'
}))

const titleStyle = computed(() => ({
  color: err.value ? 'var(--status-failed-fg)' : 'var(--text-secondary)',
  fontSize: 'var(--text-sm)',
  fontWeight: 'var(--weight-medium)'
}))
</script>

<template>
  <div :style="style">
    <Icon
      :name="err ? 'triangle-alert' : icon"
      :size="compact ? 16 : 20"
      :style="{ color: err ? 'var(--status-failed-fg)' : 'var(--text-muted)' }"
    />
    <div :style="titleStyle">{{ title }}</div>
    <div
      v-if="description"
      :style="{ fontSize: 'var(--text-xs)', maxWidth: '280px', lineHeight: 'var(--leading-normal)' }"
    >
      {{ description }}
    </div>
    <div v-if="$slots.action" :style="{ marginTop: 'var(--space-3)' }">
      <slot name="action" />
    </div>
  </div>
</template>
