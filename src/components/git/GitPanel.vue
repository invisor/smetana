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

   A click on a changed file leaves as `open` and opens it as a diff in the
   centre column; which repository it belongs to is the caller's business, since
   this component is handed the selection rather than holding it.

   At the foot of the repository list is the one thing this panel says about a
   repository it is **not** drawing: a folder somebody cloned into the project
   from a terminal, which `[project].repos` cannot grow to hold on its own. What
   is drawn there — nothing, one name, or several — is `unlistedRepos.js`, and
   the way out is the `setup` event, the same dialog the project row's own menu
   opens. This panel gains no verb of its own for it: that file is the setup
   agent's to write.

   The writes a branch row offers — checkout, merge, rebase, new branch — leave
   as events and are drawn by `BranchList`. Three more leave from the Branches
   caption rather than from a row: Pull and Push are about the branch this
   repository is **on**, so a row's menu would refuse them on nine rows in ten,
   and the caption is also the one thing on screen saying the two verbs exist.
   Beside them is the check — a `git fetch`, asking the remote what it has —
   which is about the repository rather than about any branch, and which is
   there because both verbs beside it are refused in the state somebody most
   wants to ask about: a branch level with its upstream.
   What they say and whether they may be pressed is `tracking.js`, pure and
   tested; this file draws its verdict. What is drawn here is git's refusal of
   whichever write was last refused, for the reason recorded beside the block: the branch
   section is capped and a message inside that scroller is below the fold in the
   ordinary case. A conflict is not one of those refusals and is not drawn here
   at all — it is an outcome with two doors, and the modal is `ConflictModal`.

   ## The three sections fold and are dragged

   Each caption is a `SectionHeader`, which is a button, and between the
   sections are `Resizer`s. How tall a section may be and which of them takes
   the height nobody claimed is `sectionHeights.js` — pure, tested, of the
   `gitActions.js` family — and the whole of what this file adds to it is the
   measurements, which is the half no test here can reach.

   Two of the three captions carry `divided`, the hairline that says a block
   starts here, and the repositories deliberately do not: this panel is drawn
   under the project list, which already ends in one of those, and a caption
   adding its own would draw the pair as a single 2px line. The rule sits on the
   caption rather than on the `Resizer` above it, because a folded section has
   no resizer and would lose its separator exactly when the captions are
   stacked tightest.

   **The stored number and the drawn number are kept apart**, the rule
   `panelWidths.js` states one axis over: what `settings.json` holds is what a
   person dragged to, and what a section is drawn at is that number clamped
   against the panel it is in now. Only a drag writes back, so a shortened
   window squeezes a section and a lengthened one gives back exactly what was
   asked for. Letting CSS do the squeezing instead was tried and is the one
   thing that cannot work here: a section drawn below the number it holds turns
   its own drawn height into a drag's starting point, and every attempt to pull
   it up walks the stored number down.

   The height of a row is measured off a header, which *is* one, and never read
   off the token: `--row-h` is a `calc()` over an unregistered custom property,
   so `getComputedStyle` hands back the calc unevaluated — the trap
   `terminal/theme.js` records. */
import { computed, onBeforeUnmount, ref, watchPostEffect } from 'vue'
import BranchList from './BranchList.vue'
import Button from '../core/Button.vue'
import ChangeList from './ChangeList.vue'
import CommitBox from './CommitBox.vue'
import EmptyState from '../core/EmptyState.vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import RepoList from './RepoList.vue'
import Resizer from '../shell/Resizer.vue'
import SectionHeader from './SectionHeader.vue'
import Tooltip from '../core/Tooltip.vue'
import {
  BRANCH_ROWS,
  UNDRAGGED_ROWS,
  clampRows,
  filler,
  resolveDrag
} from './sectionHeights.js'
import { fetchAction, pullAction, pushAction } from './tracking.js'
import { SETUP_LABEL, unlistedBlock } from './unlistedRepos.js'

