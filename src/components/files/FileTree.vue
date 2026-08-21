<script setup>
/* The project's files, and the one menu over the whole of them.

   One `PointerMenu` for the tree rather than one per row, the way `ProjectRail`
   and `BranchList` each keep one for their list: the panel is teleported to the
   body and only one can be open at a time, so a copy per row would be a copy per
   row of everything it holds.

   The rows this component draws are also the reason the empty space below them
   answers. A menu reachable only through a neighbouring row is a menu that
   cannot be reached at all in a project whose root is empty, or whose first
   screen is nothing but folders somebody has not opened — which is exactly the
   moment the second half of this menu, the one that makes a file, is wanted.

   What a verb does is `DesktopApp.vue`'s: the stores live there, and a component
   that imported one would be the second exception to a rule with exactly one. */
import { computed, ref } from 'vue'
import FileTreeRow from './FileTreeRow.vue'
import PointerMenu from '../overlays/PointerMenu.vue'
import { fileMenuItems } from './fileMenu.js'
import { isStubPath } from '../../paths.js'

const props = defineProps({
  nodes: { type: Array, default: () => [] },
  selectedPath: { type: String, default: undefined },
  expanded: { type: Object, default: () => ({}) },
  /* Whether there is an agent in this project a path could be typed into. The
     one thing the menu needs that the tree cannot see; everything else on it is
     about a row. */
  hasAgentSession: { type: Boolean, default: false }
})

const emit = defineEmits(['toggle', 'select', 'open', 'action'])

/* Flattened to a single list so the tree can be virtualised later without
   restructuring the markup. */
const rows = computed(() => {
  const out = []
  const walk = (list, depth) => {
    for (const n of list) {
      const open = !!props.expanded[n.path] && Array.isArray(n.children)
      out.push({
        path: n.path,
        name: n.name,
        depth,
        kind: n.kind || 'file',
        expanded: open,
        selected: n.path === props.selectedPath,
        git: n.git,
        readOnly: !!n.readOnly
      })
      if (n.kind === 'dir' && open && n.children) walk(n.children, depth + 1)
    }
  }
  walk(props.nodes, 0)
  return out
})

const menu = ref(null)
/* Which row the open menu is about, by path, so that row can draw itself
   highlighted while the panel is up. `PointerMenu` clears it on close however
   the menu leaves, which is what keeps the highlight and the panel together. */
const menuFor = ref(null)

/* A ceiling rather than a width — the panel is as wide as its widest row wants
   to be, so the ordinary menu is nowhere near this. It is measured against the
   one row that needs it: `ContextMenu` clips a label rather than wrapping it and
   gives a row no tooltip, and "Attach to agent — no agent to type into" is the
   whole reason that item is greyed. At the default type scale it comes to 292px
   in comfortable density, and this is that with a little room over it. */
const MENU_W = 300

/* What the open menu is about: a file, a folder, or the project's own root,
   which is what the space below the last row names. Kept beside the path rather
   than worked out from it, because the root is the one target with no row and no
   `kind` to read. */
const menuTarget = ref('root')

const items = computed(() =>
  fileMenuItems({
    target: menuTarget.value,
    hasAgentSession: props.hasAgentSession,
    /* Read here rather than in the pure module, which is what keeps the choice
       of noun testable: `fileManagerName` is a function of this string. */
    userAgent: typeof navigator === 'undefined' ? '' : navigator.userAgent
  })
)

const openRowMenu = (row, event) => {
  /* The "…N more" row is not a file — there is nothing on disk behind it — and
     every verb on this menu is about something on disk. A menu that opened here
     would offer to copy a path with a zero byte in it. Silence rather than a
     menu of greyed rows: the row is a count, not a thing with actions. */
  if (isStubPath(row.path)) return
  menuFor.value = row.path
  menuTarget.value = row.kind === 'dir' ? 'dir' : 'file'
  menu.value?.open(event, row.path)
}

/* The same menu, for the project's root. `preventDefault` here for the reason
   the row's handler carries one: `src/nativeMenu.js` has already refused the
   event in capture, and this says at the handler that a menu of our own is
   what opens instead. Nothing stops propagation — a row's own handler does
   that, so this only ever runs on space no row occupies. */
const openRootMenu = (event) => {
  event.preventDefault()
  menuFor.value = ''
  menuTarget.value = 'root'
  menu.value?.open(event, '')
}

/* One event out, carrying which verb, which path and what the path is — the
   same shape `newTabMenu.js` hands `onNewTab`, and matched by hand on the other
   side for want of anything that could join them.

   `target` travels with the pick rather than being worked out again there:
   `PointerMenu` closes before it emits, so `menuTarget` is the caller's to read
   only while it still means something, and two answers to "what was this menu
   about" is how the panel and the handler come apart. */
const pick = (item, path) => {
  emit('action', { kind: item.kind, path, target: menuTarget.value })
}

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  /* Fills whatever it is given, so the space below the last row is this
     component's to answer for. Without it the tree is exactly as tall as its
     rows and a secondary click below them lands on the scroll container. */
  minHeight: '100%',
  color: 'var(--text-primary)'
}
</script>

<template>
  <div role="tree" :style="rootStyle" @contextmenu="openRootMenu">
    <FileTreeRow
      v-for="r in rows"
      :key="r.path"
      v-bind="r"
      :menu-open="menuFor === r.path"
      @toggle="$emit('toggle', r.path)"
      @select="$emit('select', r.path)"
      @open="$emit('open', r.path)"
      @menu="openRowMenu(r, $event)"
    />
    <PointerMenu ref="menu" :items="items" :width="MENU_W" @select="pick" @close="menuFor = null" />
  </div>
</template>
