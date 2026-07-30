<script setup>
const props = defineProps({
  width: { type: String, default: '100%' },
  height: { type: [Number, String], default: 10 },
  radius: { type: String, default: 'var(--radius-2)' },
  lines: { type: Number, default: 1 }
})

const bar = (i) => ({
  display: 'block',
  // last line stops short, so a multi-line block reads as text and not as bars
  width: props.lines > 1 && i === props.lines - 1 ? '62%' : props.width,
  height: typeof props.height === 'number' ? `${props.height}px` : props.height,
  borderRadius: props.radius,
  background: 'var(--surface-sunken)',
  animation: 'sm-skeleton var(--dur-pulse) var(--ease-in-out) infinite'
})
</script>

<template>
  <span aria-hidden="true" :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }">
    <span v-for="i in lines" :key="i" :style="bar(i - 1)" />
  </span>
</template>
