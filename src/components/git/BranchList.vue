<script setup>
/* The local branches of one repository, and which of them it is on.

   The order is `git::by_recency`'s and is drawn exactly as it arrives — the
   branch somebody merges into every day is nowhere in particular
   alphabetically, so re-sorting here would bury the one row that matters. A
   linked worktree offers the whole repository's list rather than the single
   branch it is itself on, which is `parse_commondir`'s doing one layer down.

   The current branch is marked and is not a target: checking out the branch you
   are on is the one row in this list with nothing behind it.

   Remote branches, upstreams, ahead/behind counts and folders for `feature/…`
   are outside this epic. So are creating, renaming and deleting a branch: a
   list that reads and one act that switches is the whole of it. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  /* `[{ name, current }]` as `vcs_branches` answers. */
  branches: { type: Array, default: () => [] }
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

/* A branch name is an identifier and stays mono. The row highlights only where
   there is something to press: the branch already checked out is not a target,
   so hovering it must not promise one. */
const rowStyle = (branch) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: branch.current ? 'var(--text-primary)' : 'var(--text-secondary)',
  background:
    branch.current
      ? 'var(--surface-selected)'
      : interactiveFor(branch.name).hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: 'default',
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
   between the branch that is current and the ones that are not — the same
   reason `ChangeList` fixes its own staged mark. */
const markBox = {
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  width: `${MARK}px`,
  height: `${MARK}px`,
  flex: 'none',
  color: 'var(--text-primary)'
}

const empty = computed(() => props.branches.length === 0)
</script>

<template>
  <div>
    <div
      v-for="branch in branches"
      :key="branch.name"
      :style="rowStyle(branch)"
      v-bind="branch.current ? {} : interactiveFor(branch.name).handlers"
      @click="!branch.current && $emit('checkout', branch.name)"
    >
      <Icon name="git-branch" :size="MARK" :style="{ flex: 'none', color: 'var(--text-muted)' }" />
      <span :style="nameStyle">{{ branch.name }}</span>
      <span :style="{ flex: 1 }" />
      <!-- The tick is the whole of what says which branch this repository is
           on, and it is a glyph rather than the highlight alone: the row's
           surface is also what hover uses, and a state told apart by shade
           only would be two facts on one channel. -->
      <span :style="markBox">
        <!-- `title` rather than a role and a label on the span: it is `Icon`'s
             own way of being named, and a glyph with no name reads as nothing
             at all to a screen reader. -->
        <Icon v-if="branch.current" name="check" :size="MARK" title="Current branch" />
      </span>
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