const props = defineProps({
  repos: { type: Array, default: () => [] },
  /* The names of the repositories on disk that `.smetana/project.toml` does
     not hold, as `vcsState.unlisted` carries them. Names and not rows: there
     is nothing in this panel that can be done to one of them, and the block
     drawn from them is the panel saying so rather than offering anything.
     Empty is the ordinary answer, and it draws nothing at all. */
  unlisted: { type: Array, default: () => [] },
  /* The selected repository's absolute path. */
  selected: { type: String, default: null },
  /* `{ branch, detached, changes }`, or null when it could not be read — never
     an empty tree standing in for a failure. */
  tree: { type: Object, default: null },
  /* `[{ name, current }]` in `git::by_recency`'s order, which is drawn as it
     arrives — grouped into folders by `branchTree.js`, which keeps that
     order. */
  branches: { type: Array, default: () => [] },
  /* Which branch folders are unfolded, as `settings.project.branchFolders`
     keeps it, or null for "nobody has chosen here". Passed straight through:
     what a folder means and what a press on one leaves behind is
     `branchTree.js`, and this panel is presentational on it as on everything
     else. Per project rather than global, unlike the section folds above —
     `feature/…` is a habit of a repository, while how tall somebody likes their
     branch list is a habit of theirs. */
  branchFolders: { type: Array, default: null },
  /* Where each branch stands against its upstream, keyed by name, as
     `vcsState.tracking` holds it. It draws the marks on the rows and it is what
     the two buttons in the Branches caption are made of — an empty object is a
     repository with no remote, and every row and both buttons still answer. */
  tracking: { type: Object, default: () => ({}) },
  /* `{ allowed, reason }` from `gitActions.js`: whether the panel may write to
     this repository at all, and the sentence over the rows when it may not.
     Passed through rather than decided here — this panel draws, and the rule is
     a pure file a test can reach. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  /* What git is doing right now — `{ op, branch }` — and its refusal of the
     last write, which carries the `op` it was about so the block can name
     it. */
  busy: { type: Object, default: null },
  /* Whether a fetch somebody pressed for is still out. Its own flag and not
     `busy`, for the reason the store gives: a fetch freezes no row, so it dims
     one button and spins on it, and the branch list under it stays live. */
  fetching: { type: Boolean, default: false },
  writeError: { type: Object, default: null },
  /* `{ kind, message }` as `stores/vcs.js` normalises it. `noGit` is the one
     kind this panel branches on; everything else is git's own words, shown
     untouched, because whoever reads them knows git. */
  error: { type: Object, default: null },
  loading: { type: Boolean, default: false },
  /* The path of the change whose diff is open, marked in the list. Repository
     relative, the form every change carries. */
  openPath: { type: String, default: null },
  /* The commit message somebody is part-way through, and the two facts about
     the agent being asked to write one. Held by the store per repository rather
     than by this panel, for the reason every other value here is: this
     component draws and emits, and the draft has to survive it being taken off
     the screen — folding the section away must not throw a sentence out. */
  message: { type: String, default: '' },
  suggesting: { type: Boolean, default: false },
  suggestError: { type: Object, default: null },
  /* How the three sections are folded and how tall two of them were dragged to,
     as `settings.layout.gitSections` keeps it: `reposRows` and `branchRows` in
     rows, or null for "never dragged", plus a flag apiece. Global rather than
     per project, for the reason `settings.md` gives about the board's own view
     settings — how tall somebody likes their branch list is a habit of reading
     rather than a fact about one repository.

     Read through `fold` below rather than directly, so a caller handing over
     part of it — every gallery frame does — still gets a whole panel. */
  sections: { type: Object, default: null }
})
const emit = defineEmits([
  'select',
  /* The way out of the state the block below names, and deliberately not a
     verb of this panel's own: it opens the same setup dialog the project row's
     menu opens, and the setup agent stays the only thing in this app that
     writes `.smetana/project.toml`. It carries nothing, because there is
     nothing to choose — the project is the caller's, and the dialog is always
     the "setting up over an existing file" one, since a panel with something
     unlisted to point at is a panel with a configuration that missed it. */
  'setup',
  'checkout',
  /* The one verb here that reads. It is deliberately absent from
     `WRITE_REFUSED` below — that table names what git declined, and this asks
     git for nothing this panel then has to draw: it goes nowhere near
     `gitActions.js`, it cannot stop mid-tree, and what it opens is a window of
     its own. */
  'compare',
  'merge',
  'rebase',
  'new-branch',
  'pull',
  'push',
  'fetch',
  'commit',
  'suggest',
  'message',
  'open',
  'toggle',
  'toggle-folder',
  'resize'
])

