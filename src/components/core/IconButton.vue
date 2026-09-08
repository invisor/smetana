<script setup>
import { computed, mergeProps, toRef, useAttrs } from 'vue'
import { useInteractive } from './interactive.js'
import Icon from './Icon.vue'
import Tooltip from './Tooltip.vue'

/* Refusal is `aria-disabled`, never the native `disabled` attribute, and it is
   the decision `core/Button.vue` already carries, taken here for the same
   reason: a natively disabled button takes no focus in any browser, so it
   leaves the Tab order entirely and takes the sentence saying why it is refused
   out with it. That file's header is the account of all four guards below and
   of the way rejected — a `tabindex` on `Tooltip`'s span — and it is not
   repeated here. Nothing in this change touches what a refused icon button is
   drawn as: the style object below is the one it always had.

   What differs first is how much of the app it is about. `Button` had to be
   read together with the tooltips a few of its callers happen to put around it;
   this component puts **every** instance of itself inside one (see the block
   below), so an icon button's hint is the only prose there is about it — a
   refused one's reason included. The play in a column header is the case that
   found it: `kanban/ColumnHeader.vue` builds "Run the queue — …" out of the
   sentence saying why the queue cannot start and hands it to an `IconButton`
   refused by that same sentence, so until now the reason was the pointer's
   alone.

   What differs second is the order in the template: the guards are bound before
   `v-bind="buttonAttrs"`, where `Button` has them after. Not for the click —
   the standard settles that one, the capture pass running the whole path, the
   target included, before the bubble pass begins. It is for the other two,
   which are same-key merges: `mergeProps` concatenates `onKeydown` and
   `onMousedown` with whatever a caller brought in the order it meets them, and
   guard first is the order to want. It is also the only order that survives a
   caller passing a `@click.capture` of its own, where both listeners are
   capture and registration order is all there is left to decide by. None does
   today, which is why this is written down rather than leant on.

   One measured qualification, and it is the platform's rather than this file's:
   WebKit puts no `<button>` in the Tab order at all by default — enabled or
   refused, this component's or anything else's — so on WKWebView the stop this
   change buys appears only with Full Keyboard Access on. That is the same
   setting `git/GitPanel.vue` records for mouse focus, and it was measured the
   same way, by walking the gallery with Tab in both engines: Blink stops on
   every button, WebKit on none. Nothing here can change that and nothing here
   should try — what this file owes either engine is that the refusal is stated
   where a screen reader hears it and the panel is reachable wherever the
   keyboard reaches buttons at all.

   One measured consequence of refusing the press its focus, and it is
   `Tooltip`'s own bounded cost rather than a new one. A press inside a trigger
   arms the flag that stops a hint springing back up on the focus a press hands
   over; here no focus follows the press, so the flag is spent on the next
   arrival instead — the first Tab onto a refused button that was just pressed
   is silent, and the Tab after it opens the panel. Measured in Chromium, and
   `Button` pays the same on the same line; `Tooltip`'s header keeps the ledger
   for it, and the pointer entering the trigger again clears it.

   Neither `Tooltip` nor `ColumnHeader` needed a line changed, and that was
   checked rather than assumed: the two are a pair that once came apart over
   this very thing, and making a refused button focusable moves where a
   `focusin` comes from underneath the relay. The header is the tooltip's
   *ancestor*, so it relays the focus into a panel that cannot hear it, and the
   relay answers only when `event.target === event.currentTarget` — a focus
   landing on a button inside the header, which is what this change adds for the
   refused ones, is not the column's hint to open. The button's own `Tooltip` is
   its parent, so it hears that `focusin` bubble exactly as it does for the
   buttons that were never refused. The press side has the same shape: the
   header's `pointerdown` already ran for every enabled icon button in it, and
   already refuses to start a drag when the press landed on a `<button>`, which
   a refused one still is. */

/* The hint is `Tooltip`, never the native `title`, and it is here rather than at
   every call site so that every icon-only button in the app has one by default.
   `label` is required whatever `hint` says, so there is always something to say
   and always an accessible name: turning the panel off takes away what is drawn
   on hover and nothing a screen reader hears.

   `hint` is the way out of the default, and one caller takes it. `Toast` sits
   in the corner of the window for a few seconds and its cross would open a
   panel upwards, over the app's own content, to name a glyph that reads as
   itself — a hint whose whole life is spent in the way of something. Every
   other icon button in the app keeps its own.

   Attributes do not fall through to the wrapper: `Tooltip`'s span is the root
   now, and a caller sizing the control — `Tab`'s 16px close, `CodeBlock`'s 18px
   copy, the setup gear in the left panel's header — means the
   button, not the box around it. They are merged with `mergeProps` rather than
   spread into one object, which is what fallthrough itself does and the only
   form that keeps both sides of a collision: object spread would drop this
   component's own hover tracking the moment a caller passed `@mouseenter`.
   `$attrs` goes second so a caller's value wins, again as fallthrough did. */
