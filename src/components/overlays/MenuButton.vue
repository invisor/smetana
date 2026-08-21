<script setup>
/* A button that opens a menu, and the menu.

   Derived from `core/Dropdown.vue`, not shared with it. The placement is the
   same problem — a panel that must leave the document because an ancestor with
   `overflow` clips an absolutely positioned descendant however it is
   positioned, and this one opens inside `Panel`'s scroll container and inside
   the board's own — and the answer is the same: fixed position in window
   coordinates, flipped above when there is no room below, clamped to the
   window's edge, re-measured whenever anything scrolls under it.

   Extracting that machinery into something both files use was considered and
   refused: `Dropdown` is drawn in the settings window, the run dialog and
   `BranchSelect`, and moving the ground out from under all of them for one new
   call site is a bigger risk than one duplicated placement routine.

   What differs is what the panel holds. `Dropdown` picks a value and shows it
   in a field; this picks a verb and shows nothing afterwards, so there is no
   `modelValue`, no selected label, and a second level: an item with `children`
   opens a submenu beside itself. */
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'
import IconButton from '../core/IconButton.vue'
import ContextMenu from './ContextMenu.vue'

const props = defineProps({
  /* The `taskMenu.js` shape: {kind, label, icon?, tone?, disabled?, children?}
     or {type:'separator'} or {type:'label', label}. */
  items: { type: Array, default: () => [] },
  icon: { type: String, default: 'ellipsis' },
  /* The accessible name and the tooltip both — one string, so the panel a
     person reads and the name a screen reader announces cannot disagree. */
  label: { type: String, default: 'More actions' },
  size: { type: String, default: 'sm' },
  disabled: { type: Boolean, default: false },
  /* Both of these are ceilings rather than widths — `ContextMenu` draws itself
     as wide as its widest row and no wider. They stay two numbers rather than
     one because the two panels hold different kinds of text: a menu row can
     carry a whole sentence (see `MENU_W` in `kanban/taskMenu.js`) where a
     submenu holds one-word answers, and a submenu allowed to grow to the
     menu's ceiling would start `placeSub`'s flip firing over room it was never
     going to use. */
  width: { type: Number, default: 200 },
  subWidth: { type: Number, default: 200 }
})

const emit = defineEmits(['select'])

const open = ref(false)
const root = ref(null)
/* Two refs per panel, and both are needed. The box is the positioned wrapper —
   it takes the focus, hears the keyboard and is what gets measured; the menu is
   the `ContextMenu` instance, which is the only way to reach the rows, since a
   submenu is placed against the row it hangs off rather than against the panel
   as a whole. */
const box = ref(null)
const menu = ref(null)
const subBox = ref(null)
/* Where each panel goes, in window coordinates. Null until measured — the one
   state neither must be seen in, since it would be sitting in the corner. */
const at = ref(null)
const subAt = ref(null)
/* Which row the keyboard is on, and which row's submenu is open. Two refs
   rather than one: walking inside a submenu must not move the parent's cursor
   off the row the submenu hangs from. */
const cursor = ref(-1)
const subCursor = ref(-1)
const openSub = ref(-1)

/* The distance from the trigger, and the closest either panel may come to the
   window's edge. Operands in arithmetic against getBoundingClientRect rather
   than values handed to the browser, which is why they are numbers here and
   not token references — the same note Dropdown and Tooltip carry. */
const GAP = 4
const EDGE = 8

/* Separators, labels and greyed rows are drawn but never walked to. */
const walkable = computed(() =>
  props.items.map((it, i) => (it.type || it.disabled ? -1 : i)).filter((i) => i >= 0)
)

const subItems = computed(() =>
  openSub.value >= 0 ? (props.items[openSub.value]?.children ?? []) : []
)

const subWalkable = computed(() =>
  subItems.value.map((it, i) => (it.type || it.disabled ? -1 : i)).filter((i) => i >= 0)
)

