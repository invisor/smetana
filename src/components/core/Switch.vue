<script setup>
import { computed } from 'vue'

const props = defineProps({
  modelValue: { type: Boolean, default: false },
  disabled: { type: Boolean, default: false },
  label: { type: String, default: '' }
})

const emit = defineEmits(['update:modelValue'])

const toggle = () => {
  if (!props.disabled) emit('update:modelValue', !props.modelValue)
}

const labelStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  cursor: props.disabled ? 'not-allowed' : 'default',
  opacity: props.disabled ? 0.55 : 1,
  fontSize: 'var(--text-sm)'
}))

const trackStyle = computed(() => ({
  position: 'relative',
  width: '26px',
  height: '14px',
  flex: '0 0 auto',
  borderRadius: 'var(--radius-pill)',
  background: props.modelValue ? 'var(--action-primary-bg)' : 'var(--surface-sunken)',
  border: `var(--border-w) solid ${props.modelValue ? 'var(--action-primary-bg)' : 'var(--border-strong)'}`,
  transition: 'var(--transition-control)'
}))

const knobStyle = computed(() => ({
  position: 'absolute',
  top: '2px',
  left: props.modelValue ? '14px' : '2px',
  width: '8px',
  height: '8px',
  borderRadius: 'var(--radius-pill)',
  background: props.modelValue ? 'var(--action-primary-fg)' : 'var(--text-muted)',
  transition: 'left var(--dur-fast) var(--ease-out)'
}))
</script>

<template>
  <label :style="labelStyle">
    <span
      role="switch"
      :aria-checked="modelValue"
      :aria-label="label || undefined"
      :tabindex="disabled ? -1 : 0"
      :style="trackStyle"
      @click="toggle"
      @keydown.space.prevent="toggle"
      @keydown.enter.prevent="toggle"
    >
      <span :style="knobStyle" />
    </span>
    <span v-if="label">{{ label }}</span>
  </label>
</template>
