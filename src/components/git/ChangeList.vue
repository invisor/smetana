<script setup>
/* The uncommitted files of one repository.

   What a row says is `changeStatus.js`'s, not this file's, and what order the
   rows come in is `conflictsFirst.js`'s: a `.vue` file is the one thing no test
   here can reach, so the mark, the word, the token and the ordering all live
   outside it.

   A conflicted file is drawn loud, and in three ways rather than one: it is
   lifted to the top of the list, its mark is `!`, and `--git-conflict` is taken
   by the whole row — the mark, the name and the directory — instead of by the
   mark alone. The colour is the token the design system already chose for this
   fact and the file tree already draws with; it is deliberately not the red of
   `--status-failed-*`, which on the same screen belongs to a run that fell
   over. Background, hover and selection are untouched by all of this:
   interaction in this system is a step of surface, never a change of colour.

   A click opens the file as a diff in the centre column — one press and no
   second one: there is no preview here the way the file tree has one, since a
   diff is already a thing somebody asked to look at rather than a file they may
   be scanning past. An untracked *folder* is the one row the click does not
   answer, and it cannot: `--untracked-files=normal` reports it as a single
   record with a trailing slash, and there is no file behind that name to diff.

   **Beside the click there is a menu**, and the click's own verb is the first
   row of it — `changeMenu.js` carries the whole rule and the reason it has to,
   which is that a place whose main action is missing from its own menu reads as
   a place that cannot do it. One `PointerMenu` for the list rather than one per
   row, the shape `FileTree.vue` and `BranchList.vue` both keep, and it opens on
   **every** row including the folder: a gesture that answers on some rows and
   does nothing on others reads as a broken row rather than as a refused one.
   The row under the open panel keeps the hover surface for as long as the panel
   stands, since the panel is teleported to the body and the pointer moving into
   it leaves the row — a menu naming nothing on screen is a menu about nothing.
   A secondary click is a question about a row and not a visit to it, so it
   takes the *hover* surface and never the selected one, which would claim the
   diff in the centre had moved.

   **The rows are focusable and answer three keys, and no more than three.**
   Enter is the click's own verb, `Shift+F10` and the context-menu key open the
   same panel at the row's bottom-left corner — `BranchList.vue`'s `openRowMenu`
   and through the same `openMenu`, so the pointer and the keyboard cannot come
   to mean different things. Which press is which is `changeKeys.js`'s, pure and
   tested, for that family's reason.

   **Every row is an ordinary tab stop and there is no roving tabindex, and that
   is a debt rather than an arrangement this list has earned.** The two are one
   feature: a roving tab stop is what the arrow keys move, and the arrows are a
   later task, so a single stop with nothing to move it would be a state pointing
   at a row and no way to reach the next one. What it costs is real and is not
   bounded by anything here — this list is `tree.changes` whole, git truncates
   nothing and neither does `src-tauri/src/vcs/`, and a tree an agent has just
   swept is routinely dozens of rows, every one of them a press of Tab to get
   past. It is not "a short list"; it is a long one whose keyboard is half
   built. Whoever adds the arrows adds the roving stop with them.

   The commit box is `CommitBox.vue` and it is **not** part of this list: what
   it takes is the whole tree rather than any row here, so a row that could be
   included or left out would be promising a choice nothing behind it can make.
   That is also why there is still **no staging**: a change already carries
   `staged` and the tick draws it, but reading a flag git set is not the same as
   offering to set it, and a commit that takes everything needs no such gesture.

   **Discard is here, and it is the one thing this list does that loses work.**
   It is the last row of the menu, in a group of its own, and it opens a window
   that asks before anything happens — `changeMenu.js` carries why it sits where
   it does and what refuses it, and `DiscardChangeModal.vue` is the question. It
   is also the reason this component now takes `actions` and `busy`: the other
   five rows read, so nothing about the repository reached this list before. */
