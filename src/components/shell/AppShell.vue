<script setup>
import { computed } from 'vue'
import Resizer from './Resizer.vue'

/* Three columns under a scope bar: files/agents, the board, the task inspector.
   A control room, not an editor — the shell itself never scrolls. */
const props = defineProps({
  leftWidth: { type: Number, default: 248 },
  rightWidth: { type: Number, default: 320 },
  leftCollapsed: { type: Boolean, default: false },
  rightCollapsed: { type: Boolean, default: false },
  height: { type: [Number, String], default: '100vh' }
})

defineEmits(['resize-left', 'resize-right'])

const style = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  height: typeof props.height === 'number' ? `${props.height}px` : props.height,
  minHeight: 0,
  background: 'var(--canvas)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-body-size)/var(--leading-normal) var(--font-sans)',
  overflow: 'hidden'
}))

const leftStyle = computed(() => ({
  flex: '0 0 auto',
  width: props.leftCollapsed ? '32px' : `${props.leftWidth}px`,
  minWidth: 0,
  display: 'flex'
}))
const rightStyle = computed(() => ({
  flex: '0 0 auto',
  width: props.rightCollapsed ? '32px' : `${props.rightWidth}px`,
  minWidth: 0,
  display: 'flex'
}))
</script>

<template>
  <div :style="style">
    <slot name="scope" />
    <div :style="{ flex: 1, minHeight: 0, display: 'flex', alignItems: 'stretch' }">
      <div :style="leftStyle"><slot name="left" /></div>
      <Resizer @drag="(d, e) => $emit('resize-left', d, e)" />
      <div :style="{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', background: 'var(--canvas)' }">
        <slot name="center" />
      </div>
      <Resizer @drag="(d, e) => $emit('resize-right', d, e)" />
      <div :style="rightStyle"><slot name="right" /></div>
    </div>
  </div>
</template>