defineOptions({ inheritAttrs: false })

const props = defineProps({
  icon: { type: String, required: true },
  /* Icon-only, so the label is the accessible name — never optional. */
  label: { type: String, required: true },
  size: { type: String, default: 'md' },
  variant: { type: String, default: 'ghost' },
  disabled: { type: Boolean, default: false },
  selected: { type: Boolean, default: false },
  /* Whether the button explains itself on hover — see the note above for the
     one caller that says no. `label` is untouched by it. */
  hint: { type: Boolean, default: true }
})

const { hover, active, handlers } = useInteractive(toRef(props, 'disabled'))

const attrs = useAttrs()
const buttonAttrs = computed(() => mergeProps(handlers, attrs))

/* The press, refused, which is what the native attribute did for free. Capture,
   and `stopImmediatePropagation`, so that neither the caller's own `@click` nor
   an ancestor's hears a press a disabled control never made — a row that
   selects on a click, a header that is its own drag handle. The header above
   says why all three are bound before `v-bind="buttonAttrs"` rather than after
   it, and this is the one of the three that does not need it. */
const onClickCapture = (event) => {
  if (!props.disabled) return
  event.preventDefault()
  event.stopImmediatePropagation()
}

/* And the keyboard's two ways of pressing a focused button, which the browser
   turns into a click of its own: cancelling the keydown is what stops that
   click from ever being made, and it is the same line that stops Space
   scrolling the board out from under a button that just refused it.

   Propagation is deliberately left alone, and here that is load-bearing rather
   than tidy: alt with an arrow moves a column, and the header listens for it as
   the key travels up from whatever inside it holds the focus — which, after
   this change, can be the refused play. A refused button is not the thing to
   swallow the board's own shortcut. */
const onKeydown = (event) => {
  if (!props.disabled) return
  if (event.key !== 'Enter' && event.key !== ' ') return
  event.preventDefault()
}

/* And the press's own default action, which is the focus. Without it a press on
   a refused button pulls the keyboard out of whatever field somebody was
   filling and parks it there with no ring to say so — on every engine that
   makes a `<button>` mouse-focusable at all, which is Blink and the GTK port
   both, since that port carves form controls out of WebKit's rule.
   `git/GitPanel.vue` carries the measurement, and what the Mac port does
   instead. */
const onMousedown = (event) => {
  if (props.disabled) event.preventDefault()
}

const box = computed(() =>
  props.size === 'sm' ? 'var(--control-h-sm)' : props.size === 'lg' ? 'var(--control-h-lg)' : 'var(--control-h)'
)
const glyphSize = computed(() => (props.size === 'sm' ? 13 : props.size === 'lg' ? 18 : 15))

const bg = computed(() => {
  if (active.value) return 'var(--surface-active)'
  if (hover.value) return 'var(--surface-hover)'
  if (props.selected) return 'var(--surface-selected)'
  return props.variant === 'solid' ? 'var(--action-secondary-bg)' : 'transparent'
})

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: box.value,
  height: box.value,
  color: props.disabled ? 'var(--text-muted)' : props.selected ? 'var(--text-primary)' : 'var(--text-secondary)',
  background: bg.value,
  border: `var(--border-w) solid ${props.variant === 'solid' ? 'var(--border)' : 'transparent'}`,
  borderRadius: 'var(--radius-3)',
  cursor: props.disabled ? 'not-allowed' : 'default',
  opacity: props.disabled ? 0.6 : 1,
  transition: 'var(--transition-control)',
  padding: 0
}))
</script>

<template>
  <Tooltip :label="label" :enabled="hint">
    <button
      type="button"
      :aria-disabled="disabled || undefined"
      :aria-label="label"
      :aria-pressed="selected || undefined"
      :style="style"
      @click.capture="onClickCapture"
      @keydown="onKeydown"
      @mousedown="onMousedown"
      v-bind="buttonAttrs"
    >
      <Icon :name="icon" :size="glyphSize" />
    </button>
  </Tooltip>
</template>
