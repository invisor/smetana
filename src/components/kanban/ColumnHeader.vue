<script setup>
import { computed, nextTick } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'
import { attentionLevel, statusColors, statusGlyph } from '../status/status.js'

const props = defineProps({
  status: { type: String, required: true },
  count: { type: Number, default: 0 },
  wipLimit: { type: Number, default: null },
  /* Not every column accepts new issues: in bd an issue is born in one status
     only, and the "+" stands where pressing it actually works. */
  addable: { type: Boolean, default: true },
  /* The header doubles as the column's drag handle. It only reports the
     gesture — where the column ends up is the board's to decide, since the
     board is what holds the list. */
  movable: { type: Boolean, default: false },
  moving: { type: Boolean, default: false },
  /* A run takes the whole ready queue, so the play stands in exactly one
     column — which one is the board's to decide, the same as `addable`. */
  runnable: { type: Boolean, default: false },
  /* Why the run cannot be started just now, in words. Empty means it can. A
     lowercase fragment, because it is interpolated into `runLabel` below
     rather than standing on its own. A sentence rather than a boolean, and the
     button goes grey rather than away: a control that vanished says nothing
     about why it is not there, and the one thing somebody wants to know at
     that moment is exactly why. */
  runBlockedReason: { type: String, default: '' },
  /* Whether this column's whole contents can be moved into the queue in one
     press — `deferred` in practice, and a prop for the same reason `addable`
     and `runnable` are ones: there is no fixed set of columns here. Unlike
     those two the button also needs something to move, so an empty column
     draws none: the count beside it is already 0, and a control whose only
     possible answer is "nothing to do" says less than the number does. */
  promotable: { type: Boolean, default: false }
})

const emit = defineEmits(['add', 'grab', 'move', 'run', 'promote'])

const c = computed(() => statusColors(props.status))
const over = computed(() => props.wipLimit != null && props.count > props.wipLimit)
const label = computed(() => c.value.key.replace(/-/g, ' '))

/* A glyph rather than `StatusDot`'s silhouette: a column names one status and
   only ever that one, so the shape can say what the status *means* instead of
   telling apart six statuses standing side by side, which is the job the dot
   exists for on a card. Every column gets one, the generic tag included, so a
   custom status is not the only header on the board with a gap where the
   others have a glyph. */
const glyph = computed(() => statusGlyph(props.status))

/* The one moving thing on the board, and only while something is actually
   moving. A spinner over an empty running column claims work that is not
   there — and since it never stopped, it also said nothing when work started.
   Motion means something happened; an idle board must look idle. */
const spinning = computed(() => c.value.key === 'running' && props.count > 0)

const glyphStyle = computed(() => ({
  color: c.value.fg,
  opacity: attentionLevel(props.status) === 'quiet' ? 'var(--attn-quiet-opacity)' : 1,
  animation: spinning.value ? 'sm-spin var(--dur-pulse) linear infinite' : undefined
}))

/* A pointerdown on the "+" is a press of that button and nothing else. Without
   this the button still works — a click survives a drag that never passed its
   threshold — but the column follows the pointer while somebody aims at it. */
const onPointerdown = (event) => {
  if (!props.movable || event.button !== 0) return
  if (event.target.closest('button')) return
  emit('grab', event)
}

/* Alt, not the bare arrow: a board is scrolled with arrow keys, and taking
   them would mean a focused header could no longer be moved past by keyboard
   without dragging the column along.

   The refocus afterwards is not a nicety. Columns are keyed by status, so Vue
   moves this very element rather than rebuilding it — but moving a node is a
   removal and an insertion, and the browser blurs what it removes. Without this
   the first alt+arrow works, focus lands on the body, and the second one
   silently does nothing. `focus` also brings the column back into view, which
   is the other half of what a keyboard move owes a board wider than its pane. */
const onKeydown = (event) => {
  if (!props.movable || !event.altKey) return
  if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return
  event.preventDefault()
  const el = event.currentTarget
  emit('move', event.key === 'ArrowRight' ? 1 : -1)
  nextTick(() => el.focus())
}

