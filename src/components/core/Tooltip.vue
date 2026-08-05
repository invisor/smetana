<script setup>
/* A hint about the thing under the pointer.

   This diverges from the design system in behaviour, not in styling, the same
   way Resizer does, and for a reason the design system never had to face: its
   panel is absolutely positioned inside the trigger, which any scrolling
   ancestor clips. `ProjectList` is one — a list capped at five rows — and a
   tooltip inside it was cut off above and below by the list's own edges, so
   the only direction left to open was sideways, into the gap after the
   project's name, where the panel read as a slab wedged into the row rather
   than a hint about a glyph.

   So the panel is teleported to the body and positioned in window
   coordinates: nothing clips it, and it can be told where to go rather than
   opening blindly into whatever is there. It opens on the side asked for when
   that side has room, flips to the opposite one when it does not, and slides
   along the other axis to stay inside the window. This belongs back upstream.

   The cost is a measurement, and it is paid once per hover: the panel is put
   in the document hidden, measured at its natural size, then placed and
   revealed. A tooltip left open while its trigger scrolls away will hang
   where it was — hover ends the moment the pointer leaves, so the window for
   that is a stray frame.

   Leaving the trigger's DOM also leaves its stacking context, which is why the
   panel sits at `--z-popover` rather than `--z-tooltip`: nesting used to settle
   the order for free, and at 200 against a modal's 300 a tooltip inside a dialog
   would now go behind it. `--z-popover` is the scale's answer to that, added
   when `Dropdown` needed the same thing — the ordering is the design system's to
   state, not a local override's. */
import { computed, nextTick, ref } from 'vue'

const props = defineProps({
  label: { type: String, required: true },
  shortcut: { type: String, default: '' },
  /* Which side to open on when there is room for it. */
  side: { type: String, default: 'top' }
})

/* The distance from the trigger, carried over from the `calc(100% + 6px)` the
   design system's own CSS used, and the closest the panel may come to the
   window's edge. Neither is a token reference, and cannot be: these are
   operands in arithmetic against getBoundingClientRect, not values handed to
   the browser, and the spacing scale is not readable as a number from here. */
const GAP = 6
const EDGE = 8

const open = ref(false)
const trigger = ref(null)
const tip = ref(null)
/* Where the panel goes, in window coordinates. Null while it is in the
   document but not yet measured — the one state the panel must not be seen
   in, since it would be sitting in the window's corner. */
const at = ref(null)

const clamp = (value, min, max) => Math.max(min, Math.min(value, max))

async function show() {
  open.value = true
  at.value = null
  await nextTick()

  const anchor = trigger.value?.getBoundingClientRect()
  const panel = tip.value?.getBoundingClientRect()
  // The pointer can leave again before this resolves, which unmounts the panel.
  if (!anchor || !panel) return

  const { innerWidth: w, innerHeight: h } = window
  let top
  let left

  if (props.side === 'left' || props.side === 'right') {
    const fitsRight = anchor.right + GAP + panel.width + EDGE <= w
    const fitsLeft = anchor.left - GAP - panel.width - EDGE >= 0
    const toRight = props.side === 'left' ? !fitsLeft : fitsRight
    left = toRight ? anchor.right + GAP : anchor.left - GAP - panel.width
    top = anchor.top + anchor.height / 2 - panel.height / 2
  } else {
    const fitsAbove = anchor.top - GAP - panel.height - EDGE >= 0
    const fitsBelow = anchor.bottom + GAP + panel.height + EDGE <= h
    const above = props.side === 'bottom' ? !fitsBelow : fitsAbove
    top = above ? anchor.top - GAP - panel.height : anchor.bottom + GAP
    left = anchor.left + anchor.width / 2 - panel.width / 2
  }

  /* Both axes, whichever one the placement above already settled: it slides
     the panel along the free axis to keep it whole, and it is also the only
     thing standing between a window too small for either side and a panel
     drawn off the edge. Max() guards the degenerate case where the panel is
     wider or taller than the window itself, which would otherwise hand clamp
     a maximum below its minimum. */
  at.value = {
    top: clamp(top, EDGE, Math.max(EDGE, h - panel.height - EDGE)),
    left: clamp(left, EDGE, Math.max(EDGE, w - panel.width - EDGE))
  }
}

function hide() {
  open.value = false
  at.value = null
}

const tipStyle = computed(() => ({
  position: 'fixed',
  zIndex: 'var(--z-popover)',
  top: `${at.value?.top ?? 0}px`,
  left: `${at.value?.left ?? 0}px`,
  visibility: at.value ? 'visible' : 'hidden',
  // Nothing here is the pointer's business: the panel now sits outside the
  // trigger, so without this it could take a hover away from what it explains.
  pointerEvents: 'none',
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  whiteSpace: 'nowrap',
  padding: 'var(--space-2) var(--space-4)',
  background: 'var(--surface-overlay)',
  fontFamily: 'var(--font-sans)',
  color: 'var(--text-primary)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-2)',
  boxShadow: 'var(--shadow-overlay)',
  fontSize: 'var(--text-xs)'
}))
</script>

<template>
  <span
    ref="trigger"
    :style="{ display: 'inline-flex' }"
    @mouseenter="show"
    @mouseleave="hide"
    @focusin="show"
    @focusout="hide"
  >
    <slot />
    <Teleport to="body">
      <span v-if="open" ref="tip" role="tooltip" :style="tipStyle">
        {{ label }}
        <kbd v-if="shortcut" :style="{ color: 'var(--text-muted)', fontSize: 'var(--text-2xs)' }">{{ shortcut }}</kbd>
      </span>
    </Teleport>
  </span>
</template>
