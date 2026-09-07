<script setup>
/* The Git tab of the left sidebar: the repositories a project is made of, and
   the uncommitted files of the one being shown.

   Presentational, like every other component here — it is handed the state and
   emits what was picked, so it renders in `?view=gallery` with no back end
   behind it. The state itself is `src/stores/vcs.js` and the wiring is
   `views/DesktopApp.vue`.

   Four things can be empty and each says something of its own: git is not on
   this machine, this folder holds no repository, this repository has nothing
   uncommitted, this repository has no local branch yet — and the branch list
   has a fifth for a tab whose `origin` this app knows nothing about. One blank
   area for all of them would be a panel saying nothing several different ways,
   and the first is the one a person can act on.

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

   The writes a branch row offers — checkout, merge, rebase, new branch, delete
   — leave as events and are drawn by `BranchList`, and beside them the one item
   in that menu that asks git for nothing: pinning a branch above the tree,
   which leaves here as the whole new list. Three more leave from the Branches
   **tab row** rather than from a row of the list: Pull and Push are about the
   branch this repository is **on**, so a row's menu would refuse them on nine
   rows in ten, and the row they sit in is also the one thing on screen saying
   the two verbs exist. Beside them is the check — a `git fetch`, asking the
   remote what it has — which is about the repository rather than about any
   branch, and which is there because both verbs beside it are refused in the
   state somebody most wants to ask about: a branch level with its upstream.
   What they say and whether they may be pressed is `tracking.js`, pure and
   tested; this file draws its verdict. What is drawn here is git's refusal of
   whichever write was last refused, for the reason recorded beside the block: the branch
   section is capped and a message inside that scroller is below the fold in the
   ordinary case. A conflict is not one of those refusals and is not drawn here
   at all — it is an outcome with two doors, and the modal is `ConflictModal`.

   ## The Branches section has two sides, and a row of tabs to choose between

   Under that caption sits a row of exactly two tabs, `Local` and `Origin`, and
   `BranchList` draws one side or the other rather than both. It replaces the
   `origin` group that used to sit at the foot of the local list: on the
   repository this was asked for that heading was 346 rows down a scroller, and
   in a repository where `origin` holds nothing extra it was not drawn at all,
   which on screen reads as broken rather than as working.

   **The count in the caption is the count of the tab underneath it.** The
   number of local branches on `Local`, the number of branches `origin` has on
   `Origin` — a count describing the other list is exactly the defect that made
   this task necessary, since the group's own heading carried its number and the
   caption above went on describing only the half above it.

   The row is its own row rather than three more controls in the caption: that
   caption is 252 pixels wide and already carries a chevron, a word and a count.
   It is `shell/SegmentedTabs.vue`, the same row the two side columns draw,
   rather than a copy of its style objects here — that component's own header
   names a second copy of them as the pair that drifts. What it costs is that
   the row is not `--row-h` tall, since it sizes itself from `--control-h-sm`
   and its own padding: so `headerRows` **measures** it and divides by a row,
   which the arithmetic takes in its stride — `available` is already a fraction
   and the ceiling is floored at the end.

   **Fetch, Pull and Push sit at the right end of that row**, beside the tabs
   rather than in the caption where they used to be. Two things pushed them
   there: the caption is the narrowest strip in this panel and is about to gain
   a search button of its own, and the tab row already exists, is already
   measured, and has a whole half of itself doing nothing. They are `size="sm"`,
   which is what keeps them inside a height the segmented control had set
   already — so the row is still measured and still never asserted, and nothing
   in this file declares how tall it is. What moved is the place and nothing
   else: `tracking.js` still says what each verb is called and whether it may
   be pressed, `hasBranch` still takes Pull and Push off a detached HEAD, and
   the check still spins where it always did.

   One consequence is worth stating, because nothing on screen explains it: the
   three of them are inside `tabBox`, so **folding the Branches section away
   takes them with it**. That is the fold meaning what it says, and it is the
   price of the row being measured rather than declared — a control drawn
   outside that wrapper would be a height the arithmetic never sees.

   **Under them, while a run holds this repository, is a strip saying so once.**
   The rows below mute and go inert as they always have and each keeps its own
   tooltip, which is the reason for the pointer and the keyboard; the strip is
   the reason for the eye, and it is what stops a panel of grey rows reading as
   a panel that failed to load. It is drawn inside the same wrapper the tab row
   is, so the measurement above already counts it and there is no second
   observer. It names no agent: the verdict carries none, and a name invented
   for the sentence would be this panel claiming to know which run.

   **The check stays pressable under a run**, alone of the three. A fetch writes
   remote-tracking refs and touches neither the working tree nor the index, so
   there is nothing for a batch mid-merge to lose by it — the same argument that
   keeps the commit box's sparkle alive and lets the background sweep go out
   under a batch. Pull and Push are refused by the verdict `tracking.js` folds
   in, exactly as before.

   ## The name filter, which is the one piece of state this panel owns

   The Branches caption carries a `search` button, and pressing it turns that
   caption into a field: chevron, word and count out, glyph, `<input>` and `x`
   in, inside the same `--row-h`. **Opening moves the focus into the field and
   closing gives it back to the button**, which is one contract and not two
   halves: closing unmounts the element holding the focus, so a close that said
   nothing would drop it on `<body>` — after the opening half had taught
   somebody that this control moves their caret for them. The field keeps the
   stylesheet's own focus ring, pulled inside its own edge rather than
   suppressed, because the `x` is one Tab away inside the same plate.

   Everything about it lives **here**, in two refs, and deliberately not in
   `settings.json` — a query is something somebody
   is doing this minute and not a preference, and a stored one would come back
   over a branch list they have since stopped looking for anything in. It does
   not survive a change of repository either, which is what the watch on
   `selected` is for: a project switch arrives here as exactly that.

   What the rule sees is debounced by `--dur-fast`, the stylesheet's own step
   for a change of state, read off the root at the moment it is wanted rather
   than at import — the app-wide font size and `prefers-reduced-motion` both
   move it, and the second zeroes it, which lands as a filter answering on the
   keystroke. `filterDelay` in `branchTree.js` is the parse, because
   `getPropertyValue` hands back the unit the stylesheet was written in and
   that is a rule worth a test.

   The counts move with the field. The caption cannot carry one while it is a
   field, so `branchTabLabels` puts them in the tab row: the tab showing reads
   `3 of 346`, and the other reads `Origin 9` — its own hits against the same
   query. Without that second number a filter that matched nothing here would
   draw an empty state that is telling the truth about the wrong half of the
   repository.

   Which tab is showing is the caller's state, remembered per project
   (`settings.project.branchTab`); this panel emits the id and holds nothing.
   The two ids are `branchTree.js`'s closed list, mirrored in
   `settings/model.rs` — a value the front end offers and Rust refuses comes
   back as `local` after a restart with nothing on screen saying why.

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
import { computed, nextTick, onBeforeUnmount, ref, watch, watchPostEffect } from 'vue'
import BranchList from './BranchList.vue'
import Button from '../core/Button.vue'
import ChangeList from './ChangeList.vue'
import CommitBox from './CommitBox.vue'
import EmptyState from '../core/EmptyState.vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import RepoList from './RepoList.vue'
import Resizer from '../shell/Resizer.vue'
import SegmentedTabs from '../shell/SegmentedTabs.vue'
import SectionHeader from './SectionHeader.vue'
import Tooltip from '../core/Tooltip.vue'
import {
  DEFAULT_BRANCH_TAB,
  branchTabLabels,
  filterBranches,
  filterDelay,
  originBranches
} from './branchTree.js'
import { BRANCH_FILTER_LABEL } from './branchPicker.js'
import { DEFAULT_ROWS as COMMIT_ROWS } from './commitBox.js'
import { failureTextStyle, failureTitleStyle } from './failureStyle.js'
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
  /* The short hash HEAD is sitting on when it is on no branch at all, or null.
     It is about **the selected repository** and so is read off that
     repository's own tree by the caller, not off the project's root: a project
     can hold several repositories, and the plate this draws names the one this
     panel is showing. Passed on to `BranchList`, which draws it where the
     current branch would be. */
  detached: { type: String, default: null },
  /* Which branch folders are unfolded, as `settings.project.branchFolders`
     keeps it, or null for "nobody has chosen here". Passed straight through:
     what a folder means and what a press on one leaves behind is
     `branchTree.js`, and this panel is presentational on it as on everything
     else. Per project rather than global, unlike the section folds above —
     `feature/…` is a habit of a repository, while how tall somebody likes their
     branch list is a habit of theirs. */
  branchFolders: { type: Array, default: null },
  /* The branch names pinned above the tree, as
     `settings.project.favoriteBranches` keeps them. Passed straight through for
     the reason the folders above are: what a mark does to the order is
     `branchTree.js`, and this panel is presentational on it. */
  favoriteBranches: { type: Array, default: () => [] },
  /* What `origin` is known to have, as plain names — the Origin tab of the
     branch list is drawn from it. Passed straight through like everything else
     here, including the decision that every one of them gets a row whether or
     not this repository has a branch of that name: that is `branchTree.js`'s.
     The caller is what guarantees the list belongs to the repository this panel
     is showing, since the store holds one such list for the whole project. */
  remote: { type: Array, default: () => [] },
  /* Which folders of that tab are unfolded, as
     `settings.project.remoteBranchFolders` keeps them. Its own prop and not the
     `branchFolders` above, because it is its own settings field — `feature` on
     one tab and `feature` on the other are two different rows. */
  remoteFolders: { type: Array, default: () => [] },
  /* Which of the two sides of the branch list is showing, as
     `settings.project.branchTab` keeps it. The tab row is drawn here, in the
     section's caption, and the choice is the caller's to hold: this panel is
     presentational on it as on every other piece of state. */
  branchTab: { type: String, default: DEFAULT_BRANCH_TAB },
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
  /* How many unmerged paths the selected repository has, where git is also
     part-way through a merge or a rebase — the store's `conflict` record, whose
     absence is the whole of when the button is not drawn. Not counted off
     `changes` here: a tree can be conflicted with neither operation in
     progress (a cherry-pick, a stash pop), and the dialog this opens would be
     wrong about both of its doors there. */
  conflicts: { type: Number, default: 0 },
  /* How the three sections are folded and how tall two of them were dragged to,
     as `settings.layout.gitSections` keeps it: `reposRows` and `branchRows` in
     rows, or null for "never dragged", plus a flag apiece. Global rather than
     per project, for the reason `settings.md` gives about the board's own view
     settings — how tall somebody likes their branch list is a habit of reading
     rather than a fact about one repository.

     `commitRows` rides in the same object, and it is the one count here that is
     never null: the field it sizes has a shipped height rather than a content
     to follow, so "never dragged" is two rows and not an absence. It travels
     out on the same `resize` event under `section: 'commit'`, which is what
     keeps one writer in `DesktopApp` instead of two.

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
  /* The second reader, and the widest thing this panel opens: the window that
     picks a reference branch and a branch to check, in every repository of the
     project at once, and puts an agent on the difference. Absent from
     `WRITE_REFUSED` below for `compare`'s reason — it asks git for nothing this
     panel then has to draw. */
  'review',
  /* The other item that never reaches git. It carries the resolved list rather
     than the name, exactly as `toggle-folder` does, so the rule lives in
     `branchTree.js` where a test can read it. It is absent from `WRITE_REFUSED`
     below for `compare`'s reason and a stronger one: it writes `settings.json`
     and nothing else. */
  'favorite',
  /* The name a person asked to put on the clipboard, and the one verb of this
     panel that reaches neither git nor `settings.json`. Absent from
     `WRITE_REFUSED` below for `compare`'s reason and a stronger one still: the
     only thing that can refuse it is the clipboard, which answers in a toast
     where the caller raised it. Whole, never the leaf — the string is wanted
     for a git command somewhere else. */
  'copy-name',
  'merge',
  'rebase',
  'new-branch',
  /* The name of the branch a person asked to rename. Like the delete below it
     the window that asks is the caller's to open, and what comes back from git
     lands in `writeError` under this table's own title. */
  'rename',
  /* The name of the branch a person asked to delete. The window that asks about
     it is the caller's to open, and what comes back from git lands in
     `writeError` under this table's own title like every other write. */
  'delete',
  'pull',
  'push',
  'fetch',
  'commit',
  'suggest',
  /* The way back into the conflict dialog, from the button `CommitBox` draws
     above the commit. Absent from `WRITE_REFUSED` below for `compare`'s reason:
     it opens a dialog and asks git for nothing this panel then has to draw. */
  'resolveConflicts',
  'message',
  'open',
  'toggle',
  'toggle-folder',
  /* The whole name of a branch only `origin` has. It reaches
     `vcs_checkout_remote`, whose refusal lands in `writeError` under
     `WRITE_REFUSED`'s `checkout` title like the local switch's — the store gives
     both the same `op`, since what was pressed is a checkout either way. */
  'checkout-remote',
  /* The whole new list for the Origin tab's folds, resolved by
     `branchTree.js`, exactly as `toggle-folder` carries the local one. Absent
     from `WRITE_REFUSED` below for `favorite`'s reason: it writes
     `settings.json` and nothing else. */
  'toggle-remote-folder',
  /* Which of the two tabs was pressed, by id. Absent from `WRITE_REFUSED` for
     the same reason again — it writes a preference, and it is a read either
     way, which is why the row goes on answering while a run holds every write
     in this panel. */
  'branch-tab',
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
  branchRows: props.sections?.branchRows ?? null,
  commitRows: props.sections?.commitRows ?? COMMIT_ROWS
}))

