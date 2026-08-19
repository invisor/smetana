<script setup>
import { computed, mergeProps, toRef, useAttrs } from 'vue'
import { useInteractive } from './interactive.js'
import Icon from './Icon.vue'
import Tooltip from './Tooltip.vue'

/* The hint is `Tooltip`, never the native `title`, and it is here rather than at
   every call site so that every icon-only button in the app has one by
   construction. `label` is required, so there is always something to say.

   Attributes do not fall through to the wrapper: `Tooltip`'s span is the root
   now, and a caller sizing the control — `Tab`'s 16px close, `CodeBlock`'s 18px
   copy, the setup gear in the left panel's header — means the
   button, not the box around it. They are merged with `mergeProps` rather than
   spread into one object, which is what fallthrough itself does and the only
   form that keeps both sides of a collision: object spread would drop this
   component's own hover tracking the moment a caller passed `@mouseenter`.
   `$attrs` goes second so a caller's value wins, again as fallthrough did. */
defineOptions({ inheritAttrs: false })

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

const attrs = useAttrs()
const buttonAttrs = computed(() => mergeProps(handlers, attrs))

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
  <Tooltip :label="label">
    <button
      type="button"
      :disabled="disabled"
      :aria-label="label"
      :aria-pressed="selected || undefined"
      :style="style"
      v-bind="buttonAttrs"
    >
      <Icon :name="icon" :size="glyphSize" />
    </button>
  </Tooltip>
</template>
