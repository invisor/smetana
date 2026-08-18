<script setup>
/* The local branches of one repository, which of them it is on, and the four
   things that can be done from a row: switch to it, merge it into the current
   branch, rebase the current branch onto it, cut a new branch from it.

   **All four live in the row's right-click menu**, and the first of them is
   also the row's own click. Merging and rebasing used to be two buttons that
   appeared on the row under the pointer, which is a control per row per verb in
   a panel that also draws a file tree, a change list and a commit box; they are
   `branchMenu.js`'s three items now and the row draws its name, its mark and
   nothing else. What that costs is real and worth writing down: a right-click
   is a gesture somebody has to know about, and nothing on the row says the two
   verbs exist. The menu is `PointerMenu`, the same panel on the same gesture as
   the project list one level up, which is the closest thing to a hint there is.

   The order is `git::by_recency`'s and is drawn exactly as it arrives — the
   branch somebody merges into every day is nowhere in particular
   alphabetically, so re-sorting here would bury the one row that matters. A
   linked worktree offers the whole repository's list rather than the single
   branch it is itself on, which is `parse_commondir`'s doing one layer down.

   The current branch is marked and is not a target for the first three:
   checking out, merging or rebasing onto the branch you are already on is a row
   with nothing behind it. The fourth is live there like anywhere else — a
   branch cut from where you are standing is the ordinary case, not an edge
   one — which is why `branchMenu.js`'s refusals have two different reaches.

   Whether any of it may be offered at all is `gitActions.js` and not this
   file's — a rule about the project's runs, pure and tested, where a `.vue`
   file is the one thing no test in this repository reaches. What arrives here
   is its verdict, and both halves of it are used: the row goes inert on
   `allowed`, and the tooltip over it is the `reason` that came with it. The
   same one verdict covers every write here, since what it is about — a batch
   that may be mid-merge — is no more survivable for one of them than another.

   ## Folders

   Everything before a slash is a heading, the way GitLens draws one, and what
   a row shows is the leaf — which is the width this buys back, since the
   prefix is the same on every row under one heading and the tail is the half
   that identifies a branch. The whole name still travels on the row, because
   that is what a checkout, a merge, a rebase and a new branch's start point are
   given.

   Which rows those are is `branchTree.js`, pure and tested, of the
   `gitActions.js` family; this file draws them. The order it hands back is
   still `git::by_recency`'s — a folder stands where its most recent branch
   stood — so the promise above survives the grouping.

   A heading can be pressed while a run holds the three writes, and it is
   deliberately not dimmed with the rows: unfolding is reading, not writing, and
   a heading greyed out beside branches that are greyed out for a real reason
   would say something untrue about it.

   ## Where a branch stands against its upstream

   A row whose upstream holds commits it does not draws its name in
   `--git-modified` and a `↓N` beside it, and one that is ahead draws `↑N` in
   the neutral `--type-plain-fg` without taking the colour: what was asked for is
   a branch with something to **pull**, and colouring both would leave the two
   indistinguishable at a glance. Never colour alone, which is what the count is
   for — the mark survives a monochrome screen and anybody who does not separate
   those two hues. A folded heading carries a bare `↓` for the branches it is
   hiding, since otherwise the mark would be invisible in exactly the
   repositories that need it.

   What any of that means is `tracking.js`, pure and tested, of the
   `gitActions.js` family; this file draws its verdict and holds none of it.

   Remote branches as rows of their own are still outside this epic, and so is
   checking one out. So are renaming and deleting a branch, and so is every flag
   and strategy a merge can take: this offers the merge and the rebase git would
   do by itself, and nothing else. Creating one was outside it too until a row's
   menu had somewhere to put it. */
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import PointerMenu from '../overlays/PointerMenu.vue'
import { useInteractive } from '../core/interactive.js'
import { branchMenuItems } from './branchMenu.js'
import { branchRows, expandedFolders, toggleFolder } from './branchTree.js'
import { AHEAD_TOKEN, BEHIND_TOKEN, trackingMark } from './tracking.js'