import { computed, ref, watch } from 'vue'
import Icon from '../core/Icon.vue'
import PointerMenu from '../overlays/PointerMenu.vue'
import { useInteractive } from '../core/interactive.js'
import { basename } from '../../paths.js'
import { fileIconUrl, folderIconUrl } from '../../catppuccinIcon.js'
import { documentTheme } from '../../documentTheme.js'
import { changeStatus } from './changeStatus.js'
import { conflictsFirst } from './conflictsFirst.js'
import { changeVerb } from './changeKeys.js'
import { CHANGE_MENU_W, changeMenuItems, isFolderRecord } from './changeMenu.js'

const props = defineProps({
  changes: { type: Array, default: () => [] },
  /* The path of the change whose diff is open in the centre, so the row a
     person is reading is marked in the list they picked it from. */
  selected: { type: String, default: null },
  /* Whether the selected repository sits inside the project root at all, which
     is what refuses `Open file` and `Copy relative path` on the menu. It is a
     fact about the **repository** and not about the row — every file in a
     repository is inside the project exactly when the repository is — so it
     arrives as one boolean rather than being worked out per row, and it is
     `relativeTo(filesState.root, repo) !== null` where it is computed. A
     repository named in `[project].repos` can sit anywhere at all, so `false`
     is an ordinary state here and not a failure. */
  insideProject: { type: Boolean, default: true },
  /* `{ allowed, reason }` from `gitActions.js`, and `{ op, branch }` or null
     from the store — the pair every write in this panel is refused by, arriving
     here because the menu's last row is a write. Only the *answer* is read: the
     sentence over the group is `frozen`'s in `changeMenu.js`, so the rule and
     the wording stay in the file a test can reach.

     The defaults are a repository with nothing holding it, which is what the
     gallery's own frames and every list-only frame want. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  busy: { type: Object, default: null }
})

/* Named events and never a bare `item.kind`, `BranchList.vue`'s line: the kinds
   and the events happen to be the same words today, and a rule file free to add
   another verb must not be able to make this component emit something nobody
   declared. Each carries the whole change, since what the handler needs is the
   path and the kind together. */
const emit = defineEmits([
  'open',
  'open-file',
  'reveal',
  'copy-path',
  'copy-relative-path',
  /* The one that writes. It carries the whole change like the four above it,
     and it has to: the window's sentence is chosen by `kind`, and Rust is given
     `origPath` as well so that a rename's other half can be put back. */
  'discard'
])

/* Read here rather than in the pure module, which is what keeps the choice of
   noun testable: `fileManagerName` is a function of this string. */
const userAgent = typeof navigator === 'undefined' ? '' : navigator.userAgent

/* The order is this component's and not the store's: `dirtyCount` counts
   `tree.changes` and is indifferent to it, and a store that reordered would be
   deciding for every reader of the tree rather than for this list. */
const rows = computed(() => conflictsFirst(props.changes))

const conflicted = (change) => change.kind === 'conflicted'

/* A folder has no file behind it, so its row stays inert rather than opening a
   diff of a name. The test itself is `changeMenu.js`'s and is borrowed rather
   than written again here: the click, the Enter key and the menu's own two
   greyed rows are the same fact, and a second spelling of it is one character
   away from a greyed row over a live click. */
const openable = (change) => !isFolderRecord(change.path)

/* Hover per row, cached by path and pruned as the list changes — `RepoList.vue`
   right beside this one explains why in full: `useInteractive` tracks one
   control, so an instance built inside `rowStyle` would be thrown away on every
   re-render, and an uncached map would keep an entry per file that ever
   changed. */
const rowInteractive = new Map()
const interactiveFor = (path) => {
  let entry = rowInteractive.get(path)
  if (!entry) {
    entry = useInteractive()
    rowInteractive.set(path, entry)
  }
  return entry
}

watch(
  () => props.changes.map((change) => change.path),
  (paths) => {
    const live = new Set(paths)
    for (const path of rowInteractive.keys()) {
      if (!live.has(path)) rowInteractive.delete(path)
    }
  }
)

/* The rows themselves, so a press of the menu key has a corner to hang the
   panel off. Kept by path, the key everything else in this component is keyed
   by, and dropped when the row leaves — `BranchList.vue`'s own `setRowEl`. */
const rowEls = new Map()
const setRowEl = (path, el) => {
  if (el) rowEls.set(path, el)
  else rowEls.delete(path)
}

/* The menu, and which row it is open on. The path is kept here because the
   items are built from it and because the row under an open panel has to keep
   its highlight; everything else about the panel is `PointerMenu`'s. */
const menu = ref(null)
const menuFor = ref(null)

/* Read out of the list rather than remembered from the press, the idiom
   `BranchList.vue` uses: what the rule asks is what git says about this path
   right now, and a copy taken when the menu opened would go on describing a
   file the last refresh has since changed. */
const menuChange = computed(
  () => props.changes.find((change) => change.path === menuFor.value) ?? null
)

const items = computed(() =>
  changeMenuItems({
    path: menuChange.value?.path ?? '',
    kind: menuChange.value?.kind ?? '',
    insideProject: props.insideProject,
    userAgent,
    /* Read at the moment the panel is drawn rather than when it opened, the
       same reading `menuChange` above takes of the row: a run that starts or a
       commit that lands while the menu stands greys the row under the pointer,
       which is what every other control in this panel does. */
    allowed: props.actions?.allowed !== false,
    busy: Boolean(props.busy)
  })
)

const openMenu = (change, event) => {
  menuFor.value = change.path
  menu.value?.open(event, change)
}

/* The same panel from the keyboard, at the row's own bottom-left corner.
   `PointerMenu.open` reads two numbers off the event it is given and nothing
   else, so a rect is the whole of what a press has to hand it — and it goes
   through the same `openMenu` the right click does, so the panel is about the
   same row, carries the same items and picks through the same `pick`. */
const openRowMenu = (change) => {
  const rect = rowEls.get(change.path)?.getBoundingClientRect()
  if (!rect) return
  openMenu(change, { clientX: rect.left, clientY: rect.bottom })
}

/* The change is handed back with the pick rather than read from `menuFor`,
   which closing has already cleared — `PointerMenu` closes before it emits, and
   its header says why. */
const pick = (item, change) => {
  if (!change) return
  if (item.kind === 'open-changes') emit('open', change)
  else if (item.kind === 'open-file') emit('open-file', change)
  else if (item.kind === 'reveal') emit('reveal', change)
  else if (item.kind === 'copy-path') emit('copy-path', change)
  else if (item.kind === 'copy-relative-path') emit('copy-relative-path', change)
  else if (item.kind === 'discard') emit('discard', change)
}

/* Which verb a press means is `changeKeys.js`'s and not this file's, the shape
   `BranchList.vue` and `FileTree.vue` both keep: a table of chords is exactly
   the half of a keyboard worth pinning, and a `.vue` file is the one thing no
   test here can reach. What is left is the thin half — the word that comes back
   becomes the gesture of that name the list already answers, through the very
   functions the pointer reaches, so the two cannot come to mean different
   things.

   `preventDefault` for a verb and never for anything else, so every other key
   is left exactly as it was: Tab and Shift+Tab still walk out of the list, and
   ⌘Enter is still whatever the window makes of it. */
const onKeydown = (change, event) => {
  const verb = changeVerb(event, { openable: openable(change) })
  if (!verb) return
  event.preventDefault()
  if (verb === 'open') emit('open', change)
  else if (verb === 'menu') openRowMenu(change)
}

const rowStyle = (change) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  /* The name is drawn in the row's own colour, so a conflict is coloured
     throughout rather than in the one mark somebody has to find first. */
  color: conflicted(change) ? 'var(--git-conflict)' : 'var(--text-primary)',
  background:
    change.path === props.selected
      ? 'var(--surface-selected)'
      : /* The row with the menu open counts as hovered whether or not anything
           is over it, and whether or not the click would answer on it: the
           panel is teleported to the body, so the pointer moving into it leaves
           the row, and a menu with nothing on screen tying it to a row is a
           menu about nothing. It is the hover surface and not the selected one
           because a secondary click is a question about a row rather than a
           visit to it. */
        menuFor.value === change.path ||
          (openable(change) && interactiveFor(change.path).hover.value)
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: 'default',
  /* The focus ring is `tokens/base.css`'s own and is pulled **inside** the
     row's edge, `BranchList.vue`'s line one section down and for its measured
     reason: this list is drawn in a box `GitPanel` gives `overflow: auto` and
     no padding, so a row is flush with its left, its right and — at the top of
     the scroller — its leading edge, and three pixels of ring on each of those
     sides fall outside the padding box and are simply cut away.

     That makes **five** readers of `--border-w-strong` as the ring's width, and
     they are named rather than counted: `AttachmentStrip`'s thumbnail, the
     status footer's clipped row, a segment of `SegmentedTabs`, a branch row and
     this one. The full argument is in `.claude/rules/vcs-panel.md`, and that
     list is the miss-list for anything moving that token or `base.css`'s own
     `outline: 2px` — a name left out is a control whose ring silently stops
     fitting. */
  outlineOffset: 'calc(var(--border-w-strong) * -1)',
  transition: 'var(--transition-control)'
})

