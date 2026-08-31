<script setup>
/* Choosing what an agent reviews: which repositories, what each is measured
   against, and which side of each pair means the local branch and which means
   what `origin` has.

   **A row is a pair**, and that is the whole shape of this window. What was
   asked for is that the number of reference branches always equals the number
   of branches under review; making the row the pair means there is no
   arrangement of this table where the two numbers differ, because a base cannot
   be added without the branch beside it. A rule checked on the way out could be
   forgotten; the shape cannot be.

   Presentational like every other dialog in this panel. It is handed the table
   and the lists to fill it from and emits what was asked for; the rule that
   builds a table — which repositories have a branch of a given name, and which
   are short of it — is `reviewRows.js`, called by `DesktopApp.vue`. That split
   is not tidiness: a `.vue` file is the one thing no test in this repository can
   reach, so the whole of the rule lives outside the component that draws it.

   **The table is held here and seeded from the prop rather than driven by it.**
   Every prop of a dialog window arrives over IPC and is re-announced whenever
   anything else about the window changes, so a fully driven table would put an
   IPC round trip between a person opening a dropdown and the answer appearing
   in it — and would wipe a half-filled row every time an unrelated prop moved.
   So the announcement is adopted only when its *contents* differ from what is
   on screen, which is exactly the moment `DesktopApp.vue` has rebuilt the table
   and never the moment it re-announced the same one.

   The one thing that goes back up before Review is the branch somebody picks on
   the checked side of a lone row: that is what turns the `New review` door into
   the same table the branch row's menu opens, and building it is the rule's job
   rather than this component's. It carries **the reference branch beside the
   name**, and that is not a convenience — the columns run Repository, Base, To
   check, so filling the row left to right means the base is chosen first, and a
   message carrying only the name would have the rebuilt table put the default
   back over it at the instant attention had moved on to the other column. */
import { computed, ref, watch } from 'vue'
import Button from '../core/Button.vue'
import Dropdown from '../core/Dropdown.vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Modal from '../overlays/Modal.vue'
import {
  LOCAL,
  ORIGIN,
  canReview,
  fetchFailedCaption,
  fetchingCaption,
  localNames,
  withoutCaption
} from './reviewRows.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  /* What this dialog is called, in one place rather than two: the OS frame's
     caption comes from the same announcement that fills these props, so a title
     written into the template below would be silently overridden by it — see
     `NewBranchModal.vue`, which carries the whole argument. */
  title: { type: String, default: 'Review changes' },
  /* The table, `reviewRows.js`' shape: `{ repo, name, base, baseSide, head,
     headSide }` apiece. */
  rows: { type: Array, default: () => [] },
  /* The branch the table was built for, and '' for the door that started with
     no name. Only the caption under the table reads it. */
  branch: { type: String, default: '' },
  /* Every repository of this project, `{ name, path }` apiece — `vcsState.repos`.
     What "Add a repository" offers is this list minus whatever is already in the
     table. */
  repos: { type: Array, default: () => [] },
  /* `target_branches`' answer, `{ name, missing_in }` apiece: every local branch
     of every repository, said once for the whole project. */
  branches: { type: Array, default: () => [] },
  /* What `origin` is known to have, keyed by repository path — plain names with
     the `origin/` already off, as `vcs_remote_branches` answers. Absent for a
     repository whose list has not landed, which draws an empty list rather than
     a wrong one. */
  remote: { type: Object, default: () => ({}) },
  /* The repositories with no branch of that name, named in one line under the
     table rather than in a row each. */
  without: { type: Array, default: () => [] },
  /* What a row added by hand starts its reference side at. */
  defaultBase: { type: String, default: '' },
  /* The repositories `origin` is being fetched in right now, and the ones that
     could not be reached. Both are sentences under the table: a fetch nobody is
     told about is a wait with no explanation, and a fetch that failed changes
     what the review is about without cancelling it. */
  fetching: { type: Array, default: () => [] },
  fetchFailed: { type: Array, default: () => [] },
  busy: { type: Boolean, default: false }
})

const emit = defineEmits(['close', 'branch', 'submit'])

/* The two words a side can be, drawn in mono. `origin` is a remote's name and
   therefore an identifier; `local` is not, and is kept in the same font all the
   same — the two sit in one control beside a branch name, and a pair of
   dropdowns in two different faces reads as two libraries. */