const props = defineProps({
  /* `[{ name, current }]` as `vcs_branches` answers. */
  branches: { type: Array, default: () => [] },
  /* Where each branch stands against its upstream, keyed by name, as
     `vcsState.tracking` holds it. A branch with no record draws no mark, which
     is what a repository with no remote looks like — and what every row looked
     like before this existed. */
  tracking: { type: Object, default: () => ({}) },
  /* Which folders are unfolded, as `settings.project.branchFolders` keeps it —
     or null for "nobody has chosen here", which opens the folder the current
     branch is in. The two are different states and `branchTree.js` says why. */
  folders: { type: Array, default: null },
  /* `{ allowed, reason }` from `gitActions.js`. The default is the answer for a
     project with no run going, which is what the gallery and every
     single-branch frame want. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  /* What git is doing right now — `{ op, branch }` — or null. `op` says which
     control on that row spins, since all three leave from the same row and a
     spinner in the wrong place would name the wrong operation. */
  busy: { type: Object, default: null }
})
const emit = defineEmits(['checkout', 'merge', 'rebase', 'new-branch', 'toggle-folder'])

/* Hover is per row and `useInteractive` tracks one control at a time, so an
   instance built inside `rowStyle` would be thrown away on every re-render.
   Cached by key, exactly as `RepoList` caches by path — and by the row's key
   rather than by its name, since a heading and a branch can read the same and
   would otherwise share one hover. */
const rowInteractive = new Map()
const interactiveFor = (key) => {
  let entry = rowInteractive.get(key)
  if (!entry) {
    entry = useInteractive()
    rowInteractive.set(key, entry)
  }
  return entry
}

/* A folder and a branch can carry the same text — git will not hold both, but
   the rule draws them side by side if they ever arrive — so the kind is part of
   the identity of a row. */
const keyOf = (row) => `${row.kind}:${row.kind === 'folder' ? row.path : row.name}`

/* The menu, and which branch it is open on. The name is kept here because the
   items are built from it and because the row under an open panel has to keep
   its highlight — the panel is teleported to the body, so the pointer moving
   into it leaves the row, and a menu naming nothing on screen is a menu about
   nothing. Everything else about it is `PointerMenu`'s.
 *
 * Wide enough for "Rebase the current branch onto this", measured in the
 * gallery rather than reasoned about: 203px of `--text-sm` sans, and 70px of
 * `ContextMenu` chrome around it — 2×`--border-w`, 2×`--space-2` of panel
 * padding, 2×`--space-4` of row padding, two 14px gutters and their two
 * `--space-4` gaps. 240 clipped that row to "Rebase the current branch …" and
 * a menu row has no tooltip to recover a label from. The caption above the
 * rows is shorter at `--text-2xs`, uppercase and tracked as it is. The number
 * carries the trade every `MENU_W` in this app carries: px does not follow the
 * app-wide font size, so a person running the interface large loses the tail
 * of that one row. */
const menu = ref(null)
const menuFor = ref(null)
const MENU_W = 280

/* Read from the branches rather than from the drawn rows: what the menu asks is
   whether this is the branch the repository is on, which is a fact about the
   repository and not about how the list happens to be folded. */
const items = computed(() =>
  branchMenuItems({
    current: props.branches.some((branch) => branch.name === menuFor.value && branch.current),
    allowed: props.actions?.allowed !== false,
    busy: Boolean(props.busy)
  })
)

const openMenu = (row, event) => {
  menuFor.value = row.name
  menu.value?.open(event, row.name)
}

/* The branch is handed back with the pick rather than read from `menuFor`,
   which closing has already cleared — see `PointerMenu`'s header. Written out
   rather than emitted as `item.kind`: the kinds and the events happen to be the
   same four words today, and a rule file free to add a fifth verb must not be
   able to make this component emit something nobody declared. */
const pick = (item, name) => {
  if (item.kind === 'checkout') emit('checkout', name)
  else if (item.kind === 'merge') emit('merge', name)
  else if (item.kind === 'rebase') emit('rebase', name)
  else if (item.kind === 'new-branch') emit('new-branch', name)
}

const rows = computed(() =>
  branchRows(props.branches, expandedFolders(props.folders, props.branches))
)

