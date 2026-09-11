<script setup>
/* The one loud thing on the panel: the agent is holding a tool call open and
   cannot go on until somebody answers.

   Drawn at `loud`, which in this system means the fill rather than a tint:
   `statusColors('needs-you')` for the ground and the frame, with
   `var(--surface-raised)` as the ink — exactly what `StatusBadge` does at that
   level, which is why the ink inverts with the theme for free. That is the
   whole of the colour budget spent in one place, and the budget holds by
   construction here: a session has at most one unanswered question, so a panel
   cannot show two of these.

   **The detail wraps and is never clipped.** Every other place a command or a
   path appears in this group cuts it off with an ellipsis, because those are
   read by scanning; this one is the single place where the exact thing being
   agreed to has to be readable to its last character. `overflowWrap: anywhere`
   is what keeps a long unbroken argument inside the card rather than widening
   the panel.

   The refusal is an ordinary button and deliberately not `danger`. Red would be
   a second saturated thing inside a card that is already saturated, and the
   frame around it has said everything loudness has to say. `allow` leads as the
   primary — ink on paper, no hue — and every other option is secondary.

   The options arrive in the order the worker offers them and are drawn in that
   order; a harness offering two of them draws two buttons, with nothing here
   filling the gap. No countdown and no automatic deny: the permission listener
   holds the request open with a heartbeat for as long as a person takes. */
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import { STATUS_GLYPH, statusColors } from '../status/status.js'

defineProps({
  /* The tool's name, as the harness spells it — an identifier, so mono. */
  tool: { type: String, required: true },
  /* The one line of what it is about to do: a command, a path. */
  detail: { type: String, default: '' },
  /* Any of `allow`, `allow-always`, `deny` — `session::model::Decision`. */
  options: { type: Array, default: () => ['allow', 'allow-always', 'deny'] }
})

defineEmits(['answer'])

/* Reserved, so this is a constant rather than a computed: `needs-you` always
   resolves to the same four token references. */
const c = statusColors('needs-you')

const LABEL = { allow: 'Allow', 'allow-always': 'Allow always', deny: 'Deny' }

const label = (option) => LABEL[option] ?? option

const card = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-5)',
  padding: 'var(--space-5)',
  background: c.fg,
  color: 'var(--surface-raised)',
  border: `var(--border-w) solid ${c.fg}`,
  borderRadius: 'var(--radius-4)',
  fontFamily: 'var(--font-sans)'
}

const head = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  flexWrap: 'wrap',
  font: 'var(--weight-medium) var(--text-sm)/var(--leading-snug) var(--font-sans)'
}

const toolName = { font: 'var(--weight-semibold) var(--text-sm)/var(--leading-snug) var(--font-mono)' }

/* A bar down the left rather than a second surface. Every surface token in the
   system is a step in the neutral ladder and would read as a hole cut in the
   fill; the rule from the ink itself separates the command from the sentence
   above it and stays legible in both themes. */
const command = {
  paddingLeft: 'var(--space-4)',
  borderLeft: 'var(--border-w-strong) solid var(--surface-raised)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-code) var(--font-mono)',
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere'
}

const actions = { display: 'flex', flexWrap: 'wrap', gap: 'var(--space-3)' }

const glyph = STATUS_GLYPH[c.key]
</script>

<template>
  <div data-attention="loud" :style="card">
    <div :style="head">
      <Icon :name="glyph" :size="13" :stroke-width="2.25" />
      <span :style="toolName">{{ tool }}</span>
      <span>needs your permission</span>
    </div>
    <div v-if="detail" :style="command">{{ detail }}</div>
    <div :style="actions">
      <Button
        v-for="option in options"
        :key="option"
        size="sm"
        :variant="option === 'allow' ? 'primary' : 'secondary'"
        @click="$emit('answer', option)"
      >{{ label(option) }}</Button>
    </div>
  </div>
</template>
