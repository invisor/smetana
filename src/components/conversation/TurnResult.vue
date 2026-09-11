<script setup>
/* The close of a turn: what it spent. One dimmed mono line and nothing else —
   this is the footnote under an answer, not the answer.

   Every field of it is a number the person did not choose, so the whole line is
   `var(--font-mono)`: the token counts line up down a conversation, which is
   the only way a turn that cost ten times the last one is noticed at all.

   **A cost of `null` is omitted rather than drawn as `$0`.** There is a
   difference between a turn that was free and a harness that did not say, and
   `$0.000` under a turn that ran for four seconds is the panel inventing the
   first of them. `cost_usd` is `Option<f64>` in `session::model` for exactly
   this reason.

   The duration is written at the scale a person reads it at: milliseconds while
   it is under a second, then seconds to one decimal, then minutes — a turn of
   two minutes rendered as `124.3 s` is arithmetic somebody has to do. */
import { computed } from 'vue'

const props = defineProps({
  tokensIn: { type: Number, default: 0 },
  tokensOut: { type: Number, default: 0 },
  /* Dollars, or `null` where the harness said nothing. */
  costUsd: { type: Number, default: null },
  ms: { type: Number, default: 0 }
})

function duration(ms) {
  const value = Number(ms) || 0
  if (value < 1000) return `${Math.round(value)} ms`
  if (value < 60000) return `${(value / 1000).toFixed(1)} s`
  const minutes = Math.floor(value / 60000)
  const seconds = Math.round((value % 60000) / 1000)
  return `${minutes} m ${String(seconds).padStart(2, '0')} s`
}

const line = computed(() => {
  const parts = [`${props.tokensIn} in`, `${props.tokensOut} out`]
  if (props.costUsd != null) parts.push(`$${props.costUsd.toFixed(3)}`)
  parts.push(duration(props.ms))
  return parts.join(' · ')
})

const style = {
  padding: 'var(--space-3) var(--panel-pad)',
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}
</script>

<template>
  <div :style="style">{{ line }}</div>
</template>
