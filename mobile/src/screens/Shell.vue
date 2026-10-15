<script setup>
/* The page the working screens sit in: a header with the screen's name and the way back, the
   screen itself scrolling between, and the four tabs along the bottom.

   The safe area is spent here and in Frame.vue (through ./safeArea.js) and nowhere else - the
   header sits under the notch, the tab bar over the home indicator - so a screen draws only its
   own content. The page itself never scrolls; the middle does, which is what keeps the tab bar
   in reach on a long board. */
import { computed } from 'vue'
import Icon from '@src/components/core/Icon.vue'
import IconButton from '@src/components/core/IconButton.vue'
import { TABS } from '../nav.js'
import { inset } from './safeArea.js'

const props = defineProps({
  title: { type: String, required: true },
  tab: { type: String, required: true },
  canBack: { type: Boolean, default: false },
  /* The sentence under the title, in mono: a project's name, a task's id. */
  subtitle: { type: String, default: '' },
  /* Where the tab bar is drawn at all. A form has the keyboard on it and no use for tabs. */
  tabs: { type: Boolean, default: true },
  /* The screen scrolls itself (the conversation does) and the page must not. */
  fill: { type: Boolean, default: false }
})
const emit = defineEmits(['back', 'tab'])

const page = {
  height: '100dvh',
  display: 'flex',
  flexDirection: 'column',
  background: 'var(--surface)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)'
}
const header = {
  flex: '0 0 auto',
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  minHeight: 'var(--scope-bar-h)',
  padding: `${inset('top', 'var(--space-3)')} ${inset('right', 'var(--space-4)')} var(--space-3) ${inset('left', 'var(--space-4)')}`,
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)'
}
const heading = { flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-1)' }
const titleStyle = {
  margin: 0,
  font: 'var(--weight-medium) var(--text-md)/var(--leading-snug) var(--font-sans)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const subtitleStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const body = computed(() => ({
  flex: 1,
  minHeight: 0,
  display: 'flex',
  flexDirection: 'column',
  overflowY: props.fill ? 'hidden' : 'auto',
  overflowX: 'hidden'
}))
const bar = {
  flex: '0 0 auto',
  display: 'flex',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  background: 'var(--surface)',
  padding: `0 ${inset('right', '0px')} ${inset('bottom', '0px')} ${inset('left', '0px')}`
}
const tabStyle = (active) => ({
  flex: 1,
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  justifyContent: 'center',
  gap: 'var(--space-2)',
  minHeight: 'var(--control-h-lg)',
  padding: 'var(--space-3) 0',
  border: 0,
  borderTop: `var(--border-w-strong) solid ${active ? 'var(--focus-ring)' : 'transparent'}`,
  background: 'transparent',
  color: active ? 'var(--text-primary)' : 'var(--text-muted)',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-sans)',
  cursor: 'pointer'
})
const tabs = computed(() => TABS)
</script>

<template>
  <div :style="page">
    <header :style="header">
      <IconButton v-if="canBack" icon="chevron-left" label="Back" @click="emit('back')" />
      <div :style="heading">
        <h1 :style="titleStyle">{{ title }}</h1>
        <span v-if="subtitle" :style="subtitleStyle">{{ subtitle }}</span>
      </div>
      <slot name="actions" />
    </header>
    <main :style="body"><slot /></main>
    <nav v-if="props.tabs" :style="bar" aria-label="Screens">
      <button
        v-for="item in tabs"
        :key="item.id"
        type="button"
        :aria-current="item.id === tab ? 'page' : undefined"
        :style="tabStyle(item.id === tab)"
        @click="emit('tab', item.id)"
      >
        <Icon :name="item.icon" :size="16" />
        {{ item.label }}
      </button>
    </nav>
  </div>
</template>
