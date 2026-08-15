<script setup>
/* The uncommitted files of one repository.

   What a row says is `changeStatus.js`'s, not this file's: a `.vue` file is
   the one thing no test here can reach, so the letter, the word and the token
   live outside it.

   The rows are read-only and deliberately do not answer to a click. Opening one
   as a diff is the next task of this epic, and a row that highlighted under the
   pointer and then did nothing would promise an interaction the panel does not
   have yet.

   There is no commit box, no staging and no discard, and that is the design
   rather than an omission: work reaches a branch through `smetana:merging` in
   this project, and a person who wants to commit by hand has a terminal one tab
   away. A second road into the same act would drift from the first. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import { basename } from '../../paths.js'
import { changeStatus } from './changeStatus.js'

const props = defineProps({
  changes: { type: Array, default: () => [] }
})

const rowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
}

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

/* The file's own name reads first and its directory follows it muted — the
   shape a person scans a list of changes in. Both in mono: a path is an
   identifier. The name does not shrink and the directory does, so a deep path
   loses its middle rather than the thing being named. */
const nameStyle = { flex: 'none', overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }
const pathStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  color: 'var(--text-muted)'
}

/* `--untracked-files=normal` is git's own default and this panel's — `all`
   would walk into every untracked directory — so an untracked folder arrives as
   one record with a trailing slash. It is kept: "git/" says a folder where
   "git" would look like a file nobody can find. */
const label = (path) => (path.endsWith('/') ? `${basename(path)}/` : basename(path))

/* Everything above the thing being named. Empty at the root of the repository,
   where there is nothing to say. */
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
    <div v-for="change in changes" :key="change.path" :style="rowStyle">
      <span :style="stagedBox">
        <!-- Staged and unstaged are two different things to somebody looking at
             what an agent has been doing, and the model keeps them apart, so
             the panel does too — with a glyph rather than a shade. -->
        <Icon v-if="change.staged" name="check" :size="MARK" />
      </span>
      <span :style="letterStyle(change)">{{ changeStatus(change.kind).letter }}</span>
      <span :style="nameStyle">{{ label(change.path) }}</span>
      <span :style="pathStyle">{{ [directory(change.path), from(change)].filter(Boolean).join(' ') }}</span>
    </div>
    <!-- Its own sentence: a repository with nothing changed in it is a fact
         worth stating, and it is not the same fact as a folder that holds no
         repository. -->
    <div
      v-if="empty"
      :style="{
        padding: 'var(--space-5)',
        color: 'var(--text-muted)',
        font: 'var(--weight-regular) var(--text-xs)/1.5 var(--font-sans)'
      }"
    >
      No uncommitted files in this repository.
    </div>
  </div>
</template>