const MARK = 12

/* The mark's box is fixed at the glyph's size so a row does not shift sideways
   when a file is staged, the same reason `AgentList.vue` fixes its own. */
const stagedBox = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: `${MARK}px`,
  height: `${MARK}px`,
  flex: 'none',
  color: 'var(--text-muted)'
}

/* The letter carries the kind, and it is never the colour alone: the same rule
   `status/status.js` keeps for a status badge. */
const letterStyle = (change) => ({
  flex: 'none',
  width: `${MARK}px`,
  textAlign: 'center',
  color: `var(${changeStatus(change.kind).token})`
})

/* What the name is, drawn — the tree's own rule, so a file looks the same in the
   panel it changed in as in the tree it lives in. An untracked folder arrives as
   one record with a trailing slash and takes the folder icon.

   It is the third mark before the name, after the staged tick and the kind's
   letter, and unlike the other two it is in colours this app did not choose.
   The cost is measured rather than suspected: on a modified `.js` the status
   letter and the icon are **0-1 degrees apart in hue**, and on an added `.vue`
   the icon's green and the `A`'s green are 3 degrees apart in dark and 10 in
   light — two marks six pixels apart, one meaning "modified" and one meaning
   "JavaScript". Nothing here fixes that; it was weighed and accepted with the
   set. If this row is ever trimmed back, this glyph is the first thing to go,
   and `core/icons.js` still holds the monochrome page it would go back to. */
