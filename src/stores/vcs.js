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
  /* `{ kind, message }` — Rust's own shape, normalised here so a rejection that
     is a bare string (the browser mock, a transport failure) draws the same
     way. `kind` is what the panel branches on; the message is git's own words
     and is shown untouched. */
  error: null,
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
    /* The command itself answers with a list for anything it can read, so
       reaching here means the call failed rather than the folder being
       uninteresting. The panel says so; the raw text stays in the console. */
    console.error('[vcs] listing repositories failed:', err)
    vcsState.repos = []
    vcsState.tree = null
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
  await loadStatus()
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
  vcsState.error = null
  vcsState.loading = false
}
