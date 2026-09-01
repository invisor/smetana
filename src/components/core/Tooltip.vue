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
   state, not a local override's.

   Two rules decide when the panel goes away, and both are this component's
   rather than the controls' that draw one. A press anywhere in the window takes
   it down: `mouseleave` cannot answer for a press, which moves the pointer
   nowhere, so a menu opening under a stationary pointer used to leave the hint
   standing beside it. And focus opens the panel only when the keyboard is what
   brought it, so the focus a closing menu hands back to its trigger does not put
   the hint straight back up. Neither rule is the design system's: its panel
   closes on the two events a trigger raises for itself, which is enough only for
   a trigger nothing opens over. */
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
  /* The press rule, and `document` is the only place that hears every press:
     in the trigger, in another control, in a card on the board, in empty space.
     Capture rather than bubble, which is what `MenuButton`, `PointerMenu` and
     the notifications panel all do for the same job, and it buys one plain
     ordering fact: capture runs before the handlers on whatever was pressed, so
     the panel is already down by the time that press does whatever it does.
     It goes on when there is a panel and comes off in `hide`, so nothing of
     this component sits on `document` while it has nothing on screen, and a
     second `reveal` without a `hide` between adds nothing, since the DOM
     ignores a listener it already has. */
  document.addEventListener('pointerdown', onDocumentPointerdown, true)
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
  document.removeEventListener('pointerdown', onDocumentPointerdown, true)
}

/* Set when a press inside this trigger took the panel down, and read by
   `onFocusin` alone: it means the focus arriving here is the press's own doing
   and must not put the hint back up.

   `:focus-visible` cannot answer this on its own, and the case it misses is
   Esc. Closing a menu that way is a keyboard interaction, so the focus handed
   back to the trigger is `:focus-visible` by every browser's reckoning — and
   correctly so, since the button is wearing a focus ring. The measured order is
   `pointerdown`, `focusin` on the button, `focusout` to the menu's own box,
   then `focusin` again when Esc gives it back. The two questions are different:
   the pseudo-class asks whether the keyboard brought this focus at all, and
   this flag asks whether this particular panel was just pressed away. Both have
   to be answered, and only the second is this component's to remember.

   Only a press *inside this trigger* arms it. A press on some other control
   still takes this panel down — that is the rule above — but arming on it would
   leave a keyboard user who then tabs here with no hint at all. */
let dismissedByPress = false

/* The press's own arrival, which the flag must not be spent on. A press on a
   trigger that did not already hold the focus raises two focus events here, and
   only the second is the hand-back worth refusing: the browser focuses the
   trigger itself first, and if the flag died on that one it would be gone
   before the menu had even opened.

   This is observed rather than worked out afterwards, and that is the whole
   point of it. `focusin` cannot be asked which arrival it is: its
   `relatedTarget` is wherever the focus came *from*, which for the press's own
   arrival is wherever the person happened to be standing beforehand — an
   inactive document tab, an unselected row in the file tree, a disabled
   control, all of which carry `tabIndex < 0` and are indistinguishable from an
   overlay handing focus back. The handler below runs in capture, before the
   browser has moved the focus at all, so it can state the fact instead.

   The same phase answers the other half, and it has to be asked rather than
   assumed: **whether the press raises an arrival at all.** If the focus is
   already inside the trigger — the button was tabbed to and is then clicked —
   nothing moves and no `focusin` follows, so a flag armed as though one were
   coming would be spent on the hand-back instead, and the panel would survive
   the close. `document.activeElement` in capture is still the pre-press focus,
   which is exactly that question, and it is a fact about this trigger rather
   than an inference from some unrelated element's `tabIndex`. */
let pressArrival = false

/* The press rule and both flags, in the one place that hears every press. */
function onDocumentPointerdown(event) {
  if (trigger.value?.contains(event.target)) {
    dismissedByPress = true
    pressArrival = !trigger.value.contains(document.activeElement)
  }
  hide()
}

/* Whether the focus is being handed off rather than walked away with. A
   `tabIndex` below zero is an element the keyboard cannot reach on its own, so
   a script put the focus there — an overlay opened by this very press, which
   will hand it back when it closes — and the flag has to outlive that round
   trip. Anything the keyboard could have reached, and a focus falling all the
   way back to the document, is somebody leaving: the flag goes, and the next
   `Tab` onto this trigger shows the hint as it always did. */
