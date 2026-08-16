<script setup>
/* A menu that hangs off the point a secondary click happened at, rather than
   off a control. `MenuButton` is the same panel anchored to a trigger's rect;
   this one has no trigger at all — a row is a place, not a button, and the only
   thing a right-click hands over is a pair of window coordinates.

   It exists as a component because the machinery underneath is nobody's idea of
   a detail: a panel teleported out of the document (an absolutely positioned
   descendant is clipped by any ancestor with `overflow`, which every list in
   this app sits inside), fixed in window coordinates, flipped above the pointer
   when there is no room below, clamped to the window's edge, closed on a scroll
   anywhere underneath, closed on a press outside it, walked with the arrow keys,
   and careful about where the keyboard goes when it opens and when it leaves.
   `ProjectList` carried the whole of that inline and was the only caller;
   `BranchList` became the second, and a second copy of this is not a thing to
   keep in step by hand.

   It is deliberately **not** shared with `MenuButton`, which is the same refusal
   that file records about `Dropdown`: what differs is not the arithmetic but
   what the panel hangs off, and `MenuButton` also carries submenus, a trigger
   that has to stay pressed-looking, and three other call sites to break.

   The caller keeps the one thing this cannot know — which row the open menu is
   about — because the items are the caller's and so is the highlight under the
   panel. `close` fires whenever the menu leaves, however it leaves, so that
   there is one place to clear it. That row is also handed back with the pick,
   and that is not a convenience: closing is what clears the caller's own copy,
   the pick closes first (a handler that opens a dialog must not have the menu
   take the keyboard back out of it afterwards), so a handler reading the
   caller's ref would find it already empty. */
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'
import ContextMenu from './ContextMenu.vue'

const props = defineProps({
  /* The `ContextMenu` shape — `{kind, label, icon?, tone?, disabled?}`,
     `{type:'separator'}`, `{type:'label', label}`. */
  items: { type: Array, default: () => [] },
  /* A ceiling rather than a width; see `ContextMenu`. A row has no tooltip and
     no `title`, so a label past it clips with an ellipsis. */
  width: { type: Number, default: 200 }
})

const emit = defineEmits(['select', 'close'])

const shown = ref(false)
const box = ref(null)
/* What the caller said the menu is about, carried back with the pick. */
const owner = ref(null)
/* Where the panel goes, in window coordinates. Null until measured — the one
   state it must not be seen in, since it would be sitting in the corner. */
const at = ref(null)
const point = ref({ x: 0, y: 0 })
const cursor = ref(-1)
/* Where the keyboard was when the menu opened, so Esc can hand it back. There
   is no trigger to return it to the way `MenuButton` has one. */
const returnTo = ref(null)

/* The distance from the pointer, and the closest the panel may come to the
   window's edge. Operands in arithmetic against getBoundingClientRect rather
   than values handed to the browser, which is why they are numbers here and not
   token references — the same note MenuButton, Dropdown and Tooltip carry. */
const GAP = 4
const EDGE = 8

/* Separators, labels and greyed rows are drawn but never walked to. */
const walkable = computed(() =>
  props.items.map((it, i) => (it.type || it.disabled ? -1 : i)).filter((i) => i >= 0)
)

function place() {
  const rect = box.value?.getBoundingClientRect()
  if (!rect) return
  const { x, y } = point.value
  const room = window.innerHeight - y - GAP - EDGE
  /* Flipped above the pointer when the panel does not fit below it and there is
     more room above — the bottom row of a long list is exactly that case. The
     clamp below still applies afterwards, for a window too short for either
     side to hold the whole panel. */
  const above = rect.height > room && y - GAP - EDGE > room
  const top = above ? y - GAP - rect.height : y + GAP
  at.value = {
    left: Math.max(EDGE, Math.min(x + GAP, window.innerWidth - rect.width - EDGE)),
    top: Math.max(EDGE, Math.min(top, window.innerHeight - rect.height - EDGE))
  }
}

/* Scrolling closes rather than re-places, which is where this parts company
   with `MenuButton`. Its panel hangs off a trigger that moves with the scroll,
   so following it keeps the two together; this one hangs off a point the
   pointer was at, and the row that point named slides out from under it. A
   panel left behind would be offering verbs about a thing nothing on screen
   still connects it to. */