/* The box git's own stderr sits in. Mono and left-aligned rather than an
   `EmptyState`'s centred prose: this is machine output, and it is shown exactly
   as git wrote it. The two lines inside it are `failureStyle.js`'s and are
   shared with `DeleteBranchModal.vue`, which draws the same block inside the
   window that asked; only the box is this file's, because where it sits is the
   caller's business and what it looks like is not.

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
  /* A rename git declined, which is a name another branch already holds above
     all — `branch -m` refuses rather than writing over it, and `branchName.js`
     cannot see a branch the list has not caught up with. There is no second
     question for the window to ask, so the words land here like a checkout's. */
  rename: 'Git did not rename the branch',
  abort: 'Git did not abort',
  commit: 'Git did not commit',
  pull: 'Git did not pull',
  push: 'Git did not push',
  delete: 'Git did not delete the branch',
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

/* The name filter's own state, and the whole of it: what the field holds, what
   the rule sees, and whether the caption is a field at all. Refs here rather
   than anything reaching `settings.json`, for the reason this file's header
   gives — a query is a thing somebody is doing, not a thing they prefer.

   Two strings and not one, because they are two facts: `branchInput` is what
   was typed and has to answer the next keystroke immediately, and `branchQuery`
   is what 346 rows are rebuilt from. */
