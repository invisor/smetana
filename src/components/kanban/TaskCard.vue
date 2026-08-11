<script setup>
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import MenuButton from '../overlays/MenuButton.vue'
import DependencyBand from '../status/DependencyBand.vue'
import DependencyMark from '../status/DependencyMark.vue'
import Assignee from './Assignee.vue'
import TypeBadge from './TypeBadge.vue'
import { taskMenuItems } from './taskMenu.js'
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
  busy: { type: Boolean, default: false }
})

/* One event for all four actions rather than four events forwarded three times
   each. The payload names what was asked for; the board is not the thing that
   carries it out, and a card that emitted `delete` would be describing a write
   it has no way to make. */
defineEmits(['click', 'action'])

const hover = ref(false)
const level = computed(() => attentionLevel(props.status))
const dragging = computed(() => props.state === 'dragging')
const drop = computed(() => props.state === 'drop-target')
const changed = computed(() => props.state === 'changed')
/* An agent waiting on an answer is the one thing allowed to shout. */
const loud = computed(() => props.needsResponse || level.value === 'loud')

/* How wide the menu is, and it is a measurement rather than a taste.

   `ContextMenu` applies its `width` as a hard width and clips each label with
   an ellipsis; a menu row has no tooltip and no `title`, so whatever does not
   fit is gone with no way back. The longest label the card can produce is the
   greyed Run row, which carries `scopeBusyReason`'s whole sentence — the reason
   moved out of the play's tooltip, where it used to grow to fit, and into the
   row itself.

   Measured through CoreText at `--text-sm` (12px) in the system sans, which is
   what `--font-sans` resolves to in the webview: "Run this — a run over task
   smetana-hth is already going" is 315px, and 337px for a 14-character issue
   id. `ContextMenu` spends 48px of its width on chrome before the label —
   2×`--border-w`, 2×`--space-2` of panel padding, 2×`--space-4` of row padding,
   the 14px icon column and one `--space-4` gap — so 400 leaves the label 352px.
   That covers every id up to about 14 characters with room to spare for the
   other two webviews' fonts, where Segoe UI and Noto Sans have their own
   metrics and none of this could be measured from here.

   Compact needs no number of its own: density shrinks the space scale and
   leaves `--text-sm` alone, so the chrome costs 40px there instead of 48 and
   the label is 8px wider than it is here. Comfortable is the binding case.

   The app-wide font size is the one thing this does not follow — it is a number
   in px and the type grows past it. The long reason fits to a `uiFontSize` of
   14 and is ellipsised above that, which is the same failure the old tooltip
   never had; widening the panel to cover the top of the range would mean a
   460px menu hanging off a 212px card for everybody, to serve a setting almost
   nobody moves.

   Costing nothing on a narrow board: the panel is fixed-position, right-aligned
   to the trigger and clamped to the window by `EDGE`, so it opens leftwards
   over the card and only a window under ~416px could not hold it. */
const MENU_W = 400

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
        <span :style="idStyle">{{ id }}</span>
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
