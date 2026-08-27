<script setup>
import { computed } from 'vue'
import ColumnHeader from './ColumnHeader.vue'
import TaskCard from './TaskCard.vue'
import EmptyState from '../core/EmptyState.vue'

const props = defineProps({
  status: { type: String, required: true },
  tasks: { type: Array, default: () => [] },
  wipLimit: { type: Number, default: null },
  dropTarget: { type: Boolean, default: false },
  selectedId: { type: String, default: undefined },
  /* Which card's id was last copied, and how that went — the same two hops as
     `selectedId`, and paired for the same reason it is a single id: one
     confirmation exists on the board at a time, so copying one id takes the
     confirmation off whatever held it before. */
  copiedId: { type: String, default: undefined },
  copyState: { type: String, default: '' },
  addable: { type: Boolean, default: true },
  /* Passed through to the header, which is the handle; the gesture itself
     belongs to the board. */
  movable: { type: Boolean, default: false },
  moving: { type: Boolean, default: false },
  runnable: { type: Boolean, default: false },
  /* Why the header's own play cannot be run just now, in words; empty means it
     can — a lowercase fragment, since the play interpolates it into a sentence
     of its own. The header's only: each card carries its own reason in the
     task object, the same channel `runnable` rides, because a run holds one
     scope now rather than the whole project and one fragment for everything
     stopped being true. */
  runBlockedReason: { type: String, default: '' },
  /* Passed through to the header, which draws the button and decides on the
     count whether there is anything to draw it for. */
  promotable: { type: Boolean, default: false }
})

/* A card's own `runnable` rides in the task object and reaches TaskCard through
   the v-bind below — the board already decides everything else about a card
   that way, and adding a second channel for one flag would put the decision in
   two places. */
defineEmits(['select', 'add', 'grab', 'move', 'run', 'task-action', 'promote', 'copy-id'])

/* `moving` reaches the header and stops there. The column being dragged is not
   dimmed or lifted: it is where the pointer already is, and the eye needs it to
   read as the thing being held rather than as a hole left behind. */
const style = {
  color: 'var(--text-primary)',
  display: 'flex',
  flexDirection: 'column',
  minWidth: '212px',
  width: '212px',
  flex: '0 0 auto',
  minHeight: 0
}

const listStyle = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  flex: 1,
  minHeight: 0,
  overflow: 'auto',
  padding: '2px',
  background: props.dropTarget ? 'var(--surface-sunken)' : 'transparent',
  borderRadius: 'var(--radius-3)',
  outline: props.dropTarget ? '1px dashed var(--border-strong)' : 'none'
}))

const emptyDescription = computed(() => `Nothing in ${String(props.status).replace(/-/g, ' ')}.`)
</script>

<template>
  <div :style="style">
    <ColumnHeader
      :status="status"
      :count="tasks.length"
      :wip-limit="wipLimit"
      :addable="addable"
      :movable="movable"
      :moving="moving"
      :runnable="runnable"
      :run-blocked-reason="runBlockedReason"
      :promotable="promotable"
      @add="$emit('add', status)"
      @run="$emit('run', status)"
      @promote="$emit('promote', status)"
      @grab="$emit('grab', $event)"
      @move="$emit('move', $event)"
    />
    <div :style="listStyle">
      <template v-if="tasks.length">
        <!-- No explicit run-blocked-reason here: the card's own rides in `t`,
             through the same v-bind as `runnable`, and the column-level prop
             speaks for the header alone. `bdStatus` and `busy` ride the same
             way, for the same reason. -->
        <!-- One `action` carrying its own id, rather than four events wrapped
             here to add one. The header's `run` and `promote` above are about
             a column and stay where they are. -->
        <TaskCard
          v-for="t in tasks"
          :key="t.id"
          v-bind="t"
          :selected="t.id === selectedId"
          :copy-state="t.id === copiedId ? copyState : ''"
          @click="$emit('select', t.id)"
          @action="$emit('task-action', $event)"
          @copy-id="$emit('copy-id', $event)"
        />
      </template>
      <EmptyState v-else compact icon="minus" title="Empty" :description="emptyDescription" />
    </div>
  </div>
</template>
