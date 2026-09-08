<script setup>
/* Refusal is `aria-disabled`, never the native `disabled` attribute, and that
   is a decision about the keyboard alone: nothing below changes what a refused
   button is drawn as.

   A natively disabled button takes no focus in any browser, so it leaves the
   Tab order entirely — and takes with it the one thing that says why it is
   refused. `Tooltip` opens on `focusin`, so a disabled button inside one has a
   reason the pointer can read and the keyboard cannot reach at all. The three
   verbs over the Git panel's branches are exactly that: the hint on Fetch, Pull
   and Push is the sentence explaining why the press is unavailable, and it was
   the mouse's alone. Staying focusable is the whole of the fix; what it costs
   is the events the attribute used to swallow for free, which the three
   handlers below take over.

   The third of them is the one this cost the most to learn, and it is the
   press rather than the keyboard. The focus is the default action of
   `mousedown`, which the attribute refused by construction and a guard on
   `click` is far too late to see: on the Chromium-based webviews, pressing a
   refused button pulled the focus out of wherever it was and parked it there
   with nothing on screen to say so, since `:focus-visible` does not match a
   press. Two flows in this app put a text field directly above a button that
   is refused precisely while the field is being filled — the commit message
   over `CommitBox`'s Commit, the title over `NewTaskModal`'s Create — so half
   a commit message, a press on the greyed button, and the rest of the typing
   went into a button that swallows Space and Enter. Cancelling the press's
   default action is the whole of the fix, and it costs nothing this task
   wanted: the tab stop is what the change is for, and Tab does not go through
   `mousedown`.

   The other way was to give `Tooltip`'s wrapper span a `tabindex` when — and
   only when — what it wraps is not focusable. Rejected on blast radius rather
   than on price. The condition has to be measured out of the rendered slot, so
   the wrapper would be querying its own contents on every update to answer it;
   and wherever the answer is "nothing focusable in here" it makes a tab stop
   out of a span with no role and no accessible name. Most of this app's
   tooltips wrap precisely that — a status glyph, a column's name and count, a
   WIP badge, a file row's path — so the keyboard would gain a stop over every
   one of them to buy a reason on the few that are refused controls. This way
   the stop is the control itself, which already has a name and now says that it
   is unavailable while it wears it. */
import { computed, toRef } from 'vue'
import { useInteractive } from './interactive.js'
import Icon from './Icon.vue'

/* The primary action is ink on paper — a dark chip in light, a light chip in
   dark — with no brand hue at all. Every saturated hue is spoken for by status. */
const V = {
  primary: { bg: 'var(--action-primary-bg)', bgH: 'var(--action-primary-bg-hover)', bgA: 'var(--action-primary-bg-active)', fg: 'var(--action-primary-fg)', bd: 'transparent' },
  secondary: { bg: 'var(--action-secondary-bg)', bgH: 'var(--action-secondary-bg-hover)', bgA: 'var(--action-secondary-bg-active)', fg: 'var(--text-primary)', bd: 'var(--border)' },
  ghost: { bg: 'transparent', bgH: 'var(--surface-hover)', bgA: 'var(--surface-active)', fg: 'var(--text-secondary)', bd: 'transparent' },
  danger: { bg: 'var(--action-danger-bg)', bgH: 'var(--action-danger-bg-hover)', bgA: 'var(--action-danger-bg-active)', fg: 'var(--action-danger-fg)', bd: 'transparent' }
}
const H = { sm: 'var(--control-h-sm)', md: 'var(--control-h)', lg: 'var(--control-h-lg)' }

const props = defineProps({
  variant: { type: String, default: 'secondary' },
  size: { type: String, default: 'md' },
  icon: { type: String, default: undefined },
  iconEnd: { type: String, default: undefined },
  disabled: { type: Boolean, default: false },
  selected: { type: Boolean, default: false },
  fullWidth: { type: Boolean, default: false },
  type: { type: String, default: 'button' }
})