const handedOff = (related) => !!related && related.tabIndex < 0

/* A press on the trigger, for the one case the listener above cannot have: a
   wait that is running with no panel up yet, and therefore no listener. The
   press ends it, rather than letting a hint open two seconds later over
   whatever the press has since done — a delayed panel explains something a
   person is about to act on, and a column header is also its own drag handle.
   Once the panel is up this is the second handler to hear the same press and
   finds the work already done. The
   rule used to be narrower than this, `delay > 0` only, on the grounds that a
   press closing an undelayed tooltip was nothing anybody had asked for; a menu
   left open beside its own hint is what asked for it. */
function onPointerdown() {
  hide()
}

/* Focus opens the panel only when the keyboard brought it here. A pointer has
   its own way of asking what a glyph means — holding still over it — and what
   this excludes is the focus a script hands back: closing a menu returns the
   focus to the button that opened it, which is right for the keyboard and,
   without this, put the hint back up the instant the press had taken it down,
   beside a pointer that had long since moved on. The browser is the one that
   knows the difference, and `:focus-visible` is where it says so — a Tab
   matches, a `.focus()` after a click does not.

   The flag is the other half, and it is what makes the fallback in
   `keyboardFocus` safe: on an engine that cannot be asked about
   `:focus-visible`, the hand-back after a press is still refused here.

   **The flag guards one arrival and then dies**, and the death is the whole
   reason it cannot mute a keyboard visit later on. Without it the flag outlives
   the interaction that armed it: open a menu with the mouse, change your mind,
   click a tab, and the focus leaves for an element with a negative `tabIndex` —
   a roving tab stop, which `handedOff` cannot tell from an overlay — so
   `onFocusout` keeps the flag and the next `Tab` here finds the hint silently
   suppressed, with nothing on screen to say why.

   Which arrival ends it needs no guessing, because `pressArrival` above says
   whether the press caused one and, if it did, spends the flag on that one so
   the next is the hand-back, whatever brought it. Every closing route the
   acceptance criteria name — Esc, a row picked, a press outside — hands the
   focus back to the trigger, so the flag dies on all three, and it does so
   whether the press came from outside the trigger or from the trigger already
   holding the keyboard's focus. It cannot outlive a close into an interaction
   it knows nothing about.

   What is left is bounded rather than open-ended, and it is measured rather
   than argued: past the arrival the press causes, if it causes one, exactly one
   more is refused — so a flag left armed costs one mute keyboard visit, and the
   hint is back on the visit after it. Two routes still leave one armed, and
   neither is the press itself. A menu can be closed with the
   focus already outside it — `Tab` walks off the last row and out of the panel,
   and the close then hands nothing back. And `handedOff` is asked about a
   departure, where it cannot tell an overlay from a roving tab stop, so a press
   followed by the focus going to any `tabIndex < 0` element — an inactive
   document tab, an unselected row in the file tree — keeps the flag until the
   arrival after it.

   The cost is **per trigger and does not net out**: the flags are separate
   pieces of state on separate components, so two triggers armed this way are
   two mute first visits, one each, not one between them. That was measured on
   two icon buttons armed in turn, and it is the price of the lifecycle above.
   The alternative was to hold the `document` listener open for as long as the
   flag is set, which buys those visits back with a listener per armed tooltip
   sitting on `document` with nothing on screen — and this component is drawn on
   nearly every icon button in the app, so that trade is refused here.

   Only the template's own `focusin` is gated; `show` is left as it is for the
   ancestors that relay into it. */
function onFocusin(event) {
  if (dismissedByPress) {
    if (pressArrival) pressArrival = false
    else dismissedByPress = false
    return
  }
  if (!keyboardFocus(event.target)) return
  show()
}

/* The pointer arriving is a fresh request for the hint, whatever the last press
   was about, so it clears the flag as well as opening the panel. */
function onMouseenter() {
  dismissedByPress = false
  pressArrival = false
  show()
}

