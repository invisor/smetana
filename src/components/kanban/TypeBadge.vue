<script setup>
/* The type of an issue, drawn to be recognised before it is read.

   Same geometry as StatusBadge — height, radius, padding, glyph size — so a
   card keeps its rhythm, and two differences on purpose: sentence-case sans
   instead of uppercase mono, and no border. Those are what stop a red "Bug"
   from being mistaken for a red status pill, since the two vocabularies share
   hues and there is no free hue space left in which to separate them. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import { typeColors, typeGlyph, typeLabel } from './issueType.js'

const props = defineProps({
  /* bd's own name for the type. Anything outside bd's six renders neutral. */
  type: { type: String, required: true },
  size: { type: String, default: 'md' }
})

const c = computed(() => typeColors(props.type))
const sm = computed(() => props.size === 'sm')

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  height: sm.value ? '15px' : '18px',
  padding: `0 ${sm.value ? 5 : 6}px`,
  background: c.value.bg,
  color: c.value.fg,
  borderRadius: 'var(--radius-2)',
  font: `var(--weight-medium) ${sm.value ? 'var(--text-2xs)' : 'var(--text-xs)'}/1 var(--font-sans)`,
  whiteSpace: 'nowrap'
}))
</script>

<template>
  <span :style="style">
    <Icon :name="typeGlyph(type)" :size="sm ? 9 : 11" :stroke-width="2.25" />
    <span>{{ typeLabel(type) }}</span>
  </span>
</template>
