<script setup>
/* The local branches of one repository, which of them it is on, and the things
   that can be done from a row: switch to it, compare it with the current
   branch, ask an agent to review it, mark it as a favourite, put its name on
   the clipboard, merge it into the current branch, rebase the current branch
   onto it, cut a new branch from it, rename it, delete it.

   **Every one of them lives in the row's right-click menu**, and which they are
   is `branchMenu.js`'s answer rather than this file's — the list above is that
   rule's, written out here for a reader and kept nowhere else.

   **The row's own gesture is a double click, and it switches.** A single click
   does nothing at all, deliberately: this used to be the one place in the app
   where one click on a row of a list wrote the disk, and it sat next to the
   gesture that opens the menu — a pointer that missed by a row had already run
   `git switch`. Nothing was invented to take the single click's place, because
   selecting a row would be a state no part of this panel reads. Hover and
   press stay as they were: the row still reads as pressable, and the named way
   to the same act is the menu's own `Switch to this branch`.

   Nothing captures the pointer here, which is what makes the double click
   arrive at all — `core/interactive.js` binds `mouseenter`/`mouseleave`/
   `mousedown`/`mouseup` and takes no capture. `shell/TabBar.vue` built a whole
   `armed` machine for exactly the opposite case, where a `pointerdown` capture
   redirected the compatibility mouse events and killed `Tab.vue`'s click and
   double click both.

   Merging and rebasing used to be two buttons that appeared on the row under
   the pointer, which is a control per row per verb in a panel that also draws a
   file tree, a change list and a commit box; they are `branchMenu.js`'s items
   now and the row draws its name, its mark and nothing else. What that costs is
   real and worth writing down: a right-click is a gesture somebody has to know
   about, and nothing on the row says the two verbs exist. The menu is
   `PointerMenu`, the same panel on the same gesture as the project list one
   level up, which is the closest thing to a hint there is — and since the
   switch left the single click, it is also where the name of that act is
   written down.

   The order is `git::by_recency`'s and is drawn exactly as it arrives — the
   branch somebody merges into every day is nowhere in particular
   alphabetically, so re-sorting here would bury the one row that matters. A
   linked worktree offers the whole repository's list rather than the single
   branch it is itself on, which is `parse_commondir`'s doing one layer down.

   The current branch is marked and is not a target for the first four:
   checking out, merging or rebasing onto the branch you are already on is a row
   with nothing behind it, and a branch has no difference from itself to draw.
   Cutting a new branch is live there like anywhere else — from where you are
   standing is the ordinary case, not an edge one — which is why
   `branchMenu.js`'s refusals have three different reaches. The comparison is
   the third of them and the narrowest: it writes nothing, so a run in the
   project and an operation in this repository both leave it alone, and the row
   stays live under a caption saying everything else cannot be pressed.

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

   ## The current branch is the first row, always, and the favourites follow it

   `branchTree.js` lifts the current branch out of the tree, so it is on screen
   whatever the reflog says and whatever fold its name would otherwise put it
   behind. Under it come the branches somebody marked, in the order the list
   arrived in. Both groups draw their **whole** name rather than the leaf every
   other row draws — there is no heading above them to carry the prefix — and
   the **last row of the block** carries the hairline `SectionHeader` uses,
   saying the list proper starts below. The hairline sits inside the row's own
   `--row-h` and adds no height to it, which `box-sizing: border-box` is what
   makes true, so `GitPanel`'s arithmetic over `BRANCH_ROWS` is untouched. It
   all scrolls with the rest: what was asked for is an order, and a row pinned
   against the top of a box capped at a handful of rows would spend one of them
   on every scroll.

   A marked row draws a star **in the leading icon's place**, instead of
   `git-branch` and at the same size — a sixth glyph in front of the name would
   put the marked rows' names out of line with all the others, which is the one
   thing this list cannot afford in a column this narrow. What it does not take
   from the glyph it stands in for is the colour: the star is filled, in
   `--branch-favorite-fg`, the one yellow in the system. Position alone says a
   row is at the top and not why it is there, so the mark has to be legible as a
   mark — which the muted outline this used to be was not.

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

   ## The Origin tab

   Everything above is the **Local** tab. Beside it is `Origin`, and exactly one
   of the two is on screen: the tab row is `GitPanel`'s, in the section's own
   caption, and which side it names arrives here as `tab`.

   It draws the whole of `origin` — every branch that remote has, including the
   ones this repository also has locally. That is a change of mind about the
   group this replaces, which sat at the foot of the local list and held only
   what had no local twin. Two things made it unreachable: in a repository with
   346 local branches its heading was 346 rows down a scroller, and in one where
   `origin` holds nothing extra it was not drawn at all — which on screen is
   indistinguishable from broken. A tab called `Origin` that showed half of
   `origin` would be lying in its own title, so it shows all of it.

   Inside it the order is the one `vcs_remote_branches` answered in, which is
   alphabetical — a remote-tracking ref's reflog says when this machine last
   fetched, which is a fact about the fetch and not about anybody's work — and
   nothing is lifted to the top of it: no current branch and no favourites, so
   the alphabet holds all the way down.

   Its folds are their own settings field, `remoteBranchFolders`, and not
   entries in the list the Local tab uses: `feature` on this tab and `feature`
   on that one are two different rows, and one shared list would unfold both at
   once. The paths carry no group prefix any more, since there is no heading
   above them; an entry left over from the old shape matches nothing, which
   means a folded folder and is safe.

   **A row draws one of two glyphs and answers one of two verbs, and the same
   fact decides both.** A branch `origin` has and this repository does not draws
   `cloud`, and the double click is `Check out from origin` — a local branch is
   created, its upstream is set and HEAD moves onto it. A branch this repository
   already has draws `git-branch` at the same size and in the same colour, and
   the double click is an ordinary `vcs_checkout` onto the local branch of that
   name, which creates nothing. Which of the two a row is is `hasLocal`,
   `branchTree.js`'s answer and not this file's, so the glyph, the menu item and
   the gesture cannot come apart. The branch the repository is on carries the
   tick on the right, exactly as it does on the Local tab.

   What this tab never draws is the star and the `↓N`/`↑N`. The first is about
   the list the Local tab reorders; the second has no record to read, since
   `vcs_tracking` walks `refs/heads` — and both are answers about a local branch
   and its upstream, which the Local tab already carries. Checking a branch out
   writes the working tree either way, so the rows mute and go inert under a run
   with the local ones — the one `gitActions.js` verdict covers both — while the
   headings go on unfolding, since unfolding is reading, and so does the tab row
   above them.

   A rename here is local and stops there: no upstream is renamed, nothing is
   pushed and nothing on the remote is deleted. So is every flag and strategy a
   merge can take: this offers the merge and the rebase git would do by itself,
   and nothing else. Creating a branch was outside this list too until a row's
   menu had somewhere to put it, and renaming followed the same way. */
