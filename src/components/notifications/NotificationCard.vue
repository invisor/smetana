<script setup>
/* One thing the bell has to say, with the two answers a person can give it.

   The card is deliberately quiet: `attentionLevel`'s loud budget is one or two
   rows on a screen and it belongs to an agent waiting for a human, not to a
   folder that has grown. So there is no status colour anywhere on it — a glyph,
   a line of title, a paragraph, and two buttons, the far one of them a ghost. */
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'

defineProps({
  /* The shape `components/notifications/notifications.js` builds: whatever else
     it carries, this component draws `icon`, `title`, `body` and `actionLabel`,
     and knows nothing about which source produced them. That is the whole of
     what a second source would have to fill in. */
  notification: { type: Object, required: true }
})

defineEmits(['action', 'dismiss'])

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  padding: 'var(--card-pad)',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)'
}
const headStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  color: 'var(--text-primary)',
  font: 'var(--weight-medium) var(--text-sm)/var(--leading-snug) var(--font-sans)'
}
const bodyStyle = {
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)',
  textWrap: 'pretty'
}
const actionsStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  marginTop: 'var(--space-2)'
}
</script>

<template>
  <div :style="rootStyle">
    <div :style="headStyle">
      <Icon
        :name="notification.icon || 'bell'"
        :size="14"
        :style="{ flex: 'none', color: 'var(--text-muted)' }"
      />
      <span :style="{ minWidth: 0 }">{{ notification.title }}</span>
    </div>
    <div :style="bodyStyle">{{ notification.body }}</div>
    <div :style="actionsStyle">
      <Button
        v-if="notification.actionLabel"
        size="sm"
        variant="secondary"
        @click="$emit('action', notification)"
      >
        {{ notification.actionLabel }}
      </Button>
      <!-- Dismiss is every card's, whatever its source: it is the one answer
           that needs nothing from the thing being announced. -->
      <Button size="sm" variant="ghost" @click="$emit('dismiss', notification)">Dismiss</Button>
    </div>
  </div>
</template>
