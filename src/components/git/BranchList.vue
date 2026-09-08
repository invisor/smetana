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
   other row draws — there is no heading above them to carry the prefix. It all
   scrolls with the rest: what was asked for is an order, and a row pinned
   against the top of a box capped at a handful of rows would spend one of them
   on every scroll.

   **The list is three surfaces, and `block` is what says which.** The current
   branch sits on `--surface-selected` between a rule above and a rule below;
   the marked branches sit on `--surface` with one hairline under the last of
   them; the tree sits on `--canvas`, which is the background of this
   component's own root. Which block a row is in is `branchTree.js`'s answer,
   not this file's. It replaces a single hairline under the last lifted row,
   which said only where the tree started and left the two groups above it
   reading as one. Every rule is drawn **inside** the row's own `--row-h` and
   adds no height to it — `box-sizing: border-box` is what makes that true — so
   `GitPanel`'s arithmetic over `BRANCH_ROWS` is untouched, and the row height
   is the same in every block and in every state.

   On the Origin tab nothing is lifted, so no row carries a `block` and the
   whole list is on the canvas — including the branch the repository is
   standing on, which draws its tick there and not the selected surface.

   A whole name is cut **in the middle** rather than at the end: the last
   twelve characters are kept and the head is ellipsised, because the tail is
   the half that identifies a branch. `splitName` in `branchName.js` is the
   rule and there is no measurement in JS — two spans in a `min-width: 0` flex
   line do the rest. A leaf under a folder is untouched by it, since a leaf is
   already a tail.

   A marked row draws a star **in the leading icon's place**, instead of
   `git-branch` and at the same size — a sixth glyph in front of the name would
   put the marked rows' names out of line with all the others, which is the one
   thing this list cannot afford in a column this narrow. What it does not take
   from the glyph it stands in for is the fill: the star is drawn solid where
   every other glyph is an outline, and that is the whole of the mark. It takes
   **no hue at all**, in `--text-secondary` like any other neutral glyph,
   because the one colour this section spends is the one that says a branch is
   behind its upstream. A yellow star spent that budget on a bookmark.

   A heading can be pressed while a run holds the three writes, and it is
   deliberately not dimmed with the rows: unfolding is reading, not writing, and
   a heading greyed out beside branches that are greyed out for a real reason
   would say something untrue about it.

   ## A detached HEAD stands where the current branch would

   With HEAD on a commit rather than on a branch there is no current row for
   `branchTree.js` to lift — that rule knows nothing about this and is not going
   to: no branch stands behind the plate, so nothing in the tree changes and
   there is nothing for a pure rule to answer. This file draws one row instead,
   `HEAD · <short sha>` in the same mono, on the current block's surface between
   the current block's two rules, with the tick in the box at the end. It is
   the same fact the scope bar draws one level up, and the panel saying it is
   what stops the section reading as a repository whose branch simply failed to
   load.

   It answers nothing: no menu, no double click, no hover. There is no branch
   to check out, to merge or to rename, and a row that opened a menu of refusals
   would be offering a vocabulary about something that is not there. Pull and
   Push are gone from the tab row above for the same reason, which is
   `GitPanel`'s half of it.

   ## Each tab's empty state

   An `EmptyState` rather than a line of prose, so a tab with nothing on it
   reads as a state of the panel and not as a list that failed to draw, and the
   two of them say different things. `No local branches` is a folder git can see
   no branch in at all — a repository with no commit yet still offers one, since
   `git.rs` pushes HEAD's own name into the list. `Nothing on origin` covers a
   repository with no remote, one whose `origin` is empty, and the moment while
   the store's single remote list is about another repository; under it is the
   one control this component draws outside a row, a `Fetch` that leaves as an
   event, because "you have not fetched yet" is the only one of those three a
   person can do something about from here. `EmptyState` gains no action slot
   for it: one button in one place is not an API.

   ## Where a branch stands against its upstream

   A row whose upstream holds commits it does not draws a `↓N` beside its name
   in `--git-modified`, and one that is ahead draws `↑N` in the neutral
   `--type-plain-fg`: what was asked for is a branch with something to **pull**,
   and colouring both would leave the two indistinguishable at a glance. Never
   colour alone, which is what the count is for — the mark survives a
   monochrome screen and anybody who does not separate those two hues. A folded
   heading carries a bare `↓` for the branches it is hiding, since otherwise the
   mark would be invisible in exactly the repositories that need it.

   **The name itself takes no colour.** It used to take `--git-modified` with
   the mark, and in a repository where a hundred branches are behind that is a
   column of orange saying nothing the `↓N` beside each name did not. The
   `orange` flag `tracking.js` answers with is still read — the folded
   heading's own mark is made of it — and it stops at the mark.

   What any of that means is `tracking.js`, pure and tested, of the
   `gitActions.js` family; this file draws its verdict and holds none of it.

   ## The Origin tab

   Everything above is the **Local** tab. Beside it is `Origin`, and exactly one
   of the two is on screen: the tab row is `GitPanel`'s, drawn under the
   section's caption and carrying the three remote verbs at its right end, and
   which side it names arrives here as `tab`.

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

   ## The name filter

   With a query on, everything above is put aside and the list is the hits,
   flat, on the canvas: no folders, no headings, no three surfaces. The rows are
   `GitPanel`'s — it has both tabs' hits already, since the tab labels report
   each other's counts — so this component draws them rather than filtering
   again, and the two cannot disagree about what matched.

   A hit keeps everything a row is: its glyph, whether that is the star, the
   branch or the cloud; its tick; its `↓N`/`↑N`; its double click and its menu.
   What it loses is its block, because three surfaces are there to say where
   three groups end and a result list is one group, and its folder, because a
   filter that unfolded things would be editing a preference on the way past.
   The flat list **ignores** the folds rather than changing them: come out of
   the filter and the tree is exactly as it was left.

   The name is drawn in parts and never as HTML — the prefix up to and including
   the last slash muted, the tail in ink, and the matched run on
   `--selection-bg` with no change of weight or colour, so a row does not move
   under the match. `splitName`'s middle truncation is deliberately not applied
   here: that rule holds the last twelve characters because the tail identifies
   a branch, and a hit already says where the interesting part is by
   highlighting it — cutting the middle out from under a highlight would be the
   one thing a filter must not do.

   Nothing matched is an `EmptyState` like every other nothing in this panel,
   and its second line is the whole reason `otherHits` is a prop: with nine
   matches on the other tab it says so and where to go, and with none anywhere
   it names the query back. Under it is the second control this component draws
   outside a row, `Clear filter`, which does what the `x` in the field does.

   ## The keyboard

   The list is a `role="tree"` of `role="treeitem"` rows — the headings among
   them, which keep the `aria-expanded` being a `<button>` already gave them —
   and it holds exactly **one** tab stop: the row `focusedKey` names, or the
   first row when nothing has been focused yet and when what was focused is no
   longer drawn. That is the file tree's roving tabindex one panel over and it
   is here for that panel's reason: a section holding 346 branches must not be
   346 presses of Tab to get past.

   Which verb a press means is `branchKeys.js` and never this file, the split
   the whole `gitActions.js` family keeps. **What a verb then does is the
   gesture the pointer already had, called through the same function**: `switch`
   is `activate`, which is the double click's own handler with the same refusal
   on the current branch and under a run; `fold` and `unfold` are the heading's
   own click; and `menu` opens the same `PointerMenu` at the row's own corner,
   so the items, their refusals and `pick` are one path with the right click's
   rather than a second one free to drift. `parent` is the only verb with
   nowhere to go on some rows — the current branch and the marked ones are
   lifted to depth 0 with no heading above them — and a press that finds no
   heading moves nothing, which is what a tree does at its root.

   The focus ring is `tokens/base.css`'s own and is deliberately neither
   suppressed nor pulled inside: a row is the full width of a box that scrolls,
   so the ring is what says which of a column of identical rows the keyboard is
   on, and there is no token for its width to inset it by.

   A rename here is local and stops there: no upstream is renamed, nothing is
   pushed and nothing on the remote is deleted. So is every flag and strategy a
   merge can take: this offers the merge and the rebase git would do by itself,
   and nothing else. Creating a branch was outside this list too until a row's
   menu had somewhere to put it, and renaming followed the same way. */
