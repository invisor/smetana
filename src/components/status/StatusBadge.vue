<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import { STATUS_GLYPH, attentionLevel, statusCode, statusColors } from './status.js'

const props = defineProps({
  status: { type: String, required: true },
  count: { type: [Number, String], default: null },
  size: { type: String, default: 'md' },
  variant: { type: String, default: 'soft' },
  showGlyph: { type: Boolean, default: true }
})

const c = computed(() => statusColors(props.status))
const level = computed(() => attentionLevel(props.status))
/* A loud status fills solid; that is the whole 1–2-per-screen colour budget. */
const loud = computed(() => level.value === 'loud' && props.variant !== 'plain')
const sm = computed(() => props.size === 'sm')

const glyph = computed(() => STATUS_GLYPH[c.value.key])
const code = computed(() => statusCode(c.value.key))
const label = computed(() => c.value.key.replace(/-/g, ' '))

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  height: sm.value ? '15px' : '18px',
  padding: `0 ${sm.value ? 5 : 6}px`,
  background: loud.value ? c.value.fg : props.variant === 'plain' ? 'transparent' : c.value.bg,
  color: loud.value ? 'var(--surface-raised)' : c.value.fg,
  border: `var(--border-w) solid ${loud.value ? c.value.fg : c.value.border}`,
  borderRadius: 'var(--radius-2)',
  font: `var(--weight-medium) ${sm.value ? 'var(--text-2xs)' : 'var(--text-xs)'}/1 var(--font-mono)`,
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  whiteSpace: 'nowrap',
  opacity: level.value === 'quiet' ? 'var(--attn-quiet-opacity)' : 1
}))

const spin = computed(() => (c.value.key === 'running' ? { animation: 'sm-spin 1.6s linear infinite' } : undefined))
</script>

<template>
  <span :data-attention="level" :style="style">
    <template v-if="showGlyph">
      <Icon v-if="c.reserved" :name="glyph" :size="sm ? 9 : 11" :stroke-width="2.25" :style="spin" />
      <!-- generated statuses get a 2-letter code, so colour is never alone -->
      <span v-else :style="{ fontWeight: 'var(--weight-semibold)', opacity: 0.75 }">{{ code }}</span>
    </template>
    <span>{{ label }}</span>
    <span v-if="count != null" :style="{ opacity: 0.7 }">{{ count }}</span>
  </span>
</template>