const branchSearching = ref(false)
const branchInput = ref('')
const branchQuery = ref('')
const filterField = ref(null)
/* The button the field replaces, held because closing has to give the focus
   back to it — see `closeFilter`. It is `v-if`'d away for as long as the field
   **stands**, which is not the same as for as long as the field **has the
   focus**: one Tab reaches the `x` beside it and a press on a tab leaves the
   caption altogether, with the field still there. So the `?.` is load-bearing
   rather than defensive — the restore runs after the field has gone and the
   button is back, but nothing here may assume where the focus was in between.
   That question is `focusInside`'s. */
const filterButton = ref(null)

/* The caption row of the branches section, held for one question only: was the
   focus inside what closing is about to take away. `SectionHeader` exposes its
   own row element, the same way the repositories' header above is read for its
   height. */
const branchesHeader = ref(null)

/* The step every change of state in this system is timed at, read off the root
   when it is wanted rather than once at import: the app-wide font size and
   `prefers-reduced-motion` both move it, and a value cached at module load
   would be the value the app started in. The parse is `branchTree.js`'s, since
   `getPropertyValue` answers in whatever unit the stylesheet was written in and
   that is the half worth a test. */
const filterWait = () =>
  filterDelay(getComputedStyle(document.documentElement).getPropertyValue('--dur-fast'))

