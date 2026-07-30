<script setup>
import { computed } from 'vue'

/* Vertical connector drawn in the column gutter between two related cards.
   A gutter spine with notches, not arrows — arrows do not survive density. */
const props = defineProps({
  state: { type: String, default: 'idle' },
  height: { type: Number, default: 28 }
})

const active = computed(() => props.state === 'active')
const line = computed(() => (active.value ? 'var(--graph-line-active)' : 'var(--graph-line)'))
const opacity = computed(() => (active.value ? 1 : 0.6))

const wrapStyle = computed(() => ({
  display: 'block',
  width: '9px',
  height: `${props.height}px`,
  position: 'relative',
  flex: '0 0 auto'
}))
const stemStyle = computed(() => ({
  position: 'absolute', left: '4px', top: 0, bottom: 0, width: '1px',
  background: line.value, opacity: opacity.value
}))
const notchStyle = computed(() => ({
  position: 'absolute', left: '1px', top: '50%', width: '7px', height: '1px',
  background: line.value, opacity: opacity.value
}))
</script>

<template>
  <span aria-hidden="true" :style="wrapStyle">
    <span :style="stemStyle" />
    <span :style="notchStyle" />
  </span>
</template>
