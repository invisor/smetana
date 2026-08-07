<script setup>
import { computed } from 'vue'
import Tooltip from '../core/Tooltip.vue'
import { attentionLevel, statusColors } from './status.js'

/* Status is never colour alone. Each reserved status has its own silhouette:
   hatched square = blocked, hollow circle = ready, pulsing dot = running,
   triangle = needs-you, dimmed dot = done, square = failed. */
const props = defineProps({
  status: { type: String, required: true },
  size: { type: Number, default: 8 }
})

const c = computed(() => statusColors(props.status))
const level = computed(() => attentionLevel(props.status))

const shape = computed(() => {
  const { key, fg, bg } = c.value
  const s = props.size
  switch (key) {
    case 'needs-you':
      return {
        width: 0,
        height: 0,
        borderLeft: `${s / 2 + 1}px solid transparent`,
        borderRight: `${s / 2 + 1}px solid transparent`,
        borderBottom: `${s}px solid ${fg}`,
        background: 'transparent'
      }
    case 'blocked':
      return {
        width: `${s}px`,
        height: `${s}px`,
        background: 'transparent',
        border: `1.5px solid ${fg}`,
        backgroundImage: `repeating-linear-gradient(135deg,${fg} 0 1px,transparent 1px 3px)`
      }
    case 'ready':
      return { width: `${s}px`, height: `${s}px`, borderRadius: '50%', border: `1.5px solid ${fg}`, background: 'transparent' }
    case 'done':
      return { width: `${s}px`, height: `${s}px`, borderRadius: '50%', background: fg, opacity: 0.55 }
    case 'failed':
      return { width: `${s}px`, height: `${s}px`, borderRadius: 'var(--radius-1)', background: fg }
    case 'running':
      return {
        width: `${s}px`,
        height: `${s}px`,
        borderRadius: '50%',
        background: fg,
        animation: 'sm-pulse var(--dur-pulse) var(--ease-in-out) infinite'
      }
    default:
      return { width: `${s}px`, height: `${s}px`, borderRadius: 'var(--radius-1)', border: `1.5px solid ${fg}`, background: bg }
  }
})

const style = computed(() => ({ display: 'inline-block', ...shape.value }))
</script>

<template>
  <!-- The dot stays aria-hidden — it is a silhouette, and whatever it sits
       beside carries the accessible name. The key is a hint for the eye only,
       and it is the one thing telling a row of bare silhouettes apart in the
       gallery, which is where they are drawn and checked. `flex` moves to the
       wrapper because the wrapper is the flex item now. -->
  <Tooltip :label="c.key" :style="{ flex: '0 0 auto' }">
    <span aria-hidden="true" :data-attention="level" :style="style" />
  </Tooltip>
</template>
