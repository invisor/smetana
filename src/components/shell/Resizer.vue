<script setup>
import { computed, onBeforeUnmount, ref } from 'vue'

const props = defineProps({
  orientation: { type: String, default: 'vertical' }
})
const emit = defineEmits(['drag'])

const state = ref('idle')
let move = null
let up = null

const detach = () => {
  if (move) window.removeEventListener('mousemove', move)
  if (up) window.removeEventListener('mouseup', up)
  move = null
  up = null
}

const onMousedown = (e) => {
  state.value = 'dragging'
  const start = props.orientation === 'vertical' ? e.clientX : e.clientY
  move = (ev) => emit('drag', (props.orientation === 'vertical' ? ev.clientX : ev.clientY) - start, ev)
  up = () => {
    state.value = 'idle'
    detach()
  }
  window.addEventListener('mousemove', move)
  window.addEventListener('mouseup', up)
}

// A drag that outlives the component would keep firing against a dead listener.
onBeforeUnmount(detach)

const on = computed(() => state.value !== 'idle')
const style = computed(() => {
  const vertical = props.orientation === 'vertical'
  return {
    flex: '0 0 auto',
    position: 'relative',
    width: vertical ? 'var(--resizer-w)' : 'auto',
    height: vertical ? 'auto' : 'var(--resizer-w)',
    cursor: vertical ? 'col-resize' : 'row-resize',
    background: on.value ? 'var(--focus-ring)' : 'transparent',
    opacity: state.value === 'hover' ? 0.35 : 1,
    transition: 'background-color var(--dur-fast) var(--ease-out)'
  }
})
</script>

<template>
  <div
    role="separator"
    :aria-orientation="orientation"
    tabindex="0"
    :style="style"
    @mousedown="onMousedown"
    @mouseenter="state = state === 'dragging' ? state : 'hover'"
    @mouseleave="state = state === 'dragging' ? state : 'idle'"
  />
</template>
