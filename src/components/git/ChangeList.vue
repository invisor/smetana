<script setup>
/* The uncommitted files of one repository.

   What a row says is `changeStatus.js`'s, not this file's: a `.vue` file is
   the one thing no test here can reach, so the letter, the word and the token
   live outside it.

   A click opens the file as a diff in the centre column — one gesture and no
   second one: there is no preview here the way the file tree has one, since a
   diff is already a thing somebody asked to look at rather than a file they may
   be scanning past. An untracked *folder* is the one row that does not answer,
   and it cannot: `--untracked-files=normal` reports it as a single record with
   a trailing slash, and there is no file behind that name to diff.

   The commit box is `CommitBox.vue` and it is **not** part of this list: what
   it takes is the whole tree rather than any row here, so a row that could be
   included or left out would be promising a choice nothing behind it can make.
   That is also why there is still **no staging and no discard**. Staging is the
   one this list looks closest to having — a change already carries `staged`,
   and the tick draws it — but reading a flag git set is not the same as
   offering to set it, and a commit that takes everything needs no such gesture.
   Discard is the other kind of missing: it destroys work with nothing to undo
   it, and it is out of this epic rather than merely unbuilt. */
import { computed, watch } from 'vue'
import Icon from '../core/Icon.vue'
import { useInteractive } from '../core/interactive.js'
import { basename } from '../../paths.js'
import { fileIconUrl, folderIconUrl } from '../../catppuccinIcon.js'
import { documentTheme } from '../../documentTheme.js'
import { changeStatus } from './changeStatus.js'

const props = defineProps({
  changes: { type: Array, default: () => [] },
  /* The path of the change whose diff is open in the centre, so the row a
     person is reading is marked in the list they picked it from. */
  selected: { type: String, default: null }
})

const emit = defineEmits(['open'])

/* A folder has no file behind it, so its row stays inert rather than opening a
   diff of a name. */
const openable = (change) => !change.path.endsWith('/')

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

const rowStyle = (change) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  background:
    change.path === props.selected
      ? 'var(--surface-selected)'
      : openable(change) && interactiveFor(change.path).hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: 'default',
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
  change.path.endsWith('/')
    ? folderIconUrl(change.path, false, documentTheme.value)
    : fileIconUrl(change.path, documentTheme.value)

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
    <div
      v-for="change in changes"
      :key="change.path"
      :style="rowStyle(change)"
      v-bind="interactiveFor(change.path).handlers"
      @click="openable(change) && emit('open', change)"
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
        font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
      }"
    >
      No uncommitted files in this repository.
    </div>
  </div>
</template>
