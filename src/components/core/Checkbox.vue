<script setup>
import { computed } from 'vue'
import Icon from './Icon.vue'

const props = defineProps({
  modelValue: { type: Boolean, default: false },
  indeterminate: { type: Boolean, default: false },
  disabled: { type: Boolean, default: false },
  label: { type: String, default: '' }
})

const emit = defineEmits(['update:modelValue'])

const on = computed(() => props.modelValue || props.indeterminate)

const labelStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  cursor: props.disabled ? 'not-allowed' : 'default',
  opacity: props.disabled ? 0.55 : 1,
  fontSize: 'var(--text-sm)'
}))

const boxStyle = computed(() => ({
  width: '14px',
  height: '14px',
  flex: '0 0 auto',
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'center',
  background: on.value ? 'var(--action-primary-bg)' : 'var(--surface-raised)',
  color: on.value ? 'var(--action-primary-fg)' : 'transparent',
  border: `var(--border-w) solid ${on.value ? 'var(--action-primary-bg)' : 'var(--border-strong)'}`,
  borderRadius: 'var(--radius-1)',
  transition: 'var(--transition-control)'
}))

const inputStyle = { position: 'absolute', opacity: 0, width: 0, height: 0 }
</script>

<template>
  <label :style="labelStyle">
    <span :style="boxStyle">
      <Icon :name="indeterminate ? 'minus' : 'check'" :size="11" :stroke-width="2.5" />
    </span>
    <input
      type="checkbox"
      :checked="modelValue"
      :disabled="disabled"
      :indeterminate="indeterminate"
      :style="inputStyle"
      @change="emit('update:modelValue', $event.target.checked)"
    />
    <span v-if="label">{{ label }}</span>
  </label>
</template>
