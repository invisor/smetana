<script setup>
import { computed } from 'vue'
import IconButton from '../core/IconButton.vue'

const props = defineProps({
  title: { type: String, required: true },
  side: { type: String, default: 'left' },
  collapsed: { type: Boolean, default: false },
  collapsible: { type: Boolean, default: true }
})

defineEmits(['toggle'])

const edge = computed(() => ({
  borderRight: props.side === 'left' ? 'var(--border-w) solid var(--border)' : undefined,
  borderLeft: props.side === 'right' ? 'var(--border-w) solid var(--border)' : undefined
}))

const collapsedStyle = computed(() => ({
  width: '32px',
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 'var(--space-3)',
  padding: 'var(--space-3) 0',
  background: 'var(--surface)',
  ...edge.value
}))

const style = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  minWidth: 0,
  minHeight: 0,
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  ...edge.value
}))

const headerStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  /* The same height as the tab row: the column headers line up with it, and a
     couple of pixels' difference reads as a misalignment rather than an
     accent. */
  height: 'var(--tab-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-3) 0 var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
/* 10px uppercase mono: a label, not a sentence. */
const titleStyle = {
  flex: 1,
  minWidth: 0,
  fontSize: 'var(--text-2xs)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)',
  fontWeight: 'var(--weight-medium)',
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}
const railTitleStyle = {
  writingMode: 'vertical-rl',
  fontSize: 'var(--text-2xs)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
</script>

<template>
  <div v-if="collapsed" :style="collapsedStyle">
    <IconButton
      :icon="side === 'left' ? 'panel-left-open' : 'panel-right-open'"
      :label="`Expand ${side} panel`"
      size="sm"
      @click="$emit('toggle')"
    />
    <div :style="railTitleStyle">{{ title }}</div>
  </div>
  <div v-else :style="style">
    <div :style="headerStyle">
      <div :style="titleStyle">{{ title }}</div>
      <slot name="actions" />
      <IconButton
        v-if="collapsible"
        :icon="side === 'left' ? 'panel-left-close' : 'panel-right-close'"
        :label="`Collapse ${side} panel`"
        size="sm"
        @click="$emit('toggle')"
      />
    </div>
    <div :style="{ flex: 1, minHeight: 0, overflow: 'auto' }">
      <slot />
    </div>
    <div
      v-if="$slots.footer"
      :style="{ flex: '0 0 auto', padding: 'var(--space-3) var(--space-5)', borderTop: 'var(--border-w) solid var(--border-subtle)' }"
    >
      <slot name="footer" />
    </div>
  </div>
</template>
