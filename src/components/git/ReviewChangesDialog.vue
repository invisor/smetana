<script setup>
/* Choosing what an agent reviews: one pair of refs for the project, the
   repositories it reaches, and whichever of them differ.

   **One rule and its exceptions**, which is the whole shape of this window. It
   was a table where a row *was* a pair — a repository, its own base, its own
   branch to check, and a side switch beside each — and that shape kept the one
   thing that matters, that the number of bases is always the number of branches
   under review, at the price of four controls per row: a project of five
   repositories opened as a wall of twenty dropdowns asking the same question
   five times over. The pair is chosen once now, at the top, and every row below
   either follows it or holds its own copy of it. The count of controls no longer
   grows with the project.

   The invariant is the same and is still a property of the shape rather than a
   check on the way out: `reviewRows.js` has no operation that sets half a pair,
   and an override is made by copying the rule rather than by starting an empty
   one. A rule can be forgotten; a shape cannot.

   **Nothing here hangs outside the window.** This is an OS window of its own,
   not resizable, whose height is measured from its content
   (`views/DialogWindow.vue`), so a popover would be clipped by the frame rather
   than by a scroll container — which is exactly what the old branch dropdown
   was. The branch list is `BranchPicker.vue`, a block in the flow, and while it
   is open **the table is not drawn at all**: that is what makes the window's
   height predictable — about 514px at 720 wide, whatever the number of
   repositories and however long the list of branches.

   Presentational, like every other dialog in this panel. The rule that builds a
   form, edits one and answers what it is worth is `reviewRows.js`, outside every
   `.vue` file for the reason that whole family exists: a component is the one
   thing no test in this repository can reach, so what is left here is boxes,
   tokens and events.

   **The form is held here and seeded from the prop rather than driven by it.**
   Every prop of a dialog window arrives over IPC and is re-announced whenever
   anything else about the window changes, so a driven form would put a round
   trip between picking a branch and seeing it, and would throw away a half-made
   choice every time an unrelated prop moved. The announcement is adopted only
   when its *contents* differ from what is on screen, which is the moment the app
   window rebuilt it and never the moment it re-announced the same one. */
import { computed, nextTick, ref, watch } from 'vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Modal from '../overlays/Modal.vue'
import BranchPicker from './BranchPicker.vue'
import { repoLabel, repoPath } from './repoLabel.js'
import {
  PICK_HEAD,
  WAITING_FOR_BRANCH,
  addableRepos,
  branchesIn,
  canReview,
  footerSummary,
  isManual,
  isOverride,
  missingRepos,
  oldestFetch,
  pairLabel,
  pairOf,
  pickerScope,
  reviewNotes,
  rowStatus,
  ruleCaption,
  sideLabel,
  tableSummary,
  withOverride,
  withPick,
  withRepo,
  withoutOverride,
  withoutRepo
} from './reviewRows.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  /* What this dialog is called, in one place rather than two: the OS frame's
     caption comes from the same announcement that fills these props, so a title
     written into the template below would be silently overridden by it — see
     `NewBranchModal.vue`, which carries the whole argument. The second door
     calls itself `New review`, and it says so by announcing that instead. */
  title: { type: String, default: 'Review changes' },
  /* The form as the app window built it — `reviewRows.js`' shape:
     `{ base, head, repoIds, overrides }`. */
  form: { type: Object, default: null },
  /* Every repository of this project, `{ name, path }` apiece — `vcsState.repos`.
     It is the table's vocabulary and what `Add a repository` offers from. */
  repos: { type: Array, default: () => [] },
  /* The project's own folder, which is what a repository's path is drawn
     relative to, and the person's home folder, which is what the ones outside
     the project are shortened against. `home` is null wherever nobody has been
     asked — `repoLabel.js` then draws an absolute path, which is true rather
     than merely shorter. */
  root: { type: String, default: '' },
  home: { type: String, default: null },
  /* `target_branches`' answer, `{ name, missing_in, at }` apiece: every local
     branch of every repository, said once for the whole project. */
  branches: { type: Array, default: () => [] },
  /* What `origin` is known to have, keyed by repository path — plain names with
     the `origin/` already off, as `vcs_remote_branches` answers. Absent for a
     repository whose list has not landed, which is read as "not known" and
     never as "not there". */
  remote: { type: Object, default: () => ({}) },
  /* When each repository last fetched, in epoch seconds, keyed by path. It
     dates the branch list's `origin` rows and the sentence a row draws when a
     fetch could not reach the remote. */
  fetchedAt: { type: Object, default: () => ({}) },
  /* The repositories `origin` is being fetched in right now, and the ones that
     could not be reached — both by path. They are a line in the notes block and
     a word on the row: a fetch nobody is told about is a wait with no
     explanation, and a fetch that failed changes how old the review's answer is
     without cancelling it. */
  fetching: { type: Array, default: () => [] },
  fetchFailed: { type: Array, default: () => [] },
  busy: { type: Boolean, default: false }
})

