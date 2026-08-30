<script setup>
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import { agentsLabel, dirtyLabel } from './statusCounters.js'
import { usageAgentLabel, usageSegments, usageTooltip } from './usageFooter.js'

/* The strip along the bottom of the app window — the sibling of the scope bar
   at the top of it, and the one bar in this app that is about state rather than
   about place.

   It draws two things, one at each end. On the left, what is left of the
   agent's subscription: the two numbers exist in one other place, the Agents
   tab of the settings window, which is a second OS window somebody has to open
   on purpose. They are three words wide and read at a glance by somebody whose
   attention is on the board, which is chrome rather than content.

   On the right, what this project is doing right now — the headline, the
   uncommitted files, the live agents and a segment per run. All four used to
   sit in the scope bar, which meant the window's title bar was also its status
   bar; they came down here so that bar can go back to answering "where am I
   working" alone. One strip and not two: a second row along the bottom would
   take another row's height from the board for content that is empty at both
   ends most of the time.

   The subscription keeps the left end, and with it the window's bottom-left
   corner — the easiest target on the screen, which the row below moves its own
   padding inside the bar to keep. The reverse arrangement reads better in
   theory, the changing half first, and costs that corner; on macOS it would
   also put the press target in the bottom-right, which is the window's resize
   area.

   Presentational, like every component here: the window does the asking, so
   this renders in `?view=gallery` with nothing behind it, and every choice
   between a number, a dash and a sentence belongs to `usageFooter.js`, the
   counters' own nouns to `statusCounters.js` and the headline's words to
   `headline.js`. */
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
  busy: { type: Boolean, default: false },
  /* What the last read was refused with, in one readable line. Not the same
     thing as an allowance that could not be read — that is an answer and has
     its own sentence; this is the channel failing, and the strip has one place
     to say so. */
  error: { type: String, default: null },
  /* `null` and deliberately not `0`, the way `git/SectionHeader.vue` declares
     its own count: a working tree that could not be read has an unknown number
     of uncommitted files, which is the opposite fact to a clean one, and
     `stores/vcs.js` says so by handing over `null`. Both come out as no icon
     and no number here — the difference is that nothing in this component ever
     claims a repository is tidy on the strength of not knowing. */
  dirtyCount: { type: Number, default: null },
  /* Zero, because this one is never unknown: the store counts sessions and
     start tickets it is already holding, and there is no read behind it that
     could fail. */
  agentsActive: { type: Number, default: 0 },
  /* What this project is doing right now, from components/shell/headline.js.
     Empty is the ordinary case and draws nothing at all — most of the time
     nothing is happening, and a strip reserving room for the sentence would be
     a strip with a hole in it. */
  headline: { type: String, default: '' },
  /* The design system's attention vocabulary, so an agent waiting on somebody
     reads loud here as it does on a badge. `quiet` is the default. */
  headlineLevel: { type: String, default: 'quiet' }
})

/* Somebody asking for the reading sooner than the owner's timer would. The
   owner decides whether that turns into a probe — a press while one is still
   out has to do nothing, since two `claude /usage` processes at once is a cost
   with no answer behind it. */
defineEmits(['refresh'])

const label = computed(() => usageAgentLabel(props.usage))
const segments = computed(() => usageSegments(props.usage))
const tip = computed(() => usageTooltip(props.usage, props.busy, props.error))

/* The counters' hints, both of them a rule rather than a string: see
   `statusCounters.js` for why they are not written out here. */
const dirtyTip = computed(() => dirtyLabel(props.dirtyCount))
const agentsTip = computed(() => agentsLabel(props.agentsActive))

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
  background: 'var(--scope-bar)',
  borderTop: 'var(--border-w) solid var(--border)',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}

/* The hint is about the subscription rather than about the strip, so its
   trigger hugs the subscription's own words. It used to fill the bar, which was
   true while the bar carried one thing and stopped being true the moment it
   carried two: a press on the agents counter would have run `claude /usage`,
   and a run's stop button would have been a button inside a button — invalid,
   and behaved differently in each of the three webviews this app runs in. The
   row inside still stretches rather than centring, so what a person presses and
   what the hint opens over are the same shape. */
const fillStyle = {
  display: 'inline-flex',
  alignItems: 'stretch',
  alignSelf: 'stretch',
  flex: '0 1 auto',
  minWidth: 0
}
/* The row carries the padding, the cursor and the ring, and none of the three
   is arbitrary. The padding is here rather than on the bar because the bar is
   not the control: left there, it would be a gutter between the window's edge
   and the words, neither pressable nor hoverable — and that gutter is the
   window's bottom-left corner, the easiest target on the whole screen to hit.
   The cursor follows it for the same reason: an affordance over ground that
   does not answer a press is the affordance lying about where the control is.

   The ring is pulled inside its own edge. `base.css` draws `:focus-visible`
   with `outline-offset: 1px`, and this row's bottom edge is the window's, under
   an ancestor that clips: the ring's bottom would be cut away entirely and its
   top drawn into the board above, erasing the bar's own rule under it. Negative
   offset is `AttachmentStrip.vue`'s answer to the same clipping ancestor. */
const rowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  flex: '1 1 auto',
  minWidth: 0,
  padding: '0 var(--space-5)',
  cursor: 'pointer',
  outlineOffset: 'calc(var(--border-w-strong) * -1)'
}
/* `Icon` takes its size as an SVG attribute, which cannot be a custom property,
   so the token is handed over as CSS instead — the style wins over the
   attribute and the glyph follows the app-wide font size like everything else
   in the row. Every glyph on this strip is sized through it, the moved ones
   included: they arrived from the scope bar with a literal 12 on them, and one
   `bot` beside another `bot` a couple of pixels smaller is a difference a
   person sees without being able to say what it is. */
const glyphSize = {
  width: 'var(--icon-sm)',
  height: 'var(--icon-sm)'
}
/* The subscription's own glyph, which names nothing and takes the muted
   colour. The counters below inherit theirs from the segment instead — the
   colour is what tells the two apart. */
const glyphStyle = { ...glyphSize, color: 'var(--text-muted)' }
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

/* The other end of the strip: everything about this project rather than about
   the machine, and a sibling of the trigger above rather than anything inside
   it. The padding is its own for the reason the row's is — the bar carries the
   ground and nothing else.

   No `minWidth: 0` here, and that is the whole of how the strip gives way. A
   flex item's automatic minimum is its min-content width, and the headline
   inside clips its own overflow, so that minimum comes to the counters and the
   run segments — exactly the floor this group must not shrink past. The shrink
   factor is far above the 1 the subscription row has, so a narrow window takes
   its space from here first, and inside here the headline is the only thing
   with anything to give. */
const stateStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  flex: '0 100 auto',
  paddingRight: 'var(--space-5)'
}

/* The one segment on this strip that is a sentence rather than a name or a
   number, which is why it is the one that ellipsises: at 900px the
   subscription and both counters keep their letters and this gives way.

   Loud takes the attention colour and the glyph below with it. Colour alone is
   what `status/status.js` refuses everywhere else, and a headline saying
   somebody is waited on is exactly the case that rule is written for. */
const headlineStyle = computed(() => ({
  ...truncate,
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  flex: '0 100 auto',
  minWidth: 0,
  color: props.headlineLevel === 'loud' ? 'var(--attn-loud)' : 'var(--text-muted)'
}))

/* A counter is its glyph and its number in the one colour, and the colour is
   the whole of what tells the two apart at a glance — which is why each also
   carries a glyph of its own and a hint in words. */
const counter = (color) => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  flex: '0 0 auto',
  color
})
</script>

<template>
  <!-- The bar carries the ground, the height and the rule, and nothing else:
       the one control on it is the subscription's row, and the project's state
       at the other end is not a control at all. -->
  <div :style="barStyle">
    <!-- Empty is a real answer from `usageTooltip` — a reading in a band this
         build cannot name, printing no reset times, leaves nothing true to say —
         and a hint opening on an empty panel would be worse than none. So the
         trigger is a plain span in that case, and the strip is drawn once
         either way rather than twice under a `v-if`. -->
    <component :is="tip ? Tooltip : 'span'" :label="tip || undefined" :style="fillStyle">
      <!-- The control sits **inside** the tooltip's trigger, which is the
           nesting every other control in the tree uses (`core/IconButton.vue`).
           It is not a detail of taste: `Tooltip` opens on `focusin`, which
           bubbles up from whatever took the focus, so a focusable element
           outside it never puts the trigger on the event's path — the hint
           would open on a hover and never on a keyboard, and this hint is the
           strip's only channel for the reset times, for what a run would do,
           and for why there is no reading at all.

           A press asks the harness again, sooner than the owner's timer would.
           The accessible name is the row it draws, which is what somebody would
           have called it anyway. -->
      <span
        :style="rowStyle"
        role="button"
        tabindex="0"
        @click="$emit('refresh')"
        @keydown.enter.prevent="$emit('refresh')"
        @keydown.space.prevent="$emit('refresh')"
      >
        <Icon name="bot" :style="glyphStyle" />
        <span :style="truncate">{{ label }}</span>
        <span v-for="segment in segments" :key="segment.name" :style="segStyle">
          {{ segment.name }} {{ segment.value }}
        </span>
      </span>
    </component>

    <!-- The two ends are pushed apart rather than spaced: the subscription
         keeps the window's corner, and the project's state keeps the other. -->
    <span :style="{ flex: 1 }" />

    <!-- What this project is doing, in the order it stood in the scope bar.
         Outside the trigger above, which is the whole point of the split: a
         press here answers to nothing, and a run's stop button is a button in
         its own right rather than one nested inside another. -->
    <div :style="stateStyle">
      <!-- No `v-else`, no placeholder and no reserved width: an empty headline
           is the common case and the strip simply closes up around it. -->
      <span v-if="headline" :style="headlineStyle">
        <Icon v-if="headlineLevel === 'loud'" name="triangle-alert" :style="glyphSize" />
        <span :style="truncate">{{ headline }}</span>
      </span>

      <Tooltip v-if="dirtyCount > 0" :label="dirtyTip">
        <span :style="counter('var(--git-modified)')">
          <Icon name="file-pen" :style="glyphSize" />{{ dirtyCount }}
        </span>
      </Tooltip>
      <Tooltip v-if="agentsActive > 0" :label="agentsTip">
        <span :style="counter('var(--attn-live)')">
          <Icon name="bot" :style="glyphSize" />{{ agentsActive }}
        </span>
      </Tooltip>

      <!-- Whatever else belongs to this project right now — today the runs, one
           segment each. The strip's own gap spaces them; `RunBar` draws nothing
           for a run it was not given, so an empty list costs no width. -->
      <slot name="status" />
    </div>
  </div>
</template>