const { hover, active, handlers } = useInteractive(toRef(props, 'disabled'))

/* The press, refused. This is what the native attribute did for free, and both
   halves of it matter: `stopImmediatePropagation` keeps the caller's own
   `@click` from running, and it also keeps the press from reaching an ancestor,
   which a disabled control's press never did.

   Capture, so that ordering is a fact rather than a hope. A press lands on the
   glyph or on the label inside the button, so this listener runs in the capture
   phase on their way down, before anything at the target and before every
   bubble-phase listener on this element — the DOM says so, whatever order the
   listeners were attached in. A press landing on the button's own padding has
   no phase to separate the two, and there this runs first because a component's
   own props are merged before the attributes that fall through to it. */
const onClickCapture = (event) => {
  if (!props.disabled) return
  event.preventDefault()
  event.stopImmediatePropagation()
}

/* And the keyboard's two ways of pressing a focused button, which the browser
   turns into a click of its own. Cancelling the keydown is what stops that
   click from ever being made — Enter goes through a keypress the browser skips
   once the keydown is cancelled, Space activates on the keyup and only if the
   keydown left the button active — and it is also what stops Space scrolling
   the window under a button that just refused it.

   Propagation is deliberately left alone: the window-level shortcuts this app
   listens for are not a refused button's to swallow, and if some engine
   synthesizes the click anyway the guard above is still in front of it. */
const onKeydown = (event) => {
  if (!props.disabled) return
  if (event.key !== 'Enter' && event.key !== ' ') return
  event.preventDefault()
}

/* And the press's own default action, which is the focus. Cancelled rather
   than stopped: the header above has the account of what it costs to leave it
   alone, and cancelling is the narrowest thing that answers it — the press
   still reaches `useInteractive`, which refuses it on its own while disabled,
   and nothing about a Tab arriving here goes through `mousedown` at all.
   Bound after `v-bind="handlers"` so it merges with that tracker's own
   `onMousedown` into a pair the DOM runs in turn, rather than taking its
   place. */
const onMousedown = (event) => {
  if (props.disabled) event.preventDefault()
}

const v = computed(() => V[props.variant] || V.secondary)
const iconSize = computed(() => (props.size === 'sm' ? 13 : 14))

const bg = computed(() => {
  if (props.disabled) return 'var(--surface-sunken)'
  if (active.value) return v.value.bgA
  if (hover.value) return v.value.bgH
  if (props.selected) return 'var(--surface-selected)'
  return v.value.bg
})

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: 'var(--space-3)',
  height: H[props.size],
  padding: props.size === 'sm' ? '0 var(--space-4)' : '0 var(--space-5)',
  font: `var(--weight-medium) ${props.size === 'sm' ? 'var(--text-xs)' : 'var(--text-sm)'}/1 var(--font-sans)`,
  letterSpacing: 'var(--tracking-tight)',
  color: props.disabled ? 'var(--text-muted)' : v.value.fg,
  background: bg.value,
  border: `var(--border-w) solid ${props.disabled ? 'var(--border-subtle)' : v.value.bd}`,
  borderRadius: 'var(--radius-3)',
  cursor: props.disabled ? 'not-allowed' : 'default',
  opacity: props.disabled ? 0.7 : 1,
  width: props.fullWidth ? '100%' : undefined,
  transition: 'var(--transition-control)',
  whiteSpace: 'nowrap'
}))
</script>

<template>
  <button
    :type="type"
    :aria-disabled="disabled || undefined"
    :aria-pressed="selected || undefined"
    :style="style"
    v-bind="handlers"
    @click.capture="onClickCapture"
    @keydown="onKeydown"
    @mousedown="onMousedown"
  >
    <Icon v-if="icon" :name="icon" :size="iconSize" />
    <span v-if="$slots.default"><slot /></span>
    <Icon v-if="iconEnd" :name="iconEnd" :size="iconSize" />
  </button>
</template>
