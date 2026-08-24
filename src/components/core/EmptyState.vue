<script setup>
import { computed } from 'vue'
import Icon from './Icon.vue'

/* Terse and factual: "Empty" / "Nothing in ready." No apologies, no emoji. */
const props = defineProps({
  icon: { type: String, default: 'inbox' },
  title: { type: String, required: true },
  description: { type: String, default: '' },
  tone: { type: String, default: 'neutral' },
  compact: { type: Boolean, default: false }
})

const err = computed(() => props.tone === 'error')

const style = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 'var(--space-4)',
  textAlign: 'center',
  fontFamily: 'var(--font-sans)',
  padding: props.compact ? 'var(--space-6)' : 'var(--space-9) var(--space-6)',
  color: 'var(--text-muted)'
}))

const titleStyle = computed(() => ({
  color: err.value ? 'var(--status-failed-fg)' : 'var(--text-secondary)',
  fontSize: 'var(--text-sm)',
  fontWeight: 'var(--weight-medium)'
}))

/* How wide that line is allowed to be. Above the note rather than below it,
   so the note stays next to `detailStyle`, which is what it is about. */
const DETAIL_W = '420px'

/* The `detail` slot: one line of machine words under the sentence of human
   ones. It is a slot rather than a second string prop because what goes in it
   is never this component's to word — it is whatever the thing that failed
   said, and the caller is the only party that has it.

   Mono, because everything in it is an identifier or a diagnostic, and one
   line with an ellipsis because there is no bottom to how long such a line can
   be: bd will hand over a paragraph as readily as a sentence, and an empty
   state that grows to fit one has stopped being an empty state. What is cut off
   is not lost — the whole of it is what the second button hands to an agent,
   which is what makes this a hint rather than the payload.

   **The cap is the component's own, and that is the point of it.** `maxWidth:
   '100%'` alone is not a cap at all: the box is then whatever the caller's
   layout happens to be, so the same string ellipsises in a different place on
   every screen, and in a flex row at `min-width: auto` it does not ellipsise —
   it takes a line of its own at full width, which is exactly what the gallery
   entry was doing when it claimed to be demonstrating this.

   420px rather than the 280px `description` above uses, deliberately. That one
   is a prose measure, chosen so the eye finds the start of the next line, and
   there is no next line here. What a wider box buys is more of the diagnostic
   before the cut, and half again as much is about twenty more characters of it
   — while staying visibly narrower than any panel this sits in, so the cut is
   this component's decision and not the window's. A px value for a measure is
   what `description` already does in this file; nothing here is a colour, a
   space, a radius or a size.

   Muted rather than `--status-failed-fg`, even under `tone="error"`: the title
   above is already the loud part, and this stays information. */
const detailStyle = computed(() => ({
  fontFamily: 'var(--font-mono)',
  fontSize: 'var(--text-xs)',
  color: 'var(--text-muted)',
  maxWidth: `min(${DETAIL_W}, 100%)`,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}))
</script>

<template>
  <div :style="style">
    <Icon
      :name="err ? 'triangle-alert' : icon"
      :size="compact ? 16 : 20"
      :style="{ color: err ? 'var(--status-failed-fg)' : 'var(--text-muted)' }"
    />
    <div :style="titleStyle">{{ title }}</div>
    <div
      v-if="description"
      :style="{ fontSize: 'var(--text-xs)', maxWidth: '280px', lineHeight: 'var(--leading-normal)' }"
    >
      {{ description }}
    </div>
    <div v-if="$slots.detail" :style="detailStyle">
      <slot name="detail" />
    </div>
    <div v-if="$slots.action" :style="{ marginTop: 'var(--space-3)' }">
      <slot name="action" />
    </div>
  </div>
</template>
