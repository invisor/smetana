<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'

/* Tab kinds
   pinned  - Terminal and Kanban. Always first, no close affordance.
   file    - a normal opened file tab, closable, may be dirty or read-only.
   preview - single-click temporary tab, replaced by the next preview.
             Italic, as in VS Code: the mechanic is VS Code's, and so is its
             signal — muscle memory is the point. A departure from the design
             system, which chose a dashed line here; the debt is recorded in the
             spec
             docs/superpowers/specs/2026-08-01-file-tree-and-editor-design.md.
             A double click on a tab drops the temporary flag — hence
             `promote`. */
const props = defineProps({
  kind: { type: String, default: 'file' },
  label: { type: String, required: true },
  icon: { type: String, default: undefined },
  /* EXPERIMENT (variant A): a file tab's icon arrives as a `data:` URL from the
     Catppuccin set, while a diff tab still names a lucide glyph above. Two props
     rather than one because the two are drawn by different elements — an `<img>`
     cannot take a token colour and an `<svg>` cannot take a URL. */
  iconUrl: { type: String, default: undefined },
  active: { type: Boolean, default: false },
  dirty: { type: Boolean, default: false },
  readOnly: { type: Boolean, default: false },
  /* Why the tab is locked, in words. There is one lock but several reasons for
     it — the file cannot be opened as text, an agent holds the file — and the
     hint has to name the one that happened. The default names none of them:
     better to say little than to name the wrong one. */
  readOnlyHint: { type: String, default: 'Read-only' }
})

const emit = defineEmits(['select', 'close', 'promote'])

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
  boxShadow: props.active ? 'inset 0 2px 0 0 var(--text-primary)' : 'none',
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
  fontStyle: preview.value ? 'italic' : 'normal'
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
    @dblclick="emit('promote')"
    @mouseenter="hover = true"
    @mouseleave="hover = false"
  >
    <img
      v-if="iconUrl"
      :src="iconUrl"
      alt=""
      width="15"
      height="15"
      :style="{ display: 'block', flex: '0 0 auto' }"
    />
    <Icon v-else-if="icon" :name="icon" :size="13" :style="{ color: readOnly ? 'var(--text-muted)' : undefined }" />
    <span :style="labelStyle">{{ label }}</span>
    <!-- The hint hangs on the wrapper rather than on Icon: Icon's `title` is a
         prop that becomes an aria-label and never reaches the pointer — the SVG
         is left with nothing to hover. The wrapper gives it to the mouse, Icon
         leaves the reason to the screen reader. -->
    <Tooltip v-if="readOnly" :label="readOnlyHint" :style="{ flex: '0 0 auto' }">
      <Icon name="lock" :size="11" :title="readOnlyHint" :style="{ color: 'var(--status-blocked-fg)' }" />
    </Tooltip>
    <Tooltip v-if="dirty" label="Unsaved changes" :style="{ flex: '0 0 auto' }">
      <span
        :style="{ width: '6px', height: '6px', borderRadius: '50%', background: 'var(--attn-loud)', flex: '0 0 auto' }"
      />
    </Tooltip>
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
