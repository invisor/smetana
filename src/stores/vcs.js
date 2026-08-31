/* The Git panel: the project's repositories, which of them is selected, and the
   uncommitted files in it. The file in this directory that knows Tauri exists
   — see the list in CLAUDE.md rather than a number written here, since an
   ordinal is written once and the list keeps growing under it.

   Beside git.js rather than inside it, mirroring the split on the Rust side:
   git.js is the branch in the scope bar, read straight off `HEAD` with no
   process behind it, and everything here needs the git binary (`src-tauri/src/vcs/`).

   No worker and no watcher: `git status` costs tens of milliseconds and this
   store holds the list, so freshness is window focus, a project switch and the
   panel's own refresh button — the same answer the file tree gives. The price
   is named rather than discovered: while an agent works in the repository, this
   list is as stale as the tree beside it is. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
/* The scope bar's branch is git.js's, read straight off `HEAD` with no process
   — and a checkout here is the one moment in the app that changes it from the
   inside. Freshness there is window focus and the project switch, neither of
   which a person switching branches in this panel ever reaches, so the write
   refreshes it itself: the alternative is a bar naming the branch somebody just
   left, until they alt-tab away and back. */
import { loadHead } from './git.js'
import { settings } from './settings.js'
/* The one rule this store shares with the button that starts it — see `push`
   below. A pure module out of `components/`, which is what `runs.js`,
   `notifications.js` and `settings.js` already reach into that family for: the
   rules live beside the part of the interface they are about, and nothing in
   them is Vue. */
import { publishes } from '../components/git/tracking.js'
/* The other rule out of that same family, and here for the same reason: what
   the panel holds about a conflicted tree is joined from two sources that know
   different things, and the whole of that join is a pure module a test can
   reach. */
import { conflictRecord } from '../components/git/conflictRecord.js'

export const vcsState = reactive({
  /* The project the rest of this object is about. Also the guard token: every
     call claims it before its first await and checks it after. */
  project: null,
  repos: [],
  /* The folders one level below the project root that git can see and
     `[project].repos` does not name — names and never paths, because there is
     nothing in the app that can be done to one of them: the panel points at it
     and offers the setup agent, which is the only thing here that writes that
     file.

     Empty is the ordinary answer and means nothing needs saying. It arrives
     with `repos` in one call and is cleared everywhere `repos` is cleared, so a
     read that failed never leaves a sentence standing on screen about a folder
     nobody looked at. A project with no configuration always answers with
     nothing, by construction — `vcs/repos.rs` says why. */
  unlisted: [],
  /* The selected repository's absolute path, which is the argument every
     command in `vcs/` takes. */
  selected: null,
  /* The working tree of the selected repository, or null when it could not be
     read — never an empty tree standing in for a failure, the rule
     `cleanup::refusal` and `projectBytes` keep: an unread list and a clean one
     are opposite facts and the panel says different things about them. */
  tree: null,
  /* The selected repository's local branches, in the order `git::by_recency`
     gave them and never re-sorted here: the branch somebody merges into every
     day is nowhere in particular alphabetically. `[{ name, current }]`, and an
     empty list is an ordinary answer — a folder git can see nothing in. Not a
     repository without a first commit, which has no ref on disk and still
     answers with one branch: `git.rs` puts HEAD's own name into the list, so
     that a repository nobody has committed to still has something to merge
     into. */
  branches: [],
  /* What `origin` is known to have, as of the last fetch: plain names, sorted,
     with the `origin/` already off — `vcs_remote_branches` answers that way, and
     whoever builds a ref for git is the side that puts the prefix back.

     Beside `branches` rather than folded into it, which is the seam Rust is
     split on too: a remote branch and a local one of the same name are two
     different things to check out or to read a diff against, and one list
     holding both could not say which a name was.

     Loaded on request rather than with the panel, and by nothing on screen yet:
     the branch review window is what asked for it, and the Git panel itself has
     no use for a list of what the remote has. An empty list is the ordinary
     answer for a repository nobody has fetched into. */
  remoteBranches: [],
  /* When each of those branches was last moved here, keyed by name: epoch
     seconds, or null for a branch nothing on this machine has a record of —
     everything in a fresh clone, where the whole of `origin` is packed with no
     reflog behind it. `vcs_remote_branches` answers with the pair and this is
     the half the names above are not.

     Keyed rather than carried on the names, and that is the compatibility this
     field exists for: `remoteBranches` is a list of **strings** that two
     callers already spread and hand to a `Dropdown`, and records in its place
     would break both of them. A reader that wants an age has a name in its hand
     anyway, exactly as `tracking` above assumed.

     A name with no entry and a name whose entry is null mean the same thing —
     nothing is known about its age — and the caption draws neither. */
  remoteBranchTimes: {},
  /* When that repository last fetched: epoch seconds, or null. The modification
     time of its `FETCH_HEAD`, which is what `vcs_last_fetch` reads and the one
     record of a fetch that outlives the app — `fetchedAt` below is a throttle
     and goes with the process.

     Beside the two lists above rather than keyed by repository, because it is
     part of the same one answer: it is about `remoteBranchesRepo` and about no
     other repository, and it is read and let go with the names it was asked
     for. Null is an ordinary answer — nobody has fetched here, the folder is
     not a repository, the file could not be read — and all three draw no time
     rather than a stale one. */
  remoteFetchedAt: null,
  /* Which repository the list above is about, or null when it is empty. Every
     other list in this object is about `selected` and needs no such field;
     this one is about whatever repository a caller named, which is a fact
     nothing else here records — so without it a reader holding a path has no
     way to ask whether the list in front of it is the one it asked for.

     Written in the same assignment as the list itself, never before it and
     never after: the four are one statement, and a field naming a repository
     whose answer has not landed yet is the very lie it exists to prevent. What
     that reaches is everything asked for in one go — the names, their stamps
     and the fetch time — since a stamp left standing beside another
     repository's names is that same lie one field along. */
  remoteBranchesRepo: null,
  /* Where each local branch stands against its upstream, keyed by branch name:
     `{ upstream, ahead, behind, gone }` as `vcs_tracking` answers. Keyed rather
     than listed because every reader has a name in its hand — a row, or the
     current branch — and none of them wants the order.

     A second answer beside `branches` rather than a richer `branches`, which is
     the seam Rust is split on: the branch list is three file reads that cannot
     fail, this is a process that can. An empty object is "nothing is known",
     which is what a repository with no remote, a git that could not be run, and
     a fetch that has not happened yet all look like — and all three draw the
     same row, which is the one from before this feature existed. */
  tracking: {},
  /* `{ kind, message }` — Rust's own shape, normalised here so a rejection that
     is a bare string (the browser mock, a transport failure) draws the same
     way. `kind` is what the panel branches on; the message is git's own words
     and is shown untouched. */
  error: null,
  /* Git's refusal of a write — a checkout, a merge, a rebase — in the same
     `{ kind, message }` shape with the `op` that earned it, and kept apart from
     `error` above deliberately: that one says the working tree could not be
     read, this one says it was read and git declined to change it. Folding them
     together would put a refusal about a branch where the changes should be,
     and take the list down with it.

     One field for the three writes rather than one apiece: they cannot happen
     at once (`busy` is what says so), only one of them can have last been
     refused, and two fields for one fact are two that drift. The `op` is what
     lets the panel name which write it was without a second copy of that
     knowledge. */
  writeError: null,
  /* What git is doing right now — `{ op, branch }` — or null. One at a time:
     the panel goes inert while git works, so a second row cannot be pressed
     into an operation git is already in the middle of for the first. `op` is
     `checkout`, `merge`, `rebase`, `create`, `commit`, `abort`, `pull` or
     `push`, and the branch is the row it was asked for, which is where the
     spinner goes. The last two are about the current branch and carry its name
     for that reason: they leave from the section header rather than from a row,
     and the row they are about is the one with the tick. */
  busy: null,
  /* A merge or a rebase this repository is part-way through, with unmerged
     paths in the tree, which is an outcome and not a failure — hence its own
     field beside `writeError` rather than in it.
     `{ repo, op, ours, theirs, files }`: the repository it happened in, which
     of the two operations it was, the branch this repository was on, the branch
     that was being brought in, and every path git left unmerged.

     **Two sources fill it and they know different things.** A merge or a
     rebase this app ran answers `conflict` and records both branches, since
     the person picked the second one off a row. Everything else — a tree left
     conflicted before the app started, a merge run in a terminal, an agent's
     session in the same tree — is read afterwards by `vcs_in_progress`, which
     is exact about the operation and best-effort about the names.
     `components/git/conflictRecord.js` is the rule that joins them.

     It is a fact about the **selected** repository and is re-derived on every
     status read, which is what makes the button honest: it stands for exactly
     as long as the tree is conflicted, and goes when the tree is not. */
  conflict: null,
  /* Whether the dialog is on screen, which is a different question from
     whether there is a conflict.

     They used to be one field, and that is what made resolving with an agent a
     one-way door: taking the dialog down meant forgetting the tree, so a
     session that stopped early left a conflicted repository with nothing to
     press. Now the record stays and this flag falls — a merge this app ran
     raises it, the panel's own `Resolve conflicts` button raises it again, and
     closing lowers it. */
  conflictOpen: false,
  /* Git's refusal of the abort, drawn **inside** that modal. Its own field for
     one reason: the message belongs to the dialog the abort was pressed in,
     and one put in the panel behind it would sit under a dialog that is still
     open. */
  conflictError: null,
  /* The commit message somebody is part-way through writing, by repository
     path. Kept per repository because a project is often several of them and
     the messages are about different work; kept **here** and not in
     `settings.json` because it is a draft and not a preference — the file holds
     what somebody chose about the app, and a half-typed sentence restored three
     days later is not that.

     A key appears on the first keystroke and the whole object goes on `reset`,
     so it is bounded by the repositories of one project. */
  messages: {},
  /* Whether the agent is being asked for a message right now, and its refusal
     if it had one. Deliberately **not** `busy` and **not** `writeError`:
     generating writes nothing to the tree, so the branch rows must stay live
     under it, and "Git refused this operation" over a failure that never
     reached git would name the wrong party. */
  suggesting: false,
  suggestError: null,
  /* Whether a fetch somebody pressed for is out right now. Deliberately not
     `busy`, for the reason `suggesting` beside it is not: `busy` is what holds
     the branch rows inert, and a fetch writes remote-tracking refs alone — the
     rows it would freeze are rows it cannot affect. It is the spinner on one
     button and nothing else.

     The background sweep does **not** set it. That one is silent by design and
     a spinner is the loudest thing a caption has; a panel that started
     twirling on its own every five minutes would be reporting a decision the
     person did not make. */
  fetching: false,
  loading: false
})