const MARK = 12

/* A row is indented by its depth, on the same token the file tree indents by:
   the two are the same gesture in two panels and reading as one tree is the
   point. */
const indent = (depth) => `calc(var(--space-5) + ${depth} * var(--tree-indent))`

/* A run holds the whole list, and so does an operation already going: what a
   second press would ask for is git working in a tree git is working in. */
const blocked = computed(() => !props.actions?.allowed || Boolean(props.busy))

/* The sentence over a row nobody may press. An operation in flight is
   deliberately not one — the row itself is spinning, so a panel of prose about
   it would be in the way of the thing it explains. */
const hint = computed(() => (props.actions?.allowed ? '' : (props.actions?.reason ?? '')))

const target = (branch) => !branch.current && !blocked.value

/* A branch name is an identifier and stays mono. The row highlights only where
   there is something to press: the branch already checked out is not a target,
   and neither is any row while a run is going, so hovering must not promise
   one. Muted with the rest of the row rather than dimmed as a group — the
   current branch is still worth reading while a run holds the panel. */
const rowStyle = (branch) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  paddingLeft: indent(branch.depth ?? 0),
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: branch.current
    ? 'var(--text-primary)'
    : blocked.value
      ? 'var(--text-muted)'
      : 'var(--text-secondary)',
  background:
    branch.current
      ? 'var(--surface-selected)'
      : /* The row with the menu open counts as hovered whether or not anything
           on it may be pressed: the panel is teleported to the body, so the
           pointer moving into it leaves the row, and a menu explaining why a
           row is refused would be doing it over a row nothing points at. */
        menuFor.value === branch.name ||
          (target(branch) && interactiveFor(keyOf(branch)).hover.value)
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: blocked.value && !branch.current ? 'not-allowed' : 'default',
  transition: 'var(--transition-control)'
})

/* A heading, in the same mono as the rows under it — a folder here is the first
   segment of an identifier and not prose, which is where it differs from the
   sans captions of `SectionHeader` one level up.

   A real `<button>` for the reason that caption is one: Enter and Space, a
   place in the tab order and the focus ring `tokens/base.css` already draws,
   none of which a div with a click on it has.

   It is deliberately **not** dimmed while a run blocks the three writes.
   Unfolding is reading, and the whole meaning of the muted rows below is "this
   cannot be pressed now" — a heading that greyed out with them and then
   answered a press would spend that meaning. */
const folderStyle = (row) => {
  const { hover, active } = interactiveFor(keyOf(row))
  return {
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--space-3)',
    width: '100%',
    height: 'var(--row-h)',
    padding: '0 var(--space-5)',
    paddingLeft: indent(row.depth),
    border: 'none',
    textAlign: 'left',
    font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
    color: 'var(--text-secondary)',
    background: active.value
      ? 'var(--surface-active)'
      : hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}

/* How many branches the heading is holding, and it is drawn folded and
   unfolded alike: it is the only thing saying they are there when they are
   not on screen, and a number that appeared on folding would be one more thing
   moving under the pointer. Muted, because it is a measurement beside a name
   and not a second name. */
const countStyle = { flex: 'none', color: 'var(--text-muted)' }

/* The whole name, where it is not what the row draws. Left off while the list
   carries the blocked tooltip: a native title would open under a panel of prose
   already saying something else about the same row. */
const fullName = (row) => (!hint.value && row.name !== row.label ? row.name : undefined)

const nameStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

/* What this row's upstream is holding — the token and the two counts, from
   `tracking.js` rather than from anything worked out here: the rule is testable
   and a `.vue` file is not. */
const mark = (row) => trackingMark(props.tracking[row.name])

/* The name takes the colour when there is something to bring in, and the row's
   own muting still wins over it: while a run holds the panel every row is
   `--text-muted`, and one name in orange there would say a press was possible.
   The current branch is deliberately **not** an exception — it is the row Pull
   is about, and its `↓N` is drawn in the same token whatever the name does. */
const branchNameStyle = (row) =>
  !blocked.value && mark(row).orange
    ? { ...nameStyle, color: `var(${BEHIND_TOKEN})` }
    : nameStyle