function place() {
  const anchor = root.value?.getBoundingClientRect()
  const rect = box.value?.getBoundingClientRect()
  if (!anchor || !rect) return
  const room = window.innerHeight - anchor.bottom - GAP - EDGE
  const above = rect.height > room && anchor.top - GAP - EDGE > room
  const top = above ? anchor.top - GAP - rect.height : anchor.bottom + GAP
  /* Right-aligned to the trigger rather than left. The trigger sits at the
     right edge of a 212px card and the panel is wider than the card, so one
     hung from its left edge would run off the board. Clamped either way, which
     is what stands between the last column on a narrow window and a menu drawn
     off the screen. */
  at.value = {
    left: Math.max(EDGE, Math.min(anchor.right - rect.width, window.innerWidth - rect.width - EDGE)),
    top: Math.max(EDGE, Math.min(top, window.innerHeight - rect.height - EDGE))
  }
}

function placeSub() {
  /* The submenu hangs off the row it belongs to, found by index. That leans on
     `ContextMenu` drawing exactly one element per item — true for all three of
     its item shapes today (row, separator, label), and the one assumption here
     that a fourth shape rendering none or two would break silently: the panel
     would simply be placed against the wrong row rather than fail. */
  const row = menu.value?.$el?.children?.[openSub.value]?.getBoundingClientRect()
  const rect = subBox.value?.getBoundingClientRect()
  const parent = box.value?.getBoundingClientRect()
  if (!row || !rect || !parent) return
  /* To the right of the parent panel, flipped to its left when there is no
     room — the same rule the first panel applies vertically, in the axis a
     submenu actually runs out of room in. */
  const right = parent.right + GAP
  const left = right + rect.width + EDGE > window.innerWidth ? parent.left - GAP - rect.width : right
  subAt.value = {
    left: Math.max(EDGE, left),
    top: Math.max(EDGE, Math.min(row.top, window.innerHeight - rect.height - EDGE))
  }
}

const closeSub = () => {
  openSub.value = -1
  subCursor.value = -1
  subAt.value = null
}

const replace = () => {
  place()
  if (openSub.value >= 0) placeSub()
}

const show = async () => {
  if (props.disabled) return
  open.value = true
  cursor.value = -1
  closeSub()
  at.value = null
  document.addEventListener('pointerdown', onDocumentPointerdown, true)
  // Capture, so a scroll inside any ancestor is seen, not only the window's.
  window.addEventListener('scroll', replace, true)
  window.addEventListener('resize', replace)
  await nextTick()
  place()
  /* A second tick before the focus, and it is not spare. `place` only writes
     the position; the panel is `visibility: hidden` until that write has been
     rendered, and an element that is not visible refuses focus — silently, and
     the whole keyboard with it. */
  await nextTick()
  box.value?.focus()
}

const hide = () => {
  if (!open.value) return
  /* Read before anything is torn down, because closing is what makes the answer
     unavailable: the panels leave the document and `document.activeElement`
     falls back to `<body>`, at which point there is no telling whether the
     keyboard had been in here at all. */
  const held =
    box.value?.contains(document.activeElement) || subBox.value?.contains(document.activeElement)
  open.value = false
  closeSub()
  at.value = null
  document.removeEventListener('pointerdown', onDocumentPointerdown, true)
  window.removeEventListener('scroll', replace, true)
  window.removeEventListener('resize', replace)
  /* Hand the keyboard back to the trigger, so Esc or a picked row leaves a
     person where they started rather than at the top of the document — the menu
     is walkable by keyboard throughout and this is the one step that would make
     it a dead end.

     Only when focus was inside, and that guard is not decoration: `hide` is
     also the unmount handler and the outside-click handler. On unmount there is
     nothing to give focus back to, and on an outside click the browser is a
     moment away from focusing whatever was clicked — moving it here first would
     be a flicker at best and a theft at worst.

     The button rather than `root`: `IconButton`'s own root is `Tooltip`'s span,
     which takes no focus. */
  if (held) root.value?.querySelector('button')?.focus()
}

/* A card can leave the board under an open menu — a deletion is exactly that —
   and the two document-level listeners would outlive it. */
onBeforeUnmount(hide)

const toggle = () => (open.value ? hide() : show())

