<script setup>
/* What the bell opens: everything the app has to say right now, or a line
   saying that it has nothing.

   Presentational, and placed by whoever opens it — the same split
   `ContextMenu` and `MenuButton` keep. That is what lets the panel be looked at
   in `?view=gallery` sitting in an ordinary column instead of hanging off the
   corner of the window.

   An empty panel says so rather than opening as a blank rectangle: the list is
   derived from what the sources can see this moment, so "nothing" is an answer
   and not a failure to load. */
import EmptyState from '../core/EmptyState.vue'
import NotificationCard from './NotificationCard.vue'

defineProps({
  /* Whole notification objects, in the shape `notifications.js` builds them —
     the panel never composes prose of its own. */
  items: { type: Array, default: () => [] }
})

defineEmits(['action', 'dismiss'])

const rootStyle = {
  /* A ceiling rather than a width: one card of prose is comfortable at this
     measure, and the panel hangs under the bell in the top right corner, where
     anything wider would reach back across the whole scope bar. */
  width: 'min(360px, calc(100vw - var(--space-6) * 2))',
  maxHeight: 'min(60vh, 520px)',
  overflowY: 'auto',
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  padding: 'var(--panel-pad)',
  background: 'var(--surface-overlay)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3)',
  boxShadow: 'var(--shadow-overlay)'
}
</script>

<template>
  <div role="dialog" aria-label="Notifications" :style="rootStyle">
    <EmptyState
      v-if="!items.length"
      icon="bell"
      compact
      title="Nothing to report"
      description="Notifications appear here while they are true, and go when they are not."
    />
    <NotificationCard
      v-for="item in items"
      :key="item.id"
      :notification="item"
      @action="$emit('action', $event)"
      @dismiss="$emit('dismiss', $event)"
    />
  </div>
</template>
