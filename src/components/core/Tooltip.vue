<script setup>
/* A hint about the thing under the pointer.

   This diverges from the design system in behaviour, not in styling, the same
   way Resizer does, and for a reason the design system never had to face: its
   panel is absolutely positioned inside the trigger, which any scrolling
   ancestor clips. The project list this app used to draw was one — capped at
   five rows — and a tooltip inside it was cut off above and below by the list's
   own edges, so the only direction left to open was sideways, into the gap
   after the project's name, where the panel read as a slab wedged into the row
   rather than a hint about a glyph. The rail that replaced that list scrolls
   too, and its tiles ask for `side="right"` out of the strip for the same
   reason.

   So the panel is teleported to the body and positioned in window
   coordinates: nothing clips it, and it can be told where to go rather than
   opening blindly into whatever is there. It opens on the side asked for when
   that side has room, flips to the opposite one when it does not, and slides
   along the other axis to stay inside the window. This belongs back upstream.

   The cost is a measurement, paid once per hover — and once more each time the
   label changes while the panel is up, since the panel's size changes with it:
   the panel is put in the document hidden, measured at its natural size, then
   placed and revealed. A tooltip left open while its trigger scrolls away will
   hang where it was — hover ends the moment the pointer leaves, so the window
   for that is a stray frame.

   Leaving the trigger's DOM also leaves its stacking context, which is why the
   panel sits at `--z-popover` rather than `--z-tooltip`: nesting used to settle
   the order for free, and at 200 against a modal's 300 a tooltip inside a dialog
   would now go behind it. `--z-popover` is the scale's answer to that, added
   when `Dropdown` needed the same thing — the ordering is the design system's to
   state, not a local override's. */
import { computed, nextTick, onUnmounted, ref, watch } from 'vue'

const props = defineProps({
  label: { type: String, required: true },
  shortcut: { type: String, default: '' },
  /* Which side to open on when there is room for it. */
  side: { type: String, default: 'top' },
  /* How long the pointer — or the focus — has to stay before the panel appears,
     in milliseconds. Zero is the shipped behaviour and the default, so no
     tooltip in the app waits for anything unless it asks to: a control's own
     name is what somebody is already looking for, and making them wait for it
     would be a cost paid on every hover in the interface. What a delay buys is
     the other kind of hint — prose about the thing under the pointer, on a
     surface people cross constantly on the way to something else, where a panel
     opening at once is in the way rather than of use. */
  delay: { type: Number, default: 0 }
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

/* The wait, when there is one. It is kept here rather than in whichever
   component asked for a delay: the panel is this component's to open, and a
   timer living outside it would be a second half of the same rule, free to
   disagree with `hide` about when the panel is gone. */
let timer = null

const cancel = () => {
  if (timer === null) return
  clearTimeout(timer)
  timer = null
}

/* One entry point for both `mouseenter` and `focusin`, so a delay is one rule
   rather than two: what waits is the whole of the opening, measurement
   included, since measuring first would put the panel in the document — and on
   screen for a frame — before anybody had held still for it. */
function show() {
  cancel()
  if (props.delay > 0) {
    timer = setTimeout(() => {
      timer = null
      reveal()
    }, props.delay)
    return
  }
  reveal()
}

async function reveal() {
  open.value = true
  at.value = null
  await nextTick()
  place()
}

/* Where the panel goes, measured against where it is now. Split out of
   `reveal` for the one caller below that has to place an already open panel
   and must not touch whether it is open — a label that changes under a
   stationary pointer is a different question from a hover starting. */
function place() {
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

/* A label that changes while the panel is up is placed again, and that is the
   whole of what this does: it never opens the panel and never closes it, so
   the rule about when a tooltip appears stays where it was, in `show` and in
   whatever ancestor relays into it. The measurement in `reveal` is taken once,
   against the text the panel had then, and a wider one — `Copied` after
   `Copy id` — would otherwise grow to one side and sit off-centre over the
   thing it explains, or past the edge of the window near it. `post` so the
   panel has already been redrawn with the new text when it is measured. */
watch(
  () => props.label,
  () => {
    if (open.value) place()
  },
  { flush: 'post' }
)

function hide() {
  cancel()
  open.value = false
  at.value = null
}

/* A press on the trigger ends the wait and takes down whatever is up, and only
   where a delay was asked for. A delayed panel explains something a person is
   about to act on — a column header is also its own drag handle — so a hint
   that had just earned its two seconds would otherwise hang over a column
   already moving. With no delay this is nothing anybody asked for, and every
   tooltip in the app today would change behaviour on a press. */
function onPointerdown() {
  if (props.delay > 0) hide()
}

onUnmounted(cancel)

/* Relays for an ancestor that owns the focus, and nothing wider than that.
   `focusin` bubbles up from whatever took the focus, so an element wrapping
   this one — a column header, which is its own drag handle and therefore the
   thing that takes the focus — never puts this span on the event's path.
   Handing it the same two entry points is what keeps one timer and one rule;
   the alternative was a second wait, kept in the component that owns the
   header.

   The contract that comes with them: whoever calls `show` owns the matching
   `hide`. This is deliberately **not** a way to open a tooltip programmatically
   — no `open` state is exposed to reconcile against, so a caller that shows and
   forgets leaves a panel that only its own `hide`, a `mouseleave` or a
   `focusout` can take down, sitting over the interface until one of the three
   happens. Every caller is expected to be relaying a pair of real events. */
defineExpose({ show, hide })

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
  /* A ceiling rather than `nowrap`. A fixed box with no width is already
     shrink-to-fit, so a label that fits stays on one line exactly as it did;
     the ceiling only bites on one that does not, and the labels that do not are
     the ones this panel now carries — an absolute project path, a read-only
     reason, a run blocked by a sentence. `nowrap` drew those straight off the
     edge of the window, since the clamp in `show` can only move a panel that is
     narrower than the window it is in. `anywhere` is what breaks a path: it has
     no spaces to break at. The 2 × EDGE subtracted here is the same margin the
     clamp keeps, so the widest panel is one the clamp can still place. */
  maxWidth: `calc(100vw - ${2 * EDGE}px)`,
  overflowWrap: 'anywhere',
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
    @pointerdown="onPointerdown"
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