/* Pointerdown rather than click: a press that starts inside a panel and ends
   outside it would otherwise close the menu out from under the pointer. Both
   panels are checked as well as the trigger, and that is not optional — they
   are teleported to the body and are therefore not inside `root` at all. */
const onDocumentPointerdown = (event) => {
  const inside =
    root.value?.contains(event.target) ||
    box.value?.contains(event.target) ||
    subBox.value?.contains(event.target)
  if (!inside) hide()
}

const enterSub = async (i) => {
  if (openSub.value === i) return
  openSub.value = i
  subCursor.value = -1
  subAt.value = null
  await nextTick()
  placeSub()
}

/* -1 is the pointer leaving a row, and it deliberately does nothing: the way
   out of a submenu row is across the parent panel, and closing on the way
   would take the submenu with it. */
const onHover = async (i) => {
  if (i < 0) return
  cursor.value = i
  const item = props.items[i]
  if (item?.children && !item.disabled) await enterSub(i)
  else closeSub()
}

const pick = (item) => {
  if (!item || item.disabled || item.type) return
  if (item.children) {
    enterSub(props.items.indexOf(item))
    return
  }
  emit('select', item)
  hide()
}

/* One step along a list of walkable indices, wrapping at both ends. From
   nowhere (-1) a step down lands on the first row and a step up on the last,
   which is what makes the first arrow key after opening do the obvious thing. */
const step = (list, current, delta) => {
  if (!list.length) return -1
  const pos = list.indexOf(current)
  return list[(pos + delta + (pos < 0 ? (delta > 0 ? 0 : 1) : 0) + list.length) % list.length]
}

const onKeydown = async (event) => {
  if (event.key === 'Escape') {
    event.preventDefault()
    // The submenu first, the menu second: one Esc undoes one thing.
    if (openSub.value >= 0) closeSub()
    else hide()
  } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const delta = event.key === 'ArrowDown' ? 1 : -1
    if (openSub.value >= 0) subCursor.value = step(subWalkable.value, subCursor.value, delta)
    else cursor.value = step(walkable.value, cursor.value, delta)
  } else if (event.key === 'ArrowRight') {
    event.preventDefault()
    const item = props.items[cursor.value]
    if (item?.children && !item.disabled) {
      await enterSub(cursor.value)
      subCursor.value = subWalkable.value[0] ?? -1
    }
  } else if (event.key === 'ArrowLeft') {
    event.preventDefault()
    closeSub()
  } else if (event.key === 'Enter' || event.key === ' ') {
    event.preventDefault()
    const inSub = openSub.value >= 0 && subCursor.value >= 0
    pick(inSub ? subItems.value[subCursor.value] : props.items[cursor.value])
  }
}

const panelStyle = (position) => ({
  position: 'fixed',
  top: `${position?.top ?? 0}px`,
  left: `${position?.left ?? 0}px`,
  // Hidden until measured: one frame of it in the window's corner reads as a
  // flash, and it has to be in the document at its natural height to be
  // measured at all.
  visibility: position ? 'visible' : 'hidden',
  zIndex: 'var(--z-popover)',
  outline: 'none'
})
</script>

<template>
  <span ref="root" :style="{ display: 'inline-flex' }">
    <IconButton
      :icon="icon"
      :label="label"
      :size="size"
      :disabled="disabled"
      :aria-expanded="open"
      @click.stop="toggle"
    />

    <Teleport to="body">
      <!-- The keydown sits on this wrapper rather than on `ContextMenu`: this
           is the element that takes the focus, and a listener on a component
           tag would have to fall through as an attribute to reach a node at
           all. -->
      <div
        v-if="open"
        ref="box"
        :style="panelStyle(at)"
        tabindex="-1"
        @keydown="onKeydown"
      >
        <ContextMenu
          ref="menu"
          :items="items"
          :cursor="cursor"
          :width="width"
          @select="pick"
          @hover="onHover"
        />
      </div>

      <div v-if="open && openSub >= 0" ref="subBox" :style="panelStyle(subAt)">
        <ContextMenu
          :items="subItems"
          :cursor="subCursor"
          :width="subWidth"
          @select="pick"
          @hover="subCursor = $event"
        />
      </div>
    </Teleport>
  </span>
</template>