const style = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  height: 'var(--row-h)',
  flex: '0 0 auto',
  padding: '0 var(--space-3) 0 var(--space-4)',
  borderBottom: `var(--border-w-strong) solid ${c.value.border}`,
  marginBottom: 'var(--space-4)',
  /* Being dragged is a surface step up, the same as every other interaction in
     this system — never a colour change and never a transform, so a column
     under the pointer cannot jump away from it. */
  background: props.moving ? 'var(--surface-active)' : 'transparent',
  borderRadius: 'var(--radius-2) var(--radius-2) 0 0',
  cursor: props.movable ? (props.moving ? 'grabbing' : 'grab') : 'default',
  /* Without this a touch drag scrolls the board instead of moving the column,
     and the pointer capture never sees the moves. */
  touchAction: props.movable ? 'none' : 'auto'
}))

const nameStyle = computed(() => ({
  font: 'var(--weight-semibold) var(--text-xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: c.value.fg,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}))

/* Only a movable header takes focus, and then it owes the keyboard a name that
   says what focusing it is for — the label is the entire discoverability of
   alt+arrow. A header that cannot move stays out of the tab order: a stop that
   does nothing is worse than no stop. */
const moveLabel = computed(() => `Column ${label.value}. Alt with left or right arrow moves it.`)

/* One sentence for the tooltip and for the accessible name both — the panel a
   person reads and the name a screen reader announces must not disagree about
   the same button. This is the last play on the board: a card has no play any
   more, and the same fragment reaches it as a menu row's own label, composed by
   `kanban/taskMenu.js` in the same shape ("Run this — …"). */
const runLabel = computed(() =>
  props.runBlockedReason ? `Run the queue — ${props.runBlockedReason}` : 'Run the queue'
)

/* Counted, because the number is the whole of what makes this press decidable
   — and it is the same number the dialog is about to ask over. "Move" rather
   than "promote": the board never uses that word, and what a person sees happen
   is a column emptying into another one. */
const promoteLabel = computed(() =>
  `Move ${props.count} ${props.count === 1 ? 'task' : 'tasks'} to ready`
)

const wipStyle = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: '2px',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  color: over.value ? 'var(--status-failed-fg)' : 'var(--text-muted)'
}))
</script>

<template>
  <div
    :style="style"
    :tabindex="movable ? 0 : undefined"
    :aria-label="movable ? moveLabel : undefined"
    @pointerdown="onPointerdown"
    @keydown="onKeydown"
  >
    <Icon :name="glyph" :size="12" :stroke-width="2" :style="glyphStyle" />
    <span :style="nameStyle">{{ label }}</span>
    <span :style="{ font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }">
      {{ count }}
    </span>
    <Tooltip v-if="wipLimit != null" :label="`WIP limit ${wipLimit}`">
      <span :style="wipStyle">
        <Icon :name="over ? 'triangle-alert' : 'gauge'" :size="10" :stroke-width="2" />/{{ wipLimit }}
      </span>
    </Tooltip>
    <span :style="{ flex: 1 }" />
    <slot name="actions">
      <!-- Before the "+", so the "+" keeps the position it has always had:
           nothing a person is already aiming at moves when a project gains a
           configuration. -->
      <IconButton
        v-if="runnable"
        icon="play"
        :label="runLabel"
        size="sm"
        :disabled="!!runBlockedReason"
        @click="$emit('run')"
      />
      <!-- Also before the "+", and for the same reason. In practice no column
           carries this and the play both — one names the queue, the other names
           what is waiting outside it — but the order is fixed here rather than
           left to which of them a board happens to switch on. -->
      <IconButton
        v-if="promotable && count > 0"
        icon="arrow-right-to-line"
        :label="promoteLabel"
        size="sm"
        @click="$emit('promote')"
      />
      <IconButton v-if="addable" icon="plus" :label="`Add task to ${c.key}`" size="sm" @click="$emit('add')" />
    </slot>
  </div>
</template>
