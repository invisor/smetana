<script setup>
import { computed, ref } from 'vue'
import Icon from './Icon.vue'

const props = defineProps({
  modelValue: { type: [String, Number], default: '' },
  /* accepts ['a','b'] or [{value,label}] */
  options: { type: Array, default: () => [] },
  disabled: { type: Boolean, default: false },
  size: { type: String, default: 'md' }
})

const emit = defineEmits(['update:modelValue'])

const focus = ref(false)
const items = computed(() =>
  props.options.map((o) => (typeof o === 'object' && o !== null ? o : { value: o, label: o }))
)

const selectStyle = computed(() => ({
  appearance: 'none',
  WebkitAppearance: 'none',
  height: props.size === 'sm' ? 'var(--control-h-sm)' : 'var(--control-h)',
  padding: '0 var(--space-8) 0 var(--space-4)',
  background: props.disabled ? 'var(--surface-sunken)' : 'var(--surface-raised)',
  color: props.disabled ? 'var(--text-muted)' : 'var(--text-primary)',
  border: `var(--border-w) solid ${focus.value ? 'var(--focus-ring)' : 'var(--border)'}`,
  borderRadius: 'var(--radius-3)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-sans)',
  cursor: props.disabled ? 'not-allowed' : 'default',
  outline: 'none',
  width: '100%'
}))
</script>

<template>
  <div :style="{ position: 'relative', display: 'inline-flex', alignItems: 'center' }">
    <select
      :value="modelValue"
      :disabled="disabled"
      :style="selectStyle"
      @change="emit('update:modelValue', $event.target.value)"
      @focus="focus = true"
      @blur="focus = false"
    >
      <option v-for="o in items" :key="o.value" :value="o.value">{{ o.label }}</option>
    </select>
    <span :style="{ position: 'absolute', right: 'var(--space-3)', color: 'var(--text-muted)', pointerEvents: 'none' }">
      <Icon name="chevron-down" :size="13" />
    </span>
  </div>
</template>
