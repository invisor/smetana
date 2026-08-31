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
import FileTreeDraftRow from './FileTreeDraftRow.vue'
import PointerMenu from '../overlays/PointerMenu.vue'
import { FILE_MENU_W, fileMenuItems, folderOf } from './fileMenu.js'
import { canPasteInto } from './fileClipboard.js'
import { isStubPath } from '../../paths.js'

const props = defineProps({
  nodes: { type: Array, default: () => [] },
  selectedPath: { type: String, default: undefined },
  expanded: { type: Object, default: () => ({}) },
  /* Whether there is an agent in this project a path could be typed into right
     now. The one thing the menu needs that the tree cannot see; everything else
     on it is about a row.

     Not `hasAgentSession`, which `stores/terminals.js` exports and means
     something wider — see `fileMenu.js` for why the two must not be wired to
     each other. */
  canAttach: { type: Boolean, default: false },
  /* Whether there is a live agent here *at all*, selected or not. It changes no
     row's state, only which of the two reasons the greyed one gives: something
     to pick, or nothing to pick. */
  hasLiveAgent: { type: Boolean, default: false },
  /* What Cut or Copy put on the tree's clipboard, or null: `{ paths, mode }`,
     straight from `filesState`. Two things read it and nothing else does —
     whether Paste is offered on the folder the menu is open over, and which
     rows are drawn muted. The record itself is the store's; this component only
     draws what it says. */
  clipboard: { type: Object, default: null }
})

const emit = defineEmits(['toggle', 'select', 'open', 'action'])

/* The entry being named, or null. Two shapes, and the `kind` tells them apart:

     `{ key, kind: 'file' | 'dir', dir }`   a new entry in that folder
     `{ key, kind: 'rename', path, value }` a new name for the entry at `path`

   The first is drawn first among its folder's contents; the second is drawn
   **in place of** the row it is about, which is the whole of what says which
   entry is being renamed.

   `key` is what the cancel is checked against rather than the folder or the
   path, because it is the one field both shapes have and the one the row is
   keyed by: a draft replaced by another unmounts the first field, and a browser
   firing that field's `blur` on the way out would otherwise cancel the draft
   that replaced it.

   It is this component's own state and deliberately not a node in `nodes`: the
   tree is rebuilt from `files_list` every time `catchUp` re-reads a folder on
   window focus, and a draft mixed into that list would vanish under somebody's
   hands mid-word.

   It is one at a time, for the same reason the menu is one panel: a second
   field opened while the first is still being typed into is two answers to one
   question, and the first would be abandoned with no way to say so. */
const draft = ref(null)

/* The paths a pending Cut named, as a set for the walk below. A copy is not in
   it: nothing about a copied row changes, because nothing is going to happen to
   it. */
const cutPaths = computed(() =>
  props.clipboard?.mode === 'cut' ? new Set(props.clipboard.paths) : new Set()
)

/* Flattened to a single list so the tree can be virtualised later without
   restructuring the markup. */
const rows = computed(() => {
  const out = []
  /* `dir` is the folder whose contents this list is, which the walk needs for
     one thing only: the draft belongs to a folder rather than to a row, and the
     root's own folder is `''` — where there is no row to hang it off at all. */
  const walk = (list, depth, dir) => {
    /* First among its folder's contents, rather than sorted into them by name:
       the sorted position of a name nobody has finished typing moves on every
       keystroke, and a field that walks down the tree under the fingers is
       unusable. VS Code puts it here too. */
    if (draft.value && draft.value.kind !== 'rename' && draft.value.dir === dir) {
      out.push({ draft: true, key: draft.value.key, depth, kind: draft.value.kind, value: '' })
    }
    for (const n of list) {
      const open = !!props.expanded[n.path] && Array.isArray(n.children)
      if (draft.value?.kind === 'rename' && draft.value.path === n.path) {
        /* The field takes the row's place and its indent, and the folder's
           contents stay where they are underneath: a subtree that vanished
           while its folder was being renamed and came back on Esc would be the
           tree moving for a reason nobody asked about. The `kind` here is the
           entry's own — the draft record carries `'rename'` in that field, and
           what the row needs is the glyph. */
        out.push({
          draft: true,
          key: draft.value.key,
          depth,
          kind: n.kind || 'file',
          value: draft.value.value
        })
      } else {
        out.push({
          path: n.path,
          name: n.name,
          depth,
          kind: n.kind || 'file',
          expanded: open,
          selected: n.path === props.selectedPath,
          git: n.git,
          readOnly: !!n.readOnly,
          cut: cutPaths.value.has(n.path)
        })
      }
      if (n.kind === 'dir' && open && n.children) walk(n.children, depth + 1, n.path)
    }
  }
  walk(props.nodes, 0, '')
  return out
})