import { computed, ref } from 'vue'
import Button from '../core/Button.vue'
import EmptyState from '../core/EmptyState.vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import PointerMenu from '../overlays/PointerMenu.vue'
import { useInteractive } from '../core/interactive.js'
import { branchVerb } from './branchKeys.js'
import { branchMenuItems, originBranchMenuItems } from './branchMenu.js'
import { splitName } from './branchName.js'
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
     drawn under the section's caption; this component is handed the choice like
     every other piece of state here. `branchTree.js` holds the closed list and
     the default, which `settings/model.rs` mirrors. */
  tab: { type: String, default: DEFAULT_BRANCH_TAB },
  /* What somebody typed into the caption's field, already debounced by
     `GitPanel`. Blank means no filter at all and the two lists above are drawn;
     anything else replaces them with `hits`. It is kept as a prop rather than
     derived from `hits` being empty, because "no filter" and "a filter that
     matched nothing" are two different screens. */
  query: { type: String, default: '' },
  /* The rows to draw while `query` is on — `filterBranches`' answer for the tab
     showing, built by `GitPanel` because it needs both sides' counts anyway.
     Filtering again here would be a second copy of the same call, free to
     disagree with the number in the tab label above. */
  hits: { type: Array, default: () => [] },
  /* How many the **other** tab matched, which is the second line of the empty
     state and nothing else: a filter that found nothing here, over a repository
     where `origin` holds nine matches, must not draw a sentence that is true
     about the wrong half of it. */
  otherHits: { type: Number, default: 0 },
  /* The short hash HEAD is sitting on when it is on no branch at all, or null
     for the ordinary case. It is handed down rather than derived here: this
     component is given the branch list and would have to read a detached HEAD
     out of its absence, which is the same shape as a list that has not landed
     yet. `branchTree.js` is deliberately not told about it — no branch stands
     behind the plate, so there is no row for a pure rule to lift. */
  detached: { type: String, default: null },
  /* Whether a fetch somebody pressed for is still out, which is the whole of
     what the button under the Origin tab's empty state reads: it dims while
     the answer it would ask for is already on its way. Its own flag and not
     `busy`, exactly as `GitPanel` holds it — a fetch freezes no row. */
  fetching: { type: Boolean, default: false },
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
  /* The one verb this component offers outside a row, from under the Origin
     tab's empty state: ask the remote what it has. It carries nothing — the
     repository is the caller's — and it is the same event the tab row's own
     check leaves as, so there is one fetch in this panel and not two. */
  'fetch',
  /* The whole name of a branch only `origin` has. A second event and not
     `checkout` with a flag, for the reason the Rust command it reaches is a
     second command: what happens is a local branch being created, and a caller
     that could not tell the two apart would be choosing between them by
     accident. */
  'checkout-remote',
  /* The whole new list, resolved by `branchTree.js`, exactly as `toggle-folder`
     carries one — and a second event because it is a second settings field. */
  'toggle-remote-folder',
  /* From under the no-match state: put the filter away. It carries nothing —
     the query is the caller's, and what this asks for is the same act the `x`
     in the field performs, which is why it is one event and not "clear" and
     "close". */
  'clear-filter'
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