const onScroll = () => close()

/* Pointerdown rather than click: a press that starts inside the panel and ends
   outside it would otherwise close the menu out from under the pointer. The
   panel is teleported to the body, so it is not inside the caller's tree and
   has to be checked on its own. */
const onDocumentPointerdown = (event) => {
  if (!box.value?.contains(event.target)) close()
}

async function open(event, key = null) {
  point.value = { x: event.clientX, y: event.clientY }
  owner.value = key
  shown.value = true
  cursor.value = -1
  at.value = null
  returnTo.value = document.activeElement
  document.addEventListener('pointerdown', onDocumentPointerdown, true)
  // Capture, so a scroll inside any ancestor is seen, not only the window's.
  window.addEventListener('scroll', onScroll, true)
  window.addEventListener('resize', place)
  await nextTick()
  place()
  /* A second tick before the focus, and it is not spare. `place` only writes
     the position; the panel is `visibility: hidden` until that write has been
     rendered, and an element that is not visible refuses focus — silently, and
     the whole keyboard with it. */
  await nextTick()
  box.value?.focus()
}

function close() {
  if (!shown.value) return
  /* Read before anything is torn down, because closing is what makes the answer
     unavailable: the panel leaves the document and `document.activeElement`
     falls back to `<body>`, at which point there is no telling whether the
     keyboard had been in here at all. */
  const held = box.value?.contains(document.activeElement)
  shown.value = false
  owner.value = null
  at.value = null
  cursor.value = -1
  document.removeEventListener('pointerdown', onDocumentPointerdown, true)
  window.removeEventListener('scroll', onScroll, true)
  window.removeEventListener('resize', place)
  /* Only when focus was inside, and that guard is not decoration: `close` is
     also the unmount handler and the outside-click handler. On unmount there is
     nothing left to hand anything back to, and on an outside click the browser
     is a moment away from focusing whatever was clicked — moving it here first
     would be a flicker at best and a theft at worst. */
  const back = returnTo.value
  returnTo.value = null
  if (held && back?.isConnected) back.focus?.()
  emit('close')
}

/* The list a menu is drawn over can leave under it — a panel collapsing to its
   rail, a view torn down — and the two document-level listeners would outlive
   it, since they are the document's and not this component's. */
onBeforeUnmount(close)

/* One step along the list of walkable indices, wrapping at both ends. From
   nowhere (-1) a step down lands on the first row and a step up on the last,
   which is what makes the first arrow key after opening do the obvious thing. */
const step = (list, current, delta) => {
  if (!list.length) return -1
  const pos = list.indexOf(current)
  return list[(pos + delta + (pos < 0 ? (delta > 0 ? 0 : 1) : 0) + list.length) % list.length]
}

const pick = (item) => {
  if (!item || item.disabled || item.type) return
  const key = owner.value
  close()
  emit('select', item, key)
}

const onKeydown = (event) => {
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
  } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    cursor.value = step(walkable.value, cursor.value, event.key === 'ArrowDown' ? 1 : -1)
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    pick(props.items[cursor.value])
  }
}

const panelStyle = computed(() => ({
  position: 'fixed',
  top: `${at.value?.top ?? 0}px`,
  left: `${at.value?.left ?? 0}px`,
  // Hidden until measured: one frame of it in the window's corner reads as a
  // flash, and it has to be in the document at its natural height to be
  // measured at all.
  visibility: at.value ? 'visible' : 'hidden',
  zIndex: 'var(--z-popover)',
  outline: 'none'
}))

defineExpose({ open, close })
</script>

<template>
  <Teleport to="body">
    <!-- The keydown sits on this wrapper rather than on `ContextMenu`: this is
         the element that takes the focus, and a listener on a component tag
         would have to fall through as an attribute to reach a node at all. -->
    <div v-if="shown" ref="box" :style="panelStyle" tabindex="-1" @keydown="onKeydown">
      <ContextMenu
        :items="items"
        :cursor="cursor"
        :width="width"
        @select="pick"
        @hover="cursor = $event"
      />
    </div>
  </Teleport>
</template>