let debounce = null
const settle = (value, wait) => {
  clearTimeout(debounce)
  debounce = setTimeout(() => {
    branchQuery.value = value
  }, wait)
}
watch(branchInput, (value) => settle(value, filterWait()))
/* A timer outliving the panel would write into a ref nothing is drawing any
   more. Beside the observer's own teardown, which is the same case. */
onBeforeUnmount(() => clearTimeout(debounce))

/* Where the list was standing when the field opened. A filter flattens 346 rows
   to three and then hands them back, and without this the list comes back at
   the top with whatever somebody had scrolled to gone — the same complaint the
   fold arithmetic above is careful about, one gesture down. */
let savedScroll = 0

const openFilter = () => {
  /* A field over a folded list would be filtering something nobody can see. The
     fold is the caller's state, so this asks for it rather than setting it. */
  if (!fold.value.branchesOpen) emit('toggle', 'branches')
  savedScroll = branchBox.value?.scrollTop ?? 0
  branchSearching.value = true
  nextTick(() => filterField.value?.focus())
}

/* Whether the focus is standing in something closing is about to take away:
   the caption row, which holds the field and the `x` inside it, or the list
   below, which holds the `Clear filter` button and is rebuilt from nothing the
   moment the query goes.

   **Deliberately not "is the focus in the `<input>`".** Two of the three ways
   out are presses on buttons that are not inside it — the `x` is its sibling in
   the plate and `Clear filter` is in the list — and where the focus stands
   during a press on a button is the one thing the engines disagree about:
   WebKit leaves it where it was, Blink moves it onto the button. A test written
   against the input alone would hand the focus back on one engine and drop it
   on `<body>` on the other, for the same press.

   Read **before** the state is cleared, because by the time the restore runs
   the elements this asks about have been unmounted. */
const focusInside = () => {
  const active = document.activeElement
  if (!active) return false
  return Boolean(
    branchesHeader.value?.el?.contains(active) || branchBox.value?.contains(active)
  )
}

/* Clearing and closing are one act, which is what the `x` does and what the
   `Clear filter` button under the empty state does: a field left open and empty
   is a caption that has stopped being one for no reason. */
