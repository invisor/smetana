<script setup>
import { computed } from 'vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'

/* The strip between the board and the footer that says what the connection to
   the server is doing. Presentational, like every component here: the window
   hands over what `connectionNotice.js` answered — which is also where every
   word comes from — and hears back what was pressed, so it draws in
   `?view=gallery` with nothing behind it.

   Amber for a state that mends itself and red for one that needs a person, by
   the same two status colours a run's `needs-you` and `failed` use, so that no
   new hue enters the app for it. Never drawn when there is nothing to say, so a
   build with no server of its own pays no row for it. */
const props = defineProps({
  /* `connectionNotice()`'s answer, or `null`. */
  notice: { type: Object, default: null }
})

defineEmits(['retry', 'restart-now', 'restart-idle', 'restart-cancel'])

const tone = computed(() => (props.notice?.tone === 'blocked' ? 'failed' : 'needs-you'))

const barStyle = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  flex: '0 0 auto',
  minHeight: 'var(--scope-bar-h)',
  padding: 'var(--space-2) var(--space-5)',
  background: `var(--status-${tone.value}-bg)`,
  borderTop: `var(--border-w) solid var(--status-${tone.value}-border)`,
  color: `var(--status-${tone.value}-fg)`,
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)'
}))
const titleStyle = { fontWeight: 'var(--weight-semibold)', flex: '0 0 auto' }
const detailStyle = { flex: '1 1 auto', minWidth: 0, color: 'var(--text-secondary)' }
const actionsStyle = { display: 'flex', gap: 'var(--space-3)', flex: '0 0 auto' }
</script>

<template>
  <div v-if="notice" :style="barStyle" role="status" :data-tone="notice.tone">
    <Icon :name="notice.icon" :size="14" />
    <span :style="titleStyle">{{ notice.title }}</span>
    <span :style="detailStyle">{{ notice.detail }}</span>
    <div :style="actionsStyle">
      <Button v-if="notice.canRetry" size="sm" @click="$emit('retry')">Try now</Button>
      <Button v-if="notice.restart?.canNow" size="sm" @click="$emit('restart-now')">Restart now</Button>
      <Button v-if="notice.restart?.canWhenIdle" size="sm" @click="$emit('restart-idle')">
        Restart when finished
      </Button>
      <Button v-if="notice.restart?.canCancel" size="sm" @click="$emit('restart-cancel')">
        Cancel restart
      </Button>
    </div>
  </div>
</template>