/* How many files in the selected repository are uncommitted — the status
   footer's uncommitted-files counter. Every kind of change counts as one file:
   staged, unstaged, untracked and conflicted alike, because that is exactly
   what the panel's own change list draws, so the number in the strip is the
   number of rows in the panel and a person can check it by looking rather than
   by counting in their head. A counter that quietly left the untracked files out
   would be a difference nobody could explain without a tooltip.

   `null`, never `0`, for a tree that could not be read — the rule `tree` itself
   keeps above. An unread tree and a clean one are opposite facts, and the strip
   answers a clean one by drawing nothing and an unread one by drawing nothing
   either; what it must not do is claim a repository with unknown contents is
   tidy. The counter is hidden for anything not greater than zero, so `null` is
   all it takes.

   It is the *selected* repository and not the whole project, which is a
   decision rather than an omission: a sum over every repository means a
   `git status` apiece, with its own freshness and its own failures, and this
   number is meant to be the one already on screen. As fresh as that list and
   no fresher — there is no watcher here (see the header), so while an agent
   works in the repository this ages with the tree beside it.

   A computed in the store rather than in the view for the mechanical reason
   `runs.js` keeps `needsSetup` here: no test in this repository can reach a
   `.vue` file, so a rule living in one is covered by nothing. */
export const dirtyCount = computed(() => (vcsState.tree ? vcsState.tree.changes.length : null))

/* A rejection becomes something the panel can both branch on and print. The
   message is never rewritten: for a non-zero git it is git's own stderr, and
   the person reading it knows git better than any sentence written here. */
function asError(err) {
  if (err && typeof err === 'object' && typeof err.message === 'string') {
    return { kind: typeof err.kind === 'string' ? err.kind : 'io', message: err.message }
  }
  return { kind: 'io', message: String(err) }
}

/* How often this app may go to a remote on its own, per repository.

   A constant and not a setting: what a person actually wants to decide is
   whether this happens at all (`settings.git.autoFetch`), and a number of
   minutes in the settings window would be a control nobody can answer well.
   Five is short enough that a branch somebody else pushed to is orange by the
   time it matters and long enough that alt-tabbing does not open a socket every
   few seconds. */
const FETCH_EVERY_MS = 5 * 60 * 1000

/* When each repository was last fetched, by path, and the call still out for
   it. In memory and never in `settings.json`: it is a fact about this session,
   and a stored timestamp would mean a machine that was asleep for a week comes
   back believing it is current.

   The second is the promise and not a flag, which costs nothing here and buys
   the one case a flag answered badly: a press landing while the background
   sweep is still out. A flag makes that press a no-op — the guard refuses it,
   nothing spins, nothing is said — which is exactly the kind of control this
   button was added to remove. Holding the promise lets the press join the call
   already running: it spins over somebody else's fetch and answers when that
   one answers, which is what it would have done with a fetch of its own. */