const emit = defineEmits(['close', 'submit'])

/* The glyph sizes, which are the one kind of number this file is allowed to
   hold: the design system's own units, exactly as every other component here
   passes one to `Icon`. Everything else is a token, because both densities and
   the app-wide font size have to move it. */
const GLYPH = 13
const ARROW = 16
const PLUS = 14
const STATUS_GLYPH = 12

const EMPTY = {
  base: { ref: '', remote: false },
  head: null,
  repoIds: [],
  overrides: {},
  manual: []
}

const review = ref(EMPTY)
/* Which list is open, and what it is for: `{ side, repoId }`, with `repoId`
   null for the project's rule and a path for one row's own pair. Null is the
   ordinary state, in which the table is what is drawn. */
const picker = ref(null)
const addOpen = ref(false)
const pickerBox = ref(null)

/* Adopted by contents and never by identity. Every announcement rebuilds these
   objects on the way through IPC, so an identity watch would fire on every
   unrelated change — a fetch starting, a title moving — and throw away whatever
   somebody had chosen. */
const shape = (value) => JSON.stringify(value ?? null)
watch(
  () => shape(props.form),
  (next) => {
    if (next === shape(review.value)) return
    review.value = props.form ? JSON.parse(next) : EMPTY
    picker.value = null
    addOpen.value = false
  },
  { immediate: true }
)

/* The clock the ages are measured against, in the seconds `branchPicker.js`
   speaks in, read once. This window is opened, used and closed inside a minute;
   a ticking clock would buy an age that stayed honest while nobody was looking
   at it and cost the branch list its highlight. */
const now = ref(Math.floor(Date.now() / 1000))

/* What "does this repository have that branch" is answered from, in one place:
   the project-wide local list, and what origin is known to have where that has
   landed. */
const context = computed(() => ({
  repos: props.repos ?? [],
  branches: props.branches ?? [],
  remote: props.remote ?? {}
}))

/* A row's repository record, and whether the project still lists one.

   A repository that has left the project while this window stood open keeps its
   row rather than dropping out of the middle of a table somebody is reading, so
   `repoAt` always answers: the stand-in is named `.` so that `repoLabel` draws
   the folder it sits in, which is the honest answer and the same one that name
   gets everywhere else.

   `listedRepo` is the other half, and the difference matters in exactly one
   place. A name is what `missing_in` is keyed by, and the stand-in's `.` is
   somebody else's name — the project root's — so a branch list scoped to a
   departed repository would be filtered by the wrong repository's answer and
   then counted as though it were this one's. There is nothing truthful to draw
   there, and that is what the empty list below says. */
const listedRepo = (path) => (props.repos ?? []).find((repo) => repo?.path === path) ?? null
const repoAt = (path) => listedRepo(path) ?? { name: '.', path }

const where = (path) => repoPath(props.root, path, props.home)

const rows = computed(() =>
  (review.value.repoIds ?? []).map((id) => {
    const repo = repoAt(id)
    const override = isOverride(review.value, id)
    return {
      id,
      name: repoLabel(repo),
      path: where(id),
      override,
      manual: isManual(review.value, id),
      pair: pairLabel(pairOf(review.value, id)),
      status: rowStatus({
        override,
        fetching: (props.fetching ?? []).includes(id),
        stale: (props.fetchFailed ?? []).includes(id),
        at: props.fetchedAt?.[id] ?? null,
        now: now.value
      })
    }
  })
)

const base = computed(() => sideLabel(review.value.base))
const head = computed(() => sideLabel(review.value.head))
const caption = computed(() => ruleCaption(review.value))
const summary = computed(() => tableSummary(review.value))

