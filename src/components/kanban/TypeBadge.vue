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
  size: { type: String, default: 'md' },
  /* Drawn on a card bd holds `blocked`, beside the status badge that already
     leads it there: the type still has to be legible, but a locked card is
     one badge, not two arguing for the same amount of attention, so this one
     gives its fill up entirely — a bare `--border` outline and `--text-muted`
     rather than either type colour or the neutral `--type-plain-*` pair. */
  muted: { type: Boolean, default: false }
})

const c = computed(() => typeColors(props.type))
const sm = computed(() => props.size === 'sm')

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  height: sm.value ? '15px' : '18px',
  padding: `0 ${sm.value ? 5 : 6}px`,
  background: props.muted ? 'transparent' : c.value.bg,
  color: props.muted ? 'var(--text-muted)' : c.value.fg,
  border: props.muted ? 'var(--border-w) solid var(--border)' : 'none',
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