/* The count beside the arrow, and never the colour alone: the mark has to
   survive a monochrome screen and anybody who does not separate those hues.
   Mono, because it is a measurement. */
const behindStyle = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-1)',
  flex: 'none',
  color: `var(${BEHIND_TOKEN})`,
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}
const aheadStyle = { ...behindStyle, color: `var(${AHEAD_TOKEN})` }

/* A folded folder leaves its branches out of the list altogether, so without
   this the feature would be invisible in exactly the repositories that need it
   — one `feature/` folder holding thirty branches. No number beside it: the
   heading already carries the count of what it holds, and a second number would
   read as a subtotal of the first. */
const folderBehind = (row) =>
  !row.expanded &&
  props.branches.some(
    (branch) =>
      String(branch?.name ?? '').startsWith(`${row.path}/`) &&
      trackingMark(props.tracking[branch.name]).orange
  )

const folderMarkStyle = { flex: 'none', color: `var(${BEHIND_TOKEN})` }

/* Which operation this row is in the middle of, if any, and what to call it.
   All three spin in the one box at the end of the row: the row holds a single
   glyph at a time — the tick, or whichever operation is running — so there is
   nothing to keep apart and no width to reserve twice. When the two buttons
   were still on the row they needed a box of their own beside this one, held in
   the layout at all times so a name never jumped on hover; with them in the
   menu that box and its arithmetic are gone. */
const OPERATIONS = {
  checkout: 'Switching to this branch',
  merge: 'Merging this branch in',
  rebase: 'Rebasing onto this branch',
  create: 'Cutting a new branch from this'
}

const operation = (branch) =>
  props.busy?.branch === branch.name && OPERATIONS[props.busy?.op] ? props.busy.op : null

/* The mark's box is fixed at the glyph's size so a row does not shift sideways
   between the branch that is current and the ones that are not, or when one of
   them starts spinning — the same reason `ChangeList` fixes its own staged
   mark. */
const markBox = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: `${MARK}px`,
  height: `${MARK}px`,
  flex: 'none',
  color: 'var(--text-primary)'
}
/* The token rather than the 1.6s two other call sites write out: `motion.css`
   owns the number, and it is also where `prefers-reduced-motion` zeroes it. */
const spinStyle = { color: 'var(--attn-live)', animation: 'sm-spin var(--dur-pulse) linear infinite' }

/* Git's refusal of an operation is deliberately **not** drawn here. `GitPanel`
   puts this list inside a scroller capped at `BRANCH_ROWS`, so a failure block
   under the rows sat below the fold of a small inner box nothing scrolls for
   you — with six branches or more, which is most repositories, it was entirely
   out of view. The panel draws it outside that cap instead, next to the block
   it was copied from, which also leaves one copy of how git's stderr looks. */

const empty = computed(() => props.branches.length === 0)
</script>