const fetchedAt = new Map()
const fetching = new Map()

/* Which repository to show, out of the list that has just arrived.

   The remembered path is a hint and never the truth — the rule columnOrder.js
   states for a stored status bd no longer has. A repository that has since been
   removed from `[project].repos`, or renamed, is passed over in silence and the
   first one is shown, because a panel refusing to draw over a choice made a
   week ago would be an error about nothing a person did today. */
function pickRepo(repos, remembered) {
  if (repos.some((repo) => repo.path === remembered)) return remembered
  return repos[0]?.path ?? null
}

/* The project's repositories, and the working tree of whichever is selected.

   Guarded against its own stale response exactly as git.js, terminals.js and
   runs.js are: two calls can be in flight with no ordering guarantee on which
   invoke resolves first, and without the guard the last response would win
   rather than the last call — one project's files listed under another
   project's name, with every row in the panel then naming the wrong
   repository. */
export async function loadRepos(project) {
  /* A conflict belongs to a repository of one project, and both its doors act
     on that project: the abort names that repository, and the agent is started
     there. Somebody who has moved on gets neither, so the record and the dialog
     go with the project rather than hanging over the next one. `refresh()`
     comes back through here with the same project and leaves it standing, which
     is what lets a conflict survive the refresh that follows the merge that
     made it — and `loadStatus` re-derives it below in any case. */
  if (project !== vcsState.project) {
    clearConflict()
    /* The drafts go with the project they were about. `refresh()` comes back
       through here with the same project and leaves them standing, which is
       what lets a message survive the refresh a failed commit is followed
       by. */
    vcsState.messages = {}
    vcsState.suggestError = null
  }
  /* Assigned before every `await` in this function. On the load path below —
     and only there, since a falsy `project` returns through `reset()`, which
     lowers it again — `loading` is raised in the same breath and stays raised
     across `vcs_repos` → `selectRepo` → `loadStatus`, a handoff with no gap
     where it reads false. Both are relied on outside this store: the Git tab
     decides whether the count it can see is about the project being arrived at
     or about the one being left (`components/git/changesFold.js`), and this
     pair is the whole of how it tells. Note what is deliberately **not** done
     here — the tree is left standing rather than cleared, so that a panel does
     not blink through an empty list on every switch, which is exactly why that
     reader cannot trust `tree` alone. */
  vcsState.project = project
  if (!project) {
    reset()
    return
  }
  vcsState.loading = true
  try {
    /* `{ repos, unlisted }` and not a bare list: the two halves are read off
       one directory listing in the same breath, and asking for them separately
       would let a clone made between the two calls be in one answer and not the
       other. */
    const answer = await invoke('vcs_repos', { project })
    if (vcsState.project !== project) return
    vcsState.repos = answer.repos ?? []
    vcsState.unlisted = answer.unlisted ?? []
    /* Cleared here and not only in the catch and in `reset()`, which between
       them cover a read that failed and a window left with no project at all —
       and leave out the ordinary case, moving from one project to another,
       since `reset()` runs only when the new project is falsy. Without this a
       repository of the project being left goes on holding this list until
       somebody asks again, which is a leak across the very boundary the guard
       above exists to keep. `branches` is short of nothing here only because
       `selectRepo` → `loadBranchList` runs on every one of these and either
       replaces that list or empties it; this path calls `loadRemoteBranches`
       nowhere, by design — that loader is asked for a repository of somebody
       else's choosing, not for the one the panel settled on. */
    forgetRemoteBranches()
    await selectRepo(pickRepo(vcsState.repos, settings.project.selectedRepo))
  } catch (err) {
    if (vcsState.project !== project) return
    /* `vcs_repos` answers with a list for anything it can read and cannot
       refuse today, so reaching here means the call itself failed — a transport
       fault now, and whatever that command grows into a `Result` for later.
       `GitPanel` draws this message **in place of** the repository list, which
       is what the empty list beside it requires: "No repositories here" is a
       statement about a folder that was read, and this one was not. The raw
       text stays in the console. */
    console.error('[vcs] listing repositories failed:', err)
    vcsState.repos = []
    /* With the list gone, so is what was said about the folders beside it: a
       sentence naming a repository the configuration does not hold is a
       statement about a directory that was read, and this one was not. */
    vcsState.unlisted = []
    vcsState.tree = null
    /* With no repository left to be about, a branch list read a moment ago is
       one nothing on screen names. Both lists, for the one reason. */
    vcsState.branches = []
    forgetRemoteBranches()
    vcsState.error = asError(err)
  } finally {
    if (vcsState.project === project) vcsState.loading = false
  }
}

/* Show this repository, and remember it for the next visit.

   The write lands in the settings object the whole app shares and reaches disk
   through the same 400 ms debounce a panel drag uses — this store never calls
   `settings_save` itself, since the main window writes the whole file. */
export async function selectRepo(path) {
  vcsState.selected = path
  settings.project.selectedRepo = path
  /* Git's last refusal went with the repository it was about: a message saying
     a branch is checked out in another worktree, left standing over the branch
     list of the repository next door, would be a statement about neither. The
     conflict is deliberately not cleared here either, and for a different
     reason: `loadStatus` below re-derives it for whichever repository is now
     selected, so clearing it here would only make the panel blink through a
     state it is about to answer properly. */
  vcsState.writeError = null
  await Promise.all([loadStatus(), loadBranchList(), loadTracking()])
}

/* The selected repository's working tree.

   The guard is the pair of them, project and path: switching repository inside
   one project has the same race as switching project, and a status arriving
   after somebody moved on would put one repository's files under another
   repository's name. */
async function loadStatus() {
  const { project, selected } = vcsState
  if (!selected) {
    vcsState.tree = null
    vcsState.error = null
    return
  }
  vcsState.loading = true
  try {
    const tree = await invoke('vcs_status', { repo: selected })
    if (vcsState.project !== project || vcsState.selected !== selected) return
    /* **Replaced, never written into.** Every answer is a new object, which is
       what lets a reader watch the identity and see an answer that changed
       nothing — a switch between two projects with the same number of changes
       is otherwise indistinguishable from no answer at all. Both failure arms of
       this function leave `null` for the same reason `dirtyCount` is `null` and
       never `0`: not knowing is not a clean tree. */
    vcsState.tree = tree
    vcsState.error = null
    await readConflict(project, selected, tree)
  } catch (err) {
    if (vcsState.project !== project || vcsState.selected !== selected) return
    vcsState.tree = null
    vcsState.error = asError(err)
  } finally {
    if (vcsState.project === project && vcsState.selected === selected) vcsState.loading = false
  }
}

