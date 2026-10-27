<script setup>
/* The board, one column under another.

   The alternative was a horizontal swipe between columns. Stacked won for three reasons: every
   column's name and count is on the screen at once (a swipe hides all but one, and a board with
   eight or nine columns would need a strip of tabs to say what the swipe holds); the vertical
   gesture is the one a thumb already makes, with no conflict with the system's edge swipe for
   going back; and a column that holds nothing costs one header row instead of an empty page.
   A header folds its column away, so a long done column never stands between a person and the
   ones under it.

   Which columns exist, their order and which are drawn are the desktop's rules (./board.js).
   The cards are the desktop's `TaskCard`, without its three-dot button: the tap on a card is the
   way into its menu, so the card says `sheet` and `Workspace.vue` opens `TaskSheet.vue` over it
   (the desktop's rows, plus View task). A button would be a second door to the same place. */
import { computed, reactive } from 'vue'
import ColumnHeader from '@src/components/kanban/ColumnHeader.vue'
import TaskCard from '@src/components/kanban/TaskCard.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import Icon from '@src/components/core/Icon.vue'
import Button from '@src/components/core/Button.vue'
import { boardColumns, trackerState } from '@src/stores/tracker.js'
import { settings } from '@src/stores/settings.js'
import { phoneColumns, shownCards } from '../board.js'

const emit = defineEmits(['sheet'])

const columns = computed(() =>
  phoneColumns(boardColumns.value, settings.project.columnOrder, settings.kanban, Date.now())
)

/* Folded columns and the columns whose whole list is out, by status. Local to the screen: a
   glance, not a preference worth keeping. */
const folded = reactive(new Set())
const whole = reactive(new Set())
const toggle = (set, status) => (set.has(status) ? set.delete(status) : set.add(status))

const unhealthy = computed(() => trackerState.health.state !== 'ok')

const column = { display: 'flex', flexDirection: 'column' }
const head = (open) => ({
  background: 'transparent',
  border: 0,
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  padding: 0,
  cursor: 'pointer',
  opacity: open ? 1 : 'var(--attn-quiet-opacity)'
})
const list = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  padding: 'var(--space-4) var(--space-4) var(--space-5)'
}
const notice = {
  margin: 0,
  padding: 'var(--space-4) var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--status-failed-fg)'
}
const more = { alignSelf: 'flex-start' }
</script>

<template>
  <div>
    <p v-if="unhealthy" :style="notice">
      The tracker on the desktop is not healthy ({{ trackerState.health.state }}). The board may be out of date.
    </p>
    <EmptyState
      v-if="!trackerState.ready"
      icon="loader-circle"
      spin
      title="Reading the board"
      description="The desktop is being asked for its tasks."
    />
    <EmptyState
      v-else-if="!columns.length"
      icon="columns-3"
      title="Nothing to show"
      description="The board has no columns in this view."
    />
    <section v-for="col in columns" :key="col.status" :style="column">
      <div
        role="button"
        tabindex="0"
        :aria-expanded="!folded.has(col.status)"
        :style="head(!folded.has(col.status))"
        @click="toggle(folded, col.status)"
        @keydown.enter="toggle(folded, col.status)"
        @keydown.space.prevent="toggle(folded, col.status)"
      >
        <ColumnHeader :status="col.status" :count="col.tasks.length" :addable="false" :accent="false">
          <template #actions>
            <Icon :name="folded.has(col.status) ? 'chevron-right' : 'chevron-down'" :size="14" />
          </template>
        </ColumnHeader>
      </div>
      <div v-if="!folded.has(col.status) && col.tasks.length" :style="list">
        <TaskCard
          v-for="task in shownCards(col.tasks, whole.has(col.status)).cards"
          :key="task.id"
          v-bind="task"
          :actionable="false"
          @click="emit('sheet', task.id)"
        />
        <Button
          v-if="shownCards(col.tasks, whole.has(col.status)).hidden"
          variant="secondary"
          size="sm"
          :style="more"
          @click="toggle(whole, col.status)"
        >
          Show {{ shownCards(col.tasks, false).hidden }} more
        </Button>
      </div>
    </section>
  </div>
</template>
