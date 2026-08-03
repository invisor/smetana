<script setup>
import { computed, toRef } from 'vue'
import { useInteractive } from './interactive.js'
import Icon from './Icon.vue'

/* The primary action is ink on paper — a dark chip in light, a light chip in
   dark — with no brand hue at all. Every saturated hue is spoken for by status. */
const V = {
  primary: { bg: 'var(--action-primary-bg)', bgH: 'var(--action-primary-bg-hover)', bgA: 'var(--action-primary-bg-active)', fg: 'var(--action-primary-fg)', bd: 'transparent' },
  secondary: { bg: 'var(--action-secondary-bg)', bgH: 'var(--action-secondary-bg-hover)', bgA: 'var(--action-secondary-bg-active)', fg: 'var(--text-primary)', bd: 'var(--border)' },
  ghost: { bg: 'transparent', bgH: 'var(--surface-hover)', bgA: 'var(--surface-active)', fg: 'var(--text-secondary)', bd: 'transparent' },
  danger: { bg: 'var(--action-danger-bg)', bgH: 'var(--action-danger-bg-hover)', bgA: 'var(--action-danger-bg-active)', fg: 'var(--action-danger-fg)', bd: 'transparent' }
}
const H = { sm: 'var(--control-h-sm)', md: 'var(--control-h)', lg: 'var(--control-h-lg)' }

const props = defineProps({
  variant: { type: String, default: 'secondary' },
  size: { type: String, default: 'md' },
  icon: { type: String, default: undefined },
  iconEnd: { type: String, default: undefined },
  disabled: { type: Boolean, default: false },
  selected: { type: Boolean, default: false },
  fullWidth: { type: Boolean, default: false },
  type: { type: String, default: 'button' }
})

const { hover, active, handlers } = useInteractive(toRef(props, 'disabled'))
const v = computed(() => V[props.variant] || V.secondary)
const iconSize = computed(() => (props.size === 'sm' ? 13 : 14))

const bg = computed(() => {
  if (props.disabled) return 'var(--surface-sunken)'
  if (active.value) return v.value.bgA
  if (hover.value) return v.value.bgH
  if (props.selected) return 'var(--surface-selected)'
  return v.value.bg
})

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: 'var(--space-3)',
  height: H[props.size],
  padding: props.size === 'sm' ? '0 var(--space-4)' : '0 var(--space-5)',
  font: `var(--weight-medium) ${props.size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)'}/1 var(--font-sans)`,
  letterSpacing: 'var(--tracking-tight)',
  color: props.disabled ? 'var(--text-muted)' : v.value.fg,
  background: bg.value,
  border: `var(--border-w) solid ${props.disabled ? 'var(--border-subtle)' : v.value.bd}`,
  borderRadius: 'var(--radius-3)',
  cursor: props.disabled ? 'not-allowed' : 'default',
  opacity: props.disabled ? 0.7 : 1,
  width: props.fullWidth ? '100%' : undefined,
  transition: 'var(--transition-control)',
  whiteSpace: 'nowrap'
}))
</script>

<template>
  <button
    :type="type"
    :disabled="disabled"
    :aria-pressed="selected || undefined"
    :style="style"
    v-bind="handlers"
  >
    <Icon v-if="icon" :name="icon" :size="iconSize" />
    <span v-if="$slots.default"><slot /></span>
    <Icon v-if="iconEnd" :name="iconEnd" :size="iconSize" />
  </button>
</template>