import { computed, ref } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import PointerMenu from '../overlays/PointerMenu.vue'
import { useInteractive } from '../core/interactive.js'
import { branchMenuItems, originBranchMenuItems } from './branchMenu.js'
import {
  DEFAULT_BRANCH_TAB,
  branchRows,
  expandedFolders,
  originBranchRows,
  toggleFavorite,
  toggleFolder,
  toggleRemoteFolder
} from './branchTree.js'
import { AHEAD_TOKEN, BEHIND_TOKEN, folderBehind, trackingMark } from './tracking.js'

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
  /* The branch names pinned above the tree, as
     `settings.project.favoriteBranches` keeps them. A plain list where the
     folders above are nullable, because there is no third state: nothing is
     marked until somebody marks it. A name the selected repository does not
     have draws no row and breaks nothing. */
  favorites: { type: Array, default: () => [] },
  /* What `origin` is known to have, as plain names in the order
     `vcs_remote_branches` answered in — `vcsState.remoteBranches`, and only
     while `remoteBranchesRepo` names the repository this list is about. That
     guard is the caller's and it is load-bearing: the store holds one such list
     for whichever repository was asked about last, and the branch review window
     walks every repository of the project through it. An empty list is the
     Origin tab's own empty state and never a silently blank one. */
  remote: { type: Array, default: () => [] },
  /* Which folders of the Origin tab are unfolded, as
     `settings.project.remoteBranchFolders` keeps them — whole paths with no
     group prefix, `feature` rather than `origin/feature`. A plain list where
     `folders` above is nullable: empty means every folder on that tab is
     folded, which is the default, and there is no third state to tell apart. */
  remoteFolders: { type: Array, default: () => [] },
  /* Which of the two sides to draw, `local` or `origin`, as
     `settings.project.branchTab` keeps it. The tab row itself is `GitPanel`'s,
     in the section's caption; this component is handed the choice like every
     other piece of state here. `branchTree.js` holds the closed list and the
     default, which `settings/model.rs` mirrors. */
  tab: { type: String, default: DEFAULT_BRANCH_TAB },
  /* `{ allowed, reason }` from `gitActions.js`. The default is the answer for a
     project with no run going, which is what the gallery and every
     single-branch frame want. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  /* What git is doing right now — `{ op, branch }` — or null. `op` says which
     control on that row spins, since all three leave from the same row and a
     spinner in the wrong place would name the wrong operation. */
  busy: { type: Object, default: null }
})
const emit = defineEmits([
  'checkout',
  'compare',
  /* The other reader, and the one that does not stop at this repository: it
     opens the window that picks a reference branch and a branch to check, in
     every repository of the project at once. Compare shows and Review judges,
     which is why they are two verbs and not one. */
  'review',
  /* The whole new list, resolved by `branchTree.js`, exactly as `toggle-folder`
     carries one: the panel is told what the list became rather than working it
     out, so the rule stays in the file a test can reach. */
  'favorite',
  /* The whole name of the row, for the clipboard. Whole and never the leaf this
     row draws: the string is wanted for a git command somewhere else, where
     `spike` under a `fix/` heading is a name nothing answers to. */
  'copy-name',
  'merge',
  'rebase',
  'new-branch',
  'rename',
  'delete',
  'toggle-folder',
  /* The whole name of a branch only `origin` has. A second event and not
     `checkout` with a flag, for the reason the Rust command it reaches is a
     second command: what happens is a local branch being created, and a caller
     that could not tell the two apart would be choosing between them by
     accident. */
  'checkout-remote',
  /* The whole new list, resolved by `branchTree.js`, exactly as `toggle-folder`
     carries one — and a second event because it is a second settings field. */
  'toggle-remote-folder'
])

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