const closeFilter = () => {
  /* Captured here and not in the callback: the answer is about the DOM as it
     stands now, and the callback runs after the field has gone. */
  const restore = branchSearching.value && focusInside()
  clearTimeout(debounce)
  branchInput.value = ''
  branchQuery.value = ''
  branchSearching.value = false
  nextTick(() => {
    if (branchBox.value) branchBox.value.scrollTop = savedScroll
    /* **The other half of a contract this panel opened.** Opening the field
       moves the focus into it, so closing has to put the focus somewhere — and
       closing unmounts the element holding it, which drops it on `<body>`. Half
       a focus contract is worse than none: the opening half is what teaches
       somebody that this control moves their caret for them. It goes back to
       the button that opened the field, which is the element standing where the
       field was and the one press away from opening it again. Every way out
       comes through here — `Esc` on an empty field, the `x`, `Clear filter`
       from under the empty state — so there is one answer and not three.

       **And a fourth caller that must not restore anything**: the watch below,
       which closes the field when the repository under the panel changes. That
       is not somebody leaving the field, it is the ground moving while they are
       somewhere else entirely, so a restore there would pull the caret into
       this panel in answer to a press in another one. Hence `restore` rather
       than a count of exits: what earns the focus back is having had it, not
       which line called.

       `preventScroll`, `NewTaskModal`'s own line for reaching a control this
       way: focusing an element lets the browser scroll every ancestor to bring
       it into view, and the statement above this one has just put the branch
       list back where it was. */
    if (restore) filterButton.value?.$el?.focus({ preventScroll: true })
  })
}

/* Escape empties a field that has something in it and closes an empty one,
   which is the two-press shape every filter field in every editor keeps: the
   first press undoes the typing, the second undoes the opening. The clear is
   written out rather than routed through the watch, so the list answers at once
   — a debounce on the way out would leave the old rows standing after the
   field they came from was empty. */
const onFilterKey = (event) => {
  if (event.key !== 'Escape') return
  event.preventDefault()
  if (branchInput.value) {
    clearTimeout(debounce)
    branchInput.value = ''
    branchQuery.value = ''
    return
  }
  closeFilter()
}

/* A query does not survive a change of repository, and a project switch reaches
   this panel as exactly that — `selectedRepo` is per project, so the path under
   the panel changes whichever of the two moved. A filter carried across would
   be a field somebody left open over one repository, answering about another
   with the same three letters in it.

   **This is the one caller of `closeFilter` that moves no focus**, and it is
   why the restore is gated on a condition rather than on the call: nobody left
   the field here, the ground moved under it, and the press that moved it was in
   another panel. */
watch(
  () => props.selected,
  () => {
    if (branchSearching.value || branchQuery.value) closeFilter()
  }
)

/* The hits of each side, both of them and always: the tab showing draws its
   own, and the other one's count is what its tab label reports. Local reads the
   branch list as it stands, so a hit carries `current` and takes its star from
   the stored favourites; Origin reads `originBranches`, which is the same list
   its unfiltered rows are built from — so a filtered row's `cloud`, its menu
   and the verb behind its double click cannot come apart from an unfiltered
   one's. */
const localHits = computed(() =>
  filterBranches(props.branches, branchQuery.value, { favorites: props.favoriteBranches })
)
/* The query is checked here rather than left to `filterBranches`, and it is
   not tidiness: the entry list is this call's *argument*, so it would be built
   in full before the rule could answer `[]` for a blank one — 593 rows rebuilt
   on every change of either branch list, with no filter anywhere on screen. The
   Local side needs no such guard, since its argument is the prop itself. */
const originHits = computed(() =>
  branchQuery.value.trim()
    ? filterBranches(originBranches(props.remote, props.branches), branchQuery.value)
    : []
)
const branchHits = computed(() =>
  props.branchTab === 'origin' ? originHits.value : localHits.value
)
const otherHits = computed(() =>
  props.branchTab === 'origin' ? localHits.value.length : originHits.value.length
)

/* The two sides of the branch list, labelled — `SegmentedTabs`' own shape,
   because that component is the row this app draws for a choice like this one
   and its own header says why a second copy of those style objects is the pair
   that drifts. The ids are `branchTree.js`'s closed list and are mirrored in
   `settings/model.rs`; the words beside them are `branchTabLabels`', sentence
   case like every other label in the app — and it is that rule and not this
   file that puts the counts in them while a filter is on.

   Drawn only while the section is unfolded: a folded section has no list for a
   tab to be about, and the caption's count goes on describing whichever side
   the folded list is still on — which is what that caption's own rule already
   promises about a fold. */
const branchTabs = computed(() =>
  branchTabLabels({
    tab: props.branchTab,
    query: branchQuery.value,
    localTotal: props.branches.length,
    originTotal: props.remote.length,
    localHits: localHits.value.length,
    originHits: originHits.value.length
  })
)

/* The field itself, inside the caption's row. It is a plate rather than a
   bordered control: `Input` is a `--control-h` box with a radius and a ring of
   its own, and one of those inside a `--row-h` caption would be a control
   standing in a row rather than the row having become a field. The surface is
   what says it is a field, and it reaches both edges of the panel for the same
   reason — a plate inset from an edge reads as something sitting on the row.

   The right inset is this row's own, which is why `SectionHeader` drops the
   gutter it adds for `actions` while the field is open: the `x` lands exactly
   where the `search` button it replaced was, and the surface underneath still
   reaches the panel's edge. */
const fieldRowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  flex: 1,
  minWidth: 0,
  height: '100%',
  padding: '0 var(--space-3) 0 var(--space-5)',
  background: 'var(--surface-raised)'
}
/* Mono, because what is being typed is half of an identifier — the same face
   the rows underneath draw their names in, so the query and the thing it is
   matching are legibly the same kind of string. No border of its own: the plate
   around it is the field, and a second box inside it would be two fields.

   **The focus ring is kept and pulled inside the element**, which is
   `AttachmentStrip`'s line for the same overflow and the one workaround here.
   `base.css` draws the ring a pixel *outside*, and this input is the height of
   the row it sits in — so an outside ring stands proud of the caption top and
   bottom and **overlaps** the hairline above and the tab row beneath, worst in
   the compact density where a row is at its shortest. Overlaps rather than is
   clipped: nothing near here has a non-visible overflow, so the ring draws
   whole and in the wrong place, which is the harder defect to spot. Suppressing
   it instead was tried and is wrong: the `x` button is inside this same plate,
   one Tab from here and one Shift+Tab back, so a field with no ring, no border
   and a transparent ground is a caret nothing on screen accounts for. Inset by
   the ring's own width it is whole, and it touches nothing above or below.

   **The two `sm` buttons beside it keep the stylesheet's default ring**, and
   that is left alone deliberately. They overlap the row the same way — a
   `--control-h-sm` control is 20px in a 22px compact row — and the inset here
   is bought by this element being the height of its row, which is a fact about
   this one field. Insetting a ring generally means naming the ring's own width,
   and there is no token for it: `--border-w-strong` matching `base.css`'s 2px
   is a coincidence this file leans on knowingly, not a rule. One answer for
   every focusable control in the app is a design-system question and not a
   component's to settle. */
const fieldStyle = {
  flex: 1,
  minWidth: 0,
  height: '100%',
  border: 'none',
  outlineOffset: 'calc(var(--border-w-strong) * -1)',
  background: 'transparent',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}
const FIELD_MARK = 12
const fieldGlyphStyle = { flex: 'none', color: 'var(--text-muted)' }

/* The tab row and the three verbs in one line. The tabs take whatever the
   verbs leave and shrink to it — `minWidth: 0`, or a flex item refuses to go
   below its own content and the buttons would be pushed off the end of a
   252px panel. The right inset is this row's own, since `SegmentedTabs` draws
   its padding inside itself and a button against the panel's edge is the
   defect `SectionHeader` insets its own slot for. */
const tabRowStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  paddingRight: 'var(--space-3)'
}
const tabsSlotStyle = { flex: 1, minWidth: 0 }
/* `--space-1` between the buttons, the same step `SectionHeader`'s own slot
   keeps and for its reason: with nothing between them a pair whose refused
   state is a filled, bordered chip fuses into one slab with a seam down the
   middle and reads as a segmented control rather than as two verbs. */
const verbsStyle = { display: 'flex', alignItems: 'center', gap: 'var(--space-1)', flex: 'none' }

/* The strip that says, once, that this repository is being held. The running
   palette rather than a neutral one: what it describes is work happening
   somewhere else, which is exactly what that status means everywhere else in
   the app — and deliberately not the needs-you amber, since nothing here is
   waiting on the person reading it. `--control-h-sm` rather than `--row-h`: it
   is a strip of chrome in the same wrapper as the tab row, not a row of the
   list, and reading as one would put a fourth surface in a section that has
   three. The sentence is clipped rather than wrapped, because a strip that
   grows to two lines under a run would move every section boundary below it —
   the whole of it is on the per-row tooltip either way. */
const freezeStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-2)',
  height: 'var(--control-h-sm)',
  padding: '0 var(--space-3)',
  background: 'var(--status-running-bg)',
  borderBottom: 'var(--border-w) solid var(--status-running-border)',
  color: 'var(--status-running-fg)',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-sans)',
  overflow: 'hidden'
}
/* The clipping is on the text and not on the strip around it: `text-overflow`
   acts on a block container's own inline content, and the span inside a flex
   row is a flex item — stated on the row, it would clip the sentence with no
   ellipsis to show for it. `minWidth: 0` for the reason every shrinking flex
   item in this file has it: without it the item refuses to go below its own
   content and there is nothing left to clip. */
const freezeTextStyle = {
  flex: '0 1 auto',
  minWidth: 0,
  whiteSpace: 'nowrap',
  overflow: 'hidden',
  textOverflow: 'ellipsis'
}
/* The strip's own spinner takes the strip's own colour rather than the
   `--attn-live` the buttons and the rows spin in. The two tokens are the same
   value today; naming the one the text beside it is drawn in is what keeps
   them the same thing if either ever moves. */