const menu = ref(null)
/* Which row the open menu is about, by path, so that row can draw itself
   highlighted while the panel is up. `PointerMenu` clears it on close however
   the menu leaves, which is what keeps the highlight and the panel together. */
const menuFor = ref(null)

/* What the open menu is about: a file, a folder, or the project's own root,
   which is what the space below the last row names. Kept beside the path rather
   than worked out from it, because the root is the one target with no row and no
   `kind` to read. */
const menuTarget = ref('root')

/* Whether Delete on the open panel has been picked once already. It lives
   beside `menuTarget` — the other thing that is true only while the panel is up
   — and `PointerMenu`'s `close` clears both halves of it, that being the one
   event which arrives however the menu leaves: Esc, a click outside, a scroll,
   a pick on any other row. So a menu closed and reopened on the same row starts
   unarmed, and nothing but a second pick on the armed row deletes anything. */
const confirmingDelete = ref(false)

/* Whether the folder this menu is about can take what is on the clipboard, and
   why not when it cannot. The rule is `fileClipboard.js`'s, asked here about
   the same folder the verb would act in — `folderOf`, the answer Open in
   terminal and the making rows already use — so the row that is greyed and the
   call that would be made are talking about one place. */
const paste = computed(() =>
  canPasteInto({
    clipboard: props.clipboard,
    folder: folderOf({ path: menuFor.value ?? '', target: menuTarget.value })
  })
)

