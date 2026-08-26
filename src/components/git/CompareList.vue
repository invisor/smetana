<script setup>
/* The files one branch differs from the current one by, and which of the two
   readings of that question is being asked.

   The left half of the compare window, and a sibling of `ChangeList.vue` rather
   than a second version of it: what a row says is `changeStatus.js`'s in both,
   so a modified file looks the same in the panel it changed in and in the
   window it is compared in. What differs is what a row is *about*. This one is
   two commits, so there is no staged tick and no untracked folder — neither
   means anything between two revisions, and `CompareChange` in Rust leaves both
   fields out for exactly that reason.

   ## The switch belongs to the list

   "What has this branch changed" has two honest answers and they disagree every
   time the current branch has moved since the two diverged — the design
   document draws the graph. Both are offered, because neither is wrong and each
   is the wrong one half the time. It sits above the rows rather than in the
   window's own chrome: it changes what the list holds, and a control that
   changes a list belongs with it.

   A press is a read and nothing else, so there is no refusal here to draw and
   no run to ask about — the whole of what this window can do is read, which is
   also why `branchMenu.js` offers the item while a run holds every other verb.

   Its empty state is its own sentence, and deliberately not the one a refusal
   draws: two branches with nothing between them is an ordinary answer, and the
   window says something else entirely when the comparison could not be made at
   all. That is `GitPanel`'s rule for its three sections, kept here. */
import { computed, watch } from 'vue'
import Button from '../core/Button.vue'
import { useInteractive } from '../core/interactive.js'
import { basename } from '../../paths.js'
import { fileIconUrl } from '../../catppuccinIcon.js'
import { documentTheme } from '../../documentTheme.js'
import { changeStatus } from './changeStatus.js'

const props = defineProps({
  /* `[{ path, origPath, kind }]`, exactly as `vcs_compare` answers — the shape
     `CompareChange` serialises to. */
  files: { type: Array, default: () => [] },
  /* Which row's diff is drawn beside the list, so the file being read is marked
     in the list it was picked from. */
  selected: { type: String, default: null },
  /* 'diverged' | 'direct'. The default is the one the window opens in and the
     one a person almost always means: what this branch added since it split. */
  mode: { type: String, default: 'diverged' },
  /* Whether the list is holding an answer. It has one reader — the empty
     sentence below, which must not claim two branches are identical while the
     comparison is still being read, must not claim it at all beside a window
     saying the comparison could not be made, and must not claim it on a window
     that was never aimed at a pair, where the empty list is the absence of a
     question rather than an answer to one. That third duty is the caller's to
     know — a window with no pair says so itself, in its own words, beside this
     list. `GitPanel`'s `settled` is the same guard against the same defect,
     under the same name. */
  settled: { type: Boolean, default: true }
})

const emit = defineEmits(['select', 'update:mode'])

/* Sentence case, like every other label in this app. The words are the design
   document's own: the first names where the diff is measured from, the second
   says the two trees are held against each other as they stand. */
const MODES = [
  { value: 'diverged', label: 'From where they diverged' },
  { value: 'direct', label: 'Direct' }
]

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  minHeight: 0
}

/* The switch sits on the panel surface with a hairline under it, the same rule
   `SectionHeader` draws above a caption and for the same reason: without it the
   two controls read as the first row of the list rather than as the thing the
   list is drawn under. */
const switchStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  flex: '0 0 auto',
  padding: 'var(--space-3) var(--space-4)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}

/* Each position takes half the width whatever its label measures, so the pair
   is a switch rather than two buttons that happen to be beside each other.

   `minWidth: 0` is the load-bearing half and it was paid for: a flex item
   defaults to `min-width: auto` and refuses to shrink below its own content, so
   in a column too narrow for "From where they diverged" the pair escaped the
   panel sideways rather than staying in it. What is left in that case is a
   label cut short inside its own button, which is a control that is too small
   rather than a list drawn over. The window gives it a column wide enough that
   neither happens. */
const positionStyle = { flex: '1 1 0', minWidth: 0, overflow: 'hidden' }

const listStyle = { flex: 1, minHeight: 0, overflowY: 'auto' }