/* The identity of a row on whichever list is drawn, which is the one both the
   keyboard and `rowInteractive` are keyed on. The filtered list is deliberately
   not a third space: a filtered `feature/two` and an unfiltered one are the
   same row and share their hover, and the tab is the only thing that makes two
   rows of one name two rows. */
const navKey = (row) => (props.tab === 'origin' ? originKeyOf(row) : keyOf(row))

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

/* Whether a filter is on at all, which is one question answered once: three
   lists are drawn from it and only ever one of them at a time. A blank query is
   a field somebody has opened and not typed into, and that is the ordinary tree
   rather than a filter matching everything. */
const filtering = computed(() => props.query.trim().length > 0)

const rows = computed(() =>
  props.tab === 'origin' || filtering.value
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
  props.tab === 'origin' && !filtering.value
    ? originBranchRows(props.remote, props.branches, props.remoteFolders)
    : []
)

/* The hits, as a list of their own rather than as `props.hits` read in the
   template: what is drawn is the filter's answer only while a filter is on, and
   a stale list left standing behind a cleared query would be rows about nothing
   somebody typed. */
const hitRows = computed(() => (filtering.value ? props.hits : []))

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

/* Which rows have their pointer state tracked, which is a wider question than
   which rows can be pressed: the current branch answers no gesture and still
   takes `--surface-active` under the press, because a surface that does not
   move under a finger reads as an element that is not there. Everything else is
   `target`'s, so a row a run has frozen is tracked by nothing.

   The current branch is the one exception and keeps its handlers under a run —
   `blocked` is not asked here. What stops it promising anything is `rowStyle`,
   which gates that arm on `!blocked.value`, so the tracking goes on and the
   surface does not move. The gate is there rather than here because the answer
   this function gives is about which element listens, and the one below is
   about what is drawn: a frozen row that stopped listening would also stop
   clearing `hover` on the way out. */
const tracked = (row) => target(row) || row.block === 'current'

/* The last of the marked rows, which is the one carrying the hairline under
   that block. Read off the drawn rows rather than off `favorites`: the stored
   list holds names this repository may not have, and the rule is what decides
   which of them became a row at all. Null when nothing is marked, and the
   whole question then falls away. */
const lastFavourite = computed(
  () =>
    [...rows.value]
      .reverse()
      .find((row) => row.block === 'favourite')?.name ?? null
)

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

/* Unfolding a heading, whichever tab it is on: two settings fields, two events
   and one gesture, so the choice between them is made here rather than at each
   of the four call sites — the two clicks and the two arrows. Only one tab is
   ever on screen, so the tab is the whole of the question. */
const foldToggle = (row) => {
  if (props.tab === 'origin') {
    emit('toggle-remote-folder', toggleRemoteFolder(props.remoteFolders, row.path))
    return
  }
  emit('toggle-folder', toggleFolder(props.folders, props.branches, row.path))
}

/* **What a row does when it is acted on**, and there is one of these rather
   than one per list: the double click on every row of all three lists comes
   here, and so does Enter from the keyboard. That is the point of it — Enter is
   the double click's keyboard spelling, and two functions would be two chances
   for one of them to keep a refusal the other dropped.

   A heading unfolds, since that is the whole of what a heading does. A branch
   row is `target`'s question — not the branch already checked out, and not
   while a run or an operation in flight holds the repository — and on the
   Origin tab it is `originCheckout`'s pair, chosen by the `hasLocal` that
   travels on the row. A hit needs no arm of its own: it carries the same
   fields, since it came out of the same rule. */
const activate = (row) => {
  if (row.kind === 'folder') {
    foldToggle(row)
    return
  }
  if (props.tab === 'origin') {
    originCheckout(row)
    return
  }
  if (target(row)) emit('checkout', row.name)
}

/* ## The keyboard: which rows there are, which of them has the tab stop, and
   where the elements are.

   Whichever list is drawn, as one list: the rule builds the other two empty, so
   exactly one of the three has anything in it and the keyboard walks it without
   knowing which it is. */
const navRows = computed(() => {
  if (filtering.value) return hitRows.value
  return props.tab === 'origin' ? originRows.value : rows.value
})
const navKeys = computed(() => navRows.value.map(navKey))

/* Where the keyboard is, and the one row of the list that is in the tab order.

   It is a key and not an index, because an index is a fact about a list that is
   rebuilt on every fold, every filter and every refresh — the row under index 3
   is a different branch a moment later, and the tab stop would wander without
   anybody pressing anything. A key that no longer names a drawn row falls back
   to the first, which is also the state the list mounts in: `focusedKey` is
   null until something is focused, and the whole question is answered in one
   computed rather than in a watcher that would have to fire after every one of
   those rebuilds. */
const focusedKey = ref(null)
const tabStop = computed(() => {
  const keys = navKeys.value
  return focusedKey.value && keys.includes(focusedKey.value)
    ? focusedKey.value
    : (keys[0] ?? null)
})