/* The candidates `Add a repository` opens on: what is not in the table, with
   the reason it is not. */
const candidates = computed(() =>
  addableRepos(props.repos, review.value, context.value).map(({ repo, note }) => ({
    id: repo.path,
    name: repoLabel(repo),
    path: where(repo.path),
    note
  }))
)

/* The block of service messages. All three speak in names, because that is what
   `[project].repos` holds and what the table draws — the paths behind them are
   what travels into the intent. */
const notes = computed(() =>
  reviewNotes({
    fetching: props.fetching ?? [],
    failed: (props.fetchFailed ?? []).map((path) => repoLabel(repoAt(path))),
    missing: missingRepos(props.repos, review.value, context.value).map(repoLabel)
  })
)

const footerText = computed(() =>
  footerSummary(review.value, { busy: props.busy, notes: notes.value.length })
)

const ready = computed(() => canReview(review.value) && !props.busy)

/* ---- the branch list ----------------------------------------------------- */

/* Which branches the open list holds. The project's rule offers the whole
   project's answer; a row's own pair offers that row's repository alone, which
   is `target_branches` filtered rather than a second read per row. */
const pickerBranches = computed(() => {
  const at = picker.value?.repoId
  if (!at) return props.branches ?? []
  /* Scoped by the repository's own name, and nothing at all for one the project
     no longer lists: `branchesIn` empties each record's `missing_in` because the
     list is drawn against one repository *that has the branch*, and that
     premise is exactly what a stand-in name cannot support — every row would
     read `local · 1 repo` about a repository nobody can say anything about. */
  const repo = listedRepo(at)
  return repo ? branchesIn(props.branches, repo.name) : []
})

const pickerRepos = computed(() =>
  picker.value?.repoId ? 1 : (props.repos ?? []).length
)

/* How fresh the `origin` rows in that list are. One repository answers for
   itself; the project's rule answers with the oldest of them, since a pair set
   for every repository is only as current as the least recently fetched one. */
const pickerFetchedAt = computed(() => {
  const at = picker.value?.repoId
  if (at) return props.fetchedAt?.[at] ?? null
  return oldestFetch(
    (props.repos ?? []).map((repo) => repo?.path),
    props.fetchedAt
  )
})

const pickerPair = computed(() => pairOf(review.value, picker.value?.repoId ?? null))
const pickerSide = computed(() => pickerPair.value?.[picker.value?.side ?? 'head'] ?? null)

/* The footer of the open list says what a choice in it reaches. The name is
   `repoLabel`'s and not the configuration's, for the one name that tells a
   reader nothing: `this repository only · .` names no repository at all. */
const scope = computed(() =>
  pickerScope(picker.value?.repoId ? repoLabel(repoAt(picker.value.repoId)) : null)
)

const openPicker = (side, repoId = null) => {
  if (props.busy) return
  addOpen.value = false
  picker.value = { side, repoId }
  /* The keyboard is handed over rather than taken: `BranchPicker` deliberately
     does not focus itself, because a block in a flow that did would steal the
     caret from whatever the window opened with. This window is the side that
     knows the list is the only thing to answer while it is up. */
  nextTick(() => pickerBox.value?.focus())
}

const closePicker = () => {
  picker.value = null
}

const pick = ({ name, origin }) => {
  if (!picker.value) return
  review.value = withPick(
    review.value,
    picker.value,
    { ref: name, remote: Boolean(origin) },
    context.value
  )
  picker.value = null
}

/* ---- the table ----------------------------------------------------------- */

/* A row told to differ, which is one movement rather than two: the rule is
   frozen into it and the list opens on its checked side straight away, because
   a row that differs from the rule in nothing is not a state anybody wanted. */
const differ = (id) => {
  if (props.busy) return
  review.value = withOverride(review.value, id)
  openPicker('head', id)
}

/* Back to the project's pair — and out of the table entirely when the rule does
   not reach this repository, which `withoutOverride` decides with the same rule
   a change of branch uses. The row is then named under the table like any other
   repository the review is not in. */
const follow = (id) => {
  if (props.busy) return
  review.value = withoutOverride(review.value, id, context.value)
}

const drop = (id) => {
  if (props.busy) return
  review.value = withoutRepo(review.value, id)
}

