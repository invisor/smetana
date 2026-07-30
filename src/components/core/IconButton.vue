<script setup>
import { computed, toRef } from 'vue'
import { useInteractive } from './interactive.js'
import Icon from './Icon.vue'

const props = defineProps({
  icon: { type: String, required: true },
  /* Icon-only, so the label is the accessible name — never optional. */
  label: { type: String, required: true },
  size: { type: String, default: 'md' },
  variant: { type: String, default: 'ghost' },
  disabled: { type: Boolean, default: false },
  selected: { type: Boolean, default: false }
})

const { hover, active, handlers } = useInteractive(toRef(props, 'disabled'))

const box = computed(() =>
  props.size === 'sm' ? 'var(--control-h-sm)' : props.size === 'lg' ? 'var(--control-h-lg)' : 'var(--control-h)'
)
const glyphSize = computed(() => (props.size === 'sm' ? 13 : props.size === 'lg' ? 18 : 15))

const bg = computed(() => {
  if (active.value) return 'var(--surface-active)'
  if (hover.value) return 'var(--surface-hover)'
  if (props.selected) return 'var(--surface-selected)'
  return props.variant === 'solid' ? 'var(--action-secondary-bg)' : 'transparent'
})

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: box.value,
  height: box.value,
  color: props.disabled ? 'var(--text-muted)' : props.selected ? 'var(--text-primary)' : 'var(--text-secondary)',
  background: bg.value,
  border: `var(--border-w) solid ${props.variant === 'solid' ? 'var(--border)' : 'transparent'}`,
  borderRadius: 'var(--radius-3)',
  cursor: props.disabled ? 'not-allowed' : 'default',
  opacity: props.disabled ? 0.6 : 1,
  transition: 'var(--transition-control)',
  padding: 0
}))
</script>

<template>
  <button
    type="button"
    :disabled="disabled"
    :aria-label="label"
    :title="label"
    :aria-pressed="selected || undefined"
    :style="style"
    v-on="handlers"
  >
    <Icon :name="icon" :size="glyphSize" />
  </button>
</template>
