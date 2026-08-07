<script setup>
/* What a run's agent has taken, in the panel on the right.

   A run is the one kind of session with more than one piece of work behind it,
   so its row in the agents panel cannot open a card the way an edit's row can —
   there are several, and picking between them is the person's. This is that
   choice, and picking a row here is what opens the card below and highlights it
   on the board.

   There is no channel saying "this session claimed that issue": the list is
   reconstructed in `src/stores/terminals.js` from the run's session and what
   the tracker holds in progress. A title the tracker has not got yet is left
   out rather than faked — the id is the part that is certainly true, and it is
   the part a person matches against the terminal. */
import { ref } from 'vue'

const props = defineProps({
  /* `{ id, title }`, title optional. Ordered by the store; this draws them as
     they come so a second issue appearing does not reorder the first. */
  tasks: { type: Array, default: () => [] },
  /* The one open in the inspector below, if it is one of these. */
  selectedId: { type: String, default: null }
})

defineEmits(['select'])

/* The list keeps the hovered id itself rather than asking useInteractive per
   row: it tracks one control at a time. Press is not tracked — a row is not a
   button, it is a place, the same reasoning ProjectList.vue uses. */
const hovered = ref(null)

const body = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }

const eyebrow = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}

/* The rows run the full width of the panel and are indented back to its edge:
   the inspector column is padded by its container, and a highlighted row that
   stopped short of the edge would read as a card rather than as a list. */
const listStyle = {
  display: 'flex',
  flexDirection: 'column',
  margin: '0 calc(-1 * var(--panel-pad))'
}

const rowStyle = (task) => ({
  display: 'flex',
  alignItems: 'baseline',
  gap: 'var(--space-3)',
  minHeight: 'var(--row-h)',
  padding: '0 var(--panel-pad)',
  background:
    task.id === props.selectedId
      ? 'var(--surface-raised)'
      : hovered.value === task.id
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: 'default',
  transition: 'var(--transition-control)'
})

/* The id is an identifier and stays mono at its natural width; the title is
   prose and takes what is left, ellipsised. Without minWidth 0 a long title
   would refuse to shrink below its own text and push the id out of the panel. */
const idStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  flex: 'none'
}

const titleStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-secondary)',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
</script>

<template>
  <div :style="body">
    <span :style="eyebrow">Taken by this agent</span>
    <div :style="listStyle">
      <div
        v-for="task in tasks"
        :key="task.id"
        :style="rowStyle(task)"
        @mouseenter="hovered = task.id"
        @mouseleave="hovered = null"
        @click="$emit('select', task.id)"
      >
        <span :style="idStyle">{{ task.id }}</span>
        <span v-if="task.title" :style="titleStyle">{{ task.title }}</span>
      </div>
    </div>
  </div>
</template>
