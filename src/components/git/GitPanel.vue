<script setup>
/* The Git tab of the left sidebar: the repositories a project is made of, and
   the uncommitted files of the one being shown.

   Presentational, like every other component here — it is handed the state and
   emits what was picked, so it renders in `?view=gallery` with no back end
   behind it. The state itself is `src/stores/vcs.js` and the wiring is
   `views/DesktopApp.vue`.

   Three things can be empty and they are three different sentences: git is not
   on this machine, this folder holds no repository, this repository has nothing
   uncommitted. One blank area for all three would be a panel saying nothing in
   three different ways, and the first of them is the one a person can act on.

   No diff, no branch list and no writes: the other three tasks of this epic. */
import { computed } from 'vue'
import ChangeList from './ChangeList.vue'
import EmptyState from '../core/EmptyState.vue'
import RepoList from './RepoList.vue'

const props = defineProps({
  repos: { type: Array, default: () => [] },
  /* The selected repository's absolute path. */
  selected: { type: String, default: null },
  /* `{ branch, detached, changes }`, or null when it could not be read — never
     an empty tree standing in for a failure. */
  tree: { type: Object, default: null },
  /* `{ kind, message }` as `stores/vcs.js` normalises it. `noGit` is the one
     kind this panel branches on; everything else is git's own words, shown
     untouched, because whoever reads them knows git. */
  error: { type: Object, default: null },
  loading: { type: Boolean, default: false }
})
defineEmits(['select'])

const rootStyle = { display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }

/* A caption over a list, in prose and therefore sans, with the count beside it
   in mono because it is a measurement. */
const headerStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  padding: '0 var(--space-5)',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)'
}
const countStyle = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)' }

/* git's own stderr. Mono and left-aligned rather than an `EmptyState`'s centred
   prose: this is machine output, and it is shown exactly as git wrote it. */
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

/* There being no git at all is the panel's own state rather than one section's:
   nothing below it could be read either, and an empty repository list under it
   would say the opposite thing quietly. */
const noGit = computed(() => props.error?.kind === 'noGit')
/* Anything else git said. It is drawn whether or not there are repositories,
   and the repository list's own empty sentence gives way to it: a failure that
   left the list empty would otherwise be reported as "no repositories here",
   which states the opposite of what happened. */
const failure = computed(() => (props.error && !noGit.value ? props.error.message : ''))

/* Which read failed decides the noun. With a repository selected the message is
   about that repository's working tree; with none, nothing got as far as one
   and calling it "this repository" would name something that is not on
   screen. */
const failureTitle = computed(() =>
  props.repos.length ? 'Git could not read this repository' : 'Git could not read this folder'
)

/* A first read in flight has nothing to say yet, and the empty states are
   statements: "this folder holds no repository" must not flash over a list
   that is on its way. */
const settled = computed(() => !props.loading || props.repos.length > 0)
const changes = computed(() => props.tree?.changes ?? [])
</script>

<template>
  <div :style="rootStyle">
    <!-- Named rather than hinted at: the message carries what was looked for,
         which is the difference between a person installing git and a person
         wondering why a panel is blank. -->
    <EmptyState
      v-if="noGit"
      compact
      tone="error"
      title="Git was not found"
      :description="error.message"
    />
    <template v-else>
      <div :style="headerStyle">
        <span>Repositories</span>
        <span :style="{ flex: 1 }" />
        <span v-if="repos.length > 1" :style="countStyle">{{ repos.length }}</span>
      </div>
      <!-- The list scrolls rather than pushing the changes off the bottom: a
           folder of a dozen sibling repositories is exactly what the discovery
           arm in `vcs/repos.rs` exists for, and `0 1 auto` is what lets this
           give way while the changes below keep their share.

           With nothing in the list and a failure to report, the list is left
           out altogether: `RepoList`'s "No repositories here" is a statement
           about a folder that was read, and a read that failed has not earned
           it. The failure below says what actually happened. -->
      <div v-if="repos.length || !failure" :style="{ flex: '0 1 auto', minHeight: 0, overflow: 'auto' }">
        <RepoList v-if="settled" :repos="repos" :selected="selected" @select="$emit('select', $event)" />
      </div>

      <!-- The caption goes when there is no repository to have changed
           anything: a "Changes" heading over nothing would be a second empty
           state saying less than the one above it already said. The failure
           underneath is not gated on it, so a `vcs_repos` that refuses has
           somewhere to be drawn. -->
      <div v-if="repos.length" :style="headerStyle">
        <span>Changes</span>
        <span :style="{ flex: 1 }" />
        <span v-if="tree && changes.length" :style="countStyle">{{ changes.length }}</span>
      </div>
      <div :style="{ flex: 1, minHeight: 0, overflow: 'auto' }">
        <div v-if="failure" :style="failureStyle">
          <div :style="failureTitleStyle">{{ failureTitle }}</div>
          <div :style="failureTextStyle">{{ failure }}</div>
        </div>
        <ChangeList v-else-if="repos.length && tree" :changes="changes" />
      </div>
    </template>
  </div>
</template>
