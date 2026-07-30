<script setup>
import { computed, ref } from 'vue'

const props = defineProps({
  label: { type: String, required: true },
  shortcut: { type: String, default: '' },
  side: { type: String, default: 'top' }
})

const open = ref(false)

const pos = computed(() => {
  switch (props.side) {
    case 'bottom':
      return { top: 'calc(100% + 6px)', left: '50%', transform: 'translateX(-50%)' }
    case 'left':
      return { right: 'calc(100% + 6px)', top: '50%', transform: 'translateY(-50%)' }
    case 'right':
      return { left: 'calc(100% + 6px)', top: '50%', transform: 'translateY(-50%)' }
    default:
      return { bottom: 'calc(100% + 6px)', left: '50%', transform: 'translateX(-50%)' }
  }
})

const tipStyle = computed(() => ({
  position: 'absolute',
  zIndex: 'var(--z-tooltip)',
  ...pos.value,
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  whiteSpace: 'nowrap',
  padding: 'var(--space-2) var(--space-4)',
  background: 'var(--surface-overlay)',
  fontFamily: 'var(--font-sans)',
  color: 'var(--text-primary)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-2)',
  boxShadow: 'var(--shadow-overlay)',
  fontSize: 'var(--text-xs)'
}))
</script>

<template>
  <span
    :style="{ position: 'relative', display: 'inline-flex' }"
    @mouseenter="open = true"
    @mouseleave="open = false"
    @focusin="open = true"
    @focusout="open = false"
  >
    <slot />
    <span v-if="open" role="tooltip" :style="tipStyle">
      {{ label }}
      <kbd v-if="shortcut" :style="{ color: 'var(--text-muted)', fontSize: 'var(--text-2xs)' }">{{ shortcut }}</kbd>
    </span>
  </span>
</template>