const rootStyle = { display: 'flex', flexDirection: 'column', height: '100%', minHeight: 0 }

/* The whole of the fold state with every hole filled. A frame that hands over
   nothing at all is the ordinary case in the gallery, and it must draw the
   panel this feature shipped on top of rather than a folded ruin. */
const fold = computed(() => ({
  reposOpen: props.sections?.reposOpen ?? true,
  changesOpen: props.sections?.changesOpen ?? true,
  branchesOpen: props.sections?.branchesOpen ?? true,
  reposRows: props.sections?.reposRows ?? null,
  branchRows: props.sections?.branchRows ?? null
}))

/* git's own stderr. Mono and left-aligned rather than an `EmptyState`'s centred
   prose: this is machine output, and it is shown exactly as git wrote it.

   Used twice: inside the changes scroller for a read that failed, and as a flex
   item of this column for a write git refused. `flexShrink: 0` is for the
   second — a flex item shrinks by default, and the lists above it have
   somewhere to give way to, while a refusal clipped to a strip of its own title
   is the defect this block was moved out of the branch section to fix. It
   changes nothing at the first site, where the parent is not a flex
   container. */
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

/* What this panel has to say about a folder in the project that
   `.smetana/project.toml` does not name, or `null` for the ordinary case where
   it has nothing to say and draws nothing at all. The rule is
   `unlistedRepos.js`, pure and tested, of the `sectionHeights.js` family: what
   is left here is the drawing.

   Behind `settled` with the list itself, and for that same reason: the block is
   a statement about a directory that was read, and it must not flash over a
   list still on its way. A read that failed clears the names in the store, so
   there is nothing to draw over a failure either. */
const unlisted = computed(() => (settled.value ? unlistedBlock(props.unlisted) : null))

/* Rows, both of them, and that is load-bearing rather than tidy: this section's
   height is counted in rows and never in pixels (`sectionHeights.js`), and one
   thing drawn here at some other height would put the measured row a fraction
   away from the drawn ones for the whole of that arithmetic — so a drag would
   stop short of a boundary and leave half a row under the fold.

   The hairline is `SectionHeader`'s `divided`, for its reason one level down:
   every row in this panel is `--row-h` and quiet, so with nothing between them
   the block would read as two more repositories rather than as a remark about
   the list above. It is drawn inside the height, which `box-sizing:border-box`
   is what makes true. */
const unlistedCaptionStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  height: 'var(--row-h)',
  flexShrink: 0,
  /* The gear would otherwise sit against the panel's own edge — the inset
     `SectionHeader` gives a caption that carries controls. */
  padding: '0 var(--space-3) 0 var(--space-5)',
  borderTop: 'var(--border-w) solid var(--border-subtle)',
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-sans)',
  color: 'var(--text-muted)'
}
/* Prose in sans and the identifier in mono, which is why the caption arrives
   from the rule in two pieces rather than as one sentence with a path buried in
   it. Both shrink before the row does: a flex item refuses by default to go
   below its own content, and the gear would be pushed off the end of a narrow
   panel. */
const unlistedFileStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}
/* A name is an identifier and is drawn as the repository rows above are drawn,
   muted: these are folders the panel is pointing at rather than rows anything
   can be done to, so nothing here hovers, selects or is pressed. */
const unlistedRowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  height: 'var(--row-h)',
  flexShrink: 0,
  padding: '0 var(--space-5)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)'
}
const unlistedNameStyle = {
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
/* The glyph the repository rows carry, at the size `RepoList` draws it, and it
   is here for the alignment rather than for the picture: without it the names
   would start where the icons above them start and read as a differently
   indented list of the same kind of thing. */
const UNLISTED_MARK = 12

/* Which write git declined, in its own words above git's. One block for the
   three of them, keyed on the `op` the refusal came with: a message reading
   "did not switch branch" over a refused merge would name the wrong operation,
   and three blocks that can never be on screen together would be three copies
   of one thing.

   An `op` this build has never heard of takes the general sentence rather than
   nothing at all — a refusal with no title would be git's stderr floating under
   the branches, which is exactly the state this block exists to prevent. */
const WRITE_REFUSED = {
  checkout: 'Git did not switch branch',
  merge: 'Git did not merge',
  rebase: 'Git did not rebase',
  create: 'Git did not create the branch',
  abort: 'Git did not abort',
  commit: 'Git did not commit',
  pull: 'Git did not pull',
  push: 'Git did not push',
  /* The one entry here for something that is not a write to the tree. It is in
     this table all the same, because a fetch somebody pressed for fails the way
     a pressed write fails — in this block, in git's own words — and the
     sentence has to name what was refused rather than reach for the general
     one. */
  fetch: 'Git did not reach the remote'
}
const writeRefused = computed(
  () => WRITE_REFUSED[props.writeError?.op] ?? 'Git refused this operation'
)

/* The branch this repository is on, as the list says.

   The list rather than the tree, for the reason `currentBranch()` in the store
   reads it there: `vcs_branches` carries `current` from a HEAD read beside it,
   so the two cannot come from different moments. The tree is the fall-back for
   a repository whose branch list has not landed yet, and both are empty on a
   detached HEAD — which is what takes the two buttons off the header. */
const currentBranchName = computed(
  () => props.branches.find((branch) => branch.current)?.name ?? props.tree?.branch ?? null
)

/* Neither of the two verbs on a detached HEAD: there is no upstream to talk
   about, and two dead controls say less than nothing at all — the instinct
   every empty state in this panel already follows. The check stays, since
   asking the remote what it has is a question about the repository and a
   detached HEAD has not stopped it being one. */
const hasBranch = computed(() => Boolean(currentBranchName.value))

/* What the two of them say and whether they may be pressed, which is
   `tracking.js` and none of it this file's. `actions` — the runs verdict — goes
   in as an argument rather than being asked again there: one rule, one copy.

   `busy` is deliberately not folded into that verdict either. A refusal has a
   sentence a person can read, and "git is already working" is a state a
   spinner in the branch list is already saying. */
const pull = computed(() => pullAction(props.tracking[currentBranchName.value], props.actions))
const push = computed(() => pushAction(props.tracking[currentBranchName.value], props.actions))

/* The third of them, and the one that survives everything that dims the other
   two: a detached HEAD, a run, and — since Pull is refused when the branch is
   level — the ordinary state of a repository nobody else has pushed to. It is
   about the repository rather than about a branch, which is why it takes
   neither the tracking record nor the runs verdict. */
const check = computed(() => fetchAction(props.fetching))

/* The glyph turns while the answer is out. `Button` has no loading state and
   is not getting one for this: what says "still going" in this panel is
   `loader-circle` at `--attn-live` turning at `--dur-pulse`, which is what the
   branch rows already draw over a write, and handing it in through the slot
   keeps the one spinner this panel has in one idiom. */
const SPIN = 13
const spinStyle = { color: 'var(--attn-live)', animation: 'sm-spin var(--dur-pulse) linear infinite' }

/* Prose over a control somebody is on their way past waits; a control's own
   name does not. `Tooltip`'s own note is the rule, and a refused button here is
   the first kind — a whole sentence about something a pointer crosses on the
   way to the branch list.

   The hint sits on a wrapper rather than on the button, which is what makes it
   reachable at all: the two of them spend most of their life refused, and the
   panel explaining a refusal must not be the one thing the refusal hides. The
   wrapper is a `Tooltip` in both states rather than only in the refused one —
   this is a control whose name is a glyph, so there is always something to say,
   and one panel that changes what it says beats two elements taking turns.

   `Button` and deliberately not `IconButton`, which is the icon-only control
   everywhere else in this app: that one carries a `Tooltip` of its own around
   its `label`, and inside this wrapper the two of them opened together — the
   name above the glyph and the reason beside it, two panels over a caption
   152 pixels wide. The accessible name it enforces is passed here by hand
   instead, so nothing is lost but the second panel. */
const TIP_DELAY = 400
const hintProps = (action) =>
  action.allowed
    ? { label: action.label }
    : { label: action.reason, side: 'left', delay: TIP_DELAY }

/* Which captions are on screen. The changes and the branches have none without
   a repository to have changed anything or to hold one: a heading over nothing
   would be a second empty state saying less than the repository list's own
   sentence already said. The repositories always have theirs.

   The changes *region* is drawn regardless, because it is where a failed read
   is reported, so with no caption over it there is nothing to fold it by — and
   a section nobody can unfold must never be treated as folded. */
const changesCaption = computed(() => props.repos.length > 0)
const branchesDrawn = computed(() => props.repos.length > 0 && !failure.value)

const changesOpen = computed(() => (changesCaption.value ? fold.value.changesOpen : true))

/* What `sectionHeights.js` is asked about: the sections actually drawn, in the
   order they are drawn. */
const drawn = computed(() => {
  const list = [
    { id: 'repos', open: fold.value.reposOpen },
    { id: 'changes', open: changesOpen.value }
  ]
  if (branchesDrawn.value) list.push({ id: 'branches', open: fold.value.branchesOpen })
  return list
})
const fills = computed(() => filler(drawn.value))

const rows = (n) => `calc(var(--row-h) * ${n})`

/* The measurements, which is the half of this that no test in this repository
   can reach. A header is exactly one row, so it is what a row is measured by —
   never `getComputedStyle`, which hands back `--row-h`'s `calc()` unevaluated.
   The panel's own box is what "how much is there" means.

   Both are watched rather than read once, and both have to be: the panel's
   height moves with the window, and a row's height moves with the density and
   the app-wide font size, neither of which re-renders this component. Observing
   the header covers the second, since a row that changes height is exactly what
   those two settings do. */
const panel = ref(null)
const reposHeader = ref(null)

const rowPx = ref(0)
const panelPx = ref(0)
const measure = () => {
  rowPx.value = reposHeader.value?.el?.getBoundingClientRect().height ?? 0
  panelPx.value = panel.value?.getBoundingClientRect().height ?? 0
}
const observer = typeof ResizeObserver === 'function' ? new ResizeObserver(measure) : null
/* Re-subscribed rather than subscribed once: `noGit` takes every section off
   the screen, so the header this measures by is an element that comes and
   goes. */
watchPostEffect(() => {
  if (!observer) return
  observer.disconnect()
  if (panel.value) observer.observe(panel.value)
  if (reposHeader.value?.el) observer.observe(reposHeader.value.el)
  measure()
})
onBeforeUnmount(() => observer?.disconnect())

const available = computed(() => (rowPx.value ? panelPx.value / rowPx.value : 0))

/* What the other section is holding, which is part of this one's ceiling: what
   it was dragged to, or `UNDRAGGED_ROWS` when nobody has dragged it. An
   undragged section gives way on its own and is not owed its content — but it
   is owed a whole row, or dragging its neighbour past it draws it as a sliver
   of one. A folded section is owed nothing: it is a header, and headers are
   counted separately. */
const otherFixed = (id) => {
  if (id === 'repos') {
    return branchesDrawn.value && fold.value.branchesOpen
      ? (fold.value.branchRows ?? UNDRAGGED_ROWS)
      : 0
  }
  return fold.value.reposOpen ? (fold.value.reposRows ?? UNDRAGGED_ROWS) : 0
}

/* **Stored rows → drawn rows, and the two are different numbers.** The panel is
   clamped against the room it has now, which is the rule `panelWidths.js`
   states one axis over, and it is what makes a drag mean anything at all: left
   to shrink itself under a short panel, a section is drawn below the number it
   holds, and a drag reading its own drawn height back as the starting point
   walks the stored number down every time somebody tries to pull it up.

   Before the first measurement lands there is nothing to clamp against, so the
   stored number is drawn as it stands and corrects on the next frame. */
const drawnRows = (id) => {
  const stored = id === 'repos' ? fold.value.reposRows : fold.value.branchRows
  if (stored === null) return null
  if (!available.value) return stored
  return clampRows(stored, {
    available: available.value,
    headers: drawn.value.length,
    fixed: otherFixed(id)
  })
}

/* The filler grows into whatever is left. A dragged section is `0 0 auto` at
   the height it was clamped to — it does not shrink, because the clamp has
   already left the filler its floor and shrinking on top of that is what made
   the drawn height disagree with the stored one. A section nobody has dragged
   is still `0 1 auto` over its own content, exactly as all three were before
   any of this, so a short panel shares the squeeze between them the way it
   always has.

   `FILLER_MIN_ROWS` is deliberately not a `minHeight` here. It is honoured by
   the clamp above, and stating it twice made a short panel worse rather than
   safer: the floor came out of the sections above it, and a 260px panel drew
   the repository list as a clipped strip of one row instead of a whole one. */
const sectionStyle = (id) => {
  if (fills.value === id) return { flex: '1 1 auto', minHeight: 0, overflow: 'auto' }
  const height = drawnRows(id)
  return height === null
    ? /* The floor the clamp above already reserves for this section, stated
         where it acts. Without it the reservation only bounds how far a drag may
         go, and CSS is still free to share the squeeze straight through it —
         which drew an undragged repository list as a clipped strip while its
         neighbour held the row that had been set aside for it. One row, so
         unlike a floor under the filler it cannot crush a short panel. */
      { flex: '0 1 auto', minHeight: rows(UNDRAGGED_ROWS), overflow: 'auto' }
    : { flex: '0 0 auto', height: rows(height), minHeight: 0, overflow: 'auto' }
}

/* Untouched, the repositories follow their content — a project of one
   repository draws one row and not a reserved block of empty ones — and the
   branches follow theirs up to the cap they have always had. A drag replaces
   either with a number. */
const reposStyle = computed(() => sectionStyle('repos'))

const changesStyle = computed(() => sectionStyle('changes'))
const branchStyle = computed(() => {
  const style = sectionStyle('branches')
  return fold.value.branchRows === null && fills.value !== 'branches'
    ? { ...style, maxHeight: rows(BRANCH_ROWS) }
    : style
})

/* A separator belongs to the section above it and is drawn only where a drag
   would mean something: not over a folded section, which has no height to give,
   and not over the filler, which is already taking everything the others do not
   claim — there is nothing on its side of the strip to take height from. */
const reposResizer = computed(() => fold.value.reposOpen && fills.value !== 'repos')
const branchResizer = computed(
  () => branchesDrawn.value && fold.value.branchesOpen && fills.value !== 'branches'
)

const reposBox = ref(null)
const branchBox = ref(null)

/* Everything the drag is resolved against, snapshotted at `dragstart` — the
   contract `Resizer` states, since a delta measured from the last frame would
   let a clamped move become the next move's origin and the section would drift
   away from the pointer. */
let drag = null

const onDragStart = (section) => {
  if (!rowPx.value || !available.value) {
    drag = null
    return
  }
  /* A section that has been dragged starts from the number it holds, drawn
     exactly; one that never has starts wherever its content left it, which is
     the only honest answer for a height nobody has chosen. */
  const stored = drawnRows(section)
  const box = section === 'repos' ? reposBox.value : branchBox.value
  drag = {
    section,
    base: stored ?? (box ? box.getBoundingClientRect().height / rowPx.value : 0),
    available: available.value,
    headers: drawn.value.length,
    fixed: otherFixed(section)
  }
}

const onDrag = (section, delta) => {
  if (drag?.section !== section) return
  emit('resize', {
    section,
    rows: resolveDrag(section, { ...drag, delta: delta / rowPx.value })
  })
}

const onDragEnd = () => {
  drag = null
}

/* Double click gives a section back to its content — the branches to their cap
   and the repositories to their rows — which is this panel's answer to the same
   gesture that resets a side panel to its shipped width. */
const onReset = (section) => emit('resize', { section, rows: null })
</script>

<template>
  <div ref="panel" :style="rootStyle">
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
      <SectionHeader
        ref="reposHeader"
        label="Repositories"
        :count="repos.length > 1 ? repos.length : null"
        :open="fold.reposOpen"
        @toggle="emit('toggle', 'repos')"
      />
      <!-- The list scrolls rather than pushing the changes off the bottom: a
           folder of a dozen sibling repositories is exactly what the discovery
           arm in `vcs/repos.rs` exists for, and giving way before the changes do
           is what lets the changes keep their share.

           With nothing in the list and a failure to report, the list is left
           out altogether: `RepoList`'s "No repositories here" is a statement
           about a folder that was read, and a read that failed has not earned
           it. The failure below says what actually happened. -->
      <div v-if="fold.reposOpen" ref="reposBox" :style="reposStyle">
        <RepoList
          v-if="settled && (repos.length || !failure)"
          :repos="repos"
          :selected="selected"
          @select="$emit('select', $event)"
        />
        <!-- The foot of the list, and only when there is something to say: a
             repository somebody cloned into this project from a terminal, which
             a configured `[project].repos` can never grow to hold. The panel
             names it and points at the one door that fixes it — the setup
             agent, which is the only thing in this app that writes that file.
             With nothing unlisted this is not a caption, a row or an inset: the
             panel is exactly what it was.

             Inside the scroller with the rows rather than under it, because it
             is about that list: a folder named below the fold of a section
             somebody dragged short is a remark they can scroll to, where one
             pinned outside would take a row from the list it is about. -->
        <div v-if="unlisted" role="group" :aria-label="unlisted.summary">
          <div :style="unlistedCaptionStyle">
            <span>{{ unlisted.lead }}</span>
            <span :style="unlistedFileStyle">{{ unlisted.file }}</span>
            <span :style="{ flex: 1 }" />
            <!-- The same verb the project row's right-click menu offers, in the
                 same words and with the same glyph, since it opens the same
                 dialog. `sm`, like every other control in this panel's rows:
                 the default control height is taller than a row in the compact
                 density. -->
            <IconButton
              icon="settings-2"
              :label="SETUP_LABEL"
              size="sm"
              @click="emit('setup')"
            />
          </div>
          <div v-for="name in unlisted.names" :key="name" :style="unlistedRowStyle">
            <Icon name="folder-git-2" :size="UNLISTED_MARK" :style="{ flex: 'none' }" />
            <span :style="unlistedNameStyle">{{ name }}</span>
          </div>
        </div>
      </div>
      <Resizer
        v-if="reposResizer"
        orientation="horizontal"
        label="Resize the repository list"
        @dragstart="onDragStart('repos')"
        @drag="onDrag('repos', $event)"
        @dragend="onDragEnd"
        @reset="onReset('repos')"
      />

      <SectionHeader
        v-if="changesCaption"
        divided
        label="Changes"
        :count="tree && changes.length ? changes.length : null"
        :open="fold.changesOpen"
        @toggle="emit('toggle', 'changes')"
      />
      <!-- The changes are what somebody opened this panel for, so while they
           are unfolded they are the section that takes whatever height the
           others do not claim. -->
      <div v-if="changesOpen" :style="changesStyle">
        <!-- At the top of the list and **stuck** there rather than pinned above
             it, which is one section rather than two and is what keeps this
             region a plain scroller — `sectionHeights.js` is untouched by the
             box, and a panel too short for it scrolls to it instead of clipping
             the button off the bottom.

             Drawn only over a tree with something in it: the list's own "No
             uncommitted files in this repository" is the whole story there, and
             a message box under it would be offering to commit nothing. -->
        <CommitBox
          v-if="!failure && tree && changes.length"
          :model-value="message"
          :changes="changes.length"
          :branch="tree.branch"
          :actions="actions"
          :busy="busy"
          :suggesting="suggesting"
          :suggest-error="suggestError"
          @update:model-value="$emit('message', $event)"
          @commit="$emit('commit')"
          @suggest="$emit('suggest')"
        />
        <div v-if="failure" :style="failureStyle">
          <div :style="failureTitleStyle">{{ failureTitle }}</div>
          <div :style="failureTextStyle">{{ failure }}</div>
        </div>
        <ChangeList
          v-else-if="repos.length && tree"
          :changes="changes"
          :selected="openPath"
          @open="$emit('open', $event)"
        />
      </div>

      <!-- Third, under the changes, and gated on there being a repository for
           the same reason the changes caption is. Until it is dragged the
           section is capped at `BRANCH_ROWS`: the changes above it are what
           somebody opened this panel for, and a repository with forty branches
           must not push them off the top. -->
      <template v-if="branchesDrawn">
        <Resizer
          v-if="branchResizer"
          orientation="horizontal"
          label="Resize the branch list"
          @dragstart="onDragStart('branches')"
          @drag="onDrag('branches', $event)"
          @dragend="onDragEnd"
          @reset="onReset('branches')"
        />
        <SectionHeader
          divided
          label="Branches"
          :count="branches.length > 1 ? branches.length : null"
          :open="fold.branchesOpen"
          @toggle="emit('toggle', 'branches')"
        >
          <!-- The two remote verbs and the check, in the caption rather than in
               a row's menu:
               they are about the current branch, so on nine rows in ten the
               item would be refused, and a menu here answers about the row it
               was opened on. The caption is also the one thing on screen saying
               these two verbs exist at all.

               A `<button>` cannot hold a button, which is why the caption has a
               slot beside it rather than inside it — press one of these and the
               section would otherwise fold on the way through. -->
          <template #actions>
            <!-- First of the three and the only one always here: it is the
                 question the other two are answers to, and with both of them
                 refused over a branch that is level it is the whole of what
                 this caption can still do. On a detached HEAD it is the only
                 control in the row. -->
            <Tooltip v-bind="hintProps(check)">
              <!-- Two buttons and not one with a `v-if` inside its slot, which
                   is the version this shipped as and the defect it shipped
                   with. `Button` draws its slot as `<span v-if="$slots.default">`,
                   and a slot **function** is there whether or not the `v-if`
                   inside it renders anything: the empty span stayed a flex
                   child, the button spent its `gap` on nothing and came out
                   6px wider than the two arrows beside it — then snapped back
                   to their width the moment a fetch started, sliding the count
                   and both arrows sideways. Interaction is a surface step and
                   never a shift. Handing the slot over only in the state that
                   fills it is what keeps all three buttons one width. -->
              <Button
                v-if="fetching"
                variant="ghost"
                size="sm"
                :aria-label="check.label"
                :disabled="!check.allowed"
                @click="$emit('fetch')"
              >
                <Icon name="loader-circle" :size="SPIN" :style="spinStyle" />
              </Button>
              <Button
                v-else
                variant="ghost"
                size="sm"
                icon="refresh-cw"
                :aria-label="check.label"
                :disabled="!check.allowed"
                @click="$emit('fetch')"
              />
            </Tooltip>
            <Tooltip v-if="hasBranch" v-bind="hintProps(pull)">
              <Button
                variant="ghost"
                size="sm"
                icon="arrow-down"
                :aria-label="pull.label"
                :disabled="!pull.allowed || Boolean(busy)"
                @click="$emit('pull')"
              />
            </Tooltip>
            <Tooltip v-if="hasBranch" v-bind="hintProps(push)">
              <Button
                variant="ghost"
                size="sm"
                icon="arrow-up"
                :aria-label="push.label"
                :disabled="!push.allowed || Boolean(busy)"
                @click="$emit('push')"
              />
            </Tooltip>
          </template>
        </SectionHeader>
        <div v-if="fold.branchesOpen" ref="branchBox" :style="branchStyle">
          <BranchList
            :branches="branches"
            :tracking="tracking"
            :folders="branchFolders"
            :actions="actions"
            :busy="busy"
            @checkout="$emit('checkout', $event)"
            @compare="$emit('compare', $event)"
            @merge="$emit('merge', $event)"
            @rebase="$emit('rebase', $event)"
            @new-branch="$emit('new-branch', $event)"
            @toggle-folder="$emit('toggle-folder', $event)"
          />
        </div>
        <!-- **Outside the scroller above, and outside the fold, and that is the
             whole point.** Drawn under the rows it belonged to, it sat below the
             fold of a capped box — with six branches or more the refusal was
             entirely out of view, so a person pressed a row, the tick did not
             move, and nothing said why. Folding the section away is the same
             defect by another route, so this block does not fold with it. It is
             the same block the read failure above uses, and one copy of it:
             `failureTitleStyle` is what says which of the two this is, and
             `writeRefused` which of the writes.

             A conflict never reaches here: it is not a refusal, and what draws
             it is a modal with two doors, because a conflicted tree is a state
             this panel would otherwise be promising to show and unable to. -->
        <div v-if="writeError" :style="failureStyle">
          <div :style="failureTitleStyle">{{ writeRefused }}</div>
          <div :style="failureTextStyle">{{ writeError.message }}</div>
        </div>
      </template>
    </template>
  </div>
</template>