/* And the focus leaving for good clears them too — for good being the whole of
   what `handedOff` decides, which is a fair question of a *departure*: this one
   really is about where the focus is going. `pressArrival` goes with it, so a
   press that never moved the focus into the trigger — pressing a control that
   already had it — cannot leave the next arrival looking like the press's. */
function onFocusout(event) {
  if (!handedOff(event.relatedTarget)) {
    dismissedByPress = false
    pressArrival = false
  }
  hide()
}

/* Both halves of what this component leaves behind: the wait, and the press
   listener that `hide` would otherwise be the only one to take off. A tooltip
   goes out of the document with the control it explains — a card leaving the
   board takes several — and a listener holding a dead component's `hide` would
   outlive every one of them. */
onUnmounted(() => {
  cancel()
  document.removeEventListener('pointerdown', onDocumentPointerdown, true)
})

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
  /* Shrink-to-fit measured against the panel's own text rather than against
     wherever it is standing. A fixed box with `left` set has `100vw - left` to
     lay out in, so a panel already placed near the right edge — which only
     happens once a label changes under an open panel, since `reveal` measures
     at `left: 0` — would be measured wrapped, come out two lines tall and
     narrower than its text, and be placed from a width it then re-flows away
     from, ending up flush against the window with none of the margin the clamp
     below keeps. `max-content` makes the natural size a property of the words
     alone; the ceiling on the next line still caps a label longer than the
     window. A keyword, not a measurement, so nothing here is a hardcoded
     value. */
  width: 'max-content',
  /* A ceiling rather than `nowrap`. The panel is shrink-to-fit — that is what
     the line above is for — so a label that fits stays on one line as it did;
     the ceiling only bites on one that does not, and the labels that do not are
     the ones this panel now carries — an absolute project path, a read-only
     reason, a run blocked by a sentence. `nowrap` drew those straight off the
     edge of the window, since the clamp in `place` can only move a panel that
     is narrower than the window it is in. `anywhere` is what breaks a path: it
     has no spaces to break at.

     Two ceilings, and the panel takes whichever is lower. `--tooltip-max-w` is
     the one that decides how a hint reads: a line of prose has a length it is
     comfortable at, and a panel spanning the whole window is one line of text
     the eye has to travel the screen to follow. It is the system's number
     rather than this component's, so every one of the app's hints obeys it and
     none carries a width of its own. The window's own width stays as the second
     operand because it answers a different question — the 2 × EDGE subtracted
     there is the same margin the clamp in `place` keeps, so the widest panel is
     still one the clamp can place, whatever the ceiling above says. */
  maxWidth: `min(var(--tooltip-max-w), calc(100vw - ${2 * EDGE}px))`,
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

<script>
/* Module scope, deliberately: everything in `<script setup>` above is per
   instance, and this is a fact about the engine the app is running in. A
   tooltip is drawn on nearly every icon button in the app, so a probe living up
   there would be re-run once per button for an answer that cannot differ.

   Whether this engine can be asked about `:focus-visible` at all, worked out on
   the first focus rather than at import time, since the throw is the thing
   being avoided and asking eagerly would only move it. WebKit knows the
   pseudo-class from 15.4 and this project's floor is 15, where `matches` on a
   selector it does not recognise throws rather than answering false — inside a
   focus handler, which nothing here would survive. `ColumnHeader` carries the
   same note about the same target.

   The fallback is to open, which is what this component always did: on an
   engine that cannot answer, the keyboard keeps its hint and what is lost is
   the focus rule alone — a press anywhere still takes the panel down, and the
   flag still refuses a hand-back — so the worst case there is the old behaviour
   rather than a hint that cannot be reached. */
let knowsFocusVisible = null

function keyboardFocus(el) {
  if (knowsFocusVisible === null) {
    try {
      document.documentElement.matches(':focus-visible')
      knowsFocusVisible = true
    } catch {
      knowsFocusVisible = false
    }
  }
  if (!knowsFocusVisible) return true
  return el?.matches?.(':focus-visible') === true
}
</script>

<template>
  <span
    ref="trigger"
    :style="{ display: 'inline-flex' }"
    @mouseenter="onMouseenter"
    @mouseleave="hide"
    @focusin="onFocusin"
    @focusout="onFocusout"
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