const freezeSpinStyle = {
  flex: 'none',
  color: 'var(--status-running-fg)',
  animation: 'sm-spin var(--dur-pulse) linear infinite'
}
/* What the strip says, which is `gitActions.js`'s own sentence with the state
   it leaves the panel in on the end. The clause is added here and not in the
   rule, because the rule is read by controls that are simply disabled — a
   tooltip over a dead button has no "read only" to report. */
const freezeLine = computed(() => `${props.actions?.reason ?? ''} · read only`)

/* What the caption counts, which is the list underneath it and never the other
   one. A number describing the half a person is not looking at is the whole of
   why this feature exists: the `origin` group this replaces carried its own
   count on its heading while the caption above went on saying how many local
   branches there were, so `Branches 236` sat over a list of 593. Null below two
   for the reason `SectionHeader` refuses a zero — a section with one row says
   everything about itself by drawing it.

   Null while the field is open, and that is not a hidden count but a caption
   that is no longer there: the row is a field, the number would have nowhere to
   sit, and the counts are in the tab labels underneath for exactly the length
   of time this one is not drawn. */
const branchCount = computed(() => {
  if (branchSearching.value) return null
  const total = props.branchTab === 'origin' ? props.remote.length : props.branches.length
  return total > 1 ? total : null
})

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

/* The rows of chrome a section cannot give away: one per caption on screen,
   plus the branch list's own tab row when there is one. That row is a
   `SegmentedTabs` and is **not** `--row-h` — it sizes itself from
   `--control-h-sm` and its own padding, which comes to about 1.18 rows in both
   densities — so its height is measured above and divided by a row here rather
   than counted as one. The fraction costs nothing: `available` is a fraction
   already, and `clampRows` floors the ceiling. Folded, the tab row is not drawn
   and `tabsPx` is 0, which is the same rule the captions keep about the lists
   under them. */
const branchTabsDrawn = computed(() => branchesDrawn.value && fold.value.branchesOpen)
const headerRows = computed(
  () => drawn.value.length + (rowPx.value ? tabsPx.value / rowPx.value : 0)
)

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
/* The branch list's tab row, measured rather than told. `SegmentedTabs` sizes
   itself from `--control-h-sm` and its own padding, which is not `--row-h` and
   is not meant to be — so the arithmetic below asks how tall it actually is and
   divides by a row, exactly as it divides the panel. The alternative was a
   bespoke row pinned at `--row-h`, which is thirteen declarations copied from
   that component with nothing mechanical holding the copies together. A
   fraction is perfectly at home here: `available` is already one, and the
   ceiling is floored at the end. */
const tabBox = ref(null)

