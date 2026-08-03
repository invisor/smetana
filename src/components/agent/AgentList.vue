<script setup>
/* The project's agent list. Split out of DesktopApp.vue: that file is
   already past nine hundred lines, and a live list with a button and
   removal would have made it unreadable.

   Colour is never the only signal here: needs-you is a triangle,
   everything else is a dot. */
import { computed, watch } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import { useInteractive } from '../core/interactive.js'
import { attentionLevel } from '../status/status.js'

const props = defineProps({
  rows: { type: Array, default: () => [] },
  activeId: { type: [Number, String], default: null },
  /* There is nowhere to start an agent without a project: the call would
     reach the worker and come back as a generic failure toast. An
     affordance that cannot work says so before it is clicked. */
  canCreate: { type: Boolean, default: true }
})
defineEmits(['select', 'create', 'remove'])

const body = { flex: 1, minHeight: 0, overflow: 'auto' }

/* Hover has to be per row, and useInteractive tracks one control at a time
   — calling it fresh from inside rowStyle would throw hover state away on
   every re-render, since that would build a new pair of refs each time. So
   each row's instance is created once and cached by id, the way a keyed
   ref would be. Press is not tracked: a row is not a button, it is a
   place, the same reasoning ProjectList.vue uses. */
const rowInteractive = new Map()
const interactiveFor = (id) => {
  let entry = rowInteractive.get(id)
  if (!entry) {
    entry = useInteractive()
    rowInteractive.set(id, entry)
  }
  return entry
}

/* Sessions come and go with agents; without this the cache would keep one
   stale entry per agent that ever existed for the life of the component. */
watch(
  () => props.rows.map((row) => row.id),
  (ids) => {
    const live = new Set(ids)
    for (const id of rowInteractive.keys()) {
      if (!live.has(id)) rowInteractive.delete(id)
    }
  }
)

const rowStyle = (row) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  background:
    row.id === props.activeId
      ? 'var(--surface-raised)'
      : interactiveFor(row.id).hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: 'default',
  opacity: attentionLevel(row.state) === 'quiet' ? 'var(--attn-quiet-opacity)' : 1,
  transition: 'var(--transition-control)'
})

/* Triangle geometry has no token — there is no "--radius" for the side of a
   triangle — so these stay literal by deliberate exception; everything else
   in this file is a token reference. */
const needsYouMark = {
  width: 0,
  height: 0,
  borderLeft: '5px solid transparent',
  borderRight: '5px solid transparent',
  borderBottom: '8px solid var(--attn-loud)'
}
const runningMark = { width: '8px', height: '8px', borderRadius: '50%', background: 'var(--attn-live)' }
const quietMark = { width: '8px', height: '8px', borderRadius: '50%', background: 'var(--text-muted)' }

const markOf = (state) => {
  if (state === 'needs-you') return needsYouMark
  if (state === 'running') return runningMark
  return quietMark
}

const disabled = computed(() => !props.canCreate)
/* The one control here that really is a button in spirit — the panel's only
   action — so it gets its own single useInteractive() instance, same as
   Button.vue and IconButton.vue use for themselves, and its disabled state
   reads the same as Button.vue's: no hover, the not-allowed cursor and the
   same dimming. One control should not be a second dialect of the other. */
const addInteractive = useInteractive(disabled)
const addRow = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)',
  background: addInteractive.hover.value ? 'var(--surface-hover)' : 'transparent',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  cursor: disabled.value ? 'not-allowed' : 'default',
  opacity: disabled.value ? 0.7 : 1,
  transition: 'var(--transition-control)'
}))

const empty = computed(() => props.rows.length === 0)
</script>

<template>
  <div :style="{ display: 'flex', flexDirection: 'column', height: '100%' }">
    <div :style="body">
      <div
        v-for="row in rows"
        :key="row.id"
        :data-attention="attentionLevel(row.state)"
        :style="rowStyle(row)"
        v-bind="interactiveFor(row.id).handlers"
        @click="$emit('select', row.id)"
      >
        <span :style="markOf(row.state)" />
        <span>{{ row.name }}</span>
        <span :style="{ flex: 1 }" />
        <span :style="{ color: row.state === 'needs-you' ? 'var(--attn-loud)' : 'var(--text-muted)' }">
          {{ row.elapsed }}
        </span>
        <IconButton icon="x" size="sm" label="Remove agent" @click.stop="$emit('remove', row.id)" />
      </div>
      <div v-if="empty" :style="{ padding: 'var(--space-5)', color: 'var(--text-muted)', font: 'var(--weight-regular) var(--text-xs)/1.5 var(--font-sans)' }">
        No agents running.
      </div>
    </div>
    <div
      :style="addRow"
      :aria-disabled="disabled || undefined"
      v-bind="addInteractive.handlers"
      @click="!disabled && $emit('create')"
    >
      <Icon name="plus" :size="14" />
      <span>New agent</span>
    </div>
  </div>
</template>