/* What the tree alone cannot say: which operation left it conflicted, and the
   branches it is between.

   **Asked only of a tree that already shows unmerged paths.** A clean
   repository pays no process for it, which matters because this runs on every
   window focus, and it also means the common answer costs an `Array.filter`
   and nothing else.

   Guarded on the same `(project, selected)` pair as the read above and for the
   same reason: an answer arriving after somebody moved on would put one
   repository's operation under another repository's name.

   A probe that failed leaves the record alone rather than clearing it: git
   declining to answer is not git saying there is nothing, and the honest cost
   of the other arm is a button that vanishes under the pointer. */
async function readConflict(project, selected, tree) {
  const files = tree.changes.filter((change) => change.kind === 'conflicted').map((c) => c.path)
  if (!files.length) {
    clearConflict()
    return
  }
  let progress = null
  try {
    progress = await invoke('vcs_in_progress', { repo: selected })
  } catch (err) {
    console.error('[vcs] asking git what is in progress failed:', err)
    return
  }
  if (vcsState.project !== project || vcsState.selected !== selected) return
  const record = conflictRecord({ repo: selected, files, progress, previous: vcsState.conflict })
  vcsState.conflict = record
  if (!record) vcsState.conflictOpen = false
}

/* The record and everything drawn from it, gone: a tree that is not conflicted
   any more, or a project that is not this one. Deliberately not
   `dismissConflict`, which now only closes. */
function clearConflict() {
  vcsState.conflict = null
  vcsState.conflictError = null
  vcsState.conflictOpen = false
}

/* The selected repository's branches.

   Guarded on the same pair as `loadStatus` and for the same reason: a list
   arriving after somebody moved on would offer one repository's branches under
   another repository's name, and the row a person then clicked would check out
   a branch in a repository they are not looking at.

   `loading` is deliberately not touched here. It says a first read is in
   flight, and two functions setting one boolean means whichever finishes first
   clears it under the other; this read costs no process at all — three file
   reads through `git.rs` — so there is nothing for a person to wait through. */
async function loadBranchList() {
  const { project, selected } = vcsState
  if (!selected) {
    vcsState.branches = []
    return
  }
  try {
    const branches = await invoke('vcs_branches', { repo: selected })
    if (vcsState.project !== project || vcsState.selected !== selected) return
    vcsState.branches = branches
  } catch (err) {
    if (vcsState.project !== project || vcsState.selected !== selected) return
    /* `vcs_branches` answers with a list for everything it can read and refuses
       nothing, so reaching here means the call itself failed. An empty list is
       what a folder outside git already produces, and the same read failing for
       `vcs_status` beside it is what puts a message on screen. */
    console.error('[vcs] listing branches failed:', err)
    vcsState.branches = []
  }
}

/* The repository `loadRemoteBranches` was last asked about, which is the second
   half of that read's guard and nothing else — never what the list in the store
   is about, which is `vcsState.remoteBranchesRepo`. The two differ for exactly
   the length of one call, and that gap is the whole race: while an answer is in
   flight this names the repository somebody wants and the store still names the
   one it has.

   Module state rather than a field, for the reason `fetchedAt` above is: it is
   about a call in flight and not about anything drawn, and a reactive field
   nothing reads is a field the next reader has to work out the meaning of.
   Deliberately not lowered by `reset()` or by a project switch — a stale answer
   crossing one of those is what the project half of the guard is for, and a
   fresh call raises this again before its own await in any case. */
let remoteBranchesAsked = null

/* Let go of everything the last `loadRemoteBranches` answered with.

   One function rather than the four lines written out in each of the five
   places that drop this answer, and it is the mechanical half of the rule the
   fields above state: the names, their stamps, the fetch time and the
   repository all four are about are one statement, so a place that cleared
   three of them would leave one repository's stamps standing under the next
   repository's name — the very defect `remoteBranchesRepo` was added for, one
   field along. Adding a fifth field to this answer means adding it here, and
   nowhere else. */
function forgetRemoteBranches() {
  vcsState.remoteBranches = []
  vcsState.remoteBranchTimes = {}
  vcsState.remoteFetchedAt = null
  vcsState.remoteBranchesRepo = null
}

/* What `origin` is known to have in one repository, as of the last fetch.

   The repository is an argument rather than `vcsState.selected`, and that is
   what this read is for: the caller naming it has chosen a repository of its own
   and need not be looking at the Git panel at all.

   Guarded on the pair, project and repository, exactly as `loadStatus`,
   `loadBranchList` and `loadTracking` are — a repository changing under one
   project is the same race as the project changing, and this store answers it
   the same way everywhere. What differs is only where the second half of the
   pair is read from. Those three take it off `vcsState.selected`, which
   `selectRepo` writes synchronously before any await; nothing writes such a
   token for this loader, because the repository never comes from the store at
   all, so `remoteBranchesAsked` below is it.

   `loading` is untouched, for `loadBranchList`'s reason: this read costs no
   process at all — files off the disk through `git.rs` — so there is nothing for
   a person to wait through, and two functions setting one boolean means
   whichever finishes first clears it under the other.

   **Two commands and one answer.** `vcs_last_fetch` is asked here rather than by
   a loader of its own because it is a fact about this same repository at this
   same moment, and the four fields it lands in are guarded as one: a fetch time
   read on its own schedule would be the one thing in this answer free to be
   about another repository. Together rather than in turn, since neither waits on
   the other, and a failure in either drops both — the two are one read of one
   repository, and half of it is not a state this store keeps. */
export async function loadRemoteBranches(repo) {
  const { project } = vcsState
  remoteBranchesAsked = repo || null
  if (!repo) {
    forgetRemoteBranches()
    return
  }
  try {
    /* `lastFetch` and not `fetchedAt`, which is the throttle `Map` at the top
       of this file — the very thing the field's own comment contrasts this with.
       Shadowing it here breaks nothing today and would hand the next edit that
       reached for `fetchedAt.get(repo)` inside this function a number. */
    const [branches, lastFetch] = await Promise.all([
      invoke('vcs_remote_branches', { repo }),
      invoke('vcs_last_fetch', { repo })
    ])
    if (vcsState.project !== project || remoteBranchesAsked !== repo) return
    /* The names on their own, which is what this field has always held and what
       two callers spread into a `Dropdown`'s options; the stamps arrive beside
       them keyed by name rather than in their place. All four assignments are
       one statement — see `remoteBranchesRepo`. */
    vcsState.remoteBranches = branches.map((branch) => branch.name)
    vcsState.remoteBranchTimes = Object.fromEntries(
      /* `at` is the field's name on the wire — `git::RemoteBranch` renames it
         there, and the epic's contract is what both lists a person picks a
         branch from are read by. */
      branches.map((branch) => [branch.name, branch.at ?? null])
    )
    vcsState.remoteFetchedAt = lastFetch ?? null
    vcsState.remoteBranchesRepo = repo
  } catch (err) {
    if (vcsState.project !== project || remoteBranchesAsked !== repo) return
    /* Neither command refuses anything — both answer with what they could read
       — so reaching here means a call itself failed. An empty list is what a
       repository nobody has fetched into already gives, and it is the right
       thing to be left holding: a list of branches that may no longer be there
       is worse than none, and a fetch time with no list under it would be a
       freshness claimed about nothing. The raw text stays in the console, where
       the failure of a read nothing on screen announces belongs. */
    console.error('[vcs] listing remote branches failed:', err)
    forgetRemoteBranches()
  }
}

