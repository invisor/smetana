<script setup>
import { computed, ref } from 'vue'

const props = defineProps({
  modelValue: { type: String, default: '' },
  placeholder: { type: String, default: '' },
  /* Height is the browser's own `rows`, not a computed one: a field measured
     in lines of its own type stays right when the type scale or the density
     changes, and there is no token that means "four lines". */
  rows: { type: Number, default: 4 },
  /* identifier -> mono, prose -> sans, never the reverse */
  mono: { type: Boolean, default: false },
  invalid: { type: Boolean, default: false },
  disabled: { type: Boolean, default: false },
  readOnly: { type: Boolean, default: false }
})

const emit = defineEmits(['update:modelValue'])

const focus = ref(false)
const borderColor = computed(() =>
  props.invalid ? 'var(--status-failed-fg)' : focus.value ? 'var(--focus-ring)' : 'var(--border)'
)

const style = computed(() => ({
  display: 'block',
  width: '100%',
  padding: 'var(--space-3) var(--space-4)',
  background: props.disabled ? 'var(--surface-sunken)' : 'var(--surface-raised)',
  color: props.disabled ? 'var(--text-muted)' : 'var(--text-primary)',
  border: `var(--border-w) solid ${borderColor.value}`,
  borderRadius: 'var(--radius-3)',
  boxShadow: focus.value ? 'inset 0 0 0 1px var(--focus-ring)' : 'none',
  fontFamily: props.mono ? 'var(--font-mono)' : 'var(--font-sans)',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  outline: 'none',
  /* The grip is a browser-drawn control with no place in this system, and a
     field a person can drag out of the dialog it sits in is worse than one
     that scrolls. */
  resize: 'none',
  transition: 'var(--transition-control)'
}))
</script>

<template>
  <textarea
    :value="modelValue"
    :placeholder="placeholder"
    :rows="rows"
    :disabled="disabled"
    :readonly="readOnly"
    :aria-invalid="invalid || undefined"
    :style="style"
    @input="emit('update:modelValue', $event.target.value)"
    @focus="focus = true"
    @blur="focus = false"
  />
</template>
