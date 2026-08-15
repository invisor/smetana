<script setup>
/* The repositories a project is made of, with the branch each is on and a mark
   on the one being shown.

   A project is often one repository and sometimes five — `[project].repos` in
   `.smetana/project.toml`, or every folder one level down that git can see
   (`src-tauri/src/vcs/repos.rs`). One row each either way: a list of one is a
   list, and hiding it would leave the branch beside the changes with nothing
   saying which repository they belong to.

   The branch is `git::head`'s answer, which is a file read rather than a
   process — the whole list costs one read per repository. A detached HEAD is
   not dressed up as a branch: it is drawn as its short hash and said to be
   detached, the same as the scope bar does it. */
import { computed, watch } from 'vue'
import Icon from '../core/Icon.vue'
import { useInteractive } from '../core/interactive.js'

const props = defineProps({
  repos: { type: Array, default: () => [] },
  /* The selected repository's absolute path, which is what every command in
     `vcs/` takes and therefore what a row is identified by. */
  selected: { type: String, default: null }
})
defineEmits(['select'])

/* Hover is per row, and `useInteractive` tracks one control at a time — an
   instance built inside `rowStyle` would be thrown away on every re-render.
   Cached by path, exactly as `AgentList.vue` caches by session id. */
const rowInteractive = new Map()
const interactiveFor = (path) => {
  let entry = rowInteractive.get(path)
  if (!entry) {
    entry = useInteractive()
    rowInteractive.set(path, entry)
  }
  return entry
}

/* Repositories come and go — a run's provisioning phase cuts worktrees — and
   without this the cache would keep one entry per folder that ever appeared. */
watch(
  () => props.repos.map((repo) => repo.path),
  (paths) => {
    const live = new Set(paths)
    for (const path of rowInteractive.keys()) {
      if (!live.has(path)) rowInteractive.delete(path)
    }
  }
)

const rowStyle = (repo) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  background:
    repo.path === props.selected
      ? 'var(--surface-selected)'
      : interactiveFor(repo.path).hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: 'default',
  transition: 'var(--transition-control)'
})

const MARK = 12

/* The name is an identifier and stays mono; both halves shrink before the row
   does, since a flex item refuses by default to go below its own content and
   the branch would be pushed off the end. */
const nameStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const branchBox = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  flex: '0 1 auto',
  minWidth: 0,
  color: 'var(--text-muted)'
}
const branchStyle = { overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }

/* A branch, a short hash said to be detached, or an em dash — the three
   answers `git.rs` can give, drawn the way the scope bar draws them so a person
   reading both sees one fact stated once. */
const branchLabel = (repo) => {
  if (repo.branch) return repo.branch
  if (repo.detached) return `${repo.detached} (detached)`
  return '—'
}

const empty = computed(() => props.repos.length === 0)
</script>

<template>
  <div>
    <div
      v-for="repo in repos"
      :key="repo.path"
      :style="rowStyle(repo)"
      v-bind="interactiveFor(repo.path).handlers"
      @click="$emit('select', repo.path)"
    >
      <Icon
        name="folder-git-2"
        :size="MARK"
        :style="{ flex: 'none', color: repo.path === selected ? 'var(--text-primary)' : 'var(--text-muted)' }"
      />
      <span :style="nameStyle">{{ repo.name }}</span>
      <span :style="{ flex: 1 }" />
      <span :style="branchBox">
        <Icon name="git-branch" :size="MARK" :style="{ flex: 'none' }" />
        <span :style="branchStyle">{{ branchLabel(repo) }}</span>
      </span>
    </div>
    <!-- Its own sentence, and not the one the changes section gives: nothing
         here being a repository and a repository with nothing changed in it are
         opposite facts, and one blank area for both would be the panel saying
         nothing twice. -->
    <div
      v-if="empty"
      :style="{
        padding: 'var(--space-5)',
        color: 'var(--text-muted)',
        font: 'var(--weight-regular) var(--text-xs)/1.5 var(--font-sans)'
      }"
    >
      No repositories here. This folder holds none that git can see.
    </div>
  </div>
</template>