<template>
  <div>
    <!-- The wrapper is a `Tooltip` only where there is something to explain,
         and a plain `div` otherwise: a tooltip on every row of a list somebody
         is reading would open on the way past each one. The hint has to sit on
         a wrapper rather than on a disabled control, because a native disabled
         button raises no pointer events at all and the panel explaining itself
         would be the one thing a person could not reach — the note `RunModal`
         carries beside its own blocked switch. The panel opens to the right,
         where the window has room: this list sits against the left edge and a
         whole sentence over it would cover the rows it is about. -->
    <template v-for="row in rows" :key="keyOf(row)">
      <!-- A heading, and the one row here that is a button: it is pressed to
           unfold and nothing else, so the keyboard comes free with the element
           rather than being written out. It carries no merge and no rebase —
           there is no such thing as merging a folder — and no tooltip, since
           what the tooltip explains is a refusal that does not reach it. -->
      <button
        v-if="row.kind === 'folder'"
        type="button"
        :style="folderStyle(row)"
        :aria-expanded="row.expanded"
        v-bind="interactiveFor(keyOf(row)).handlers"
        @click="emit('toggle-folder', toggleFolder(folders, branches, row.path))"
      >
        <Icon
          :name="row.expanded ? 'chevron-down' : 'chevron-right'"
          :size="MARK"
          :style="{ flex: 'none' }"
        />
        <Icon
          :name="row.expanded ? 'folder-open' : 'folder'"
          :size="MARK"
          :style="{ flex: 'none', color: 'var(--text-muted)' }"
        />
        <span :style="nameStyle">{{ row.label }}</span>
        <span :style="{ flex: 1 }" />
        <!-- What the fold is hiding, with no number on it: the count next door
             is already a number about this heading, and a second one beside it
             would read as a subtotal of the first. -->
        <Icon
          v-if="folderBehind(row)"
          name="arrow-down"
          :size="MARK"
          :style="folderMarkStyle"
          title="Branches in here are behind their upstream"
        />
        <span :style="countStyle">{{ row.count }}</span>
      </button>
      <component
        :is="hint ? Tooltip : 'div'"
        v-else
        v-bind="hint ? { label: hint, side: 'right' } : {}"
        :style="{ display: 'block' }"
      >
        <!-- `.prevent` for the browser's own menu, which `main.js` refuses
             across the whole app anyway: it is said here too because this row
             is where the reason is legible — a person right-clicking a branch
             is offered this panel and nothing else.

             The menu opens on every branch row, including the one with the
             tick and every row a run has frozen. A gesture that answers on some
             rows and does nothing on others reads as a broken row rather than a
             refused one; `branchMenu.js` puts the refusal at the top of the
             panel instead, once, and greys what it is about. -->
        <div
          :style="rowStyle(row)"
          :aria-disabled="target(row) ? undefined : 'true'"
          v-bind="target(row) ? interactiveFor(keyOf(row)).handlers : {}"
          @click="target(row) && $emit('checkout', row.name)"
          @contextmenu.prevent="openMenu(row, $event)"
        >
          <Icon name="git-branch" :size="MARK" :style="{ flex: 'none', color: 'var(--text-muted)' }" />
          <!-- The leaf, with the whole name behind it: under a heading the
               prefix is on every row and the tail is the half that identifies
               one, so drawing the prefix again spends the width the folder was
               made to save. -->
          <span :style="branchNameStyle(row)" :title="fullName(row)">{{ row.label }}</span>
          <!-- Beside the name rather than at the end of the row: it is a fact
               about this branch, where the box at the end is about what the row
               is doing. `↓` colours the name and `↑` does not — what was asked
               for is a branch with something to pull. -->
          <span v-if="mark(row).behind" :style="behindStyle">
            <Icon name="arrow-down" :size="MARK" />{{ mark(row).behind }}
          </span>
          <span v-if="mark(row).ahead" :style="aheadStyle">
            <Icon name="arrow-up" :size="MARK" />{{ mark(row).ahead }}
          </span>
          <span :style="{ flex: 1 }" />
          <!-- The tick is the whole of what says which branch this repository
               is on, and it is a glyph rather than the highlight alone: the
               row's surface is also what hover uses, and a state told apart by
               shade only would be two facts on one channel. `title` rather than
               a role and a label on the span — it is `Icon`'s own way of being
               named, and a glyph with no name reads as nothing at all to a
               screen reader. -->
          <span :style="markBox">
            <Icon
              v-if="operation(row)"
              name="loader-circle"
              :size="MARK"
              :style="spinStyle"
              :title="OPERATIONS[operation(row)]"
            />
            <Icon v-else-if="row.current" name="check" :size="MARK" title="Current branch" />
          </span>
        </div>
      </component>
    </template>
    <!-- Its own sentence, like every other empty state in this panel, and
         deliberately narrow about what it can mean. A repository with no commit
         yet still offers one branch — `git.rs` pushes HEAD's own name into the
         list precisely so an unborn repository has something to merge into — so
         what reaches here is a folder git can see nothing in at all. -->
    <div
      v-if="empty"
      :style="{
        padding: 'var(--space-5)',
        color: 'var(--text-muted)',
        font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
      }"
    >
      No local branches in this repository.
    </div>
    <PointerMenu ref="menu" :items="items" :width="MENU_W" @select="pick" @close="menuFor = null" />
  </div>
</template>
