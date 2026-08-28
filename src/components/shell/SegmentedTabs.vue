<script setup>
/* A panel's own tab row: micro type, because these are section names and not
   open files. It sits directly under a panel header, at the top of what it
   scopes rather than at the far end of it, and it is drawn as segments rather
   than as full-height tabs.

   One component for both side columns rather than one row each. The left
   column had the only copy, written inline in `DesktopApp.vue`; the right
   column wanted the same row, and a second copy of these two style objects a
   thousand lines away from the first — obliged to match, with nothing
   mechanical holding them together — is exactly the pair that drifts. The
   values here are the left column's, moved rather than rewritten.

   The inset rule and the raised fill an earlier version had are gone with the
   position: a rule under a tab was that row's answer to sitting against the
   column's edge, and a segmented row marks its active segment by fill. The
   focus ring stays — it was kept explicitly at the design review, and these are
   the one control in the left column that has one, so it is left to the
   stylesheet's own `:focus-visible` rather than suppressed here. */
import { ref } from 'vue'

const props = defineProps({
  /* `{ id, label }` each. The caller owns the list and its order; this draws
     whatever it is handed, which is what lets two columns with different
     vocabularies share one row. */
  tabs: { type: Array, default: () => [] },
  modelValue: { type: String, default: null }
})

const emit = defineEmits(['update:modelValue'])

const hovered = ref(null)

const barStyle = {
  display: 'flex',
  alignItems: 'stretch',
  gap: 'var(--space-1)',
  flex: '0 0 auto',
  padding: 'var(--space-2) var(--space-3)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}

const segmentStyle = (tab) => {
  const active = props.modelValue === tab.id
  return {
    flex: 1,
    minWidth: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    /* The handoff draws a 22px segment; this is `--control-h-sm`, which is 24
       comfortable and 20 compact. The token rather than the number, because a
       literal would be the one height in a side column that neither density
       nor the app-wide font size reaches — and 22 is inside the two anyway. */
    height: 'var(--control-h-sm)',
    borderRadius: 'var(--radius-2)',
    font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
    letterSpacing: 'var(--tracking-caps)',
    textTransform: 'uppercase',
    color: active ? 'var(--text-primary)' : 'var(--text-muted)',
    background: active
      ? 'var(--surface-selected)'
      : hovered.value === tab.id
        ? 'var(--surface-hover)'
        : 'transparent',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}
</script>

<template>
  <div role="tablist" :style="barStyle">
    <div
      v-for="tab in tabs"
      :key="tab.id"
      role="tab"
      :aria-selected="modelValue === tab.id"
      :tabindex="modelValue === tab.id ? 0 : -1"
      :style="segmentStyle(tab)"
      @click="emit('update:modelValue', tab.id)"
      @mouseenter="hovered = tab.id"
      @mouseleave="hovered = null"
    >
      {{ tab.label }}
    </div>
  </div>
</template>
