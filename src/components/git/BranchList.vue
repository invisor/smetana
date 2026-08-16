<script setup>
/* The local branches of one repository, which of them it is on, and the three
   things that can be done from a row: switch to it, merge it into the current
   branch, rebase the current branch onto it.

   The order is `git::by_recency`'s and is drawn exactly as it arrives — the
   branch somebody merges into every day is nowhere in particular
   alphabetically, so re-sorting here would bury the one row that matters. A
   linked worktree offers the whole repository's list rather than the single
   branch it is itself on, which is `parse_commondir`'s doing one layer down.

   The current branch is marked and is not a target for any of the three:
   checking out, merging or rebasing onto the branch you are already on is the
   one row in this list with nothing behind it.

   Whether any of it may be offered at all is `gitActions.js` and not this
   file's — a rule about the project's runs, pure and tested, where a `.vue`
   file is the one thing no test in this repository reaches. What arrives here
   is its verdict, and both halves of it are used: the row goes inert on
   `allowed`, and the tooltip over it is the `reason` that came with it. The
   same one verdict covers all three writes, since what it is about — a batch
   that may be mid-merge — is no more survivable for one of them than another.

   ## Folders

   Everything before a slash is a heading, the way GitLens draws one, and what
   a row shows is the leaf — which is the width this buys back, since the
   prefix is the same on every row under one heading and the tail is the half
   that identifies a branch. The whole name still travels on the row, because
   that is what a checkout, a merge and a rebase are given.

   Which rows those are is `branchTree.js`, pure and tested, of the
   `gitActions.js` family; this file draws them. The order it hands back is
   still `git::by_recency`'s — a folder stands where its most recent branch
   stood — so the promise above survives the grouping.

   A heading can be pressed while a run holds the three writes, and it is
   deliberately not dimmed with the rows: unfolding is reading, not writing, and
   a heading greyed out beside branches that are greyed out for a real reason
   would say something untrue about it.

   Remote branches, upstreams and ahead/behind counts are outside this epic. So
   are creating, renaming and deleting a branch, and so is every flag and
   strategy a merge can take: this offers the merge and the rebase git would do
   by itself, and nothing else. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'
import { useInteractive } from '../core/interactive.js'
import { branchRows, expandedFolders, toggleFolder } from './branchTree.js'

const props = defineProps({
  /* `[{ name, current }]` as `vcs_branches` answers. */
  branches: { type: Array, default: () => [] },
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
const emit = defineEmits(['checkout', 'merge', 'rebase', 'toggle-folder'])

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

/* Which control on this row is the one git is busy with. `null` everywhere
   else, including on the very row whose checkout is running: what spins there
   is the mark, not a button. */
const spinning = (branch, op) => props.busy?.branch === branch.name && props.busy?.op === op

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
      : target(branch) && interactiveFor(keyOf(branch)).hover.value
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

/* The two buttons appear on the row under the pointer, and the spinner of an
   operation in flight takes their place — one box, so a row never holds both
   and nothing shifts when one becomes the other.

   The box is held in the layout the rest of the time (`visibility` rather than
   `v-if`), so nothing on the row moves sideways as the pointer crosses it and a
   name is measured against the width it will actually have. Drawing eighty
   buttons on a list of forty branches at all times would be a panel of controls
   with the names squeezed between them; letting them appear and take the space
   then would make every name jump on hover.

   They are deliberately not offered on the current branch or while anything is
   blocked: merging a branch into itself is a no-op git answers "Already up to
   date" to, and the whole list is inert under a run either way. */
const working = (branch) => spinning(branch, 'merge') || spinning(branch, 'rebase')

const controlsStyle = (branch) => ({
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'flex-end',
  gap: 'var(--space-1)',
  flex: 'none',
  /* The width of the two buttons, held whatever is in the box: the spinner is
     one glyph and would otherwise let the name grow the moment an operation
     started and shrink again when it ended. Arithmetic on tokens rather than a
     number, so it follows both densities and the app-wide font size the way the
     buttons themselves do. */
  minWidth: 'calc(var(--control-h-sm) * 2 + var(--space-1))',
  visibility:
    working(branch) || (target(branch) && interactiveFor(keyOf(branch)).hover.value)
      ? 'visible'
      : 'hidden'
})

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
        <span :style="countStyle">{{ row.count }}</span>
      </button>
      <component
        :is="hint ? Tooltip : 'div'"
        v-else
        v-bind="hint ? { label: hint, side: 'right' } : {}"
        :style="{ display: 'block' }"
      >
        <div
          :style="rowStyle(row)"
          :aria-disabled="target(row) ? undefined : 'true'"
          v-bind="target(row) ? interactiveFor(keyOf(row)).handlers : {}"
          @click="target(row) && $emit('checkout', row.name)"
        >
          <Icon name="git-branch" :size="MARK" :style="{ flex: 'none', color: 'var(--text-muted)' }" />
          <!-- The leaf, with the whole name behind it: under a heading the
               prefix is on every row and the tail is the half that identifies
               one, so drawing the prefix again spends the width the folder was
               made to save. -->
          <span :style="nameStyle" :title="fullName(row)">{{ row.label }}</span>
          <span :style="{ flex: 1 }" />
          <!-- `stop` on both: the row itself is the checkout, so a press that
               reached it would switch branch as well as merge. -->
          <span :style="controlsStyle(row)">
            <Icon
              v-if="working(row)"
              name="loader-circle"
              :size="MARK"
              :style="spinStyle"
              :title="spinning(row, 'merge') ? 'Merging this branch in' : 'Rebasing onto this branch'"
            />
            <template v-else>
              <IconButton
                icon="git-merge"
                size="sm"
                label="Merge into the current branch"
                @click.stop="$emit('merge', row.name)"
              />
              <IconButton
                icon="git-graph"
                size="sm"
                label="Rebase the current branch onto this"
                @click.stop="$emit('rebase', row.name)"
              />
            </template>
          </span>
          <!-- The tick is the whole of what says which branch this repository
               is on, and it is a glyph rather than the highlight alone: the
               row's surface is also what hover uses, and a state told apart by
               shade only would be two facts on one channel. `title` rather than
               a role and a label on the span — it is `Icon`'s own way of being
               named, and a glyph with no name reads as nothing at all to a
               screen reader. -->
          <span :style="markBox">
            <Icon
              v-if="spinning(row, 'checkout')"
              name="loader-circle"
              :size="MARK"
              :style="spinStyle"
              title="Switching to this branch"
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
  </div>
</template>