/* Where the selected repository's branches stand against their upstreams.

   Guarded on the pair, project and repository, exactly as `loadStatus` is: a
   list arriving after somebody moved on would mark one repository's branches
   with another repository's counts, and the mark is what a person then acts on.

   `loading` is deliberately untouched, for `loadBranchList`'s reason: two
   functions setting one boolean means whichever finishes first clears it under
   the other.

   A failure is quiet. `vcs_tracking` refuses nothing, so reaching the catch
   means the call itself failed — and a panel that shouted about it would be
   shouting about a mark, on a machine that may simply have no remote. */
async function loadTracking() {
  const { project, selected } = vcsState
  if (!selected) {
    vcsState.tracking = {}
    return
  }
  try {
    const records = await invoke('vcs_tracking', { repo: selected })
    if (vcsState.project !== project || vcsState.selected !== selected) return
    vcsState.tracking = Object.fromEntries(records.map((record) => [record.branch, record]))
  } catch (err) {
    if (vcsState.project !== project || vcsState.selected !== selected) return
    console.error('[vcs] reading upstreams failed:', err)
    vcsState.tracking = {}
  }
}

/* The branch the selected repository is on right now, as the list says.

   Read before an operation rather than after it, because a rebase leaves HEAD
   detached while it is stopped on a conflict: asked afterwards, the question
   "which branch was this" has no answer at all. The tree is the fall-back for a
   repository whose branch list has not landed yet. */
function currentBranch() {
  return vcsState.branches.find((branch) => branch.current)?.name ?? vcsState.tree?.branch ?? null
}

/* Which operation a conflicted tree is aborted as, where that is not the name
   of the write that made it.

   A pull is `git pull --no-rebase`, so what git stopped in the middle of is a
   merge: `git merge --abort` is the call that puts the tree back, and the
   record's `op` is the whole of what `abortConflict` reads — `OpKind` in
   `vcs/model.rs` knows two words and `pull` is not one of them. The refusal
   block keeps the write's own name, because "Git did not pull" is what somebody
   pressed. */
const CONFLICT_OP = { pull: 'merge' }

/* The mechanics every write in this panel shares, with one `invoke` handed in.

   Whether a write may be offered at all is `components/git/gitActions.js` — a
   rule about the project's runs, kept out of here because a store is not where
   a `.vue` file's disabled state is decided and because a test can reach that
   file. This function is the mechanics only, and deliberately does not repeat
   the rule: a second copy would be the half that drifts.

   Git decides the rest. A branch checked out in another worktree, a tree that
   would have to be overwritten, a merge with nothing to merge — all come back
   as `VcsError::Git` with git's own stderr in them, and the panel prints that
   as it stands.

   `theirs` is what the conflict record calls the other side, and it is the same
   branch as `busy`'s for every write but the pull: there the row the spinner
   belongs on is the current branch, while what git was bringing in is that
   branch's upstream, and the modal's sentence is about the second.

   What follows a write that worked is the whole list again rather than the
   working tree alone: the branch each repository is on is drawn in its row, so
   a status-only refresh would leave the row naming the branch somebody just
   left. The scope bar goes with it, one store over — and after a rebase that is
   not a nicety, since HEAD can be detached now and the bar says so. */
async function write(op, branch, call, theirs = branch) {
  const { project, selected } = vcsState
  /* A branch is **not** required here, though three of the four writes cannot
     do without one and guard it themselves. A commit is about the tree rather
     than about a row, and on a detached HEAD there is no branch to name at all
     — a guard here would have turned that into a button that did nothing and
     said nothing. `busy` still carries the branch for the three that have one,
     since that is what puts the spinner on the right row. */
  if (!selected || vcsState.busy) return false
  const ours = currentBranch()
  vcsState.busy = { op, branch }
  vcsState.writeError = null
  try {
    const outcome = await call(selected)
    /* The project alone, deliberately, where the failure path below guards the
       pair. Repository rows are not held by `busy`, so somebody can pick
       another repository while git works; the branch did move on disk, and
       leaving on the pair would mean nothing refreshed and the row and the mark
       stayed wrong until the next window focus. `refresh()` re-reads every
       repository and re-picks the remembered one, so it is right whichever is
       selected by the time it runs — the `selected` half is what `loadStatus`
       and `loadBranchList` need, not this. */
    if (vcsState.project !== project) return true
    /* Rust's own shape, read for the one word the panel branches on. The
       repository named here is the one the operation ran in, captured before
       the await: everything the modal then does — the abort, the agent's
       prompt — is about that repository and not about whichever row is
       selected by the time git answers. */
    if (outcome?.kind === 'conflict') {
      vcsState.conflict = {
        repo: selected,
        op: CONFLICT_OP[op] ?? op,
        ours,
        theirs,
        files: outcome.files ?? []
      }
      vcsState.conflictError = null
      /* Raised here and nowhere else on this path: a merge somebody pressed is
         the one case where the dialog is what they are waiting for, so it
         comes up by itself. Every other way a conflicted tree is discovered —
         the probe below, a restart, a terminal — leaves the flag down and puts
         the panel's button there instead. */
      vcsState.conflictOpen = true
    }
    await refresh()
    /* Awaited, unlike the sweep in `catchUp` that fires the same call and walks
       on: here it is the second half of one act, so a write that has finished
       means the panel and the bar over it agree. */
    await loadHead(project)
    return true
  } catch (err) {
    if (vcsState.project !== project || vcsState.selected !== selected) return false
    vcsState.writeError = { ...asError(err), op }
    return false
  } finally {
    /* Cleared whoever the project is now: this flag is what holds the panel
       inert, and a switch landing mid-write would otherwise leave the new
       project's list dead with nothing on screen to say why. */
    if (vcsState.busy?.op === op && vcsState.busy?.branch === branch) vcsState.busy = null
  }
}

