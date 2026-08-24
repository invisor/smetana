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

/* The `detail` slot: one line of machine words under the sentence of human
   ones. It is a slot rather than a second string prop because what goes in it
   is never this component's to word — it is whatever the thing that failed
   said, and the caller is the only party that has it.

   Mono, because everything in it is an identifier or a diagnostic, and one
   line with an ellipsis because there is no bottom to how long such a line can
   be: bd will hand over a paragraph as readily as a sentence, and an empty
   state that grows to fit one has stopped being an empty state. What is cut off
   is not lost — the whole of it is what the second button hands to an agent.

   Muted rather than `--status-failed-fg`, even under `tone="error"`: the title
   above is already the loud part, and this stays information. */
const detailStyle = computed(() => ({
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-xs)',
  color: 'var(--text-muted)',
  maxWidth: '100%',
  minWidth: 0,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
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
    <div v-if="$slots.detail" :style="detailStyle">
      <slot name="detail" />
    </div>
    <div v-if="$slots.action" :style="{ marginTop: 'var(--space-3)' }">
      <slot name="action" />
    </div>
  </div>
</template>
