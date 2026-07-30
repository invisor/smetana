<script setup>
import { computed } from 'vue'
import { parseAnsi } from './ansi.js'

const props = defineProps({
  text: { type: String, default: '' },
  segments: { type: Array, default: null }
})

const segs = computed(() => props.segments || parseAnsi(props.text || ''))

const segStyle = (s) => ({
  color: s.color || 'inherit',
  fontWeight: s.bold ? 'var(--weight-semibold)' : undefined,
  opacity: s.dim ? 0.65 : 1
})
</script>

<template>
  <span><span v-for="(s, i) in segs" :key="i" :style="segStyle(s)">{{ s.text }}</span></span>
</template>