/* Switch the selected repository to another of its branches. */
export async function checkout(branch) {
  if (!branch) return
  await write('checkout', branch, (repo) => invoke('vcs_checkout', { repo, branch }))
}

/* Bring another branch into the one this repository is on.

   Two answers rather than one: `clean`, and `conflict` with the paths git left
   unmerged. The second is not a failure — nothing was committed and nothing was
   lost — so it lands in `conflict` and opens the modal, while the panel behind
   it refreshes and shows exactly the tree git left. */
export async function merge(branch) {
  if (!branch) return
  await write('merge', branch, (repo) => invoke('vcs_merge', { repo, branch }))
}

/* Cut a new branch from another one — the row the right-click menu was opened
   on, which is what `start` carries.

   `busy` is keyed on the branch it is cut **from** rather than on the one being
   made: the panel puts the spinner on a row it is already drawing, and the new
   branch has no row until the refresh that follows. Which is also why the whole
   list comes back afterwards rather than the working tree alone — `write` does
   that for every write here, and this is the one where a row appears. */
export async function createBranch({ name, from, switch: switchTo = true }) {
  const wanted = (name ?? '').trim()
  if (!wanted || !from) return
  await write('create', from, (repo) =>
    invoke('vcs_create_branch', { repo, name: wanted, start: from, switch: switchTo })
  )
}

/* Replay this repository's branch on top of another one. The same two answers,
   and the same door out of the second. */
export async function rebase(onto) {
  if (!onto) return
  await write('rebase', onto, (repo) => invoke('vcs_rebase', { repo, onto }))
}

/* Delete a local branch.

   Through `write` like everything else here, so it takes the same `busy`, lands
   its refusal in the same block under `GitPanel`'s "Git did not delete the
   branch", and brings the whole list back afterwards — which is what makes the
   row disappear.

   **It is the one write in this file that hands the refusal back out**, and the
   reason is that the window which asked the question is the thing that has to
   decide what to do about it. A branch git declined because it is not merged
   has a way forward and one that is held by another worktree does not, so the
   window offers a second button for the first and only Cancel for the second —
   a decision the panel behind it cannot make, since it draws one block of git's
   words and no buttons at all. The refusal still goes to `writeError` on its way
   past: what happened is a fact about this repository whether or not the window
   is still standing.

   Read back off `writeError` rather than plumbed out of `write`: that function
   also answers `false` for a call it never made (git already busy) and for one
   whose project or repository moved underneath, and neither of those is a
   refusal anybody should be shown a second button about. The `op` is what tells
   them apart.

   The name is struck from the favourites on the way out. A pinned branch that
   no longer exists costs nothing while it sits in the file — `branchRows` draws
   no row for a name the repository does not have — but it would come back the
   day somebody cut a branch of that name again, pinned by a decision they made
   about a different branch. */
export async function deleteBranch(branch, { force = false } = {}) {
  if (!branch) return false
  /* Which project's list `settings.project` is holding, captured before the
     await and checked after it. `write()` answers `true` for a project that
     moved underneath — deliberately, since the branch did go on disk — and
     `settings.project` is merged **in place** on a switch, so by the time this
     resumes that object can be the next project's. Without this, a name pinned
     in both projects (`main`, `develop`) would be quietly unpinned in the one
     somebody had just switched to.
     `activeProject` rather than `vcsState.project`: `projects.js` sets it
     synchronously, before the layout is read and before `loadRepos` runs, so it
     is the earliest signal of a switch and can never be behind the object it is
     a statement about. */
  const of = settings.activeProject
  const gone = await write('delete', branch, (repo) =>
    invoke('vcs_delete_branch', { repo, branch, force })
  )
  if (!gone) {
    if (vcsState.writeError?.op === 'delete') throw vcsState.writeError
    return false
  }
  const marked = settings.project.favoriteBranches
  if (settings.activeProject === of && Array.isArray(marked) && marked.includes(branch)) {
    settings.project.favoriteBranches = marked.filter((name) => name !== branch)
  }
  return true
}

/* Bring the upstream's commits into the branch this repository is on.

   Through `write` like every other write here, so it takes the same `busy`, the
   same refusal block and the same refresh afterwards — and `busy` is keyed on
   the current branch, which is the row the spinner belongs on.

   The conflict it can end in is `merge`'s, and it is recorded as `merge`
   deliberately: `vcs_pull` runs `git pull --no-rebase`, so the abort that puts
   the tree back is `git merge --abort`, and the record's own `op` is what
   `abortConflict` reads. */
export async function pull() {
  const branch = currentBranch()
  /* The upstream is what a conflict here is *with*, and the modal draws it:
     "Git stopped merging origin/main into main" is the sentence, where the
     branch's own name on both sides would be one about nothing. Pull is refused
     without an upstream (`components/git/tracking.js`), so the fall-back is for
     a caller that went round the button rather than for a state on screen. */
  const upstream = branch ? vcsState.tracking[branch]?.upstream : null
  await write('pull', branch, (repo) => invoke('vcs_pull', { repo }), upstream ?? branch)
}

/* Send this branch's commits to its upstream, publishing the branch if it has
   none.

   The decision is taken here rather than in Rust because the tracking record is
   already on this side and is what the button was drawn from — and a stale
   answer is harmless either way: `-u` against a branch that has since gained an
   upstream sets the same one again, and a plain push of one that has since lost
   it is refused in git's own words.

   Which of the two it is, is `publishes` in `components/git/tracking.js` and
   never a second copy of that expression here: the caption is drawn from the
   same call, and a rule written out twice is one that will one day say "Publish
   branch" over a plain `git push`. Answered here rather than carried on the
   event so that a caller going round the button — the panel is not the only way
   into this store — cannot ask for the wrong one. */
export async function push() {
  const branch = currentBranch()
  const setUpstream = publishes(branch ? vcsState.tracking[branch] : null)
  await write('push', branch, (repo) => invoke('vcs_push', { repo, setUpstream }))
}

/* What is in the draft for the repository on screen. Empty for one nobody has
   typed into, which is the same thing to a `Textarea`. */
export function draftMessage() {
  return vcsState.messages[vcsState.selected] ?? ''
}

/* Somebody typed, or the agent answered. Keyed by repository, so switching to
   another and back finds the sentence where it was left. */
export function setMessage(text) {
  if (!vcsState.selected) return
  vcsState.messages[vcsState.selected] = text
}

