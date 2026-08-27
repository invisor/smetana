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

/* A resize is three events, not one: the consumer snapshots a width on start,
   clamps every delta against it, and persists on end. Forwarding only `drag`
   would leave it no way to know which drag a delta belongs to. */
defineEmits([
  'resize-left-start',
  'resize-left',
  'resize-left-end',
  'reset-left',
  'resize-right-start',
  'resize-right',
  'resize-right-end',
  'reset-right'
])

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
      <Resizer
        label="Resize left panel"
        @dragstart="$emit('resize-left-start')"
        @drag="(d, e) => $emit('resize-left', d, e)"
        @dragend="$emit('resize-left-end')"
        @reset="$emit('reset-left')"
      />
      <div :style="{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', background: 'var(--canvas)' }">
        <slot name="center" />
      </div>
      <Resizer
        label="Resize right panel"
        @dragstart="$emit('resize-right-start')"
        @drag="(d, e) => $emit('resize-right', d, e)"
        @dragend="$emit('resize-right-end')"
        @reset="$emit('reset-right')"
      />
      <div :style="rightStyle"><slot name="right" /></div>
    </div>
    <!-- The mirror of `scope` above: a strip about the machine rather than
         about the board, so it runs outside the three columns and their
         resizers — one that stopped at the board's edges would read as a
         caption to the board. Empty is the ordinary case and costs nothing:
         the row above it is the only thing that flexes, so with no footer
         nothing on screen moves. -->
    <slot name="footer" />
  </div>
</template>