const SIDES = [
  { value: LOCAL, label: 'local' },
  { value: ORIGIN, label: 'origin' }
]

const table = ref([])

/* Adopted by contents and never by identity. Every announcement rebuilds these
   objects on the way through IPC, so an identity watch would fire on every
   unrelated change — a fetch starting, a title moving — and throw away whatever
   somebody had half chosen. */
const shape = (list) => JSON.stringify(list ?? [])
watch(
  () => shape(props.rows),
  (next) => {
    if (next !== shape(table.value)) table.value = JSON.parse(next)
  },
  { immediate: true }
)

const inTable = computed(() => table.value.map((row) => row.repo))
const spare = computed(() =>
  (props.repos ?? []).filter((repo) => repo?.path && !inTable.value.includes(repo.path))
)
const spareOptions = computed(() =>
  spare.value.map((repo) => ({ value: repo.path, label: repo.name }))
)

/* What one side of one row may be set to. Local is the project-wide answer
   filtered to this repository; origin is that repository's own read. A name in
   one and not the other is ordinary — a branch that lives only on the server,
   and a branch nobody has pushed — which is why the two lists are never
   merged. */
const namesFor = (row, side) =>
  side === ORIGIN ? props.remote?.[row.repo] ?? [] : localNames(props.branches, row.name)

const edit = (index, field, value) => {
  const row = table.value[index]
  if (!row) return
  /* The one edit that is not this component's to make: the first name put on
     the checked side of a lone **empty** row, which is the whole of the
     `New review` door. Picking there is what builds the rest of the table, and
     the rule that builds one lives outside this file; what comes back is a
     whole new table through the prop.

     Three clauses and every one of them earns its place. `length === 1` because
     a table that already names several repositories is somebody's, not the
     rule's, to rearrange. `headSide === LOCAL` because the rule looks a name up
     in `target_branches`, which knows only local branches — a branch that lives
     only on the server would come back missing from everywhere and take the
     row away with it. And `!row.head` because the ordinary single-repository
     project *always* has one row: without it, changing the branch under review
     would rebuild the table and throw away whatever reference branch had been
     chosen beside it. */
  if (field === 'head' && table.value.length === 1 && row.headSide === LOCAL && !row.head) {
    /* The base goes up with the name. The edit itself is deliberately not
       applied here — what comes back is a whole new table, and applying it
       first would leave a row on screen for a tick that the rule is about to
       replace — but the base is a choice somebody has already made, and the
       rule has a slot for exactly that: it is `pickBranch`'s `remembered`, the
       first of the three terms, and one that has since left the list is skipped
       there like any other. */
    emit('branch', { name: value, base: row.base })
    return
  }
  table.value[index] = { ...row, [field]: value }
}

const add = (path) => {
  const repo = spare.value.find((r) => r.path === path)
  if (!repo) return
  /* The checked side starts empty deliberately. This row is here because the
     branch the table was built for is not in this repository, so the one thing
     that cannot be filled in for somebody is the name it goes by here. */
  table.value = [
    ...table.value,
    {
      repo: repo.path,
      name: repo.name,
      base: props.defaultBase,
      baseSide: LOCAL,
      head: '',
      headSide: LOCAL
    }
  ]
}

const drop = (index) => {
  table.value = table.value.filter((_, i) => i !== index)
}

/* The repositories still worth naming: one somebody has since added by hand is
   in the table, and a line saying it has no such branch would be a sentence
   about a row directly above it. */
const missing = computed(() =>
  withoutCaption(
    (props.without ?? []).filter((name) => !table.value.some((row) => row.name === name)),
    props.branch
  )
)
const fetchingNote = computed(() => fetchingCaption(props.fetching))
const failedNote = computed(() => fetchFailedCaption(props.fetchFailed))

const ready = computed(() => canReview(table.value) && !props.busy)

const submit = () => {
  if (!ready.value) return
  emit('submit', { rows: table.value.map((row) => ({ ...row })) })
}

const body = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)'
}

/* One grid for the header and every row, so the four columns line up down the
   whole table. The repository takes the least of it: it is one short name, and
   the branch fields beside it hold names that run long. */
const grid = {
  display: 'grid',
  gridTemplateColumns: 'minmax(0, 3fr) minmax(0, 7fr) minmax(0, 7fr) auto',
  alignItems: 'center',
  gap: 'var(--space-4)'
}

