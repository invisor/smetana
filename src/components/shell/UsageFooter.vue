<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import { usageAgentLabel, usageSegments, usageTooltip } from './usageFooter.js'

/* What is left of the agent's subscription, along the bottom of the app
   window — the sibling of the scope bar at the top of it.

   The two numbers exist in one other place, the Agents tab of the settings
   window, which is a second OS window somebody has to open on purpose. They are
   three words wide and read at a glance by somebody whose attention is on the
   board, which is chrome rather than content: it belongs in a bar that is
   always there and never asks for the eye.

   Presentational, like every component here: the window does the asking, so
   this renders in `?view=gallery` with nothing behind it, and every choice
   between a number, a dash and a sentence belongs to `usageFooter.js`. */
const props = defineProps({
  /* `agent_usage`'s answer whole, in Rust's own shape, or `null` before there
     has been one — the same prop `AgentSettings.vue` takes, and nothing between
     Rust and here unpacks it into flags. */
  usage: { type: Object, default: null },
  /* A probe is out. Unlike the settings block, this does **not** blank the
     numbers: the block clears its rows before every read because a block
     sitting there showing the previous answer would be claiming a reading that
     is being replaced as it is read, and this strip never labels its figure
     fresh, so it claims nothing by keeping it. Blanking a permanent strip every
     ten minutes is a flicker in the corner of somebody's eye. All `busy` does
     here is put a sentence in the hint. */
  busy: { type: Boolean, default: false }
})

/* Somebody asking for the reading sooner than the owner's timer would. The
   owner decides whether that turns into a probe — a press while one is still
   out has to do nothing, since two `claude /usage` processes at once is a cost
   with no answer behind it. */
defineEmits(['refresh'])

const label = computed(() => usageAgentLabel(props.usage))
const segments = computed(() => usageSegments(props.usage))
const tip = computed(() => usageTooltip(props.usage, props.busy))

/* The scope bar's ground, the scope bar's height and the scope bar's type, with
   a rule on top where that one has a rule underneath. **No token of its own:**
   the two strips are the same kind of thing, and a `--footer-bar-h` defined to
   the same two numbers would be a second value to keep in step with the first,
   in both densities, for a difference nobody could see. */
const barStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--scope-bar-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-5)',
  background: 'var(--scope-bar)',
  borderTop: 'var(--border-w) solid var(--border)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  cursor: 'pointer'
}

/* The hint is about the whole strip rather than about any one segment of it, so
   its trigger fills the bar instead of hugging the words. */
const fillStyle = {
  display: 'inline-flex',
  alignItems: 'center',
  alignSelf: 'stretch',
  flex: '1 1 auto',
  minWidth: 0
}
const rowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  flex: '1 1 auto',
  minWidth: 0
}
/* `Icon` takes its size as an SVG attribute, which cannot be a custom property,
   so the token is handed over as CSS instead — the style wins over the
   attribute and the glyph follows the app-wide font size like everything else
   in the row. */
const glyphStyle = {
  width: 'var(--icon-sm)',
  height: 'var(--icon-sm)',
  color: 'var(--text-muted)'
}
const truncate = { whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }
/* Fixed, and never dropped when the half behind it is missing: a segment that
   came and went would move the rest of the row under somebody's eye every ten
   minutes. What changes is the number, or the dash standing where one was not
   read.

   No colour by band, no glyph change and no attention level, at any reading.
   The saturated range in this system belongs to status, and a third colour
   vocabulary at the bottom of the screen would compete with the board's own;
   `loud` is budgeted at one or two rows on a screen, and a strip that is always
   present cannot spend that budget on a figure that is usually unremarkable.
   What replaces it is the number itself — `92%` is legible, and the band's
   consequence is one hover away. */
const segStyle = { whiteSpace: 'nowrap', flex: '0 0 auto' }
</script>

<template>
  <!-- The whole strip is the control: a press asks the harness again, sooner
       than the owner's timer would. Its accessible name is the row it draws,
       which is what somebody would have called it anyway. -->
  <div
    :style="barStyle"
    role="button"
    tabindex="0"
    @click="$emit('refresh')"
    @keydown.enter.prevent="$emit('refresh')"
    @keydown.space.prevent="$emit('refresh')"
  >
    <!-- Empty is a real answer from `usageTooltip` — a reading in a band this
         build cannot name, printing no reset times, leaves nothing true to say —
         and a hint opening on an empty panel would be worse than none. So the
         trigger is a plain span in that case, and the strip is drawn once
         either way rather than twice under a `v-if`. -->
    <component :is="tip ? Tooltip : 'span'" :label="tip || undefined" :style="fillStyle">
      <span :style="rowStyle">
        <Icon name="bot" :style="glyphStyle" />
        <span :style="truncate">{{ label }}</span>
        <span v-for="segment in segments" :key="segment.name" :style="segStyle">
          {{ segment.name }} {{ segment.value }}
        </span>
      </span>
    </component>
  </div>
</template>
