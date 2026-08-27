<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import MenuButton from '../overlays/MenuButton.vue'
import DependencyBand from '../status/DependencyBand.vue'
import DependencyMark from '../status/DependencyMark.vue'
import Assignee from './Assignee.vue'
import TypeBadge from './TypeBadge.vue'
import { copyLabel } from './copyId.js'
import { MENU_W, taskMenuItems } from './taskMenu.js'
import { attentionLevel } from '../status/status.js'

const props = defineProps({
  id: { type: String, required: true },
  title: { type: String, required: true },
  /* The card no longer draws its status: it is already the column it sits in,
     and saying it twice spends the one badge a card has on nothing. What the
     status still decides is the card's loudness — the border, the flash, the
     dimming of anything done — so the prop stays. */
  status: { type: String, default: 'ready' },
  /* bd's own status, which the column is not: a blocked card sits in the
     `blocked` column while bd holds it at `open`. The menu's Move to… offers
     bd's vocabulary, so it needs bd's word. */
  bdStatus: { type: String, default: '' },
  /* bd's issue type, drawn in the corner the status used to hold. Absent on a
     card the tracker has not typed, and then nothing is drawn. */
  type: { type: String, default: undefined },
  assignee: { type: Object, default: null },
  blockedBy: { type: Number, default: 0 },
  blocks: { type: Number, default: 0 },
  /* The ids behind those counts, for the hint on the marks. Optional: a board
     that has only the numbers still draws the card. */
  blockedByIds: { type: Array, default: () => [] },
  blockingIds: { type: Array, default: () => [] },
  spawnedFrom: { type: String, default: undefined },
  needsResponse: { type: Boolean, default: false },
  state: { type: String, default: 'default' },
  changedBy: { type: String, default: undefined },
  selected: { type: Boolean, default: false },
  /* Whether this card can be run on its own. The board decides — it is a
     product rule and it depends on things this component has never heard of. */
  runnable: { type: Boolean, default: false },
  /* Why it cannot be run just now, in words; empty means it can. A lowercase
     fragment, because `taskMenu.js` interpolates it into the row's own label
     rather than standing it on its own. The row is drawn greyed rather than
     taken away, and the sentence is what it is drawn for — a Run that simply
     disappeared while a run was going would read as the board having lost a
     feature. */
  runBlockedReason: { type: String, default: '' },
  /* A write on this issue is in flight — a status change or a deletion. Every
     row greys, for the reason the panel's own select used to: a bd call takes
     about two seconds and a live menu invites a second choice racing the
     first. */
  busy: { type: Boolean, default: false },
  /* What happened to the last attempt to copy *this* card's id: `''` before
     anything was asked, `'copied'` or `'failed'` after. It changes the text of
     one tooltip and nothing else — no colour, no surface, no size. The copying
     itself belongs to the view: exactly one file under `src/components/`
     imports a store, and it is not this one. */
  copyState: { type: String, default: '' }
})

/* One event for all four actions rather than four events forwarded three times
   each. The payload names what was asked for; the board is not the thing that
   carries it out, and a card that emitted `delete` would be describing a write
   it has no way to make. */
defineEmits(['click', 'action', 'copy-id'])

/* What the id's tooltip says, from `copyId.js` — the same three words the
   inspector's id uses, in the one place a test can read them. */
const idLabel = computed(() => copyLabel(props.copyState))

const hover = ref(false)
const level = computed(() => attentionLevel(props.status))
const dragging = computed(() => props.state === 'dragging')
const drop = computed(() => props.state === 'drop-target')
const changed = computed(() => props.state === 'changed')
/* An agent waiting on an answer is the one thing allowed to shout. */
const loud = computed(() => props.needsResponse || level.value === 'loud')

/* What the menu offers, and every refusal in it. The rule is `taskMenu.js`,
   which is the part of this a test can reach — a card is a `.vue` and nothing
   in this repository can open one. */
const menuItems = computed(() =>
  taskMenuItems({
    bdStatus: props.bdStatus,
    runnable: props.runnable,
    runBlockedReason: props.runBlockedReason,
    busy: props.busy
  })
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

/* The pointer is the whole of what the id's new job changes about how it looks:
   it stays the same muted mono it has always been, with no underline, no rule
   and no hover colour. A copy is not a place to go and must not read as a
   link. */
const idStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  letterSpacing: 'var(--tracking-tight)',
  cursor: 'pointer'
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
/* `overflowWrap` is what the two inspectors already carry, and the card wanted it
   for the same reason: a title is bd's text, and bd's text contains identifiers.
   A browser breaks a line at spaces and hyphens and at nothing else, so
   `some_test_name_like_this` is one word to it — wider than a 212px column, and
   drawn straight over the card's own border and the column beside it, since
   nothing here clips. `anywhere` rather than `break-word` to match the inspectors,
   and because it is the one of the two that also lets the flex column shrink
   below that word: `break-word` breaks the line but still reports the whole token
   as the minimum width, which is how the overflow comes back the moment a panel
   drag makes the board narrower than one identifier. */
const titleStyle = {
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-snug)',
  color: 'var(--text-primary)',
  textWrap: 'pretty',
  overflowWrap: 'anywhere'
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
        <!-- `.stop`, so a click on the id is a question about the id and not a
             visit to the task: the right-hand panel keeps showing whatever it
             was showing. A click anywhere else on the card still selects it.
             A span rather than a button, unlike the inspector's id: there is
             one id there and dozens here, and a tab stop per card is a cost
             everybody crossing the board with a keyboard pays for a
             convenience only the pointer has. -->
        <Tooltip :label="idLabel">
          <span :style="idStyle" @click.stop="$emit('copy-id', id)">{{ id }}</span>
        </Tooltip>
        <span :style="{ flex: 1 }" />
        <!-- Drawn whether or not the pointer is here, and on every card rather
             than only a runnable one. Acting on a task is what a person comes
             to this board to do, and a control that only exists under the
             pointer has to be found before it can be used. It is quiet — a
             muted glyph on no surface until it is hovered itself — which is
             what lets it be always there without joining the card's argument
             for attention. Edit, Delete and Move to… are meaningful on a done
             card and on a blocked one, so what a card cannot do is a greyed
             row inside, never a missing button outside. -->
        <MenuButton
          :items="menuItems"
          :label="`Actions for ${id}`"
          :width="MENU_W"
          size="sm"
          @select="$emit('action', { kind: $event.kind, id, value: $event.value })"
        />
        <Tooltip v-if="needsResponse" label="Agent is waiting for your answer">
          <span :style="askStyle">
            <Icon name="message-circle-question-mark" :size="9" :stroke-width="2.5" />ASK
          </span>
        </Tooltip>
        <Tooltip v-if="changedBy" :label="`Changed by ${changedBy} since you last looked`">
          <span :style="newStyle">
            <Icon name="dot" :size="12" :stroke-width="3" />new
          </span>
        </Tooltip>
      </div>
      <div :style="titleStyle">{{ title }}</div>
      <div :style="{ display: 'flex', alignItems: 'center', gap: 'var(--space-5)', flexWrap: 'wrap' }">
        <TypeBadge v-if="type" :type="type" size="sm" />
        <span :style="{ flex: 1 }" />
        <DependencyMark
          :blocked-by="blockedBy"
          :blocks="blocks"
          :blocked-by-ids="blockedByIds"
          :blocking-ids="blockingIds"
          :spawned-from="spawnedFrom"
          size="sm"
        />
        <Assignee v-if="assignee" :kind="assignee.kind" :name="assignee.name" />
      </div>
    </div>
  </div>
</template>