const rowPx = ref(0)
const panelPx = ref(0)
const tabsPx = ref(0)
const measure = () => {
  rowPx.value = reposHeader.value?.el?.getBoundingClientRect().height ?? 0
  panelPx.value = panel.value?.getBoundingClientRect().height ?? 0
  tabsPx.value = tabBox.value?.getBoundingClientRect().height ?? 0
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
  if (tabBox.value) observer.observe(tabBox.value)
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
    headers: headerRows.value,
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
    headers: headerRows.value,
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
          :rows="fold.commitRows"
          :conflicts="conflicts"
          @update:model-value="$emit('message', $event)"
          @commit="$emit('commit')"
          @suggest="$emit('suggest')"
          @resolve-conflicts="$emit('resolveConflicts')"
          @resize="$emit('resize', { section: 'commit', rows: $event })"
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
        <!-- The caption carries a chevron, a word, a count and the one button
             that turns the whole row into a field. The three verbs that used to
             be in its `actions` slot are one row down, at the end of the tab
             row: this strip is 252 pixels wide, and the row under it already
             exists and is already measured — a filter field needs the width of
             a caption, which is why it takes the caption's place rather than
             standing beside it. -->
        <SectionHeader
          ref="branchesHeader"
          divided
          label="Branches"
          :count="branchCount"
          :open="fold.branchesOpen"
          :searching="branchSearching"
          @toggle="emit('toggle', 'branches')"
        >
          <template #actions>
            <!-- Gone while the field is open, and the `x` inside the field is
                 what takes its place — two controls for opening and closing one
                 thing, in one row, would be a row saying it twice. `Button` and
                 not `IconButton` for the reason the three verbs below give: this
                 sits in a 252px caption and a tooltip of its own would open over
                 the list it is about. -->
            <Button
              v-if="!branchSearching"
              ref="filterButton"
              variant="ghost"
              size="sm"
              icon="search"
              :aria-label="BRANCH_FILTER_LABEL"
              @click="openFilter"
            />
          </template>
          <!-- The row as a field: the glyph that says what it is, the input,
               and the `x` that clears and closes in one press. All three inside
               the caption's own `--row-h`, so nothing in the arithmetic over
               this panel notices that a caption became a field. -->
          <template #editor>
            <div :style="fieldRowStyle">
              <Icon name="search" :size="FIELD_MARK" :style="fieldGlyphStyle" />
              <input
                ref="filterField"
                v-model="branchInput"
                type="text"
                :placeholder="BRANCH_FILTER_LABEL"
                :aria-label="BRANCH_FILTER_LABEL"
                :style="fieldStyle"
                @keydown="onFilterKey"
              />
              <Button
                variant="ghost"
                size="sm"
                icon="x"
                aria-label="Clear filter"
                @click="closeFilter"
              />
            </div>
          </template>
        </SectionHeader>
        <!-- The two sides and the three verbs, in one row directly under the
             caption.

             `SegmentedTabs` and not a row written here, so the two tab rows in
             this app are one control rather than two that have to be kept
             looking alike — the reason that component's own header gives for
             existing at all. It sizes itself from `--control-h-sm` and its own
             padding, which is not `--row-h`, so the wrapper is what
             `headerRows` measures rather than a height this file asserts. The
             buttons are `sm` and sit inside that height, so moving them here
             changed no measurement at all.

             The tabs are live while a run holds every write in this panel:
             choosing which side of the repository to look at is reading, the
             same rule that keeps a folder heading pressable. -->
        <div v-if="branchTabsDrawn" ref="tabBox" :style="{ flex: '0 0 auto' }">
          <div :style="tabRowStyle">
            <div :style="tabsSlotStyle">
              <SegmentedTabs
                :tabs="branchTabs"
                :model-value="branchTab"
                @update:model-value="$emit('branch-tab', $event)"
              />
            </div>
            <div :style="verbsStyle">
              <!-- First of the three and the only one always here: it is the
                   question the other two are answers to, and with both of them
                   refused over a branch that is level it is the whole of what
                   this row can still do. On a detached HEAD it is the only
                   verb drawn, and under a run it is the only one live. -->
              <Tooltip v-bind="hintProps(check)">
                <!-- Two buttons and not one with a `v-if` inside its slot,
                     which is the version this shipped as and the defect it
                     shipped with. `Button` draws its slot as
                     `<span v-if="$slots.default">`, and a slot **function** is
                     there whether or not the `v-if` inside it renders anything:
                     the empty span stayed a flex child, the button spent its
                     `gap` on nothing and came out 6px wider than the two arrows
                     beside it — then snapped back to their width the moment a
                     fetch started, sliding both arrows sideways. Interaction is
                     a surface step and never a shift. Handing the slot over
                     only in the state that fills it is what keeps all three
                     buttons one width. -->
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
            </div>
          </div>
          <!-- Said once, for the eye. Every row below is muted and inert and
               carries the same sentence on a tooltip, which answers the pointer
               and the keyboard and answers nothing to somebody glancing at a
               panel of grey rows. Inside this wrapper on purpose: `tabsPx`
               already measures it, so a strip that appears and goes with a run
               moves the section arithmetic with it and needs no observer of its
               own. -->
          <div v-if="actions && actions.allowed === false" role="status" :style="freezeStyle">
            <Icon name="loader-circle" :size="SPIN" :style="freezeSpinStyle" />
            <span :style="freezeTextStyle">{{ freezeLine }}</span>
          </div>
        </div>
        <div v-if="fold.branchesOpen" ref="branchBox" :style="branchStyle">
          <BranchList
            :branches="branches"
            :tracking="tracking"
            :folders="branchFolders"
            :favorites="favoriteBranches"
            :remote="remote"
            :remote-folders="remoteFolders"
            :tab="branchTab"
            :query="branchQuery"
            :hits="branchHits"
            :other-hits="otherHits"
            :detached="detached"
            :fetching="fetching"
            :actions="actions"
            :busy="busy"
            @checkout="$emit('checkout', $event)"
            @fetch="$emit('fetch')"
            @compare="$emit('compare', $event)"
            @review="$emit('review', $event)"
            @favorite="$emit('favorite', $event)"
            @copy-name="$emit('copy-name', $event)"
            @merge="$emit('merge', $event)"
            @rebase="$emit('rebase', $event)"
            @new-branch="$emit('new-branch', $event)"
            @rename="$emit('rename', $event)"
            @delete="$emit('delete', $event)"
            @toggle-folder="$emit('toggle-folder', $event)"
            @checkout-remote="$emit('checkout-remote', $event)"
            @toggle-remote-folder="$emit('toggle-remote-folder', $event)"
            @clear-filter="closeFilter"
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
