<script setup>
import { computed, ref } from 'vue'

const props = defineProps({
  modelValue: { type: [String, Number], default: '' },
  placeholder: { type: String, default: '' },
  /* identifier -> mono, prose -> sans, never the reverse */
  mono: { type: Boolean, default: false },
  size: { type: String, default: 'md' },
  invalid: { type: Boolean, default: false },
  disabled: { type: Boolean, default: false },
  readOnly: { type: Boolean, default: false },
  type: { type: String, default: 'text' }
})

const emit = defineEmits(['update:modelValue'])

const focus = ref(false)
/* The field itself, so a caller can put the keyboard in it. A dialog that opens
   for one name and does not focus its own field makes a person click before
   they can type — see `git/NewBranchModal.vue`, which is what asked for this.
   Exposed rather than an `autofocus` prop: attributes fall through to the
   wrapper below, not to the input, so the attribute would land on a `div` and
   do nothing at all. */
const el = ref(null)
/* `select` beside it for the field that opens already filled: a dialog handed
   the name it is about wants the whole of it taken over by the first keystroke,
   and a caret parked at one end would make somebody clear it by hand — see
   `git/RenameBranchModal.vue`, which is what asked for this. It focuses first
   rather than leaving that to the caller, because a selection in an unfocused
   field is invisible and the next key goes somewhere else entirely. */
defineExpose({
  focus: () => el.value?.focus(),
  select: () => {
    el.value?.focus()
    el.value?.select()
  }
})
const borderColor = computed(() =>
  props.invalid ? 'var(--status-failed-fg)' : focus.value ? 'var(--focus-ring)' : 'var(--border)'
)

const wrapStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '100%',
  height: props.size === 'sm' ? 'var(--control-h-sm)' : 'var(--control-h)',
  padding: '0 var(--space-4)',
  background: props.disabled ? 'var(--surface-sunken)' : 'var(--surface-raised)',
  border: `var(--border-w) solid ${borderColor.value}`,
  borderRadius: 'var(--radius-3)',
  boxShadow: focus.value ? 'inset 0 0 0 1px var(--focus-ring)' : 'none',
  transition: 'var(--transition-control)'
}))

const inputStyle = computed(() => ({
  flex: 1,
  minWidth: 0,
  border: 'none',
  outline: 'none',
  background: 'transparent',
  color: props.disabled ? 'var(--text-muted)' : 'var(--text-primary)',
  fontFamily: props.mono ? 'var(--font-mono)' : 'var(--font-sans)',
  fontSize: props.size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)'
}))
</script>

<template>
  <div :style="wrapStyle">
    <span v-if="$slots.prefix" :style="{ color: 'var(--text-muted)', display: 'flex' }">
      <slot name="prefix" />
    </span>
    <input
      ref="el"
      :type="type"
      :value="modelValue"
      :placeholder="placeholder"
      :disabled="disabled"
      :readonly="readOnly"
      :aria-invalid="invalid || undefined"
      :style="inputStyle"
      @input="emit('update:modelValue', $event.target.value)"
      @focus="focus = true"
      @blur="focus = false"
    />
  </div>
</template>
