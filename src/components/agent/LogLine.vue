<script setup>
import { computed } from 'vue'
import AnsiText from './AnsiText.vue'

const props = defineProps({
  time: { type: String, default: '' },
  text: { type: String, default: '' },
  segments: { type: Array, default: null },
  level: { type: String, default: undefined },
  match: { type: Boolean, default: false }
})

const tone = computed(() =>
  props.level === 'error' ? 'var(--status-failed-fg)' : props.level === 'warn' ? 'var(--attn-loud)' : undefined
)

const style = computed(() => ({
  display: 'flex',
  gap: 'var(--space-5)',
  minHeight: 'var(--log-line-h)',
  lineHeight: 'var(--log-line-h)',
  padding: '0 var(--space-5)',
  background: props.match ? 'var(--editor-match-highlight)' : 'transparent',
  font: 'var(--weight-regular) var(--text-code-size)/var(--log-line-h) var(--font-mono)',
  whiteSpace: 'pre-wrap',
  wordBreak: 'break-word',
  color: tone.value
}))

const timeStyle = {
  flex: '0 0 auto',
  color: 'var(--editor-line-number)',
  userSelect: 'none',
  fontVariantNumeric: 'tabular-nums'
}
</script>

<template>
  <div :style="style">
    <span v-if="time" :style="timeStyle">{{ time }}</span>
    <span :style="{ flex: 1, minWidth: 0 }">
      <AnsiText :text="text" :segments="segments" />
    </span>
  </div>
</template>
