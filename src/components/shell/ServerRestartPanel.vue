<script setup>
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'

/* What the scope bar's mark for an outdated server opens: the same notice the banner above the footer
   carries, said where it cannot be dismissed. Presentational like the banner: the window hands over
   what `connectionNotice.js`'s `serverIndicator()` answered, which is also where every word comes
   from, and hears back which choice was pressed. The mark stays whatever is pressed here; only the
   server being replaced removes it. */
defineProps({
  /* `serverIndicator()`'s answer. */
  notice: { type: Object, required: true }
})

defineEmits(['restart-now', 'restart-idle', 'restart-cancel'])

/* A surface like the bell's panel, so the two overlays that hang from the same bar read as one family. */
const rootStyle = {
  width: 'min(var(--panel-right-w), calc(100vw - var(--space-6) * 2))',
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  padding: 'var(--panel-pad)',
  background: 'var(--surface-raised)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)',
  boxShadow: 'var(--shadow-overlay)'
}
const headStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  fontWeight: 'var(--weight-semibold)',
  fontSize: 'var(--text-sm)'
}
const detailStyle = {
  color: 'var(--text-secondary)',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-snug)'
}
const actionsStyle = { display: 'flex', flexWrap: 'wrap', gap: 'var(--space-3)' }
</script>

<template>
  <div role="dialog" :aria-label="notice.label" :style="rootStyle">
    <div :style="headStyle">
      <Icon name="triangle-alert" :size="14" :style="{ color: 'var(--attn-loud)' }" />
      <span>{{ notice.title }}</span>
    </div>
    <div :style="detailStyle">{{ notice.detail }}</div>
    <div v-if="notice.restart" :style="actionsStyle">
      <Button v-if="notice.restart.canNow" size="sm" @click="$emit('restart-now')">Restart now</Button>
      <Button v-if="notice.restart.canWhenIdle" size="sm" @click="$emit('restart-idle')">
        Restart when finished
      </Button>
      <Button v-if="notice.restart.canCancel" size="sm" @click="$emit('restart-cancel')">
        Cancel restart
      </Button>
    </div>
  </div>
</template>
