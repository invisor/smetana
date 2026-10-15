<script setup>
/* One task, read-only: the desktop's own inspector, which draws the status, the type, the
   markdown of every prose field and the record beneath, plus the two dependency lists as
   things to open - the inspector names what blocks a task and leaves the tracing to the
   person, and on a phone a name is only useful if it can be tapped. */
import { computed } from 'vue'
import TaskInspector from '@src/components/kanban/TaskInspector.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import { boardColumns, dependencyEdges, issueById, toUiStatus } from '@src/stores/tracker.js'
import { settings } from '@src/stores/settings.js'
import { columnOf } from '../board.js'

const props = defineProps({ id: { type: String, required: true } })
const emit = defineEmits(['open'])

const issue = computed(() => issueById(props.id))
const uiStatus = computed(
  () => columnOf(boardColumns.value, props.id) ?? toUiStatus(issue.value?.status)
)

const link = (id) => ({ id, title: issueById(id)?.title ?? '' })
const blockedBy = computed(() => (dependencyEdges.value.blockedBy.get(props.id) ?? []).map(link))
const blocks = computed(() => (dependencyEdges.value.blocking.get(props.id) ?? []).map(link))

const page = { padding: 'var(--space-5)', display: 'flex', flexDirection: 'column', gap: 'var(--space-6)' }
const heading = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
const group = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }
const item = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4)',
  textAlign: 'left',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)',
  color: 'var(--text-primary)',
  cursor: 'pointer'
}
const itemId = { font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }
const itemTitle = { font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)' }
</script>

<template>
  <EmptyState
    v-if="!issue"
    icon="inbox"
    title="This task is not on the board"
    description="It may have been deleted on the desktop."
  />
  <div v-else :style="page">
    <TaskInspector :issue="issue" :ui-status="uiStatus" :base="settings.activeProject ?? ''" />
    <section v-for="[label, list] in [['Blocked by', blockedBy], ['Blocks', blocks]]" v-show="list.length" :key="label" :style="group">
      <span :style="heading">{{ label }}</span>
      <button v-for="dep in list" :key="dep.id" type="button" :style="item" @click="emit('open', dep.id)">
        <span :style="itemId">{{ dep.id }}</span>
        <span :style="itemTitle">{{ dep.title }}</span>
      </button>
    </section>
  </div>
</template>