/* And the same identity for a row of the Origin tab. The two tabs are never on
   screen together, so this is not the `v-for` key collision it once was — what
   it keeps apart is `rowInteractive`, which is cached for the life of the
   component and would otherwise hand a local `feature` heading and an origin
   one the same hover. It is also why `rowStyle` and `folderStyle` below take a
   key rather than deriving one: the two lists share the drawing and not the
   identity. */
const originKeyOf = (row) => `origin:${keyOf(row)}`

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
/* Which rule builds the open menu's items is the tab rather than a flag set
   when it opened: exactly one side is on screen, so the row under the pointer
   was certainly on that side. One `PointerMenu` for both and not two — only one
   menu can be open, the panel is teleported to the body either way, and a
   second instance would be a second thing to keep closed. */
const MENU_W = 280

const items = computed(() => {
  /* Read from the branches rather than from the drawn rows, which is the same
     idiom both arms below use: what the menu asks is whether the repository has
     this branch and whether it is standing on it, and both are facts about the
     repository rather than about how the list happens to be folded. On the
     Origin tab `hasLocal` is the whole of what chooses between the two verbs,
     and it is asked here the way `branchTree.js` asks it — by whole name. */
  const named = props.branches.filter((branch) => branch.name === menuFor.value)
  const current = named.some((branch) => branch.current)
  return props.tab === 'origin'
    ? originBranchMenuItems({
        allowed: props.actions?.allowed !== false,
        busy: Boolean(props.busy),
        hasLocal: named.length > 0,
        current
      })
    : branchMenuItems({
        current,
        allowed: props.actions?.allowed !== false,
        busy: Boolean(props.busy),
        /* Read from the stored list rather than from the row the menu was
           opened on, for the reason `current` above is read from the branches:
           what the item's label is about is whether this name is marked, which
           is a fact about `settings.json` and not about how the list happens to
           be drawn. */
        favorite: (props.favorites ?? []).includes(menuFor.value)
      })
})