const items = computed(() =>
  fileMenuItems({
    target: menuTarget.value,
    canAttach: props.canAttach,
    hasLiveAgent: props.hasLiveAgent,
    confirmingDelete: confirmingDelete.value,
    canPaste: paste.value.ok,
    pasteReason: paste.value.reason,
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

/* The two verbs that make something never leave this component as they are
   picked: what they do first is put a field in the tree, and the tree is here.
   `DesktopApp.vue` hears about them later, as `create-file` or `create-dir`
   with a name in hand — which is also why nothing is undone if the person
   changes their mind, since nothing has happened yet.

   The folder the field goes into is `folderOf`'s answer and not this
   component's: New file on a folder puts it inside, on a file beside it, and on
   the root menu at the top — the same sentence Open in terminal asks. A folder
   that is not open cannot show a row inside it, so it is opened first, through
   the toggle the tree already emits rather than by writing to a prop this
   component does not own. */
const startDraft = (kind, path) => {
  const dir = folderOf({ path, target: menuTarget.value })
  if (dir !== '' && !props.expanded[dir]) emit('toggle', dir)
  /* A zero byte in the key, the same trick the “…N more” stub uses: no
     filesystem lets that character into a name, so a folder holding a file
     actually called `draft` still cannot collide with this row. */
  draft.value = { key: `${dir}\u0000draft`, kind, dir }
}

/* Rename, which leaves this component no more than the two making verbs do:
   what it puts on screen is the same field, filled, where the row was. The name
   it opens with is the row's own, read from the tree rather than carried by the
   menu — the pick hands over a path, and the name is the last segment of it,
   which is exactly what a rename is about.

   `DesktopApp.vue` hears about it later, as `commit-rename` with a name in
   hand, so nothing has to be undone if the person changes their mind. */
const startRename = (path) => {
  const row = rows.value.find((r) => r.path === path)
  if (!row) return
  /* `target` travels with the draft because the pick that opens it and the
     commit that closes it are separated by however long somebody types, and by
     then `menuTarget` is about a panel that has gone. It is the row's own kind
     and never a constant: nothing reads it today, and a value that is wrong on
     every folder is a trap for whoever reads it first. */
  draft.value = {
    key: `${path}\u0000rename`,
    kind: 'rename',
    path,
    value: row.name,
    target: row.kind === 'dir' ? 'dir' : 'file'
  }
}

/* Esc, or the field losing the keyboard. The draft's key travels with it and is
   checked, because a draft replaced by another unmounts the first field, and a
   browser that fires that field's `blur` on the way out would otherwise cancel
   the draft that replaced it. */
const cancelDraft = (key) => {
  if (draft.value?.key === key) draft.value = null
}

/* Enter in the field. The name goes up as it was typed: what it comes to is
   `newEntry.js`'s rule and `DesktopApp.vue` applies it, because the two
   outcomes that are not a file — nothing at all, and a refusal with a sentence
   — are a toast's business and toasts live there. The draft closes either way:
   the field has answered, and a refusal names the mistake in words rather than
   by leaving the cursor where it was. */
const commitDraft = (name) => {
  const pending = draft.value
  draft.value = null
  if (!pending) return
  if (pending.kind === 'rename') {
    /* The name goes up raw, exactly as the two making verbs send theirs: what a
       typed name comes to — and a name typed back unchanged, which is a cancel
       rather than a rename — is `newEntry.js`'s rule and `DesktopApp.vue`'s to
       apply, because the answers that are not a rename are a toast's business
       and toasts live there. */
    emit('action', { kind: 'commit-rename', path: pending.path, target: pending.target, name })
    return
  }
  emit('action', {
    kind: pending.kind === 'dir' ? 'create-dir' : 'create-file',
    path: pending.dir,
    target: 'dir',
    name
  })
}

/* One event out, carrying which verb, which path and what the path is — the
   same shape `newTabMenu.js` hands `onNewTab`, and matched by hand on the other
   side for want of anything that could join them.

   `target` travels with the pick rather than being worked out again there:
   `PointerMenu` closes before it emits, so `menuTarget` is the caller's to read
   only while it still means something, and two answers to "what was this menu
   about" is how the panel and the handler come apart.

   Delete is the one row picked twice. The first pick arms it and emits nothing
   at all — `keepOpen` on that row is what leaves the panel up to be read a
   second time — and only the pick that arrives with the row already armed
   travels.

   Which pick this is comes off the **item** and not off `confirmingDelete`, and
   that is the whole of it: the armed row closes the panel, closing emits
   `close`, and `onMenuClose` has already put the flag back to false by the time
   this handler runs. Read here it would say "not armed" on every pick, so the
   row would arm itself for ever over a panel that had just gone. The item is
   the copy that cannot go stale — it was built for the pick that is being
   handled. */
const pick = (item, path) => {
  if (item.kind === 'new-file') return startDraft('file', path)
  if (item.kind === 'new-folder') return startDraft('dir', path)
  if (item.kind === 'rename') return startRename(path)
  if (item.kind === 'delete' && item.keepOpen) {
    confirmingDelete.value = true
    return
  }
  emit('action', { kind: item.kind, path, target: menuTarget.value })
}

/* However the panel left. Both flags are about the panel and nothing else, so
   both go with it — the armed Delete above all, which is the whole of the
   promise that Esc, a click elsewhere and a scroll delete nothing. */
const onMenuClose = () => {
  menuFor.value = null
  confirmingDelete.value = false
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
    <template v-for="r in rows" :key="r.key ?? r.path">
      <!-- The draft is a row of the same list rather than something drawn over
           it: that is what puts it at the depth of the folder it is going into
           and pushes the rows below it down, which is the whole of what says
           where the entry will be. -->
      <FileTreeDraftRow
        v-if="r.draft"
        :kind="r.kind"
        :depth="r.depth"
        :value="r.value"
        @commit="commitDraft"
        @cancel="cancelDraft(r.key)"
      />
      <FileTreeRow
        v-else
        v-bind="r"
        :menu-open="menuFor === r.path"
        @toggle="$emit('toggle', r.path)"
        @select="$emit('select', r.path)"
        @open="$emit('open', r.path)"
        @menu="openRowMenu(r, $event)"
      />
    </template>
    <PointerMenu ref="menu" :items="items" :width="FILE_MENU_W" @select="pick" @close="onMenuClose" />
  </div>
</template>
