<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'

const T = {
  info: { icon: 'info', fg: 'var(--text-secondary)' },
  success: { icon: 'check', fg: 'var(--status-done-fg)' },
  warning: { icon: 'triangle-alert', fg: 'var(--attn-loud)' },
  error: { icon: 'x', fg: 'var(--status-failed-fg)' }
}

const props = defineProps({
  tone: { type: String, default: 'info' },
  /* "claude-1 needs you" / "bd-a1b2 · worktree name collision · 4m" */
  title: { type: String, required: true },
  description: { type: String, default: '' },
  closable: { type: Boolean, default: true }
})

defineEmits(['close'])

const t = computed(() => T[props.tone] || T.info)

const style = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: 'var(--space-4)',
  width: '320px',
  padding: 'var(--space-4) var(--space-4) var(--space-4) var(--space-5)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-body-size)/var(--leading-normal) var(--font-sans)',
  background: 'var(--surface-overlay)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-4)',
  boxShadow: 'var(--shadow-overlay)'
}
</script>

<template>
  <div role="status" :style="style">
    <Icon :name="t.icon" :size="14" :style="{ color: t.fg, marginTop: '2px' }" />
    <div :style="{ flex: 1, minWidth: 0 }">
      <div :style="{ fontSize: 'var(--text-sm)', fontWeight: 'var(--weight-medium)' }">{{ title }}</div>
      <div
        v-if="description"
        :style="{ fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', marginTop: '2px' }"
      >{{ description }}</div>
      <div v-if="$slots.action" :style="{ marginTop: 'var(--space-4)' }">
        <slot name="action" />
      </div>
    </div>
    <IconButton v-if="closable" icon="x" label="Dismiss" size="sm" @click="$emit('close')" />
  </div>
</template>