/* Commit everything the panel is showing, under the draft message.

   `git add --all` then `git commit` on the far side, which is why the count on
   the button is the whole list: this app has no staging of its own, so the
   honest scope is what is on screen. The branch is passed for `busy` to carry
   and may be null — a detached HEAD is a tree somebody can still commit to.

   The draft is cleared on success only, and "success" is what `write` says
   rather than what `writeError` looks like afterwards: `write` also bails
   without doing anything when git is already busy, and reading the absence of
   an error as a commit would have thrown the sentence away with nothing
   committed. A commit git refused is one somebody is about to try again, and
   that sentence is the thing they would otherwise type twice. */
export async function commit() {
  const repo = vcsState.selected
  const message = draftMessage().trim()
  if (!repo || !message) return
  const committed = await write('commit', currentBranch(), (selected) =>
    invoke('vcs_commit', { repo: selected, message })
  )
  if (committed) delete vcsState.messages[repo]
}

/* Ask the agent for a commit message and put it in the field.

   Its own flag rather than `busy`, because this reads: the tree is not touched,
   so nothing about the branch rows below has to go inert while a model thinks.
   Its own error field for the same reason — the block that says "Git refused
   this operation" would be naming a party that was never asked.

   Guarded on the pair, project and repository, exactly as `loadStatus` is: an
   answer landing after somebody switched repositories would drop one
   repository's commit message into another repository's field, which is the one
   way this can do real damage. */
export async function suggestMessage() {
  const { project, selected } = vcsState
  if (!selected || vcsState.suggesting) return
  vcsState.suggesting = true
  vcsState.suggestError = null
  try {
    const message = await invoke('vcs_suggest_message', { repo: selected })
    if (vcsState.project !== project || vcsState.selected !== selected) return
    vcsState.messages[selected] = message
  } catch (err) {
    if (vcsState.project !== project || vcsState.selected !== selected) return
    vcsState.suggestError = asError(err)
  } finally {
    vcsState.suggesting = false
  }
}

/* The first door out of a conflict: put the tree back exactly as it was.

   `git merge --abort` or `git rebase --abort`, decided by the record's own
   `op` rather than by anything the component passes — the dialog draws what is
   in the store and answers about the same thing.

   The record is cleared only on git's answer. It is the one thing on screen
   saying this tree is conflicted, and taking it down before git said it had put
   the tree back would leave that state with nothing naming it. */
export async function abortConflict() {
  const conflict = vcsState.conflict
  if (!conflict || vcsState.busy) return
  const project = vcsState.project
  vcsState.busy = { op: 'abort', branch: conflict.theirs }
  vcsState.conflictError = null
  try {
    await invoke('vcs_abort', { repo: conflict.repo, op: conflict.op })
    clearConflict()
    if (vcsState.project !== project) return
    await refresh()
    await loadHead(project)
  } catch (err) {
    vcsState.conflictError = asError(err)
  } finally {
    if (vcsState.busy?.op === 'abort') vcsState.busy = null
  }
}

/* Take the dialog down and **keep** the record.

   The second door out of a conflict — an agent session on the tree, which is
   left exactly as git left it — comes through here, and so does the close
   cross. Starting the session is `DesktopApp.vue`'s, because it is also a
   switch to the agents side tab and to the terminal in the centre, and no
   store in this app opens a tab.

   This used to forget the conflict, and that was the bug: "Resolve with an
   agent" leaves the tree exactly as git left it, so a session that stopped
   early left a conflicted repository the panel could see and not act on. The
   record now goes only when the tree is clean or the project changes. */
export function dismissConflict() {
  vcsState.conflictOpen = false
  vcsState.conflictError = null
}

/* The way back in, which is the panel's `Resolve conflicts` button. It opens
   nothing on its own — there is a record or there is not, and the button is
   drawn from the same fact. */
export function openConflict() {
  if (!vcsState.conflict) return
  vcsState.conflictOpen = true
  vcsState.conflictError = null
}

/* One file as HEAD has it — the left-hand side of a diff. `path` is relative to
   the repository, exactly as `vcs_status` reported it.

   `null` is a file HEAD does not have: an added or an untracked one, and every
   file of a repository with no commit in it yet. That is not a failure and the
   caller diffs it against an empty document, so it is deliberately not
   normalised into a string here — a caption saying which of the two happened is
   the panel's to draw.

   No state of its own and no guard: nothing here is written into `vcsState`,
   the answer belongs to whoever asked, and the diff tab that asked already
   carries the guard against its own stale response (`tabs.js`). The refusal is
   raised as it arrives so the tab can say which of them it was.

   **The kinds are mostly `FilesError`'s own and deliberately not all of them.**
   The three a file shares with the editor — binary, too large, not UTF-8 — are
   pinned to it by a test in `vcs/model.rs`, and `fileErrorText` has had the
   words for those from the start. But this command is git, so it can also
   refuse in ways no file read can: `timeout`, when a call outstays `run.rs`'s
   ceiling, is in that table for exactly this caller. Anything added to
   `VcsError` that this command can produce needs an entry in `ERRORS` beside
   it, or it draws the fallback — "Could not read this file." — with nothing
   failing anywhere. */
export async function fileAtHead(repo, path) {
  return invoke('vcs_file_at_head', { repo, path })
}

/* The refresh button in the panel header, and window focus.

   The whole list rather than the selected tree alone: a repository can appear
   or disappear while the window was away — a worktree cut by a run's
   provisioning phase is exactly that — and a panel that refreshed only the
   files would keep a row for a folder that is gone. */
export async function refresh() {
  await loadRepos(vcsState.project)
}

/* Ask the remote whether there is anything new, for the selected repository.

   Called from window focus and from the project switch — the same two moments
   everything else in this panel refreshes on — and from nowhere else. Three
   things keep it from being a cost:

   - the setting, which is the person's own answer to whether this app may open
     a socket by itself;
   - the throttle, so a person alt-tabbing does not fetch every few seconds;
   - one in flight per repository, since a second request would queue behind a
     network call that may be running for a minute.

   Nothing waits for it. The branch list, the status and the marks are drawn
   from what is already known, and when this lands the tracking read is repeated
   and the marks change under them.

   **Its failure is silent, and that is the design.** `writeError` draws "Git
   refused this operation" and there was no operation: nobody pressed anything.
   A laptop off the network must be usable all day without a red block in the
   sidebar, and what is on screen stays as stale as the last fetch that worked —
   the same promise the file tree beside it makes. */
export async function autoFetch() {
  const { selected } = vcsState
  if (!selected || !settings.git.autoFetch) return
  if (fetching.has(selected)) return
  const last = fetchedAt.get(selected) ?? 0
  if (Date.now() - last < FETCH_EVERY_MS) return
  await fetchRepo(selected, false)
}