const icon = (change) =>
  isFolderRecord(change.path)
    ? folderIconUrl(change.path, false, documentTheme.value)
    : fileIconUrl(change.path, documentTheme.value)

/* The file's own name reads first and its directory follows it muted — the
   shape a person scans a list of changes in. Both in mono: a path is an
   identifier. The name does not shrink and the directory does, so a deep path
   loses its middle rather than the thing being named. */
const nameStyle = { flex: 'none', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }
const pathStyle = (change) => ({
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  /* The directory follows the row: muted is what keeps the name ahead of it on
     an ordinary row, and on a conflicted one the whole line is the mark. */
  color: conflicted(change) ? 'var(--git-conflict)' : 'var(--text-muted)'
})

/* `--untracked-files=normal` is git's own default and this panel's — `all`
   would walk into every untracked directory — so an untracked folder arrives as
   one record with a trailing slash. It is kept: "git/" says a folder where
   "git" would look like a file nobody can find.

   Whether to keep it is `isFolderRecord`'s answer and not a second reading of
   the same string: which glyph the row draws, whether the click answers, and
   whether the menu greys its first two rows are all that one question, and a
   row drawing a file icon under a greyed `Open changes` is what a second
   spelling buys. */
const label = (path) => (isFolderRecord(path) ? `${basename(path)}/` : basename(path))

/* Everything above the thing being named. Empty at the root of the repository,
   where there is nothing to say.

   The trailing separator is dropped here by hand, and this is the **one** place
   in this component that is deliberately not `isFolderRecord`: what it does is
   normalise a string before a lexical split, the idiom `dirname` in
   `src/paths.js` keeps, and it would do exactly the same to a path that names
   no directory at all. It is about the shape of the string and not about
   whether there is a file behind the row, so widening what counts as a folder
   record must not reach it. */
