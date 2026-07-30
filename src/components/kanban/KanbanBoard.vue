<script setup>
import KanbanColumn from './KanbanColumn.vue'
import EmptyState from '../core/EmptyState.vue'

/* Columns come from the tracker's status set, which users extend — the board
   never assumes four fixed columns. */
defineProps({
  columns: { type: Array, default: () => [] },
  selectedId: { type: String, default: undefined }
})

defineEmits(['select'])

const style = {
  display: 'flex',
  gap: 'var(--space-6)',
  alignItems: 'stretch',
  flex: 1,
  minHeight: 0,
  overflow: 'auto',
  padding: 'var(--panel-pad)'
}
</script>

<template>
  <EmptyState
    v-if="!columns.length"
    icon="columns-3"
    title="No board yet"
    description="Connect a tracker to pull tasks, or create the first task locally."
  />
  <div v-else :style="style">
    <KanbanColumn
      v-for="c in columns"
      :key="c.status"
      v-bind="c"
      :selected-id="selectedId"
      @select="$emit('select', $event)"
    />
  </div>
</template>
