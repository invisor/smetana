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
  addable: { type: Boolean, default: true }
})

defineEmits(['select', 'add'])

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
      @add="$emit('add', status)"
    />
    <div :style="listStyle">
      <template v-if="tasks.length">
        <TaskCard
          v-for="t in tasks"
          :key="t.id"
          v-bind="t"
          :selected="t.id === selectedId"
          @click="$emit('select', t.id)"
        />
      </template>
      <EmptyState v-else compact icon="minus" title="Empty" :description="emptyDescription" />
    </div>
  </div>
</template>