const directory = (path) => {
  const trimmed = path.endsWith('/') ? path.slice(0, -1) : path
  const cut = trimmed.lastIndexOf('/')
  return cut > 0 ? trimmed.slice(0, cut) : ''
}

/* A rename's other half. Named rather than left out: a row saying only where a
   file arrived is the one thing that cannot be checked against `git status`. */
const from = (change) => (change.origPath ? `← ${change.origPath}` : '')

const empty = computed(() => props.changes.length === 0)
</script>

<template>
  <div>
    <!-- `.prevent` for the browser's own menu, which `src/nativeMenu.js`
         refuses across the whole document anyway — refusing a default stops no
         propagation, so this handler still runs, and the row keeps its own
         answer whichever of the two got there first.

         `tabindex="0"` on every row rather than a roving one, and the header
         says why: the roving stop is the arrow keys' other half and the arrows
         are a later task. The list is not short — see there.

         `aria-current` and deliberately not `aria-selected`: what the surface
         under this row means is that its diff is the one open in the centre,
         which is a fact about the rest of the window rather than a selection
         inside this list — there is none, and the menu is careful not to claim
         one. The row already names its one-letter mark for a screen reader, and
         this is the other fact that was drawn and never said.

         **The structure is declared so that the attribute is heard.** A
         focusable `<div>` with no role maps to a generic container, where
         `aria-current` is supported inconsistently and can be dropped without a
         word — which would leave the attribute above as decoration. So the rows
         sit in a `role="list"` of `role="listitem"`s, the shape both sibling
         lists in this panel declare for the same reason (`FileTree.vue` and
         `BranchList.vue` are `role="tree"` over `role="treeitem"`, and
         `BranchList` carries `aria-current` on exactly this kind of row). A
         flat list and not a tree, because that is what this is: no folders, no
         depth and nothing to expand. The wrapper holds the rows alone — the
         empty state below it is prose about the list and not an item of it. -->
    <div v-if="!empty" role="list">
      <div
        v-for="change in rows"
        :key="change.path"
        :ref="(el) => setRowEl(change.path, el)"
        role="listitem"
        tabindex="0"
        :aria-current="change.path === selected ? 'true' : undefined"
        :style="rowStyle(change)"
        v-bind="interactiveFor(change.path).handlers"
        @click="openable(change) && emit('open', change)"
        @contextmenu.prevent="openMenu(change, $event)"
        @keydown="onKeydown(change, $event)"
      >
        <span :style="stagedBox">
          <!-- Staged and unstaged are two different things to somebody looking at
               what an agent has been doing, and the model keeps them apart, so
               the panel does too — with a glyph rather than a shade. -->
          <Icon v-if="change.staged" name="check" :size="MARK" />
        </span>
        <!-- The word is what the letter stands for, and it is the accessible name
             of a mark that is otherwise one character: `M` reads as nothing at
             all to a screen reader. -->
        <span
          role="img"
          :aria-label="changeStatus(change.kind).label"
          :style="letterStyle(change)"
        >{{ changeStatus(change.kind).letter }}</span>
        <img :src="icon(change)" alt="" :width="MARK + 2" :height="MARK + 2" :style="{ display: 'block', flex: 'none' }" />
        <span :style="nameStyle">{{ label(change.path) }}</span>
        <span :style="pathStyle(change)">{{ [directory(change.path), from(change)].filter(Boolean).join(' ') }}</span>
      </div>
    </div>
    <!-- Its own sentence: a repository with nothing changed in it is a fact
         worth stating, and it is not the same fact as a folder that holds no
         repository. -->
    <div
      v-if="empty"
      :style="{
        padding: 'var(--space-5)',
        color: 'var(--text-muted)',
        font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
      }"
    >
      No uncommitted files in this repository.
    </div>
    <PointerMenu
      ref="menu"
      :items="items"
      :width="CHANGE_MENU_W"
      @select="pick"
      @close="menuFor = null"
    />
  </div>
</template>
