<script setup>
/* The Git tab of the left sidebar: the repositories a project is made of, and
   the uncommitted files of the one being shown.

   Presentational, like every other component here — it is handed the state and
   emits what was picked, so it renders in `?view=gallery` with no back end
   behind it. The state itself is `src/stores/vcs.js` and the wiring is
   `views/DesktopApp.vue`.

   Four things can be empty and they are four different sentences: git is not on
   this machine, this folder holds no repository, this repository has nothing
   uncommitted, this repository has no local branch yet. One blank area for all
   of them would be a panel saying nothing four different ways, and the first is
   the one a person can act on.

   No diff, no merge and no rebase: the other tasks of this epic. */
import { computed } from 'vue'
import BranchList from './BranchList.vue'
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
  /* `[{ name, current }]` in `git::by_recency`'s order, which is drawn as it
     arrives. */
  branches: { type: Array, default: () => [] },
  /* `{ allowed, reason }` from `gitActions.js`: whether a checkout may be
     offered at all, and the sentence over it when it may not. Passed through
     rather than decided here — this panel draws, and the rule is a pure file a
     test can reach. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  /* The branch a checkout is in flight for, and git's refusal of the last
     one. */
  checkingOut: { type: String, default: null },
  checkoutError: { type: Object, default: null },
  /* `{ kind, message }` as `stores/vcs.js` normalises it. `noGit` is the one
     kind this panel branches on; everything else is git's own words, shown
     untouched, because whoever reads them knows git. */
  error: { type: Object, default: null },
  loading: { type: Boolean, default: false }
})
defineEmits(['select', 'checkout'])

const rootStyle = { display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }

/* A caption over a list, in prose and therefore sans, with the count beside it
   in mono because it is a measurement.

   `flexShrink: 0` because a caption is a flex item in this column and a flex
   item shrinks by default: with three sections crowding a short panel the
   captions gave way with the lists, `--row-h` became a starting point and the
   text was clipped — the very defect a short list hid in `Dropdown`'s options.
   A caption is the one thing here that must not move. */
const headerStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  flexShrink: 0,
  padding: '0 var(--space-5)',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)'
}

/* How many branch rows the section may claim before it scrolls inside itself.
   A count and not a height, so it follows `--row-h` through both densities and
   the app-wide font size.

   The cap is what makes "the branches must not push the changes off the top"
   true. Without it the section's basis is its content, so a repository with
   forty branches claims forty rows of the column and the list somebody opened
   this panel for is squeezed to nothing — measured at 0px in a short panel
   before this. Six rows is enough to reach for a branch and short enough that
   the changes stay the content. */
const BRANCH_ROWS = 6
const countStyle = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)' }

/* git's own stderr. Mono and left-aligned rather than an `EmptyState`'s centred
   prose: this is machine output, and it is shown exactly as git wrote it.

   Used twice: inside the changes scroller for a read that failed, and as a flex
   item of this column for a checkout git refused. `flexShrink: 0` is for the
   second — a flex item shrinks by default, and the lists above it have
   somewhere to give way to, while a refusal clipped to a strip of its own title
   is the defect this block was moved out of the branch cap to fix. It changes
   nothing at the first site, where the parent is not a flex container. */
const failureStyle = {
  padding: 'var(--space-5)',
  display: 'flex',
  flexDirection: 'column',
  flexShrink: 0,
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
      <!-- `1 1 auto` rather than `flex: 1`: with a basis of zero this section
           only grew into what was left over, while its neighbours claimed their
           content first, so adding the branch list below squeezed the changes
           to nothing. On a basis of its own content it takes the largest share
           of a crowded column and gives ground in proportion, which is what
           says the changes are what this panel is for. -->
      <div :style="{ flex: '1 1 auto', minHeight: 0, overflow: 'auto' }">
        <div v-if="failure" :style="failureStyle">
          <div :style="failureTitleStyle">{{ failureTitle }}</div>
          <div :style="failureTextStyle">{{ failure }}</div>
        </div>
        <ChangeList v-else-if="repos.length && tree" :changes="changes" />
      </div>

      <!-- Third, under the changes, and gated on there being a repository for
           the same reason the changes caption is: a "Branches" heading over
           nothing says less than the repository list's own empty sentence
           already did. The section shrinks and never grows, and it is capped at
           `BRANCH_ROWS` on top of that: the changes above it are what somebody
           opened this panel for, and a repository with forty branches must not
           push them off the top. -->
      <template v-if="repos.length && !failure">
        <div :style="headerStyle">
          <span>Branches</span>
          <span :style="{ flex: 1 }" />
          <span v-if="branches.length > 1" :style="countStyle">{{ branches.length }}</span>
        </div>
        <div
          :style="{
            flex: '0 1 auto',
            minHeight: 0,
            maxHeight: `calc(var(--row-h) * ${BRANCH_ROWS})`,
            overflow: 'auto'
          }"
        >
          <BranchList
            :branches="branches"
            :actions="actions"
            :checking-out="checkingOut"
            @checkout="$emit('checkout', $event)"
          />
        </div>
        <!-- **Outside the scroller above, and that is the whole point.** Drawn
             under the rows it belonged to, it sat below the fold of a box
             capped at `BRANCH_ROWS` — with six branches or more the refusal was
             entirely out of view, so a person pressed a row, the tick did not
             move, and nothing said why. It is the same block the read failure
             above uses, and one copy of it: `failureTitleStyle` is what says
             which of the two this is. -->
        <div v-if="checkoutError" :style="failureStyle">
          <div :style="failureTitleStyle">Git did not switch branch</div>
          <div :style="failureTextStyle">{{ checkoutError.message }}</div>
        </div>
      </template>
    </template>
  </div>
</template>
