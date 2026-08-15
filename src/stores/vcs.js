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
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
/* The scope bar's branch is git.js's, read straight off `HEAD` with no process
   — and a checkout here is the one moment in the app that changes it from the
   inside. Freshness there is window focus and the project switch, neither of
   which a person switching branches in this panel ever reaches, so the write
   refreshes it itself: the alternative is a bar naming the branch somebody just
   left, until they alt-tab away and back. */
import { loadHead } from './git.js'
import { settings } from './settings.js'

export const vcsState = reactive({
  /* The project the rest of this object is about. Also the guard token: every
     call claims it before its first await and checks it after. */
  project: null,
  repos: [],
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
     empty list is an ordinary answer — a folder outside git has none, and so
     has a repository whose first commit is not written yet. */
  branches: [],
  /* `{ kind, message }` — Rust's own shape, normalised here so a rejection that
     is a bare string (the browser mock, a transport failure) draws the same
     way. `kind` is what the panel branches on; the message is git's own words
     and is shown untouched. */
  error: null,
  /* Git's refusal of a checkout, in the same `{ kind, message }` shape and kept
     apart from `error` above deliberately: that one says the working tree could
     not be read, this one says it was read and a switch was refused. Folding
     them together would put a refusal about a branch where the changes should
     be, and take the list down with it. */
  checkoutError: null,
  /* The branch a checkout is in flight for, or null. One at a time: the panel
     goes inert while git works, so a second row cannot be pressed into a
     checkout git is already refusing for the first. */
  checkingOut: null,
  loading: false
})

/* A rejection becomes something the panel can both branch on and print. The
   message is never rewritten: for a non-zero git it is git's own stderr, and
   the person reading it knows git better than any sentence written here. */
function asError(err) {
  if (err && typeof err === 'object' && typeof err.message === 'string') {
    return { kind: typeof err.kind === 'string' ? err.kind : 'io', message: err.message }
  }
  return { kind: 'io', message: String(err) }
}

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
  vcsState.project = project
  if (!project) {
    reset()
    return
  }
  vcsState.loading = true
  try {
    const repos = await invoke('vcs_repos', { project })
    if (vcsState.project !== project) return
    vcsState.repos = repos
    await selectRepo(pickRepo(repos, settings.project.selectedRepo))
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
    vcsState.tree = null
    /* With no repository left to be about, a branch list read a moment ago is
       one nothing on screen names. */
    vcsState.branches = []
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
     list of the repository next door, would be a statement about neither. */
  vcsState.checkoutError = null
  await Promise.all([loadStatus(), loadBranchList()])
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
    vcsState.tree = tree
    vcsState.error = null
  } catch (err) {
    if (vcsState.project !== project || vcsState.selected !== selected) return
    vcsState.tree = null
    vcsState.error = asError(err)
  } finally {
    if (vcsState.project === project && vcsState.selected === selected) vcsState.loading = false
  }
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

/* Switch the selected repository to another of its branches.

   The one write this panel has. Whether it may be offered at all is
   `components/git/gitActions.js` — a rule about the project's runs, kept out of
   here because a store is not where a `.vue` file's disabled state is decided
   and because a test can reach that file. This function is the mechanics only,
   and deliberately does not repeat the rule: a second copy would be the half
   that drifts.

   Git decides the rest. A branch checked out in another worktree, a working
   tree that would have to be overwritten — both come back as `VcsError::Git`
   with git's own stderr in them, and the panel prints that as it stands.

   What follows a checkout that worked is the whole list again rather than the
   working tree alone: the branch each repository is on is drawn in its row, so
   a status-only refresh would leave the row naming the branch somebody just
   left. The scope bar goes with it, one store over. */
export async function checkout(branch) {
  const { project, selected } = vcsState
  if (!selected || !branch || vcsState.checkingOut) return
  vcsState.checkingOut = branch
  vcsState.checkoutError = null
  try {
    await invoke('vcs_checkout', { repo: selected, branch })
    if (vcsState.project !== project || vcsState.selected !== selected) return
    await refresh()
    /* Awaited, unlike the sweep in `catchUp` that fires the same call and walks
       on: here it is the second half of one act, so a checkout that has
       finished means the panel and the bar over it agree. */
    await loadHead(project)
  } catch (err) {
    if (vcsState.project !== project || vcsState.selected !== selected) return
    vcsState.checkoutError = asError(err)
  } finally {
    /* Cleared whoever the project is now: this flag is what holds the panel
       inert, and a switch landing mid-checkout would otherwise leave the new
       project's list dead with nothing on screen to say why. */
    if (vcsState.checkingOut === branch) vcsState.checkingOut = null
  }
}

/* The refresh button in the panel header, and window focus.

   The whole list rather than the selected tree alone: a repository can appear
   or disappear while the window was away — a worktree cut by a run's
   provisioning phase is exactly that — and a panel that refreshed only the
   files would keep a row for a folder that is gone. */
export async function refresh() {
  await loadRepos(vcsState.project)
}

function reset() {
  vcsState.repos = []
  vcsState.selected = null
  vcsState.tree = null
  vcsState.branches = []
  vcsState.error = null
  vcsState.checkoutError = null
  vcsState.loading = false
}
