<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import { useInteractive } from '../core/interactive.js'

/* What the scope bar keeps of the search: a button saying the search exists and
   which key opens it. Everything else moved into the palette, because the bar
   has no width to give a list that has to hold a whole task title — a 360px
   dropdown hung off the right edge over a 260px field sitting where flex left it
   read as two unrelated objects, and that was the container's fault rather than
   the search's. */
defineEmits(['open'])

const { hover, handlers } = useInteractive()

const button = ref(null)

/* The palette hands the keyboard back here when it closes, which is the one
   thing a modal owes: Esc must not drop somebody at the top of the document. */
defineExpose({ focus: () => button.value?.focus() })

const buttonStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '200px',
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-4)',
  background: hover.value ? 'var(--surface-hover)' : 'var(--surface)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)',
  color: 'var(--text-muted)',
  cursor: 'pointer',
  transition: 'var(--transition-control)'
}))

const labelStyle = {
  flex: 1,
  minWidth: 0,
  textAlign: 'left',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-sans)'
}

/* The one chip on the bar. Mono, because a key is an identifier. */
const chipStyle = {
  flex: '0 0 auto',
  padding: '0 var(--space-2)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-2)',
  font: 'var(--weight-regular) var(--text-2xs)/1.6 var(--font-mono)'
}
</script>

<template>
  <button
    ref="button"
    :style="buttonStyle"
    type="button"
    aria-label="Search tasks"
    aria-keyshortcuts="Meta+K Control+K"
    v-bind="handlers"
    @click="$emit('open')"
  >
    <Icon name="search" :size="13" />
    <span :style="labelStyle">Search tasks</span>
    <span :style="chipStyle">⌘K</span>
  </button>
</template>