/* The drawn elements, by the same key, so a press can move the focus to a row
   the browser has no reason to move it to on its own. A plain `Map` and not a
   ref: nothing here is drawn from it, and a reactive one would rebuild the list
   it is a map of. Vue clears an entry by calling the ref with null on the way
   out, which is what keeps this from holding elements that have left the
   document. */
const rowEls = new Map()
const setRowEl = (key, el) => {
  if (el) rowEls.set(key, el)
  else rowEls.delete(key)
}

const focusRow = (key) => {
  if (!key) return
  focusedKey.value = key
  rowEls.get(key)?.focus()
}

/* The heading a row sits under: the nearest folder above it at one less depth.
   Nothing at all for a row at depth 0, which is every row of a filtered list
   and both of the lifted blocks — the current branch and the marked ones are
   drawn above the tree with no heading over them, so there is nowhere to go
   out to and the press does nothing. */
const parentKey = (at) => {
  const list = navRows.value
  const depth = list[at]?.depth ?? 0
  for (let above = at - 1; above >= 0; above -= 1) {
    const row = list[above]
    if (row.kind === 'folder' && row.depth === depth - 1) return navKey(row)
  }
  return null
}

/* The menu, opened from the keyboard at the row's own bottom-left corner.
   `PointerMenu.open` reads two numbers off the event it is given and nothing
   else, so a rect is the whole of what a press has to hand it — and it goes
   through the same `openMenu` the right click does, so the panel is about the
   same row, carries the same items and picks through the same `pick`.

   A heading has no menu on either gesture: there is no such thing as merging a
   folder, and a panel of refusals over one would be offering a vocabulary about
   something that is not a branch. */
const openRowMenu = (row) => {
  if (row.kind === 'folder') return
  const rect = rowEls.get(navKey(row))?.getBoundingClientRect()
  if (!rect) return
  openMenu(row, { clientX: rect.left, clientY: rect.bottom })
}

/* The press itself, on the root rather than on the window — `FileTree`'s rule
   and for its reason: the handler runs only when the focus is already inside
   this list, which is a row, which is the thing every verb here is about.

   `preventDefault` for a verb and never for anything else, so every other key
   is left exactly as it was: Space still folds a heading through the button it
   is, Tab still leaves, and the browser's own find is still the browser's until
   `GitPanel` takes it. Enter is the one press that would otherwise be answered
   twice — a `<button>` activates on it by itself — so cancelling the default is
   also what keeps a heading from folding and unfolding in one press. */
const onKeydown = (event) => {
  const at = navKeys.value.indexOf(tabStop.value)
  if (at < 0) return
  const row = navRows.value[at]
  const verb = branchVerb(event, {
    folder: row.kind === 'folder',
    expanded: Boolean(row.expanded)
  })
  if (!verb) return
  event.preventDefault()
  if (verb === 'up') focusRow(navKeys.value[at - 1])
  else if (verb === 'down') focusRow(navKeys.value[at + 1])
  else if (verb === 'parent') focusRow(parentKey(at))
  else if (verb === 'fold' || verb === 'unfold') foldToggle(row)
  else if (verb === 'switch') activate(row)
  else if (verb === 'menu') openRowMenu(row)
}

/* **The focus ring, pulled inside the row's own edge.** `tokens/base.css` draws
   it 2px wide a pixel *outside* the element, and a row here is flush with the
   left, the right and — at the top of the list — the leading edge of the box
   `GitPanel` scrolls (`overflow: auto`, no padding), so three pixels of ring on
   each of those sides fall outside that box's padding box and are simply cut
   away. Measured in Chromium rather than reasoned about: a focused current
   branch drew one horizontal line under itself, sitting exactly where the
   current block's own `--border` rule already is, which reads as a border and
   not as a ring. And it is not only the first row — `.focus()` scrolls a row
   that was out of view flush against the leading edge, so every arrow press
   that scrolls would clip the ring of the row it just moved to.

   The same line `AttachmentStrip.vue`'s thumbnail and `fieldStyle` in
   `GitPanel.vue` carry, for the same clipping, and it makes three readers of
   `--border-w-strong` as a stand-in for a width that has no token: 2px is the
   ring's width in `base.css` and this token's value, and the two matching is a
   coincidence leaned on knowingly. One answer for every focusable control in
   the app is a design-system question rather than a component's, and a third
   call site is the argument for it rather than the answer.

   Suppressing the ring was never on the table: a roving tabindex means the
   keyboard is on exactly one of a column of identical rows, and the ring is the
   only thing that says which. */
const ringInset = 'calc(var(--border-w-strong) * -1)'