const headStyle = {
  fontSize: 'var(--text-xs)',
  color: 'var(--text-muted)'
}

/* A repository's name is an identifier wherever it is drawn. */
const repoStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

/* The branch and the side share a cell: the branch takes what is left and the
   side keeps a fixed share of it, so the four side controls line up down the
   table however long the names beside them are. */
const pairStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  minWidth: 0
}
const nameCell = { flex: '1 1 0', minWidth: 0 }
const sideCell = { flex: '0 0 38%', minWidth: 0 }

const noteStyle = {
  fontSize: 'var(--text-xs)',
  color: 'var(--text-muted)'
}

/* The two live sentences under the table. Neither is a failure — one is a wait
   and the other is a review that is going ahead over a slightly older origin —
   so both are the panel's quiet idiom rather than the failed red. */
const workingStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  fontSize: 'var(--text-xs)',
  color: 'var(--attn-live)'
}
/* The one spinner this panel has, in the one idiom it has: `loader-circle` at
   `--attn-live` turning at `--dur-pulse`, which is what a branch row draws over
   a write and what the Git panel's own fetch button draws. */
const spinStyle = { animation: 'sm-spin var(--dur-pulse) linear infinite' }
</script>

<template>
  <!-- The width is read only outside a dialog window — inside one `Modal` takes
       the whole frame, which is already the registry's number. It is here so
       that `?view=gallery` draws this dialog at the width it has in the app, and
       it has to agree with `review-changes` in `views/dialogRegistry.js`. -->
  <Modal :open="open" :title="title" :width="720" :closable="!busy" @close="$emit('close')">
    <div :style="body">
      <div :style="grid">
        <div :style="headStyle">Repository</div>
        <div :style="headStyle">Base</div>
        <div :style="headStyle">To check</div>
        <!-- The remove column has no caption: a header over one icon button per
             row would name the control rather than the column. -->
        <div />
        <template v-for="(row, index) in table" :key="row.repo">
          <div :style="repoStyle" :title="row.repo">{{ row.name }}</div>
          <div :style="pairStyle">
            <div :style="nameCell">
              <Dropdown
                :model-value="row.base"
                :options="namesFor(row, row.baseSide)"
                mono
                size="sm"
                searchable
                search-label="Filter branches"
                placeholder="Pick a branch"
                :disabled="busy"
                @update:model-value="edit(index, 'base', $event)"
              />
            </div>
            <div :style="sideCell">
              <Dropdown
                :model-value="row.baseSide"
                :options="SIDES"
                mono
                size="sm"
                :disabled="busy"
                @update:model-value="edit(index, 'baseSide', $event)"
              />
            </div>
          </div>
          <div :style="pairStyle">
            <div :style="nameCell">
              <Dropdown
                :model-value="row.head"
                :options="namesFor(row, row.headSide)"
                mono
                size="sm"
                searchable
                search-label="Filter branches"
                placeholder="Pick a branch"
                :disabled="busy"
                @update:model-value="edit(index, 'head', $event)"
              />
            </div>
            <div :style="sideCell">
              <Dropdown
                :model-value="row.headSide"
                :options="SIDES"
                mono
                size="sm"
                :disabled="busy"
                @update:model-value="edit(index, 'headSide', $event)"
              />
            </div>
          </div>
          <IconButton
            icon="x"
            size="sm"
            :label="`Take ${row.name} out of the review`"
            :disabled="busy"
            @click="drop(index)"
          />
        </template>
      </div>
      <!-- Only what is not already in the table: offering a repository that has
           a row would be a second row for one repository, which is a pair this
           window has no way to mean. -->
      <div v-if="spareOptions.length" :style="{ maxWidth: '50%' }">
        <Dropdown
          :model-value="''"
          :options="spareOptions"
          size="sm"
          placeholder="Add a repository"
          :disabled="busy"
          @update:model-value="add($event)"
        />
      </div>
      <div v-if="missing" :style="noteStyle">{{ missing }}</div>
      <div v-if="failedNote" :style="noteStyle">{{ failedNote }}</div>
      <div v-if="fetchingNote" :style="workingStyle">
        <Icon name="loader-circle" :size="13" :style="spinStyle" />
        <span>{{ fetchingNote }}</span>
      </div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!ready" @click="submit">Review</Button>
    </template>
  </Modal>
</template>