const toggleAdd = () => {
  if (props.busy) return
  addOpen.value = !addOpen.value
}

/* A repository added by hand arrives as an override — the rule's branch is not
   in it, which is why it was not in the table — and the list opens on its
   checked side, since the one thing nobody can fill in for somebody is the name
   the branch goes by there. */
const add = (id) => {
  if (props.busy) return
  const added = withRepo(review.value, id)
  /* `withRepo` answers with the form it was given when it declines — no branch
     to check yet, or a repository that already has a row. The list must not open
     over a row that does not exist: what was picked in it would be written into
     `overrides` under a repository outside `repoIds`, which is a pair with no
     row to draw it, invisible until some later change of branch brought the row
     back wearing it. The panel below is not offered at all while the checked
     side is empty; this is the second lock on the same door, so the two cannot
     drift apart. */
  if (added === review.value) return
  review.value = added
  addOpen.value = false
  openPicker('head', id)
}

const submit = () => {
  if (!ready.value) return
  /* Both panels close on the way out. The footer is drawn whatever is above it,
     so Review can be pressed with the branch list open — and a list left
     standing over a window that has gone quiet is a control somebody would go
     on trying to use. */
  picker.value = null
  addOpen.value = false
  emit('submit', { form: JSON.parse(JSON.stringify(review.value)) })
}

/* ---- what it all looks like ---------------------------------------------- */

/* The whole body recedes in busy rather than any one control going grey: every
   action in it is off at once, and dimming them one at a time would read as a
   form with some of it still live. `--attn-quiet-opacity` is the system's one
   "this recedes" value — the same one a done column's cards are drawn at — and
   using it here is what keeps the number out of this file. */
const bodyStyle = computed(() => ({
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-6)',
  opacity: props.busy ? 'var(--attn-quiet-opacity)' : 1
}))

const blockStyle = { display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }

/* The 10px caps over a block. The one place this system uses capitals. */
const microHeading = {
  font: 'var(--weight-semibold) var(--text-2xs)/1 var(--font-sans)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}

const ruleRow = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-5)',
  minWidth: 0
}

/* The two fields of the pair.

   They are shares of the row rather than widths, and that is deliberate: the
   design asks for about 200px on the base, and a pixel in this file is the one
   thing that does not move with the density or with the app-wide font size. The
   base holds a name like `main` and the checked side holds
   `feature/smetana-4nsa-remote-branches-repo`, so the room is spent where the
   long names are.

   The checked side carries the stronger border, because that is the side under
   examination; and the difference between `local` and `origin` is the muted
   `origin/` inside the value and nothing else — there is no second control for
   it anywhere in this window. */
const fieldStyle = (kind) => ({
  display: 'flex',
  alignItems: 'center',
  flex: kind === 'base' ? '3 1 0' : '7 1 0',
  minWidth: 0,
  height: 'var(--control-h-lg)',
  padding: '0 var(--space-5)',
  background: props.busy || (kind === 'head' && !review.value.head)
    ? 'var(--surface-sunken)'
    : 'var(--surface)',
  border: `var(--border-w) ${kind === 'head' && !review.value.head ? 'dashed' : 'solid'} ${
    props.busy ? 'var(--border-subtle)' : kind === 'head' ? 'var(--border-strong)' : 'var(--border)'
  }`,
  borderRadius: 'var(--radius-3)',
  font: `${kind === 'head' ? 'var(--weight-medium)' : 'var(--weight-regular)'} var(--text-md)/1 var(--font-mono)`,
  color: 'var(--text-primary)',
  textAlign: 'left',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  cursor: props.busy ? 'not-allowed' : 'default',
  transition: 'var(--transition-control)'
})

/* The value inside a field, and it is a box of its own rather than the button's
   own text: `text-overflow` reaches a block container's own inline content and
   not a flex item's, so a name too long for the field would be cut off
   mid-letter with nothing saying it went on. */
const valueStyle = {
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

const mutedInk = { color: 'var(--text-muted)' }
const arrowStyle = { flex: 'none', color: 'var(--text-secondary)' }

const captionStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}

const tableHeadStyle = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'var(--space-4)',
  padding: '0 var(--space-2) var(--space-3)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}

const metaStyle = {
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  whiteSpace: 'nowrap'
}

