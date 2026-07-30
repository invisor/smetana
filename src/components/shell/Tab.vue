<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'

/* Tab kinds
   pinned  - Chat and Kanban. Always first, no close affordance.
   file    - a normal opened file tab, closable, may be dirty or agent-locked.
   preview - single-click temporary tab, replaced by the next preview.
             VS Code uses italics; here the tab label sits on a DOTTED baseline
             rule and the tab has no top accent even when active, so "temporary"
             reads as an unfinished edge rather than a different font. */
const props = defineProps({
  kind: { type: String, default: 'file' },
  label: { type: String, required: true },
  icon: { type: String, default: undefined },
  active: { type: Boolean, default: false },
  dirty: { type: Boolean, default: false },
  readOnly: { type: Boolean, default: false }
})

const emit = defineEmits(['select', 'close'])

const hover = ref(false)
const preview = computed(() => props.kind === 'preview')

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  flex: '0 0 auto',
  height: 'var(--tab-h)',
  padding: '0 var(--space-4) 0 var(--space-5)',
  position: 'relative',
  background: props.active ? 'var(--surface-raised)' : hover.value ? 'var(--surface-hover)' : 'transparent',
  color: props.active ? 'var(--text-primary)' : 'var(--text-secondary)',
  borderRight: 'var(--border-w) solid var(--border-subtle)',
  boxShadow: props.active && !preview.value ? 'inset 0 2px 0 0 var(--text-primary)' : 'none',
  font: `var(--weight-regular) var(--text-sm)/1 ${props.kind === 'pinned' ? 'var(--font-sans)' : 'var(--font-mono)'}`,
  cursor: 'default',
  maxWidth: '200px',
  transition: 'var(--transition-control)'
}))

const labelStyle = computed(() => ({
  minWidth: 0,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  borderBottom: preview.value ? '1px dotted var(--text-muted)' : '1px solid transparent',
  paddingBottom: '1px',
  opacity: preview.value ? 0.85 : 1
}))

const onClose = (e) => {
  e.stopPropagation()
  emit('close')
}
</script>

<template>
  <div
    role="tab"
    :aria-selected="active"
    :tabindex="active ? 0 : -1"
    :data-tab-kind="kind"
    :style="style"
    @click="emit('select')"
    @mouseenter="hover = true"
    @mouseleave="hover = false"
  >
    <Icon v-if="icon" :name="icon" :size="13" :style="{ color: readOnly ? 'var(--text-muted)' : undefined }" />
    <span :style="labelStyle">{{ label }}</span>
    <Icon
      v-if="readOnly"
      name="lock"
      :size="11"
      title="Read-only while an agent is working"
      :style="{ color: 'var(--status-blocked-fg)' }"
    />
    <span
      v-if="dirty"
      title="Unsaved changes"
      :style="{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--attn-loud)', flex: '0 0 auto' }"
    />
    <span v-if="kind !== 'pinned'" :style="{ width: '16px', display: 'flex', justifyContent: 'center' }">
      <IconButton
        v-if="hover || active"
        icon="x"
        :label="`Close ${label}`"
        size="sm"
        :style="{ width: '16px', height: '16px' }"
        @click="onClose"
      />
    </span>
  </div>
</template>
