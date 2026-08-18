<script setup>
/* The caption over one section of the Git panel, and the control that folds it.

   A caption over a list, in prose and therefore sans, with the count beside it
   in mono because it is a measurement. That much it has always been; what it
   gained is being a button, so the section under it can be folded away and the
   whole row is the target rather than a chevron somebody has to aim at.

   **The count stays on screen while the section is folded**, and that is the
   point of folding one: a person who folds the branches away is saying they do
   not want to read the list, not that they no longer want to know there are
   nine of them.

   A real `<button>` and not a div with a click on it, which buys the keyboard
   for free — Enter and Space, focus in the tab order, the focus ring
   `tokens/base.css` already draws — and `aria-expanded` says which way it is
   pointing to anything reading the page aloud.

   The root element is exposed because `GitPanel` measures it: a header is
   exactly `--row-h` tall, which is the one row height the panel's whole
   arithmetic is in, and that token is a `calc()` over an unregistered custom
   property, so `getComputedStyle` hands back the calc unevaluated rather than a
   number — the trap `terminal/theme.js` records. Measuring the row that is
   already on screen beats standing a throwaway element up beside it.

   **The caption is not the row any more, and the wrapper around it is.** The
   `actions` slot puts controls in the header — the Git panel's Pull and Push —
   and they cannot go inside the caption: it is a `<button>`, a button inside a
   button is invalid HTML, and a press on Pull would fold the section on its way
   through. So they are a sibling, and what moves with them is the height and
   the hairline: `sectionHeights.js` learns what a row is by measuring the
   element exposed here, and a rule that spanned only the caption would leave
   the controls standing above the line. The caption keeps `height: 100%` so its
   own hover surface is still the whole row and still reads as "press here to
   fold", and `flex: 1` so it is everything the controls do not take.

   `divided` is the rule above the caption, and it is what makes a caption read
   as the start of a block rather than as one more row of the list above it: the
   sections are all `--row-h`, all quiet, and with nothing between them a panel
   of three ran together into one column of rows. It is the same hairline
   `Panel` draws under its title and `ProjectList` under its rows, and it is
   asked for rather than assumed, because the topmost caption in a panel already
   has one of those above it and two hairlines meeting is a 2px line. */
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  label: { type: String, required: true },
  /* Drawn only when there is one to draw, and deliberately not a `0`: a section
     that is empty says so in its own words inside itself, and a zero in the
     caption would be the same fact stated twice, once in the wrong idiom. */
  count: { type: Number, default: null },
  open: { type: Boolean, default: true },
  /* Whether this caption carries the hairline that separates it from whatever
     is above. Off by default: a caption on its own is a caption, and only a
     stack of them wants the rule. */
  divided: { type: Boolean, default: false }
})
defineEmits(['toggle'])

const el = ref(null)
defineExpose({ el })

const { hover, active, handlers } = useInteractive()

/* The row, and it is this element rather than the caption that `GitPanel`
   measures. `flexShrink: 0` because a caption is a flex item in that column and
   a flex item shrinks by default: with three sections crowding a short panel
   the captions gave way with the lists, `--row-h` became a starting point and
   the text was clipped — the very defect a short list hid in `Dropdown`'s
   options. A caption is the one thing here that must not move.

   A function of whether the slot was filled rather than a `computed` over
   `useSlots()`: a slot's presence is not a reactive dependency, so a cached
   answer would go on insetting a caption whose controls have since gone — the
   Git panel takes both of its buttons off the header on a detached HEAD. */
const rowStyle = (hasActions) => ({
  display: 'flex',
  alignItems: 'center',
  width: '100%',
  height: 'var(--row-h)',
  flexShrink: 0,
  /* The controls would otherwise sit against the panel's own edge. Only where
     there are controls: adding it always would move the count of every caption
     in the panel by a step, for a header that has nothing to inset. */
  paddingRight: hasActions ? 'var(--space-3)' : '0',
  /* Inside the height rather than on top of it, which is `box-sizing:
     border-box` doing its work: a header is what `GitPanel` measures a row by,
     and a rule that added a pixel to two of the three captions would put the
     measured row and the drawn rows a pixel apart for the whole of that panel's
     arithmetic. */
  borderTop: props.divided ? 'var(--border-w) solid var(--border-subtle)' : 'none'
})

/* The caption itself: everything the `actions` slot does not take, at the whole
   height of the row so its hover surface is the row rather than a strip through
   the middle of one.

   `border: 'none'` and never `undefined` on a `<button>`: Vue removes a style
   property handed `undefined`, and a button with no border of its own is handed
   straight back to the user agent's `2px outset` — which drew a white rule over
   every caption in the panel. */
const style = computed(() => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  flex: 1,
  minWidth: 0,
  height: '100%',
  padding: '0 var(--space-5)',
  border: 'none',
  textAlign: 'left',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)',
  /* Interaction is a surface step and never a colour change, so a caption in a
     dense column cannot jump under the pointer. */
  background: active.value
    ? 'var(--surface-active)'
    : hover.value
      ? 'var(--surface-hover)'
      : 'transparent',
  cursor: 'default',
  transition: 'var(--transition-control)'
}))

const MARK = 12
const countStyle = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)' }
</script>

<template>
  <div ref="el" :style="rowStyle(Boolean($slots.actions))">
    <button
      type="button"
      :style="style"
      :aria-expanded="open"
      v-bind="handlers"
      @click="$emit('toggle')"
    >
      <Icon :name="open ? 'chevron-down' : 'chevron-right'" :size="MARK" :style="{ flex: 'none' }" />
      <span>{{ label }}</span>
      <span :style="{ flex: 1 }" />
      <span v-if="count" :style="countStyle">{{ count }}</span>
    </button>
    <!-- Beside the caption and never inside it: a button inside a button is
         invalid, and a press on one of these would fold the section on its way
         through. -->
    <slot name="actions" />
  </div>
</template>
