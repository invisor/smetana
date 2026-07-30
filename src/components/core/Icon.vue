<script setup>
/* Icons are Lucide (ISC). This wrapper only renders the icon's path data, so
   nothing is drawn by hand and the build tree-shakes to the handful of glyphs
   actually registered in icons.js — see the ~10 MB binary budget.
   Swap icons.js and nothing else in the system changes. */
import { computed, watchEffect } from 'vue'
import { iconNodes } from './icons.js'

const props = defineProps({
  name: { type: String, required: true },
  size: { type: [Number, String], default: 16 },
  strokeWidth: { type: [Number, String], default: 1.75 },
  title: { type: String, default: undefined }
})

/* A lucide IconNode is the whole element: ["svg", attrs, children].
   Only the children are ours to draw — the svg attrs are set below so size,
   stroke width and colour follow the design system, not lucide's defaults. */
const children = computed(() => iconNodes[props.name]?.[2] || [])

if (import.meta.env.DEV) {
  // A missing glyph renders as an empty box; say so instead of failing silently.
  watchEffect(() => {
    if (!iconNodes[props.name]) console.warn(`Icon: "${props.name}" is not registered in icons.js`)
  })
}
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    :stroke-width="strokeWidth"
    stroke-linecap="round"
    stroke-linejoin="round"
    :aria-hidden="title ? undefined : 'true'"
    :aria-label="title"
    style="display: block; flex: 0 0 auto"
  >
    <component :is="child[0]" v-for="(child, i) in children" :key="i" v-bind="child[1]" />
  </svg>
</template>
