<script setup>
/* A panel's own tab row: a segmented control under a panel header, at the top
   of what it scopes rather than at the far end of it.

   One component for both side columns rather than one row each. The left
   column had the only copy, written inline in `DesktopApp.vue`; the right
   column wanted the same row, and a second copy of these style objects a
   thousand lines away from the first — obliged to match, with nothing
   mechanical holding them together — is exactly the pair that drifts.

   **The row is the design handoff's**, direction `1a` of the Branches section
   in the frame that carries every one of its states, `2a`. What that design
   settles and where it was overruled is
   `.smetana/docs/superpowers/specs/2026-09-08-branches-section-direction-1a-design.md`,
   which is outside the repository — `.smetana/` is not committed, so on another
   machine that path leads nowhere and this paragraph is what is left. It is the
   whole component that was brought to it rather than a variant for the one
   panel that noticed: the row it draws is the same row in all three of its
   callers, so a shape for the Git panel alone would have been two segmented
   controls in one app — the pair this component exists to prevent — and the
   earlier row broke a rule of the product besides: it set its labels in
   uppercase, and this app is sentence case everywhere.

   What the handoff draws, and what is here: **a group, and segments flush
   inside it.** The group is the bordered, rounded, sunken box
   (`--surface-sunken` under `--border`, `--radius-3`, `overflow: hidden`); the
   segments divide it between them with no gap, and the divider is each
   segment's own `border-right` except the last one's. Both halves are
   load-bearing. Without the group a divider is a line floating between two
   labels rather than the seam of one control, and without the segments being
   flush there is nothing for that seam to be the seam of. The active segment
   is a fill and **not** a plate: `--surface-selected` edge to edge with no
   radius of its own, the group's `overflow: hidden` taking its outer corners.

   The inset rule and the raised fill an earlier version had are gone with the
   position: a rule under a tab was that row's answer to sitting against the
   column's edge, and a segmented row marks its active segment by fill.

   **A hover here is `--surface` and not `--surface-hover`, and the ground is
   why.** `--surface-hover` is the step written for a control standing on
   `--surface`, and measured where it belongs it is 1.137:1 in the light theme
   and 1.175:1 in the dark. An unselected segment stands on the group's
   `--surface-sunken` instead, and over *that* the same token comes to 1.028:1
   light — three units per channel, which on screen is nothing at all: with the
   group's ground sunken the row answered the pointer with a blank in one of the
   two themes, and hover is the only pointer affordance it has, since the cursor
   stays `default` and interaction here is never a transform. `--surface` over
   the sunken ground is 1.169:1 light and 1.095:1 dark — the closest of the
   surface tokens to the system's own step in **both** themes, where
   `--surface-active` is half a step in the light theme and a press-sized 1.414
   in the dark, and `--surface-raised` is white paper the row never sits on.
   It is also the step that means something: over a sunken ground the segment
   under the pointer rises to the surface the panel itself is drawn on, which is
   `core/interactive.js`'s rule read literally.

   One consequence, taken knowingly: in the light theme a hovered segment is
   *lighter* than the selected one, because `--surface-selected` there is a
   blue tint rather than a lightness step — 1.005:1 against the ground it sits
   on. What tells the two apart in that theme is the tint and `--text-primary`,
   not brightness, and that is the design system's arrangement rather than this
   row's.

   **The focus ring is pulled inside the segment**, and that is the group's
   doing: `overflow: hidden` is what clips the active fill to the radius, and it
   clips an outside ring just as willingly — a segment is flush with the group's
   top and bottom, and the end ones with its left and right, so
   `base.css`'s ring at `outline-offset: 1px` would be cut away on every side
   that matters. `outlineOffset: calc(var(--border-w-strong) * -1)` is the same
   answer, and the same knowing lean on 2px being both that token's value and
   the stylesheet's outline width, that `AttachmentStrip`, the status footer's
   clipped row, the branch filter field and a branch row already make. The ring
   is never suppressed: with a roving tabindex it is the only thing that says
   which of the segments the keyboard is on.

   **The row is a `tablist` and each segment a `tab`**, which is what it is in
   all three of its callers: the left column's panels, the right column's, and
   the two sides of the Git panel's branch list. There is no fourth kind of row
   drawn with this and there is not meant to be — a row of segments that chose
   something other than which panel is underneath would want its own component
   rather than this one's roles.

   The tab order holds **one** segment, the selected one, and the arrows move
   between them — the tab pattern's own rule, and the reason it has one: a row
   of three panels must not be three presses of Tab on the way past. They wrap
   at both ends, since a row of two would otherwise answer only one of the two
   keys. Switching is what the press does rather than something a second press
   confirms: the panel underneath is already on screen, so there is nothing a
   delayed activation would be protecting.

   `event.key` and not `event.code`, which is `PointerMenu`'s line for the same
   two keys: an arrow reads the same either way, and the discipline `code` is
   for is a **letter** whose key changes under a Russian layout or Caps Lock. */
import { ref } from 'vue'

