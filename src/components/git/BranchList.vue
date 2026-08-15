<script setup>
/* The local branches of one repository, which of them it is on, and switching
   to another.

   The order is `git::by_recency`'s and is drawn exactly as it arrives — the
   branch somebody merges into every day is nowhere in particular
   alphabetically, so re-sorting here would bury the one row that matters. A
   linked worktree offers the whole repository's list rather than the single
   branch it is itself on, which is `parse_commondir`'s doing one layer down.

   The current branch is marked and is not a target: checking out the branch you
   are on is the one row in this list with nothing behind it.

   Whether a checkout may be offered at all is `gitActions.js` and not this
   file's — a rule about the project's runs, pure and tested, where a `.vue`
   file is the one thing no test in this repository reaches. What arrives here
   is its verdict, and both halves of it are used: the row goes inert on
   `allowed`, and the tooltip over it is the `reason` that came with it.

   Remote branches, upstreams, ahead/behind counts and folders for `feature/…`
   are outside this epic. So are creating, renaming and deleting a branch: a
   list that reads and one act that switches is the whole of it. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import Tooltip from '../core/Tooltip.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  /* `[{ name, current }]` as `vcs_branches` answers. */
  branches: { type: Array, default: () => [] },
  /* `{ allowed, reason }` from `gitActions.js`. The default is the answer for a
     project with no run going, which is what the gallery and every
     single-branch frame want. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  /* The branch a checkout is in flight for, or null. */
  checkingOut: { type: String, default: null },
  /* Git's own refusal, `{ kind, message }`, shown exactly as git wrote it. */
  error: { type: Object, default: null }
})
defineEmits(['checkout'])

/* Hover is per row and `useInteractive` tracks one control at a time, so an
   instance built inside `rowStyle` would be thrown away on every re-render.
   Cached by name, exactly as `RepoList` caches by path. */
const rowInteractive = new Map()
const interactiveFor = (name) => {
  let entry = rowInteractive.get(name)
  if (!entry) {
    entry = useInteractive()
    rowInteractive.set(name, entry)
  }
  return entry
}

const MARK = 12

/* A run holds the whole list, and so does a checkout already going: what a
   second press would ask for is git working in a tree git is working in. */
const blocked = computed(() => !props.actions?.allowed || Boolean(props.checkingOut))

/* The sentence over a row nobody may press. A checkout in flight is deliberately
   not one — it is over in a moment and the row itself is spinning, so a panel of
   prose about it would be in the way of the thing it explains. */
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
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: branch.current
    ? 'var(--text-primary)'
    : blocked.value
      ? 'var(--text-muted)'
      : 'var(--text-secondary)',
  background:
    branch.current
      ? 'var(--surface-selected)'
      : target(branch) && interactiveFor(branch.name).hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: blocked.value && !branch.current ? 'not-allowed' : 'default',
  transition: 'var(--transition-control)'
})

const nameStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

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

/* Git's own words, mono and left aligned: this is machine output and the person
   reading it knows git — a branch held by another worktree and a working tree
   that would be overwritten are both sentences git already writes better than
   we would. The shape is `GitPanel`'s failure block, one section down. */
const failureStyle = {
  padding: 'var(--space-5)',
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)'
}
const failureTitleStyle = {
  font: 'var(--weight-medium) var(--text-sm)/1 var(--font-sans)',
  color: 'var(--status-failed-fg)'
}
const failureTextStyle = {
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-secondary)',
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere'
}

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
    <component
      :is="hint ? Tooltip : 'div'"
      v-for="branch in branches"
      :key="branch.name"
      v-bind="hint ? { label: hint, side: 'right' } : {}"
      :style="{ display: 'block' }"
    >
      <div
        :style="rowStyle(branch)"
        :aria-disabled="target(branch) ? undefined : 'true'"
        v-bind="target(branch) ? interactiveFor(branch.name).handlers : {}"
        @click="target(branch) && $emit('checkout', branch.name)"
      >
        <Icon name="git-branch" :size="MARK" :style="{ flex: 'none', color: 'var(--text-muted)' }" />
        <span :style="nameStyle">{{ branch.name }}</span>
        <span :style="{ flex: 1 }" />
        <!-- The tick is the whole of what says which branch this repository is
             on, and it is a glyph rather than the highlight alone: the row's
             surface is also what hover uses, and a state told apart by shade
             only would be two facts on one channel. `title` rather than a role
             and a label on the span — it is `Icon`'s own way of being named,
             and a glyph with no name reads as nothing at all to a screen
             reader. -->
        <span :style="markBox">
          <Icon
            v-if="checkingOut === branch.name"
            name="loader-circle"
            :size="MARK"
            :style="spinStyle"
            title="Switching to this branch"
          />
          <Icon v-else-if="branch.current" name="check" :size="MARK" title="Current branch" />
        </span>
      </div>
    </component>
    <!-- Under the list rather than over it: the list is what a person is
         looking at, and the refusal is about the row they just pressed in
         it. -->
    <div v-if="error" :style="failureStyle">
      <div :style="failureTitleStyle">Git did not switch branch</div>
      <div :style="failureTextStyle">{{ error.message }}</div>
    </div>
    <!-- Its own sentence, like every other empty state in this panel. A
         repository whose first commit is not written yet has no ref on disk at
         all, and that is a fact worth stating rather than a blank area. -->
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