/* One grid template for every row, so the five columns line up down the whole
   table however long the names in them are. The name is the narrowest — it is
   one short word — and the pair takes what is left, since it holds two branch
   names at once. */
const ROW_COLUMNS = 'minmax(0, 2fr) minmax(0, 3fr) minmax(0, 4fr) auto auto'

/* A row that differs is lifted onto `--surface`, which is what makes the
   exceptions readable down a table of rows that all follow one rule. The
   highlight is a surface step and never a colour change and never a transform,
   so a row cannot jump under the pointer in a list this dense. */
const rowStyle = (row, hovered) => ({
  display: 'grid',
  gridTemplateColumns: ROW_COLUMNS,
  alignItems: 'center',
  gap: 'var(--space-5)',
  minHeight: 'var(--row-h)',
  padding: 'var(--space-2)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  background: hovered && !props.busy
    ? 'var(--surface-hover)'
    : row.override
      ? 'var(--surface)'
      : 'transparent',
  cursor: props.busy ? 'not-allowed' : 'default',
  transition: 'var(--transition-control)'
})

const nameStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  minWidth: 0,
  font: 'var(--weight-medium) var(--text-sm)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

/* The mark on a row somebody named themselves. Sentence case is the rule
   everywhere prose is drawn; this is a three-letter tag rather than a word, and
   it is drawn in the same 10px caps the block headings are. */
const badgeStyle = {
  flex: 'none',
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-sans)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-1)',
  padding: '0 var(--space-2)'
}

const pathStyle = {
  minWidth: 0,
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

const pairStyle = {
  minWidth: 0,
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)',
  color: 'var(--text-primary)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

/* The row's own sentence, in the quiet idiom whichever of the four it is: a
   fetch that could not reach the remote is how old an answer is rather than a
   failure, so it wears the same muted colour as `follows the rule` and never
   the failed red. */
const statusStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)',
  whiteSpace: 'nowrap'
}

const spinStyle = { animation: 'sm-spin var(--dur-pulse) linear infinite' }

/* The last row of the block, and an action rather than a control: a dropdown
   here offered a list of repositories under a placeholder that read like a
   value, half the width of the window, and nobody found it. */
const addRowStyle = (hovered) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  width: '100%',
  minHeight: 'var(--row-h)',
  padding: '0 var(--space-2)',
  background: hovered && !props.busy ? 'var(--surface-hover)' : 'transparent',
  border: 'none',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-sm)/1 var(--font-sans)',
  textAlign: 'left',
  cursor: props.busy ? 'not-allowed' : 'default',
  transition: 'var(--transition-control)'
})

/* The panel it opens, and the branch list, in one geometry: both are a block
   that pushes the window taller rather than anything that hangs over it. */
const panelStyle = {
  marginTop: 'var(--space-4)',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border-strong)',
  borderRadius: 'var(--radius-3)',
  overflow: 'hidden'
}

const panelHeadStyle = {
  display: 'flex',
  alignItems: 'center',
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-5)',
  background: 'var(--surface)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  ...microHeading
}

const candidateStyle = (hovered) => ({
  display: 'grid',
  gridTemplateColumns: 'minmax(0, 2fr) minmax(0, 4fr) auto',
  alignItems: 'center',
  gap: 'var(--space-5)',
  width: '100%',
  minHeight: 'var(--row-h)',
  padding: 'var(--space-2) var(--space-5)',
  background: hovered ? 'var(--surface-hover)' : 'transparent',
  border: 'none',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  textAlign: 'left',
  cursor: 'default',
  transition: 'var(--transition-control)'
})

const noteHintStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)',
  whiteSpace: 'nowrap'
}

/* The empty state's placeholder, for the `New review` door: two rows of the
   height the real ones will have, so the window does not jump by a block when
   the branch is picked, and one line saying what they are waiting for. */
const waitingStyle = {
  opacity: 'var(--attn-quiet-opacity)'
}

const waitingRowStyle = {
  display: 'flex',
  alignItems: 'center',
  minHeight: 'var(--row-h)',
  padding: 'var(--space-2)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)'
}

/* The three service messages as one block: a border around them and a rule
   between them, because three sentences loose under a table read as one
   paragraph and each of these is about a different thing. */
const notesStyle = {
  background: 'var(--surface)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)',
  overflow: 'hidden'
}

