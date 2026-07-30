<script setup>
import { computed } from 'vue'

/* THE ONE LOUD IDEA: tasks arrive from a graph tracker (blocks / spawned-from /
   relates-to). This is where the boldness is spent.
   - blockedBy > 0  -> diagonal hatch band on the card edge (pattern, not colour)
   - blocks > 0     -> hairline band
   Hatch is a repeating-linear-gradient: cheap to repaint while scrolling. */
const props = defineProps({
  blockedBy: { type: Number, default: 0 },
  blocks: { type: Number, default: 0 },
  orientation: { type: String, default: 'vertical' }
})

const on = computed(() => props.blockedBy > 0)

const style = computed(() => {
  const vertical = props.orientation === 'vertical'
  return {
    flex: '0 0 auto',
    width: vertical ? 'var(--accent-bar-w)' : '100%',
    height: vertical ? 'auto' : 'var(--accent-bar-w)',
    alignSelf: 'stretch',
    background: on.value ? undefined : props.blocks > 0 ? 'var(--graph-line)' : 'transparent',
    backgroundImage: on.value
      ? 'repeating-linear-gradient(135deg,var(--hatch-blocked) 0 1.5px,transparent 1.5px 4px)'
      : undefined,
    borderRadius: 'var(--radius-1)'
  }
})
</script>

<template>
  <span aria-hidden="true" :style="style" />
</template>
