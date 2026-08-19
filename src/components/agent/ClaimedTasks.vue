<script setup>
/* What a run's agent has taken, in the panel on the right.

   A run is the one kind of session with more than one piece of work behind it,
   so its row in the agents panel cannot open a card the way an edit's row can —
   there are several, and picking between them is the person's. This is that
   choice, and picking a row here is what opens the card below and highlights it
   on the board.

   There is no channel saying "this session claimed that issue": the list is
   reconstructed in `src/stores/terminals.js` from the run's session and what
   the tracker holds in progress. Because it is built *out of* the tracker's own
   issues, an id here is always one the tracker has — a row for an issue nobody
   has heard of is impossible by construction, and the caller has an issue to
   read a title off every time. The `v-if` on the title is not for that case: it
   guards an issue that arrived without one, which bd should never send and
   which would otherwise draw a gap where a title goes. */
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
   button, it is a place, the same reasoning AgentList.vue uses. */
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

/* The row centres its content and the baseline grouping happens one box in,
   which is the shape AgentList already has. It is not
   interchangeable with putting `baseline` here: a single-line flex container
   whose cross size comes from the row height stretches its line to that height,
   and baseline-aligned items sit at the line's *start* — `align-items:
   baseline` cannot centre anything. The row drew its words flush against the
   top edge of the hover band with a bar of empty background under them, at both
   densities and in both themes, because it is geometry rather than colour. */
const rowStyle = (task) => ({
  display: 'flex',
  alignItems: 'center',
  height: 'var(--row-h)',
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

/* The inner box is where baseline belongs: it is content-sized, so its line box
   is the text's own and the two families line up on it. Mono and sans differ in
   where their glyphs sit within the line, and centring the pair instead would
   leave the id and the title at visibly different heights. */
const pairStyle = {
  display: 'flex',
  alignItems: 'baseline',
  gap: 'var(--space-3)',
  minWidth: 0,
  overflow: 'hidden',
  whiteSpace: 'nowrap'
}

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
        <span :style="pairStyle">
          <span :style="idStyle">{{ task.id }}</span>
          <span v-if="task.title" :style="titleStyle">{{ task.title }}</span>
        </span>
      </div>
    </div>
  </div>
</template>
