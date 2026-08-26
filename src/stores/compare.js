/* One branch compared with the branch this repository is on, for the compare
   window.

   A store of its own rather than a corner of `vcs.js`, and the reason is which
   window it lives in: this state belongs to a second webview that is opened,
   read and closed, while `vcs.js` is the panel in the app window and outlives
   every one of them. It is also, like the rest of `stores/`, one of the only
   files in `src/` that knows Tauri is there.

   **Every file read goes by the shas `vcs_compare` resolved, never by the
   branch name.** HEAD moves while this window stands open — an agent committing
   into the same tree is the ordinary case in this app — and asking by name each
   time would let the file list belong to one commit and the bytes on screen to
   another, with nothing saying so. */
import { invoke } from '@tauri-apps/api/core'
import { emit, listen } from '@tauri-apps/api/event'
import { reactive } from 'vue'

/* Which pair an already-open window has just been re-aimed at. Not a setting
   and nothing to do with `settings.json`; a message about a window, which is
   what `app.js`'s `settings:show` is too. */
export const COMPARE_SHOW = 'compare:show'

export const compareState = reactive({
  repo: null,
  branch: null,
  /* 'diverged' | 'direct'. The default is the one the window opens in, and the
     one a person almost always means: what this branch added since it split. */
  mode: 'diverged',
  left: '',
  right: '',
  files: [],
  selected: null,
  head: '',
  work: '',
  missingLeft: false,
  /* The comparison's own refusal — no such branch, no shared history — as
     against `fileError`, which is about the one file on screen. Two states,
     because they are two different sentences in two different places. */
  error: null,
  fileError: null,
  loading: false,
  fileLoading: false
})

/* Both stores answer a refusal in Rust's own `{ kind, message }` shape and a
   transport failure as anything at all; one shape here means the view has one
   thing to draw. Lifted from `stores/tabs.js`, which needs the identical rule
   for the identical reason. */
function asError(error) {
  if (error && typeof error === 'object' && typeof error.kind === 'string') return error
  return { kind: 'io', message: String(error?.message ?? error) }
}

/* The guard is `seq` and it plays `generation`'s part: two reads of one pane can
   be in flight with no ordering guarantee, and without it the last *response*
   would win rather than the last *call* — the same defect `loadDiff`,
   `terminals.js` and `git.js` all guard against, here landing as one file's text
   under another file's name. */
let compareSeq = 0
let fileSeq = 0

/* What the window is looking at, read off its own query string on mount and
   again whenever it is re-aimed.

   **The same pair again is a refresh, not a new aim.** Right-clicking the
   branch that is already on screen is the natural way to ask for the list
   again, and it arrives here as a `compare:show` for the pair this window is
   already looking at. Clearing the selection on that would throw away the file
   somebody is in the middle of reading — which is the very thing
   `compare_window_open` focuses an open window rather than rebuilding it in
   order not to do. A different pair is a different question, and the file open
   on the old one has no meaning under it, so that case clears as before. */
export async function aim(repo, branch) {
  if (repo === compareState.repo && branch === compareState.branch) {
    await refresh()
    return
  }
  compareState.repo = repo
  compareState.branch = branch
  compareState.selected = null
  compareState.head = ''
  compareState.work = ''
  compareState.fileError = null
  await refresh()
}

export async function setMode(mode) {
  compareState.mode = mode === 'direct' ? 'direct' : 'diverged'
  await refresh()
}

/* Re-runs the comparison at the mode it is in. Called on a mode change, and on
   window focus — the freshness answer the Git panel, the file tree and the
   branch in the scope bar all give, and for the same reason: a git call per
   change is a process per change. */
export async function refresh() {
  const { repo, branch, mode } = compareState
  if (!repo || !branch) return
  const seq = ++compareSeq
  compareState.loading = true
  try {
    const out = await invoke('vcs_compare', { repo, branch, mode })
    if (seq !== compareSeq) return
    compareState.left = out.left
    compareState.right = out.right
    compareState.files = out.files
    compareState.error = null
  } catch (err) {
    if (seq !== compareSeq) return
    /* The list is emptied rather than left standing under a refusal: rows read
       off one comparison, drawn beside a sentence about another, are the one
       failure this window can actually mislead somebody with. */
    compareState.files = []
    compareState.left = ''
    compareState.right = ''
    compareState.error = asError(err)
  } finally {
    if (seq === compareSeq) compareState.loading = false
  }
  /* A file that survived the new list stays open; one that did not is dropped
     rather than left on screen belonging to nothing. */
  if (compareState.selected && !compareState.files.some((f) => f.path === compareState.selected)) {
    compareState.selected = null
    compareState.head = ''
    compareState.work = ''
  } else if (compareState.selected) {
    await select(compareState.selected)
  }
}

/* One file's two sides, read at the two shas this comparison resolved. Both at
   once, and neither is the other's business. */
export async function select(path) {
  const { repo, left, right } = compareState
  if (!repo || !left || !right) return
  const seq = ++fileSeq
  compareState.selected = path
  compareState.fileLoading = true

  const [head, work] = await Promise.all([
    invoke('vcs_file_at_rev', { repo, rev: left, path }).then(
      (text) => ({ text, error: null }),
      (error) => ({ text: null, error: asError(error) })
    ),
    invoke('vcs_file_at_rev', { repo, rev: right, path }).then(
      (text) => ({ text, error: null }),
      (error) => ({ text: null, error: asError(error) })
    )
  ])

  if (seq !== fileSeq) return
  compareState.head = head.text ?? ''
  compareState.work = work.text ?? ''
  /* `null` is a revision that does not have this file — added on one side,
     deleted on the other. The empty pane is the truth, and this flag is what
     lets the caption say which of the two empties it is looking at. */
  compareState.missingLeft = head.error === null && head.text === null
  compareState.fileError = head.error ?? work.error
  compareState.fileLoading = false
}

/* Opens the window on a pair, or re-aims the open one.

   The pair travels twice, and both halves are needed for one press to work in
   both states — the shape `openSettingsWindow` records in `app.js`. A window
   being built reads it off the URL it already loads; an open one is focused
   rather than rebuilt, so it never sees a new URL and the event is the only way
   to reach it. A fresh window is not listening yet and simply misses the event,
   having already read the parameters.

   In a browser there is no window to make: the mock has no `compare_window_open`
   and the menu item is a no-op there, exactly as the gear is. The window itself
   is still reachable through `?view=compare`, which is what the dev server
   checks it with. */
export async function openCompareWindow(repo, branch) {
  try {
    await invoke('compare_window_open', { repo, branch })
  } catch (err) {
    console.error('[compare] the compare window did not open:', err)
    return
  }
  try {
    await emit(COMPARE_SHOW, { repo, branch })
  } catch (err) {
    /* The window is open on whatever it was showing, which is a smaller failure
       than not opening at all. */
    console.warn('[compare] the compare window was not told what to compare:', err)
  }
}

/* The compare window's half: which pair it has just been asked for. */
export async function watchCompareTarget(onTarget) {
  return listen(COMPARE_SHOW, (event) =>
    onTarget(event.payload?.repo ?? null, event.payload?.branch ?? null)
  )
}
