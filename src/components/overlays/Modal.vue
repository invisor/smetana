<script setup>
import { computed } from 'vue'
import IconButton from '../core/IconButton.vue'

/* Destructive confirms state the consequence, not an apology:
   "Discard worktree?" / "wt/bd-a1b2 has 3 uncommitted files and 1 agent still running." */
const props = defineProps({
  open: { type: Boolean, default: true },
  title: { type: String, required: true },
  description: { type: String, default: '' },
  width: { type: Number, default: 440 },
  closable: { type: Boolean, default: true }
})

defineEmits(['close'])

const scrimStyle = {
  position: 'absolute',
  inset: 0,
  zIndex: 'var(--z-modal)',
  background: 'var(--overlay-scrim)',
  display: 'flex',
  alignItems: 'flex-start',
  justifyContent: 'center',
  paddingTop: '8vh'
}

const dialogStyle = computed(() => ({
  width: `${props.width}px`,
  maxWidth: '92%',
  background: 'var(--surface-overlay)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-body-size)/var(--leading-normal) var(--font-sans)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-4)',
  boxShadow: 'var(--shadow-modal)'
}))

const footerStyle = {
  display: 'flex',
  justifyContent: 'flex-end',
  gap: 'var(--space-4)',
  padding: 'var(--space-4) var(--space-5)',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)'
}
</script>

<template>
  <div v-if="open" :style="scrimStyle">
    <div role="dialog" aria-modal="true" :aria-label="title" :style="dialogStyle">
      <div :style="{ display: 'flex', alignItems: 'flex-start', gap: 'var(--space-5)', padding: 'var(--space-5) var(--space-5) var(--space-4)' }">
        <div :style="{ flex: 1, minWidth: 0 }">
          <div :style="{ fontSize: 'var(--text-md)', fontWeight: 'var(--weight-semibold)' }">{{ title }}</div>
          <div
            v-if="description"
            :style="{ marginTop: 'var(--space-2)', fontSize: 'var(--text-sm)', color: 'var(--text-secondary)' }"
          >{{ description }}</div>
        </div>
        <IconButton v-if="closable" icon="x" label="Close" size="sm" @click="$emit('close')" />
      </div>
      <div v-if="$slots.default" :style="{ padding: '0 var(--space-5) var(--space-5)' }">
        <slot />
      </div>
      <div v-if="$slots.footer" :style="footerStyle">
        <slot name="footer" />
      </div>
    </div>
  </div>
</template>