const noteRowStyle = (last) => ({
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  padding: 'var(--space-4) var(--space-5)',
  borderBottom: last ? 'none' : 'var(--border-w) solid var(--border-subtle)'
})

const noteTextStyle = {
  flex: 1,
  minWidth: 0,
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-secondary)'
}

/* An identifier inside a sentence keeps its own face. The family alone, so the
   line it sits in keeps its size and weight. */
const identStyle = { fontFamily: 'var(--font-mono)' }

const footerTextStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  marginRight: 'auto',
  minWidth: 0,
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}

/* Which row the pointer is over, for the surface step. One number rather than a
   tracker per row: a row is not a component here, and `useInteractive` is for a
   control that exists once. */
const hovered = ref('')
const over = (id) => {
  hovered.value = id
}
const out = () => {
  hovered.value = ''
}
</script>

<template>
  <!-- The width is read only outside a dialog window — inside one `Modal` takes
       the whole frame, which is already the registry's number. It is here so
       that `?view=gallery` draws this dialog at the width it has in the app, and
       it has to agree with `review-changes` in `views/dialogRegistry.js`. -->
  <Modal
    :open="open"
    :title="title"
    :width="720"
    :closable="!busy"
    body-padding="var(--space-6)"
    footer-padding="var(--space-5) var(--space-6)"
    @close="$emit('close')"
  >
    <div :style="bodyStyle">
      <div :style="blockStyle">
        <span :style="microHeading">Reads the difference</span>
        <div :style="ruleRow">
          <button
            type="button"
            :style="fieldStyle('base')"
            :disabled="busy"
            aria-label="The branch the review reads from"
            @click="openPicker('base')"
          >
            <span :style="valueStyle"
              ><span :style="mutedInk">{{ base.prefix }}</span>{{ base.ref }}</span
            >
          </button>
          <Icon name="arrow-right" :size="ARROW" :style="arrowStyle" />
          <button
            type="button"
            :style="fieldStyle('head')"
            :disabled="busy"
            aria-label="The branch under review"
            @click="openPicker('head')"
          >
            <span v-if="review.head" :style="valueStyle"
              ><span :style="mutedInk">{{ head.prefix }}</span>{{ head.ref }}</span
            >
            <span v-else :style="{ ...valueStyle, ...mutedInk }">{{ PICK_HEAD }}</span>
          </button>
        </div>
        <span :style="captionStyle">{{ caption }}</span>
      </div>

      <!-- While the list is open the table is not drawn, and that is what keeps
           this window's height predictable: the list is capped at nine rows'
           worth of `--row-h` and scrolls inside itself, so the ceiling is the
           same whatever the project is made of. -->
      <BranchPicker
        v-if="picker"
        ref="pickerBox"
        :branches="pickerBranches"
        :repos="pickerRepos"
        :fetched-at="pickerFetchedAt"
        :now="now"
        :selected="pickerSide?.ref ?? ''"
        :selected-origin="Boolean(pickerSide?.remote)"
        :scope="scope"
        @select="pick"
        @close="closePicker"
      />

      <div v-else :style="{ display: 'flex', flexDirection: 'column' }">
        <div :style="tableHeadStyle">
          <span :style="microHeading">Repository</span>
          <span v-if="review.head" :style="metaStyle">{{ summary }}</span>
        </div>

        <!-- The `New review` door before a branch is picked. Two rows of the
             height the real ones have, so that picking one fills the table
             rather than growing the window by a block nobody expected. -->
        <div v-if="!review.head" :style="waitingStyle">
          <div :style="waitingRowStyle">{{ WAITING_FOR_BRANCH }}</div>
          <div :style="waitingRowStyle" />
        </div>

        <template v-else>
          <div
            v-for="row in rows"
            :key="row.id"
            role="button"
            :tabindex="busy ? -1 : 0"
            :aria-label="`Give ${row.name} a pair of its own`"
            :style="rowStyle(row, hovered === row.id)"
            @mouseenter="over(row.id)"
            @mouseleave="out"
            @click="differ(row.id)"
            @keydown.enter.self.prevent="differ(row.id)"
            @keydown.space.self.prevent="differ(row.id)"
          >
            <span :style="nameStyle">
              <span :style="{ overflow: 'hidden', textOverflow: 'ellipsis' }">{{ row.name }}</span>
              <span v-if="row.manual" :style="badgeStyle">man</span>
            </span>
            <span :style="pathStyle" :title="row.id">{{ row.path }}</span>
            <span :style="pairStyle">
              <template v-if="row.override">
                <span :style="mutedInk">{{ row.pair.base.prefix }}</span>{{ row.pair.base.ref }} →
                <span :style="mutedInk">{{ row.pair.head.prefix }}</span>{{ row.pair.head.ref }}
              </template>
            </span>
            <span :style="statusStyle">
              <Icon
                v-if="row.status.icon"
                :name="row.status.icon"
                :size="STATUS_GLYPH"
                :style="row.status.spin ? spinStyle : undefined"
              />
              {{ row.status.text }}
            </span>
            <!-- Three actions and one place for them: give this row a pair of its
                 own, put it back on the project's, or take a repository somebody
                 added out again. -->
            <IconButton
              v-if="row.manual"
              icon="x"
              size="sm"
              :label="`Take ${row.name} out of the review`"
              :disabled="busy"
              @click.stop="drop(row.id)"
            />
            <IconButton
              v-else-if="row.override"
              icon="undo-2"
              size="sm"
              :label="`Put ${row.name} back on the project's pair`"
              :disabled="busy"
              @click.stop="follow(row.id)"
            />
            <IconButton
              v-else
              icon="pencil"
              size="sm"
              :label="`Give ${row.name} a pair of its own`"
              :disabled="busy"
              @click.stop="differ(row.id)"
            />
          </div>
        </template>

        <!-- The last row of the block, and its panel, and neither of them is
             drawn while there is no branch to check. The empty state is two
             rows waiting for one, and adding a repository to a review that has
             no branch is not something this form can mean: `withRepo` refuses
             it, so an add row offered there is a control that does nothing,
             over a panel captioning every candidate `no such branch` about a
             branch nobody has chosen. -->
        <template v-if="review.head">
          <button
            type="button"
            :style="addRowStyle(hovered === 'add')"
            :disabled="busy"
            @mouseenter="over('add')"
            @mouseleave="out"
            @click="toggleAdd"
          >
            <Icon name="plus" :size="PLUS" />
            Add a repository
          </button>

          <div v-if="addOpen" :style="panelStyle">
            <div :style="panelHeadStyle">Repositories not in this review</div>
            <button
              v-for="candidate in candidates"
              :key="candidate.id"
              type="button"
              :style="candidateStyle(hovered === candidate.id)"
              @mouseenter="over(candidate.id)"
              @mouseleave="out"
              @click="add(candidate.id)"
            >
              <span :style="nameStyle">{{ candidate.name }}</span>
              <span :style="pathStyle" :title="candidate.id">{{ candidate.path }}</span>
              <span :style="noteHintStyle">{{ candidate.note }}</span>
            </button>
            <div v-if="!candidates.length" :style="candidateStyle(false)">
              <span :style="noteHintStyle">Every repository of this project is in the review.</span>
            </div>
          </div>
        </template>
      </div>

      <!-- One block for all three, each with its own glyph. None of them is an
           error: a fetch in flight is a wait, a fetch that failed is a review
           going ahead over an older copy of origin, and a repository without
           such a branch is an ordinary fact about a project made of several. -->
      <div v-if="notes.length" :style="notesStyle">
        <div
          v-for="(note, index) in notes"
          :key="note.key"
          :style="noteRowStyle(index === notes.length - 1)"
        >
          <Icon
            :name="note.icon"
            :size="GLYPH"
            :style="{ flex: 'none', color: 'var(--text-muted)', ...(note.spin ? spinStyle : {}) }"
          />
          <span :style="noteTextStyle"
            ><span
              v-for="(part, at) in note.parts"
              :key="at"
              :style="part.mono ? identStyle : undefined"
              >{{ part.text }}</span
            ></span
          >
        </div>
      </div>
    </div>
    <template #footer>
      <span :style="footerTextStyle">
        <Icon v-if="busy" name="loader-circle" :size="GLYPH" :style="spinStyle" />
        {{ footerText }}
      </span>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!ready" @click="submit">
        {{ busy ? 'Reviewing…' : 'Review' }}
      </Button>
    </template>
  </Modal>
</template>