/* A branch name is an identifier and stays mono. The row highlights only where
   there is something to press: the branch already checked out is not a target,
   and neither is any row while a run is going, so hovering must not promise
   one. Muted with the rest of the row rather than dimmed as a group — the
   current branch is still worth reading while a run holds the panel.

   **The background is the row's block**, which `branchTree.js` answers and this
   file only draws: the current branch on `--surface-selected`, a marked one on
   `--surface`, everything else transparent over the canvas this component's
   root paints. Interaction is a step of surface on top of that and never a
   colour or a transform, the rule `interactive.js` states — so the current
   branch keeps its own surface under the pointer and takes `--surface-active`
   only under the press, where a row that reacted by changing colour would be
   saying something about what it is rather than about being touched. */
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
    branch.block === 'current'
      ? !blocked.value && interactiveFor(key).active.value
        ? 'var(--surface-active)'
        : 'var(--surface-selected)'
      : /* The row with the menu open counts as hovered whether or not anything
           on it may be pressed: the panel is teleported to the body, so the
           pointer moving into it leaves the row, and a menu explaining why a
           row is refused would be doing it over a row nothing points at. */
        menuFor.value === branch.name || (target(branch) && interactiveFor(key).hover.value)
        ? 'var(--surface-hover)'
        : branch.block === 'favourite'
          ? 'var(--surface)'
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
  /* The two rules that close the current branch's own surface, and the one
     under the last marked row. They are drawn **inside** the row's `--row-h`,
     which `box-sizing: border-box` is what makes true, so `GitPanel`'s
     arithmetic over `BRANCH_ROWS` never sees them and every row in this list is
     exactly one row tall whatever block it is in.

     `--border` around the current branch and `--border-subtle` under the
     favourites, which is the difference between closing a plate and saying
     where a group ends: the first is a thing on its own and the second is the
     bottom of a run of rows. Under the **last** marked row and not under each,
     because the fact stated is about the bottom of the block. */
  borderTop: branch.block === 'current' ? 'var(--border-w) solid var(--border)' : 'none',
  borderBottom:
    branch.block === 'current'
      ? 'var(--border-w) solid var(--border)'
      : branch.block === 'favourite' && lastFavourite.value === branch.name
        ? 'var(--border-w) solid var(--border-subtle)'
        : 'none',
  outlineOffset: ringInset,
  transition: 'var(--transition-control)'
})

/* The plate a detached HEAD draws where the current branch would be: the same
   row, on the same surface, between the same two rules, and inert. It is a
   constant rather than a call into `rowStyle` because there is no row behind
   it — no depth to indent by, no block to look up, nothing to hover — and
   writing it as a branch row with every branch-shaped field faked would be a
   row pretending to be one. */
const detachedStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  background: 'var(--surface-selected)',
  borderTop: 'var(--border-w) solid var(--border)',
  borderBottom: 'var(--border-w) solid var(--border)',
  cursor: 'default',
  userSelect: 'none'
}

/* Whether to draw it, and the second half of the test is what keeps it honest:
   the hash is about the repository this panel has selected, and a list that
   still holds a current branch is a list that has not caught up with the
   checkout yet. Two rows claiming to be where HEAD is would be worse than a
   moment without the plate. Local only — the Origin tab lifts nothing and has
   no place for it. */
const detachedPlate = computed(
  () =>
    props.tab !== 'origin' &&
    /* Not over a filter result. The plate is the current block, and a flat list
       of hits has no blocks at all — a plate above three matched rows would be
       claiming to be the first of them. What it says is on the scope bar one
       level up either way, and closing the filter brings it back. */
    !filtering.value &&
    Boolean(props.detached) &&
    !props.branches.some((branch) => branch?.current)
)

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
    outlineOffset: ringInset,
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

/* The name takes no colour at all, and that is a decision rather than an
   omission. It used to take `--git-modified` with the mark beside it, which in
   a repository where a hundred branches are behind their upstream is a column
   of orange saying nothing the `↓N` on each row did not — and a hue that is on
   most of the rows has stopped meaning anything. `trackingMark`'s own `orange`
   flag is untouched: the folded heading's bare `↓` is still made of it, and so
   are its tests.

   A function rather than the constant it now returns, because the template
   asks it per row and the question — what colour is this name — is one this
   file may have to answer again. */
const branchNameStyle = () => nameStyle

/* A whole name, cut in the middle. The head shrinks and ellipsises, the tail
   is held whole, and the line they sit in is `min-width: 0` so the head has
   somewhere to give way to — without it a flex item refuses to go below its
   own content and the row simply overflows. Where the cut falls is
   `branchName.js`'s `splitName`, so the one thing worth testing here is
   outside the file no test can reach. */
const wholeNameStyle = { display: 'flex', minWidth: 0, flex: '0 1 auto' }
const headStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const tailStyle = { flex: '0 0 auto', whiteSpace: 'nowrap' }

/* The leading glyph, which is the branch icon on an ordinary row and the star
   on a marked one. What the star does not take from the glyph it stands in for
   is the fill: it is drawn solid where every other glyph in this panel is an
   outline, and being the one filled shape in the column is the whole of what
   makes it a mark. Both `color` and `fill`, and that is the trick — `Icon` sets
   `fill="none"` as a presentation attribute, which any CSS declaration
   overrides, while the outline is still drawn with `stroke="currentColor"`.
   Filling alone would leave a filled body inside a differently coloured
   outline, which reads as a rendering fault rather than as a filled star.

   **It takes no hue.** `--text-secondary`, one step up from the `--text-muted`
   an unmarked row's glyph keeps, so it is legible as a mark without spending a
   colour: the one colour this section spends is `--git-modified`, and what
   that means here is distance from upstream. A yellow of its own said
   "bookmark" in the same breath, in a section where every other coloured thing
   is about the remote. */
const leadStyle = (row) =>
  row.favorite
    ? { flex: 'none', color: 'var(--text-secondary)', fill: 'var(--text-secondary)' }
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
  /* The two that leave from the tab row rather than from a row of the list. They
     are about the current branch and `busy` carries its name, so the spinner
     lands on the row with the tick — which is the rule this panel already keeps
     for every other write. */
  pull: 'Pulling into this branch',
  push: 'Pushing this branch'
}

const operation = (branch) =>
  props.busy?.branch === branch.name && OPERATIONS[props.busy?.op] ? props.busy.op : null