const props = defineProps({
  /* `{ id, label, count }` each, and `count` is optional. The caller owns the
     list and its order; this draws whatever it is handed, which is what lets
     two columns with different vocabularies share one row.

     The split into two strings is the handoff's and not a convenience: a tab
     label is prose and is set in sans, and a figure beside it is a number and
     stays mono, so the two cannot be one string. Only the Git panel's tabs
     carry a figure, and only while its filter is on. */
  tabs: { type: Array, default: () => [] },
  modelValue: { type: String, default: null }
})

const emit = defineEmits(['update:modelValue'])

const hovered = ref(null)

/* One step along the row, wrapping at both ends, with the keyboard following
   the choice it just made — the segment that becomes selected is the one that
   becomes the tab stop, so leaving the focus behind would put the ring on a
   segment that is no longer either.

   The sibling is reached through the row rather than through a list of refs
   this component would then have to keep: the segments are the only children
   `groupStyle`'s element has, so the index into the tabs is the index into the
   row. */
const onKeydown = (event, at) => {
  const step = event.key === 'ArrowRight' ? 1 : event.key === 'ArrowLeft' ? -1 : 0
  if (!step || !props.tabs.length) return
  event.preventDefault()
  const next = (at + step + props.tabs.length) % props.tabs.length
  emit('update:modelValue', props.tabs[next].id)
  event.currentTarget?.parentElement?.children?.[next]?.focus?.()
}

/* The row the group sits in: the padding that keeps the control off the panel's
   edges, and the hairline that separates it from whatever it scopes. Its height
   is the group's plus that padding and that rule — about 1.18 rows in either
   density, which is why `GitPanel.vue` measures this row rather than asserting
   it. */
const barStyle = {
  display: 'flex',
  alignItems: 'center',
  flex: '0 0 auto',
  padding: 'var(--space-2) var(--space-3)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}

/* The group. `--control-h-sm` is where the handoff's 22px segment lands — 24
   comfortable, 20 compact — and the token rather than the number, because a
   literal would be the one height in a side column that neither density nor the
   app-wide font size reaches. `overflow: hidden` is what rounds the active
   fill's outer corners without the fill knowing anything about the radius. */
const groupStyle = {
  flex: 1,
  minWidth: 0,
  display: 'flex',
  height: 'var(--control-h-sm)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)',
  overflow: 'hidden',
  background: 'var(--surface-sunken)'
}

const segmentStyle = (tab, at) => {
  const active = props.modelValue === tab.id
  return {
    flex: 1,
    minWidth: 0,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 'var(--space-2)',
    /* A label longer than its share is cut at the seam rather than over the
       neighbour: the group clips at its own edge, which would leave a middle
       segment's overflow lying across the segment beside it. */
    overflow: 'hidden',
    whiteSpace: 'nowrap',
    font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
    letterSpacing: 'var(--tracking-normal)',
    /* Square, and stated rather than left out: `base.css` gives a
       `:focus-visible` element `--radius-2`, which on a segment would round the
       fill for as long as the keyboard is on it and leave the group's ground
       showing in four notches. `--radius-0` and not a bare `0` — a radius is a
       radius even when it is none, and the scale has a name for this one. */
    borderRadius: 'var(--radius-0)',
    outlineOffset: 'calc(var(--border-w-strong) * -1)',
    /* The seam between two segments, and never after the last one — a border on
       the end segment would be a second line a pixel inside the group's own. */
    borderRight: at === props.tabs.length - 1 ? undefined : 'var(--border-w) solid var(--border)',
    color: active ? 'var(--text-primary)' : 'var(--text-muted)',
    background: active
      ? 'var(--surface-selected)'
      : hovered.value === tab.id
        ? 'var(--surface)'
        : 'transparent',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}

/* The figure beside a label. Mono, because it is a number and this app sets
   numbers and identifiers in mono wherever they stand next to prose — the
   count in a section caption is the same figure in the same face, and while the
   filter is on this is where that count has gone. A step quieter than the label
   on the selected segment; on the others it takes the segment's own muted
   colour, which is already as quiet as the label. */
const countStyle = (tab) => ({
  fontFamily: 'var(--font-mono)',
  color: props.modelValue === tab.id ? 'var(--text-secondary)' : undefined
})
</script>

<template>
  <div :style="barStyle">
    <div role="tablist" :style="groupStyle">
      <div
        v-for="(tab, at) in tabs"
        :key="tab.id"
        role="tab"
        :aria-selected="modelValue === tab.id"
        :tabindex="modelValue === tab.id ? 0 : -1"
        :style="segmentStyle(tab, at)"
        @click="emit('update:modelValue', tab.id)"
        @keydown="onKeydown($event, at)"
        @mouseenter="hovered = tab.id"
        @mouseleave="hovered = null"
      >
        <span v-if="tab.label">{{ tab.label }}</span>
        <span v-if="tab.count" :style="countStyle(tab)">{{ tab.count }}</span>
      </div>
    </div>
  </div>
</template>
