<script setup>
/* The Kanban tab: how much of the board is worth drawing. Two settings, each
   with a list of exceptions under it — the columns that stay whatever the
   setting says.

   Presentational, like every component here: handed the values and the current
   project's columns, emitting what was picked. The window does the asking and
   the telling, so this renders in `?view=gallery` with nothing behind it.

   Both lists are drawn even when the setting above them makes them do nothing —
   disabled rather than hidden, because a screen that says what is there is
   worth more than one that saves four rows.

   The rule these four settings feed lives entirely in `kanban/boardView.js`,
   which is also where the two closed lists and the grouping below come from:
   a `.vue` file is the one thing no test in this repository reaches. */
import { computed } from 'vue'
import Checkbox from '../core/Checkbox.vue'
import Dropdown from '../core/Dropdown.vue'
import SettingsRow from './SettingsRow.vue'
import {
  COLUMN_MODE_CHOICES,
  INTERVAL_CHOICES,
  columnChoices,
  columnLabel,
  toggleColumn
} from '../kanban/boardView.js'

const props = defineProps({
  columns: { type: String, default: 'all' },
  alwaysShow: { type: Array, default: () => [] },
  interval: { type: String, default: 'all' },
  unlimited: { type: Array, default: () => [] },
  /* The statuses the active project's board actually has, in the board's own
     order — `blocked` among them when something is blocked, which is why this
     arrives from the app window rather than from Rust. Empty is an ordinary
     state: no project open, or a board still being read. */
  boardColumns: { type: Array, default: () => [] }
})

const emit = defineEmits([
  'update:columns',
  'update:alwaysShow',
  'update:interval',
  'update:unlimited'
])

const always = computed(() => columnChoices(props.alwaysShow, props.boardColumns))
const everything = computed(() => columnChoices(props.unlimited, props.boardColumns))

/* A list only does something under one value of the setting above it. */
const alwaysLive = computed(() => props.columns === 'non-empty')
const unlimitedLive = computed(() => props.interval !== 'all')

const listStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  padding: 'var(--space-4) 0 var(--space-5)'
}
const captionStyle = {
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
/* The second group's caption sits above names saved against some other
   project's board. It is the whole price of storing these lists globally: a
   name nobody can see goes on filtering this board and cannot be taken off. */
const elsewhereStyle = {
  marginTop: 'var(--space-4)',
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
const emptyStyle = {
  color: 'var(--text-muted)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
</script>

<template>
  <div>
    <SettingsRow
      label="Columns"
      description="A column with nothing in it can keep its place on the board or give it up."
    >
      <Dropdown
        :model-value="props.columns"
        :options="COLUMN_MODE_CHOICES"
        @update:model-value="emit('update:columns', $event)"
      />
    </SettingsRow>
    <div :style="listStyle">
      <span :style="captionStyle">Always show these columns, even when empty</span>
      <span v-if="!always.onBoard.length && !always.elsewhere.length" :style="emptyStyle">
        No board to take columns from.
      </span>
      <Checkbox
        v-for="entry in always.onBoard"
        :key="entry.name"
        :model-value="entry.checked"
        :disabled="!alwaysLive"
        :label="columnLabel(entry.name)"
        @update:model-value="emit('update:alwaysShow', toggleColumn(props.alwaysShow, entry.name, $event))"
      />
      <span v-if="always.elsewhere.length" :style="elsewhereStyle">
        Saved from another project's board
      </span>
      <Checkbox
        v-for="entry in always.elsewhere"
        :key="entry.name"
        :model-value="entry.checked"
        :disabled="!alwaysLive"
        :label="columnLabel(entry.name)"
        @update:model-value="emit('update:alwaysShow', toggleColumn(props.alwaysShow, entry.name, $event))"
      />
    </div>

    <SettingsRow
      label="Show tasks from"
      description="Measured on when a task last changed, so work picked up today stays in view however old it is."
    >
      <Dropdown
        :model-value="props.interval"
        :options="INTERVAL_CHOICES"
        @update:model-value="emit('update:interval', $event)"
      />
    </SettingsRow>
    <div :style="listStyle">
      <span :style="captionStyle">Show everything in these columns, whatever the period</span>
      <span v-if="!everything.onBoard.length && !everything.elsewhere.length" :style="emptyStyle">
        No board to take columns from.
      </span>
      <Checkbox
        v-for="entry in everything.onBoard"
        :key="entry.name"
        :model-value="entry.checked"
        :disabled="!unlimitedLive"
        :label="columnLabel(entry.name)"
        @update:model-value="emit('update:unlimited', toggleColumn(props.unlimited, entry.name, $event))"
      />
      <span v-if="everything.elsewhere.length" :style="elsewhereStyle">
        Saved from another project's board
      </span>
      <Checkbox
        v-for="entry in everything.elsewhere"
        :key="entry.name"
        :model-value="entry.checked"
        :disabled="!unlimitedLive"
        :label="columnLabel(entry.name)"
        @update:model-value="emit('update:unlimited', toggleColumn(props.unlimited, entry.name, $event))"
      />
    </div>
  </div>
</template>