/* What the spinner is called, and it is **one name for all seven operations**
   rather than the sentence `OPERATIONS` holds for each. That is the design
   handoff's own wording and it is a trade rather than an oversight: a screen
   reader hears `git is working` where it used to hear `Renaming this branch`,
   which is less, and in exchange the one thing said on a row nobody can press
   is the one thing that is true of every one of them. `OPERATIONS` is still
   what decides that a spinner is drawn at all, so restoring the longer names is
   one line here if the trade is ever re-weighed.

   `Icon`'s `title` and not a bare attribute: that prop is what the glyph's
   `aria-label` is drawn from, and it is also what stops the `aria-hidden` the
   glyph would otherwise carry — an unnamed icon is decoration, and this one is
   the only thing on the row saying git has not finished. */
const SPINNER_LABEL = 'git is working'

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
const empty = computed(() => {
  /* Never under a filter, even where the tab is genuinely empty: "there are no
     branches here" and "nothing matched what you typed" are two different
     sentences, and the second one is `noMatch`'s below. */
  if (filtering.value) return false
  /* Read off the rows on the Origin tab rather than off the list handed in: a
     name the rule drops — an empty string, a `/` on its own — leaves a tab with
     nothing on it, and a sentence is what this panel draws for nothing on it. A
     folded folder is still a row, so folding everything away never reaches
     here. */
  return props.tab === 'origin' ? originRows.value.length === 0 : props.branches.length === 0
})
/* An `EmptyState` rather than the line of prose this used to be, so a tab with
   nothing on it reads as a state of the panel rather than as a list that failed
   to draw. `compact`, because the box it sits in is capped at a handful of rows
   and the roomy version's padding alone is taller than that. The second line of
   each says what to do about it or what it means, which is the difference
   between a sentence and a shrug — and the Origin tab, whose one actionable
   case is a fetch nobody has run, gets a button under it. */
const EMPTY_COPY = {
  local: {
    title: 'No local branches',
    description: 'The first commit creates one. Fetch to see what origin has.'
  },
  origin: {
    title: 'Nothing on origin',
    description: 'Either the remote is empty or you have not fetched yet.'
  }
}
const emptyCopy = computed(() => EMPTY_COPY[props.tab] ?? EMPTY_COPY.local)

/* A matched name, in parts. The prefix — everything up to and including the
   last slash — is muted and the tail is in ink, which is the same split the
   folders make and the reason a flat list can go without headings at all: the
   half that identifies a branch is still the loud half. Over both of them lies
   the matched run, on `--selection-bg` and nothing else — no weight, no colour,
   no border — because a row whose text got heavier under the match would move
   under the eye that is scanning it, and this list is scanned.

   **Parts and never a string of HTML.** A branch name is a string from the
   repository, and building `<span>`s out of it by hand is how a list of names
   becomes an injection. Vue draws these as text nodes; nothing is parsed.

   The two ranges are pushed through one function because the match can straddle
   the slash — `feat/nxc` is one of the queries this exists for — so each half
   of the name is cut against the match rather than the other way about. */
const markedParts = (row) => {
  const parts = []
  const push = (from, to, base) => {
    if (from >= to) return
    const start = Math.max(from, row.match.start)
    const end = Math.min(to, row.match.end)
    if (start >= end) {
      parts.push({ text: row.name.slice(from, to), style: base })
      return
    }
    if (from < start) parts.push({ text: row.name.slice(from, start), style: base })
    parts.push({
      text: row.name.slice(start, end),
      style: { ...base, background: 'var(--selection-bg)', borderRadius: 'var(--radius-1)' }
    })
    if (end < to) parts.push({ text: row.name.slice(end, to), style: base })
  }
  push(0, row.prefix.length, { color: 'var(--text-muted)' })
  push(row.prefix.length, row.name.length, { color: 'var(--text-primary)' })
  return parts
}

/* The same three glyphs the unfiltered rows draw, chosen the same way: the star
   where somebody marked the branch, the cloud where only `origin` has it, and
   the branch glyph otherwise. `favorite` and `hasLocal` both travel on the hit,
   so this is a reading of the rule's answer rather than a second copy of it. */
const hitGlyph = (row) => {
  if (row.favorite) return 'star'
  if (props.tab === 'origin' && !row.hasLocal) return 'cloud'
  return 'git-branch'
}
const hitGlyphTitle = (row) => {
  if (row.favorite) return 'A favourite branch'
  if (props.tab !== 'origin') return undefined
  return row.hasLocal ? 'Also a local branch' : 'Only on origin'
}

/* What the row's upstream is holding, and nothing at all on the Origin tab —
   `vcs_tracking` walks `refs/heads`, so there is no record to read there, which
   is the same reason an unfiltered origin row draws no mark. */
const hitMark = (row) => (props.tab === 'origin' ? { behind: 0, ahead: 0 } : mark(row))

/* Nothing matched, which is a state of the filter rather than of the tab, so it
   says both things: what was looked in, and what the other side found. The
   second line is the one that matters — a filter answering "no origin branch
   matches" over a Local tab holding three of them would be true and useless.
   The query is named back only where there is nothing anywhere, since that is
   the case where the thing to fix is what was typed. */
const noMatch = computed(() => filtering.value && props.hits.length === 0)
const noMatchCopy = computed(() => {
  const here = props.tab === 'origin' ? 'origin' : 'local'
  const other = props.tab === 'origin' ? 'Local' : 'Origin'
  return {
    title: `No ${here} branch matches`,
    description:
      props.otherHits > 0
        ? `${other} has ${props.otherHits}. Switch tabs to see them.`
        : `${props.query.trim()} is not in any ${here} branch name. ${other} has none either.`
  }
})
</script>

