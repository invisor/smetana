<script setup>
/* The project's agent list. Split out of DesktopApp.vue: that file is
   already past nine hundred lines, and a live list with removal would have
   made it unreadable.

   Starting an agent is not here: it belongs to a project, and it is offered
   on the project's own row in ProjectList.vue. This component is the
   sessions and nothing else.

   Colour is never the only signal here: needs-you is the status system's
   warning triangle, everything else is a dot.

   A row is captioned by the work, not by the process: `claude-7` names a
   process, and five of those in a column said nothing about who was doing
   what. The caption arrives in two pieces because they are set differently —
   `label` is prose and goes in sans, `tasks` are issue ids and go in mono. */
import { computed, watch } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import { useInteractive } from '../core/interactive.js'
import { STATUS_GLYPH, attentionLevel } from '../status/status.js'

const props = defineProps({
  rows: { type: Array, default: () => [] },
  activeId: { type: [Number, String], default: null }
})
defineEmits(['select', 'remove'])

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
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
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

/* needs-you draws STATUS_GLYPH's own glyph — the same triangle-with-a-bang
   the badges use — rather than a shape built here: one status, one picture
   across the app. Everything else stays a dot, so the loud row is still
   distinguishable by silhouette alone.

   The mark's box is fixed at the icon's size, and the dots sit centred
   inside it: the two marks differ in shape, and a row must not shift by
   four pixels when an agent starts waiting. */
const MARK = 12
const markBox = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: `${MARK}px`,
  height: `${MARK}px`,
  flex: 'none'
}
const runningMark = { width: '8px', height: '8px', borderRadius: '50%', background: 'var(--attn-live)' }
const quietMark = { width: '8px', height: '8px', borderRadius: '50%', background: 'var(--text-muted)' }

const dotOf = (state) => (state === 'running' ? runningMark : quietMark)

/* One glyph for every agent. Lucide has no brand marks, and telling claude
   from codex on a row was never the question this list has to answer — what
   the agent is doing was. It sits at the same size as the state mark beside
   it, so the two read as one pair rather than as two competing signals. */
const AGENT_ICON = 'bot'

/* minWidth 0 and the ellipsis are what keep a long caption — a run holding
   four issues — from pushing the elapsed time and the remove button off the
   end of the row: a flex item refuses by default to shrink below its own
   content. Baseline rather than centre, because the two halves are set in
   different families and centring would leave them sitting at different
   heights. */
const captionBox = {
  display: 'flex',
  alignItems: 'baseline',
  gap: 'var(--space-2)',
  minWidth: 0,
  overflow: 'hidden',
  whiteSpace: 'nowrap'
}
const idsStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}
const labelStyle = { overflow: 'hidden', textOverflow: 'ellipsis' }
/* The elapsed time stays mono: it is a measurement in a column, and a
   proportional face would let "18m" and "2h 14m" wander sideways as the clock
   ticks. */
const elapsedStyle = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)', flex: 'none' }

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
        <span :style="markBox">
          <Icon
            v-if="row.state === 'needs-you'"
            :name="STATUS_GLYPH['needs-you']"
            :size="MARK"
            :stroke-width="2.25"
            :style="{ color: 'var(--attn-loud)' }"
          />
          <span v-else :style="dotOf(row.state)" />
        </span>
        <Icon :name="AGENT_ICON" :size="MARK" :style="{ flex: 'none', color: 'var(--text-muted)' }" />
        <span :style="captionBox">
          <span v-if="row.label" :style="labelStyle">{{ row.label }}</span>
          <span v-if="row.tasks?.length" :style="idsStyle">{{ row.tasks.join(', ') }}</span>
        </span>
        <span :style="{ flex: 1 }" />
        <span :style="[elapsedStyle, { color: row.state === 'needs-you' ? 'var(--attn-loud)' : 'var(--text-muted)' }]">
          {{ row.elapsed }}
        </span>
        <!-- A row that is still starting has no process behind it to take away,
             and the id it carries is this window's own rather than the worker's,
             so removing it would ask about a session nobody has. Disabled rather
             than left out: the button is the widest thing on the row, and a row
             that grew one a second after appearing would shift everything on it
             sideways exactly as somebody started reading it. -->
        <IconButton
          icon="x"
          size="sm"
          label="Remove agent"
          :disabled="!!row.starting"
          @click.stop="$emit('remove', row.id)"
        />
      </div>
      <div v-if="empty" :style="{ padding: 'var(--space-5)', color: 'var(--text-muted)', font: 'var(--weight-regular) var(--text-xs)/1.5 var(--font-sans)' }">
        No agents running. Start one with + on the project row.
      </div>
    </div>
  </div>
</template>
