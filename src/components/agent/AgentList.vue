<script setup>
/* The project's agent list. Split out of DesktopApp.vue: that file is
   already past nine hundred lines, and a live list with a button and
   removal would have made it unreadable.

   Colour is never the only signal here: needs-you is a triangle,
   everything else is a dot. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import { attentionLevel } from '../status/status.js'

const props = defineProps({
  rows: { type: Array, default: () => [] },
  activeId: { type: [Number, String], default: null }
})
defineEmits(['select', 'create', 'remove'])

const body = { flex: 1, minHeight: 0, overflow: 'auto' }

const rowStyle = (row) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  background: row.id === props.activeId ? 'var(--surface-raised)' : 'transparent',
  cursor: 'default',
  opacity: attentionLevel(row.state) === 'quiet' ? 'var(--attn-quiet-opacity)' : 1
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

const addRow = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  cursor: 'default'
}

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
    <div :style="addRow" @click="$emit('create')">
      <Icon name="plus" :size="14" />
      <span>New agent</span>
    </div>
  </div>
</template>