/* The same call, pressed.

   It exists because the two buttons beside it are refused in exactly the state
   somebody most wants to check: a branch that is level dims both, and the
   number they are dimmed over is as old as the last sweep that worked. With
   `git.autoFetch` off there has been no sweep at all, and without this control
   the panel would hold a count it offered no way of refreshing — a fact
   somebody is deciding on that they cannot ask about again.

   Three things it deliberately does not share with the sweep above. **It
   ignores the setting**, because the setting is about what this app does on
   its own and a press is not that — the prose for it says as much: with it off
   the app makes no network call of its own, and both buttons go on working.
   **It ignores the throttle**, and resets it: five minutes is a budget for
   calls nobody asked for, and a person who presses twice knows they pressed
   twice. **Its failure is loud**, in the same block, with the same `op`
   machinery every pressed write here uses — the argument that keeps the
   sweep's own failure silent is that nobody pressed anything, and here
   somebody did.

   What it keeps is the one-in-flight guard, since a second `git fetch` in the
   same repository would only queue behind the first — and that guard is also
   what dims the button while it is out. */
export async function fetchNow() {
  const { selected } = vcsState
  if (!selected) return false
  vcsState.fetching = true
  /* Cleared on the way out rather than on the way back, which is what every
     other pressed write here does: the block under the branches is about the
     last thing somebody asked for, and leaving a refused pull on screen while
     its replacement is already out would be the panel answering a question
     nobody is asking any more. */
  vcsState.writeError = null
  try {
    /* The sweep's own call, joined rather than duplicated: a second
       `git fetch` in one repository would only queue behind the first, and the
       answer this press is waiting for is the answer that one is about to
       bring back. Its failure stays silent — nobody pressed *that* one, and
       the press cannot retroactively make the sweep loud — so what this press
       gets is the verdict without the block. */
    const inFlight = fetching.get(selected)
    if (inFlight) return await inFlight
    return await fetchRepo(selected, true)
  } finally {
    /* Cleared whatever the panel is looking at now: this flag only dims one
       button, and a repository switched under a fetch would otherwise leave
       the new one's caption spinning over a call about a repository nobody is
       looking at. */
    vcsState.fetching = false
  }
}

/* A fetch in a repository somebody named, for a caller that is not this panel.

   The branch review window is the one, and the reason it needs a fetch at all
   is the reason `origin/main` is worth naming as a side of a pair: what that
   ref points at is only as current as the last fetch, so a review of it drawn
   under a branch name somebody pushed this morning would be about the wrong
   commit with nothing anywhere saying so.

   Quiet, unlike `fetchNow` above, and the difference is where the failure is
   drawn rather than whether anybody pressed anything. This one *was* pressed —
   but the window that pressed it says so itself, in its own sentence, beside
   the table the review is about; `writeError` would put "Git did not reach the
   remote" over a branch list nobody was looking at, about a repository that may
   not even be the selected one.

   It joins the call already in flight rather than being refused by it, exactly
   as `fetchNow` does and for that function's reason: what this caller is
   waiting for is the answer that call is about to bring back.

   `true` when the fetch worked and `false` when it did not, which is what lets
   the window name the repositories it could not reach — and go ahead anyway. */
export async function fetchIn(repo) {
  if (!repo) return false
  const inFlight = fetching.get(repo)
  if (inFlight) return await inFlight
  return await fetchRepo(repo, false)
}

/* `git fetch --prune` in one repository, and what to do with the answer.

   The one place the network call is written, shared by the sweep and by the
   button, so the two cannot come apart on the part that matters — the guard,
   the stamp, and re-reading the tracking when it lands. What differs is
   handed in: `loud` is whether a failure reaches the screen.

   Stamped even on failure, so an unreachable remote is asked once every five
   minutes rather than on every window focus. A pressed fetch stamps too: it
   did reach out, and the sweep behind it has no reason to go again a second
   later. */
function fetchRepo(repo, loud) {
  const call = runFetch(repo, loud).finally(() => fetching.delete(repo))
  fetching.set(repo, call)
  return call
}

/* The call itself, which is the half above holds on to. Split in two so the
   map is written before the first `await` inside it: a version that registered
   itself from within its own body would leave a window in which a second
   caller sees no call in flight and starts one. */
async function runFetch(repo, loud) {
  const { project } = vcsState
  try {
    await invoke('vcs_fetch', { repo })
    fetchedAt.set(repo, Date.now())
    if (vcsState.project !== project || vcsState.selected !== repo) return true
    await loadTracking()
    return true
  } catch (err) {
    fetchedAt.set(repo, Date.now())
    if (!loud) {
      /* Recorded where a developer can see it and nowhere a person can: the
         ordinary cases here are no network, no credentials and no remote, and
         none of them is something this panel should interrupt anybody about. */
      console.error('[vcs] background fetch failed:', err)
      return false
    }
    if (vcsState.project !== project || vcsState.selected !== repo) return false
    vcsState.writeError = { ...asError(err), op: 'fetch' }
    return false
  }
}

function reset() {
  vcsState.repos = []
  /* With `repos` and for its reason: it is a statement about the project being
     left, and the next one is not owed it. */
  vcsState.unlisted = []
  vcsState.selected = null
  vcsState.tree = null
  vcsState.branches = []
  forgetRemoteBranches()
  vcsState.tracking = {}
  vcsState.error = null
  vcsState.writeError = null
  /* A conflict belongs to a repository of the project being left, and there is
     nothing in the new one for its two doors to act on: the abort names a
     repository nobody is looking at, and the agent would be started in the
     wrong project. The tree itself is untouched and says what it is the moment
     that project comes back. */
  vcsState.conflict = null
  vcsState.conflictError = null
  vcsState.conflictOpen = false
  /* The drafts go with the project they were about. Keeping them would mean a
     sentence about another project's work sitting over this one's file list,
     one keystroke away from being committed there. */
  vcsState.messages = {}
  vcsState.suggestError = null
  /* The spinner and not the guard, which is the split `fetching` below is
     about: the call may still be running, and its own `finally` is what takes
     the repository back out of that set. What goes here is only the dimming of
     a button in a panel that is about to draw another project. */
  vcsState.fetching = false
  vcsState.loading = false
  /* The throttle's memory, and only when there is no project at all — this
     runs on the way to an empty window and nowhere else, so switching between
     two projects deliberately keeps the stamps: the same repository opened
     again a minute later has not become stale because somebody looked at
     something else. Dropped here because a window with no project is a session
     ending, and a repository re-opened after that should ask the remote once
     rather than wait out a throttle set before it.

     `fetching` is deliberately **not** cleared with it. It is not memory but a
     hold on a call that is still running: a fetch in flight goes on running
     across this, and its own `finally` takes the repository back out of the map
     when it lands. Emptying it here would drop that hold while the call it
     holds is still open, so re-opening that project could start a second
     `git fetch` in the same repository — the one thing this map exists to
     prevent. */
  fetchedAt.clear()
}