const openMenu = (row, event) => {
  menuFor.value = row.name
  menu.value?.open(event, row.name)
}

/* The branch is handed back with the pick rather than read from `menuFor`,
   which closing has already cleared — see `PointerMenu`'s header. Written out
   rather than emitted as `item.kind`: the kinds and the events happen to be the
   same words today, and a rule file free to add another verb must not be able
   to make this component emit something nobody declared. Two of them have
   arrived exactly that way since, which is this comment having been right
   twice. */
const pick = (item, name) => {
  if (item.kind === 'checkout') emit('checkout', name)
  else if (item.kind === 'compare') emit('compare', name)
  else if (item.kind === 'review') emit('review', name)
  else if (item.kind === 'favorite') emit('favorite', toggleFavorite(props.favorites, name))
  else if (item.kind === 'copy-name') emit('copy-name', name)
  else if (item.kind === 'merge') emit('merge', name)
  else if (item.kind === 'rebase') emit('rebase', name)
  else if (item.kind === 'new-branch') emit('new-branch', name)
  else if (item.kind === 'rename') emit('rename', name)
  else if (item.kind === 'delete') emit('delete', name)
  else if (item.kind === 'checkout-remote') emit('checkout-remote', name)
}

const rows = computed(() =>
  props.tab === 'origin'
    ? []
    : branchRows(
        props.branches,
        expandedFolders(props.folders, props.branches),
        props.favorites
      )
)

/* The Origin tab's rows, and `[]` for every other tab rather than a condition
   in the template: which of the two lists is built is one question and it is
   answered once, here. */
