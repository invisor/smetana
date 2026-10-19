<script setup>
/* A row that folds the content under it away.

   The caption is a real `<button type="button">`, which buys the keyboard
   (Enter, Space), the tab stop and the focus ring `tokens/base.css` draws, and
   `aria-expanded` says which way it points. The content is rendered only while
   open, so what is hidden takes no room and no tab stops.

   Controlled: whoever draws it decides whether it is open (`v-model:open`).
   That is what lets a caller hold it open while something inside is wrong. */
import { computed } from 'vue'
import Icon from './Icon.vue'
import { useInteractive } from './interactive.js'

const props = defineProps({
  label: { type: String, required: true },
  open: { type: Boolean, default: false },
  disabled: { type: Boolean, default: false }
})
const emit = defineEmits(['update:open'])

const { hover, active, handlers } = useInteractive()

const MARK = 12

const wrap = { display: 'flex', flexDirection: 'column', gap: 'var(--space-6)' }

/* `border: 'none'` and never `undefined` on a `<button>`: the user agent would
   hand it back its own outset border. */
const style = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '100%',
  padding: 'var(--space-2) var(--space-3)',
  border: 'none',
  borderRadius: 'var(--radius-2)',
  textAlign: 'left',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-secondary)',
  /* Interaction is a surface step and never a colour change or a transform. */
  background:
    !props.disabled && active.value
      ? 'var(--surface-active)'
      : !props.disabled && hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
  opacity: props.disabled ? 0.7 : undefined,
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

const markStyle = { flex: 'none', color: 'var(--text-muted)' }
</script>

<template>
  <div :style="wrap">
    <button
      type="button"
      :style="style"
      :disabled="disabled"
      :aria-expanded="open"
      v-bind="handlers"
      @click="emit('update:open', !open)"
    >
      <Icon :name="open ? 'chevron-down' : 'chevron-right'" :size="MARK" :style="markStyle" />
      <span>{{ label }}</span>
    </button>
    <slot v-if="open" />
  </div>
</template>