/* Hover per row, cached by path and pruned as the list changes — the rule
   `ChangeList` and `RepoList` both record: `useInteractive` tracks one control,
   so an instance built inside `rowStyle` would be thrown away on every
   re-render, and an uncached map would keep an entry per file ever compared. */
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
  () => props.files.map((file) => file.path),
  (paths) => {
    const live = new Set(paths)
    for (const path of rowInteractive.keys()) {
      if (!live.has(path)) rowInteractive.delete(path)
    }
  }
)

const rowStyle = (file) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  background:
    file.path === props.selected
      ? 'var(--surface-selected)'
      : interactiveFor(file.path).hover.value
        ? 'var(--surface-hover)'
        : 'transparent',
  cursor: 'default',
  transition: 'var(--transition-control)'
})

const MARK = 12

/* The letter carries the kind and is never the colour alone — `status.js`'s
   rule for a badge, kept by every mark in this app. */
const letterStyle = (file) => ({
  flex: 'none',
  width: `${MARK}px`,
  textAlign: 'center',
  color: `var(${changeStatus(file.kind).token})`
})

/* What the name is drawn as, from the same source the tree and the change list
   draw it from: one file has to look the same wherever this app names it. The
   cost of these colours is measured and accepted where `catppuccinIcon.js`
   records it. A path here is always a file — a comparison of two commits has no
   untracked directory to report. */
const icon = (file) => fileIconUrl(file.path, documentTheme.value)

/* The file's own name reads first and its directory follows it muted, which is
   the shape a person scans a list of changes in. Both mono, because a path is
   an identifier — the same reading `ChangeList` gives one row over. */
const nameStyle = {
  flex: 'none',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const pathStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  color: 'var(--text-muted)'
}

/* Everything above the thing being named. Empty at the root of the repository,
   where there is nothing to say. */
const directory = (path) => {
  const cut = path.lastIndexOf('/')
  return cut > 0 ? path.slice(0, cut) : ''
}

/* A rename's other half, on the one row rather than on a second one: a row
   saying only where a file arrived is the one thing that cannot be checked
   against git. `git diff --name-status` reports a copy the same way, and it
   reads the same here. */
const from = (file) => (file.origPath ? `← ${file.origPath}` : '')

/* Nothing between two branches is an ordinary answer, so it is only worth
   saying once the answer is in: an empty list under a comparison still being
   read is not an identical pair, it is a question nobody has answered yet. */
const empty = computed(() => props.settled && props.files.length === 0)
</script>

<template>
  <div :style="rootStyle">
    <div :style="switchStyle" role="group" aria-label="What to compare">
      <Button
        v-for="position in MODES"
        :key="position.value"
        size="sm"
        :selected="mode === position.value"
        :style="positionStyle"
        @click="emit('update:mode', position.value)"
      >{{ position.label }}</Button>
    </div>
    <div :style="listStyle">
      <div
        v-for="file in files"
        :key="file.path"
        :style="rowStyle(file)"
        v-bind="interactiveFor(file.path).handlers"
        @click="emit('select', file.path)"
      >
        <!-- The word is what the letter stands for, and it is the accessible
             name of a mark that is otherwise one character: `M` reads as
             nothing at all to a screen reader. -->
        <span
          role="img"
          :aria-label="changeStatus(file.kind).label"
          :style="letterStyle(file)"
        >{{ changeStatus(file.kind).letter }}</span>
        <img
          :src="icon(file)"
          alt=""
          :width="MARK + 2"
          :height="MARK + 2"
          :style="{ display: 'block', flex: 'none' }"
        />
        <span :style="nameStyle">{{ basename(file.path) }}</span>
        <span :style="pathStyle">{{ [directory(file.path), from(file)].filter(Boolean).join(' ') }}</span>
      </div>
      <!-- Its own sentence, and not the one a refusal draws: two branches with
           nothing between them is an outcome worth stating plainly, while a
           comparison that could not be made at all is the window's to explain
           in its own words. -->
      <div
        v-if="empty"
        :style="{
          padding: 'var(--space-5)',
          color: 'var(--text-muted)',
          font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)'
        }"
      >
        These two branches are identical.
      </div>
    </div>
  </div>
</template>