const originRows = computed(() =>
  props.tab === 'origin' ? originBranchRows(props.remote, props.branches, props.remoteFolders) : []
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

/* The double click on a row of the Origin tab, and the one place in this file
   where one gesture reaches two events. `hasLocal` is `branchTree.js`'s answer:
   a branch this repository already has is an ordinary switch onto the local
   branch of that name, and one it does not have is the checkout that creates it
   with `origin/<name>` as its upstream. Written out rather than emitted through
   a computed name, for the reason `pick` below carries: a rule file must not be
   able to make this component emit something nobody declared. */
const originCheckout = (row) => {
  if (!target(row)) return
  if (row.hasLocal) emit('checkout', row.name)
  else emit('checkout-remote', row.name)
}

/* A branch name is an identifier and stays mono. The row highlights only where
   there is something to press: the branch already checked out is not a target,
   and neither is any row while a run is going, so hovering must not promise
   one. Muted with the rest of the row rather than dimmed as a group — the
   current branch is still worth reading while a run holds the panel. */
const rowStyle = (branch, key = keyOf(branch)) => ({
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
        menuFor.value === branch.name || (target(branch) && interactiveFor(key).hover.value)
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: blocked.value && !branch.current ? 'not-allowed' : 'default',
  /* The row switches on a double click, and a double click on text is also
     how a browser selects a word — so without this the second press left the
     leaf highlighted in `--selection-bg` until somebody clicked elsewhere,
     over a row whose surface is already saying something about hover and the
     current branch. Taken off the row and nothing else in this panel: the
     names in the change list and in the tree are still selectable, and this
     one is only unselectable because the gesture on top of it needs the
     press. `agent/LogLine.vue` and `agent/CodeBlock.vue` do the same for the
     gutter they draw beside text somebody is meant to copy. */
  userSelect: 'none',
  /* The rule under the **last** row of the top block, the same hairline
     `SectionHeader` draws above a caption and for the same reason: without it
     those rows read as more rows of the list rather than as the thing the list
     is being read against. Under the last one and not under each, because it
     states one fact — the real list starts below — and that fact is about the
     bottom of the block. `branchTree.js` says which row carries it. */
  borderBottom: branch.divider ? 'var(--border-w) solid var(--border-subtle)' : 'none',
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
const folderStyle = (row, key = keyOf(row)) => {
  const { hover, active } = interactiveFor(key)
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

/* The whole name, where the row cannot be relied on to show it. That is a row
   drawing its leaf under a heading, and it is also the pinned row, which draws
   the whole name and is the one row most likely to run out of width for it —
   the prefix a heading would have carried is on the row itself there. Left off
   while the list carries the blocked tooltip: a native title would open under a
   panel of prose already saying something else about the same row. */
const fullName = (row) =>
  !hint.value && (row.pinned || row.name !== row.label) ? row.name : undefined

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

/* The leading glyph, which is the branch icon on an ordinary row and the star
   on a marked one. The star is the only thing in this panel drawn in a colour
   of its own: `--branch-favorite-fg` is a yellow kept for exactly this mark, so
   a marked row is readable at a glance without a sixth glyph or a second
   column. Both `color` and `fill`, and that is the whole trick — `Icon` sets
   `fill="none"` as a presentation attribute, which any CSS declaration
   overrides, while the outline is still drawn with `stroke="currentColor"`.
   Filling alone would leave a yellow body inside a grey outline, which reads as
   a rendering fault rather than as a filled star. An unmarked row keeps
   `--text-muted` and no fill at all. */
const leadStyle = (row) =>
  row.favorite
    ? { flex: 'none', color: 'var(--branch-favorite-fg)', fill: 'var(--branch-favorite-fg)' }
    : { flex: 'none', color: 'var(--text-muted)' }

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

/* What a fold is hiding, which is `tracking.js`'s answer and not this file's —
   the unfolded case is the only half that belongs here, since a heading with
   its rows on screen has nothing left to stand in for. */
const folderMark = (row) =>
  !row.expanded && folderBehind(row.path, props.branches, props.tracking, props.favorites)

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
  create: 'Cutting a new branch from this',
  /* The one that leaves from a window rather than from this panel at all, and
     it is in this table for `create`'s reason: the dialog closes or stands on
     its second question, and either way the row it was about is still on
     screen, still spinning, until the refresh takes it away. A row dimmed with
     nothing on it saying which branch git is working on is the state this table
     exists to prevent. */
  delete: 'Deleting this branch',
  /* The other one that leaves from a window, and it is keyed on the name the
     branch had when git was asked: the row under the spinner is the old name
     until the refresh brings the list back under the new one. */
  rename: 'Renaming this branch',
  /* The two that leave from the section header rather than from a row. They
     are about the current branch and `busy` carries its name, so the spinner
     lands on the row with the tick — which is the rule this panel already keeps
     for every other write. */
  pull: 'Pulling into this branch',
  push: 'Pushing this branch'
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

/* Each tab has its own sentence and they say different things, which is this
   panel's rule one level down: a repository git can see no branch in at all,
   and a repository whose `origin` this app knows nothing about — no remote, an
   empty one, or a list that has not landed for this repository yet. One blank
   area for both would be a tab saying nothing two different ways. */
const empty = computed(() =>
  /* Read off the rows on the Origin tab rather than off the list handed in: a
     name the rule drops — an empty string, a `/` on its own — leaves a tab with
     nothing on it, and a sentence is what this panel draws for nothing on it. A
     folded folder is still a row, so folding everything away never reaches
     here. */
  props.tab === 'origin' ? originRows.value.length === 0 : props.branches.length === 0
)
const EMPTY_LINE = {
  local: 'No local branches in this repository.',
  origin: 'Nothing on origin for this repository.'
}
const emptyLine = computed(() => EMPTY_LINE[props.tab] ?? EMPTY_LINE.local)
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
          v-if="folderMark(row)"
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
          @dblclick="target(row) && $emit('checkout', row.name)"
          @contextmenu.prevent="openMenu(row, $event)"
        >
          <!-- The star stands **in** the branch glyph's place rather than
               beside it: a sixth icon before the name would shift the marked
               rows' names against every other row's, which is the one thing a
               column this narrow cannot afford. Same size as the glyph it
               stands in for, but a colour of its own — filled yellow, since
               position alone cannot say what the mark means and the muted
               outline it used to be said nothing either. `leadStyle` carries
               the reason. -->
          <Icon
            :name="row.favorite ? 'star' : 'git-branch'"
            :size="MARK"
            :style="leadStyle(row)"
            :title="row.favorite ? 'A favourite branch' : undefined"
          />
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
    <!-- The Origin tab, in place of everything above rather than under it:
         the rule answers with no rows at all on the Local tab, so which side is
         drawn is one question answered once, in the script. -->
    <template v-for="row in originRows" :key="originKeyOf(row)">
      <!-- A heading: pressed to unfold and nothing else, so it stays live under
           a run exactly as the local headings do. It carries its count folded
           and unfolded alike, for that heading's own reason. -->
      <button
        v-if="row.kind === 'folder'"
        type="button"
        :style="folderStyle(row, originKeyOf(row))"
        :aria-expanded="row.expanded"
        v-bind="interactiveFor(originKeyOf(row)).handlers"
        @click="emit('toggle-remote-folder', toggleRemoteFolder(remoteFolders, row.path))"
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
        <!-- No `↓` here where a local heading may carry one: this tab holds no
             tracking record at all, so there is nothing a fold could be
             hiding. -->
        <span :style="countStyle">{{ row.count }}</span>
      </button>
      <component
        :is="hint ? Tooltip : 'div'"
        v-else
        v-bind="hint ? { label: hint, side: 'right' } : {}"
        :style="{ display: 'block' }"
      >
        <!-- The same row as the Local tab's with two things taken away and one
             chosen per row. Gone are the star and the `↓N`/`↑N`: the first is
             about the list the other tab reorders, and the second has no record
             to read, since `vcs_tracking` walks `refs/heads`. What is chosen is
             the leading glyph and the verb behind the double click, and one
             fact decides both — a branch this repository already has draws
             `git-branch` and is an ordinary switch, and one only `origin` has
             draws `cloud` and is the checkout that creates it. The tick stays
             where it is on the other tab: the repository can only be standing
             on a branch it has. -->
        <div
          :style="rowStyle(row, originKeyOf(row))"
          :aria-disabled="target(row) ? undefined : 'true'"
          v-bind="target(row) ? interactiveFor(originKeyOf(row)).handlers : {}"
          @dblclick="originCheckout(row)"
          @contextmenu.prevent="openMenu(row, $event)"
        >
          <Icon
            :name="row.hasLocal ? 'git-branch' : 'cloud'"
            :size="MARK"
            :style="{ flex: 'none', color: 'var(--text-muted)' }"
            :title="row.hasLocal ? 'Also a local branch' : 'Only on origin'"
          />
          <span :style="nameStyle" :title="fullName(row)">{{ row.label }}</span>
          <span :style="{ flex: 1 }" />
          <!-- The same box the rows above keep, holding the spinner while git
               is working on this branch and the tick where the repository is
               standing on it. -->
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
    <!-- One sentence per tab, like every other empty state in this panel, and
         deliberately narrow about what each can mean. A repository with no
         commit yet still offers one local branch — `git.rs` pushes HEAD's own
         name into the list precisely so an unborn repository has something to
         merge into — so what reaches the first is a folder git can see nothing
         in at all. The second covers a repository with no remote, one whose
         `origin` is empty, and the moment while the store's single remote list
         is about another repository: all three are "this app knows of no branch
         on origin here", and none of them is a blank area. -->
    <div
      v-if="empty"
      :style="{
        padding: 'var(--space-5)',
        color: 'var(--text-muted)',
        font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
      }"
    >
      {{ emptyLine }}
    </div>
    <PointerMenu ref="menu" :items="items" :width="MENU_W" @select="pick" @close="menuFor = null" />
  </div>
</template>
