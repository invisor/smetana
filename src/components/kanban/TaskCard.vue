<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'
import DependencyBand from '../status/DependencyBand.vue'
import DependencyMark from '../status/DependencyMark.vue'
import Assignee from './Assignee.vue'
import TypeBadge from './TypeBadge.vue'
import { attentionLevel } from '../status/status.js'

const props = defineProps({
  id: { type: String, required: true },
  title: { type: String, required: true },
  /* The card no longer draws its status: it is already the column it sits in,
     and saying it twice spends the one badge a card has on nothing. What the
     status still decides is the card's loudness — the border, the flash, the
     dimming of anything done — so the prop stays. */
  status: { type: String, default: 'ready' },
  /* bd's issue type, drawn in the corner the status used to hold. Absent on a
     card the tracker has not typed, and then nothing is drawn. */
  type: { type: String, default: undefined },
  assignee: { type: Object, default: null },
  blockedBy: { type: Number, default: 0 },
  blocks: { type: Number, default: 0 },
  spawnedFrom: { type: String, default: undefined },
  needsResponse: { type: Boolean, default: false },
  state: { type: String, default: 'default' },
  changedBy: { type: String, default: undefined },
  selected: { type: Boolean, default: false },
  /* Whether this card can be run on its own. The board decides — it is a
     product rule and it depends on things this component has never heard of. */
  runnable: { type: Boolean, default: false },
  /* Why it cannot be run just now, in words; empty means it can. A lowercase
     fragment, because it is interpolated into `runLabel` below rather than
     standing on its own. The button is drawn inactive rather than taken away,
     and the sentence is what it is drawn for — a play that simply disappeared
     while a run was going would read as the board having lost a feature. */
  runBlockedReason: { type: String, default: '' }
})

defineEmits(['click', 'run'])

const hover = ref(false)
const level = computed(() => attentionLevel(props.status))
const dragging = computed(() => props.state === 'dragging')
const drop = computed(() => props.state === 'drop-target')
const changed = computed(() => props.state === 'changed')
/* An agent waiting on an answer is the one thing allowed to shout. */
const loud = computed(() => props.needsResponse || level.value === 'loud')

/* One sentence for the tooltip and for the accessible name both. Two strings
   would mean the panel a person reads and the name a screen reader announces
   disagreeing about the same button — and `ColumnHeader` composes its own play
   the same way, so the two paths say the same thing in the same words. */
const runLabel = computed(() =>
  props.runBlockedReason ? `Run this — ${props.runBlockedReason}` : 'Run this'
)

const borderColor = computed(() => {
  if (props.selected) return 'var(--focus-ring)'
  if (drop.value) return 'var(--border-strong)'
  if (loud.value) return 'var(--attn-loud)'
  return 'var(--border)'
})

const style = computed(() => ({
  display: 'flex',
  gap: 'var(--space-4)',
  alignItems: 'stretch',
  padding: 'var(--card-pad)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  background: drop.value ? 'var(--surface-sunken)' : 'var(--surface-raised)',
  border: `var(--border-w) solid ${borderColor.value}`,
  borderStyle: drop.value ? 'dashed' : 'solid',
  borderRadius: 'var(--radius-3)',
  boxShadow: dragging.value
    ? 'var(--shadow-drag)'
    : props.selected
      ? '0 0 0 1px var(--focus-ring)'
      : 'var(--shadow-raised)',
  opacity: level.value === 'quiet' && !hover.value ? 'var(--attn-quiet-opacity)' : dragging.value ? 0.9 : 1,
  transform: dragging.value ? 'rotate(-.4deg)' : 'none',
  // "this changed while you weren't looking" — twice, then gone
  animation: changed.value ? 'sm-flash var(--dur-flash) var(--ease-out) 2' : undefined,
  cursor: 'default',
  transition: 'var(--transition-control)',
  outline: hover.value && !props.selected ? '1px solid var(--border-strong)' : 'none',
  outlineOffset: '-1px'
}))

const idStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  letterSpacing: 'var(--tracking-tight)'
}
const askStyle = {
  display: 'inline-flex', alignItems: 'center', gap: '3px', padding: '0 5px', height: '15px',
  background: 'var(--attn-loud)', color: 'var(--attn-loud-contrast)', borderRadius: 'var(--radius-2)',
  font: 'var(--weight-semibold) var(--text-2xs)/1 var(--font-mono)', letterSpacing: 'var(--tracking-caps)'
}
const newStyle = {
  display: 'inline-flex', alignItems: 'center', gap: '3px', color: 'var(--attn-loud)',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)'
}
const titleStyle = {
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-snug)',
  color: 'var(--text-primary)',
  textWrap: 'pretty'
}
</script>

<template>
  <div
    tabindex="0"
    :data-attention="level"
    :style="style"
    @click="$emit('click')"
    @mouseenter="hover = true"
    @mouseleave="hover = false"
  >
    <DependencyBand :blocked-by="blockedBy" :blocks="blocks" />
    <div :style="{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }">
      <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }">
        <span :style="idStyle">{{ id }}</span>
        <span :style="{ flex: 1 }" />
        <!-- Drawn whether or not the pointer is here. Starting a task is the
             one thing a person comes to this board to do, and a control that
             only exists under the pointer has to be found before it can be
             used. It is quiet — a muted glyph on no surface until it is
             hovered itself — which is what lets it be always there without
             joining the card's argument for attention. -->
        <!-- Wrapped the way `ColumnHeader` wraps its own play, and for the
             reason that one proved: a native `title` is browser chrome, not
             page content — it waits a second, cannot be styled, and what a
             disabled control does with it is the engine's business, which is
             three different engines here. `Tooltip` is page content and its
             wrapper span takes the hover even though its only child is
             disabled. -->
        <Tooltip v-if="runnable" :label="runLabel" title="">
          <IconButton
            icon="play"
            :label="runLabel"
            size="sm"
            :disabled="!!runBlockedReason"
            @click.stop="$emit('run')"
          />
        </Tooltip>
        <span v-if="needsResponse" title="Agent is waiting for your answer" :style="askStyle">
          <Icon name="message-circle-question-mark" :size="9" :stroke-width="2.5" />ASK
        </span>
        <span v-if="changedBy" :title="`Changed by ${changedBy} since you last looked`" :style="newStyle">
          <Icon name="dot" :size="12" :stroke-width="3" />new
        </span>
      </div>
      <div :style="titleStyle">{{ title }}</div>
      <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-5)', flexWrap: 'wrap' }">
        <TypeBadge v-if="type" :type="type" size="sm" />
        <span :style="{ flex: 1 }" />
        <DependencyMark :blocked-by="blockedBy" :blocks="blocks" :spawned-from="spawnedFrom" size="sm" />
        <Assignee v-if="assignee" :kind="assignee.kind" :name="assignee.name" />
      </div>
    </div>
  </div>
</template>