<template>
  <!-- The canvas under the tree, which is the third of the section's three
       surfaces and is the root's rather than each row's: a tree row draws no
       background of its own, so what shows through is this. `minHeight: 100%`
       so a short list does not leave the panel's own `--surface` showing under
       the last row, where the block would read as ending somewhere nobody put
       an end to it. -->
  <!-- `role="tree"`, and the press handler that goes with it: one keydown for
       the whole list rather than one per row, reading `branchKeys.js` about
       whichever row holds the tab stop. It is here and not on the window for
       `FileTree`'s reason — a verb about a row must not fire while the focus is
       in a terminal two panels over. -->
  <div
    role="tree"
    :style="{ background: 'var(--canvas)', minHeight: '100%' }"
    @keydown="onKeydown"
  >
    <!-- Where HEAD is when it is on no branch: the current block's surface and
         its two rules, with the tick in the box every row keeps at its end.
         Above the rows rather than among them, because that is where the branch
         it stands in for would be. It answers nothing at all — no menu, no
         double click, no hover — since there is no branch here to check out, to
         merge or to rename. -->
    <div v-if="detachedPlate" :style="detachedStyle">
      <Icon name="git-branch" :size="MARK" :style="{ flex: 'none', color: 'var(--text-secondary)' }" />
      <span :style="nameStyle">HEAD · {{ detached }}</span>
      <span :style="{ flex: 1 }" />
      <span :style="markBox">
        <Icon name="check" :size="MARK" title="Detached HEAD" />
      </span>
    </div>
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
        :ref="(el) => setRowEl(navKey(row), el)"
        type="button"
        role="treeitem"
        :style="folderStyle(row)"
        :aria-expanded="row.expanded"
        :tabindex="tabStop === navKey(row) ? 0 : -1"
        v-bind="interactiveFor(keyOf(row)).handlers"
        @focus="focusedKey = navKey(row)"
        @click="foldToggle(row)"
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
          :ref="(el) => setRowEl(navKey(row), el)"
          role="treeitem"
          :style="rowStyle(row)"
          :aria-disabled="target(row) ? undefined : 'true'"
          :aria-current="row.current ? 'true' : undefined"
          :tabindex="tabStop === navKey(row) ? 0 : -1"
          v-bind="tracked(row) ? interactiveFor(keyOf(row)).handlers : {}"
          @focus="focusedKey = navKey(row)"
          @dblclick="activate(row)"
          @contextmenu.prevent="openMenu(row, $event)"
        >
          <!-- The star stands **in** the branch glyph's place rather than
               beside it: a sixth icon before the name would shift the marked
               rows' names against every other row's, which is the one thing a
               column this narrow cannot afford. Same size as the glyph it
               stands in for and the same neutral family — what makes it a mark
               is that it is filled where every other glyph here is an outline.
               `leadStyle` carries the reason it takes no hue. -->
          <Icon
            :name="row.favorite ? 'star' : 'git-branch'"
            :size="MARK"
            :style="leadStyle(row)"
            :title="row.favorite ? 'A favourite branch' : undefined"
          />
          <!-- A whole name, cut in the middle: the last twelve characters are
               held whole and the head gives way, because the tail is the half
               that identifies a branch — a column of `feat/nxc-204-kickbox-emai…`
               tells nobody which row is which. Where the cut falls is
               `splitName`'s and the two spans are adjacent on purpose: any
               whitespace between them would be drawn inside the name. -->
          <span v-if="row.pinned" :style="wholeNameStyle" :title="fullName(row)">
            <span :style="headStyle">{{ splitName(row.name).head
            }}</span><span :style="tailStyle">{{ splitName(row.name).tail }}</span>
          </span>
          <!-- The leaf, with the whole name behind it: under a heading the
               prefix is on every row and the tail is the half that identifies
               one, so drawing the prefix again spends the width the folder was
               made to save. It is already a tail, so nothing is cut out of its
               middle. -->
          <span v-else :style="branchNameStyle(row)" :title="fullName(row)">{{ row.label }}</span>
          <!-- Beside the name rather than at the end of the row: it is a fact
               about this branch, where the box at the end is about what the row
               is doing. `↓` takes `--git-modified` and `↑` stays neutral —
               what was asked for is a branch with something to pull — and
               neither of them reaches the name. -->
          <span
            v-if="mark(row).behind"
            :style="behindStyle"
            :aria-label="`${mark(row).behind} behind`"
          >
            <Icon name="arrow-down" :size="MARK" />{{ mark(row).behind }}
          </span>
          <span
            v-if="mark(row).ahead"
            :style="aheadStyle"
            :aria-label="`${mark(row).ahead} ahead`"
          >
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
              :title="SPINNER_LABEL"
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
        :ref="(el) => setRowEl(navKey(row), el)"
        type="button"
        role="treeitem"
        :style="folderStyle(row, originKeyOf(row))"
        :aria-expanded="row.expanded"
        :tabindex="tabStop === navKey(row) ? 0 : -1"
        v-bind="interactiveFor(originKeyOf(row)).handlers"
        @focus="focusedKey = navKey(row)"
        @click="foldToggle(row)"
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
          :ref="(el) => setRowEl(navKey(row), el)"
          role="treeitem"
          :style="rowStyle(row, originKeyOf(row))"
          :aria-disabled="target(row) ? undefined : 'true'"
          :aria-current="row.current ? 'true' : undefined"
          :tabindex="tabStop === navKey(row) ? 0 : -1"
          v-bind="target(row) ? interactiveFor(originKeyOf(row)).handlers : {}"
          @focus="focusedKey = navKey(row)"
          @dblclick="activate(row)"
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
              :title="SPINNER_LABEL"
            />
            <Icon v-else-if="row.current" name="check" :size="MARK" title="Current branch" />
          </span>
        </div>
      </component>
    </template>
    <!-- The filter's answer, in place of both lists above: they are built as
         empty while a query is on, so which of the three is drawn is one
         question answered once, in the script. Flat on the canvas, with no
         folders and no block surfaces — three surfaces are there to say where
         three groups end, and this is one group. -->
    <template v-for="row in hitRows" :key="navKey(row)">
      <component
        :is="hint ? Tooltip : 'div'"
        v-bind="hint ? { label: hint, side: 'right' } : {}"
        :style="{ display: 'block' }"
      >
        <div
          :ref="(el) => setRowEl(navKey(row), el)"
          role="treeitem"
          :style="rowStyle(row, navKey(row))"
          :aria-disabled="target(row) ? undefined : 'true'"
          :aria-current="row.current ? 'true' : undefined"
          :tabindex="tabStop === navKey(row) ? 0 : -1"
          v-bind="tracked(row) ? interactiveFor(navKey(row)).handlers : {}"
          @focus="focusedKey = navKey(row)"
          @dblclick="activate(row)"
          @contextmenu.prevent="openMenu(row, $event)"
        >
          <Icon
            :name="hitGlyph(row)"
            :size="MARK"
            :style="leadStyle(row)"
            :title="hitGlyphTitle(row)"
          />
          <!-- The whole name in parts: the prefix muted, the tail in ink and
               the matched run on `--selection-bg`. It truncates at the end like
               any other row — `splitName`'s middle cut is deliberately not
               applied here, since a highlight is already saying which part of
               the name is the interesting one and cutting the middle out from
               under it would hide exactly that. -->
          <span :style="nameStyle" :title="fullName(row)"
            ><span v-for="(part, at) in markedParts(row)" :key="at" :style="part.style">{{
              part.text
            }}</span></span
          >
          <span
            v-if="hitMark(row).behind"
            :style="behindStyle"
            :aria-label="`${hitMark(row).behind} behind`"
          >
            <Icon name="arrow-down" :size="MARK" />{{ hitMark(row).behind }}
          </span>
          <span
            v-if="hitMark(row).ahead"
            :style="aheadStyle"
            :aria-label="`${hitMark(row).ahead} ahead`"
          >
            <Icon name="arrow-up" :size="MARK" />{{ hitMark(row).ahead }}
          </span>
          <span :style="{ flex: 1 }" />
          <span :style="markBox">
            <Icon
              v-if="operation(row)"
              name="loader-circle"
              :size="MARK"
              :style="spinStyle"
              :title="SPINNER_LABEL"
            />
            <Icon v-else-if="row.current" name="check" :size="MARK" title="Current branch" />
          </span>
        </div>
      </component>
    </template>
    <!-- Nothing matched, which is a state of the filter and not of the tab: the
         sentence under the title carries the other tab's hit count where it has
         any, since a filter that found nothing here over nine matches on the
         other side would otherwise be hiding them behind a true sentence. The
         button under it does what the `x` in the field does — clears and
         closes, one act — and it is drawn under `EmptyState` rather than
         through a slot on it, for the reason the Fetch below it is. -->
    <div v-if="noMatch" :style="{ padding: 'var(--space-5)' }">
      <EmptyState
        compact
        icon="search"
        :title="noMatchCopy.title"
        :description="noMatchCopy.description"
      />
      <div :style="{ display: 'flex', justifyContent: 'center', marginTop: 'var(--space-4)' }">
        <Button variant="secondary" size="sm" icon="x" @click="emit('clear-filter')">
          Clear filter
        </Button>
      </div>
    </div>
    <!-- One state per tab, like every other empty state in this panel, and
         deliberately narrow about what each can mean. A repository with no
         commit yet still offers one local branch — `git.rs` pushes HEAD's own
         name into the list precisely so an unborn repository has something to
         merge into — so what reaches the first is a folder git can see nothing
         in at all. The second covers a repository with no remote, one whose
         `origin` is empty, and the moment while the store's single remote list
         is about another repository: all three are "this app knows of no branch
         on origin here", and none of them is a blank area. -->
    <div v-if="empty" :style="{ padding: 'var(--space-5)' }">
      <EmptyState
        compact
        icon="git-branch"
        :title="emptyCopy.title"
        :description="emptyCopy.description"
      />
      <!-- The one of the three cases a person can act on from here, and the
           only control this component draws outside a row. It is under
           `EmptyState` rather than inside it: one button in one place does not
           earn that component a new slot, and the shared empty state is drawn
           in a dozen other places that would have to go on ignoring it. The
           Local tab has no such button — a first commit is not something this
           panel can make. -->
      <div
        v-if="tab === 'origin'"
        :style="{ display: 'flex', justifyContent: 'center', marginTop: 'var(--space-4)' }"
      >
        <Button
          variant="secondary"
          size="sm"
          icon="refresh-cw"
          :disabled="fetching"
          @click="emit('fetch')"
        >
          Fetch
        </Button>
      </div>
    </div>
    <PointerMenu ref="menu" :items="items" :width="MENU_W" @select="pick" @close="menuFor = null" />
  </div>
</template>
