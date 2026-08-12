/* The active project's branch, for the scope bar. Sixth of the files in this
   directory that know Tauri exists; components see a reactive object.

   Deliberately the smallest store here: git is read, never written, there is
   one value, and it costs a file read. Freshness comes from window focus and
   from switching projects — the same answer files.js gives, and for the same
   reason: a watcher subsystem is more machinery than one line of text is
   worth. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export const gitState = reactive({
  branch: null,
  /* Local branch names, for the run dialog's "merge into" field. Loaded when
     that dialog opens rather than on every project switch: it is a directory
     read, but it is one nobody needs until they are looking at the field. */
  branches: [],
  /* A short hash when HEAD is detached. Kept apart from `branch` rather than
     written into it: a bar that shows a hash where a branch name goes has to
     say so, and a component cannot tell the two apart once they share a
     field. */
  detached: null,
  project: null
})

function clear() {
  gitState.branch = null
  gitState.detached = null
}

/* Guarded against its own stale response the same way loadSessions is: two
   calls can be in flight with no ordering guarantee on which invoke resolves
   first, and without the guard the last response would win rather than the
   last call — the bar would name one project's branch under another
   project's name. */
export async function loadHead(project) {
  gitState.project = project
  if (!project) {
    clear()
    return
  }
  try {
    const head = await invoke('git_head', { project })
    if (gitState.project !== project) return
    gitState.branch = head.branch ?? null
    gitState.detached = head.detached ?? null
  } catch (err) {
    if (gitState.project !== project) return
    // Not a folder's fault and not worth a toast: the back end answers with an
    // empty head for anything it cannot read, so reaching here means the call
    // itself failed. The bar shows no branch; the reason stays in the console.
    console.error('[git] head failed:', err)
    clear()
  }
}

/* Which project the list currently in the store belongs to, and the project of
   the last call — one variable serving both, since a call always claims the
   list it is about to replace. Module state rather than part of gitState:
   nothing on screen reads it. */
let branchesFor = null

/* The local branches, for the run dialog's field.

   Two rules, and the second one was paid for. A list belonging to *another*
   project is cleared the moment this one is asked about: offering the branches
   of a repository somebody has already left is worse than offering none. A list
   belonging to *this* project is left alone while it is read again — clearing
   unconditionally emptied the field under the dialog that had just opened, and
   nothing refilled it, which is what smetana-6gs was.

   Keeping it costs one thing, and it is a real one: for the length of a
   target_branches call the field can be filled from a list that is one read out
   of date. A branch deleted since the last open is still on offer in that window,
   and somebody who picks it by hand inside it has their choice frozen by
   RunModal's `branchChosen` — the run then goes out with create_target false
   against a branch that is not there. Clearing first made that impossible,
   because Run stayed disabled until the fresh list landed. The trade is taken
   knowingly: the window is a single call, the field self-corrects the moment
   the list arrives as long as nobody has chosen, and what the bad case costs is
   a run that fails at the merge — against a dialog that offered nothing, every
   time, to everybody.

   Guarded against its own stale response the way loadHead is: the last call
   wins, not the last answer. The dialog cannot be opened twice at once, but a
   project switch and an open can overlap, and the guard costs a comparison.

   `target_branches` and not `git_branches`: for a project whose
   `[project].repos` names several repositories, the project path is the folder
   that holds them and whatever repository happens to sit at that folder is not
   one of the ones a run merges into. Rust reads the config and asks each of
   them, in one call, because a repository list passed down from here would
   arrive from `runs.js` on its own schedule and race the dialog opening. */
export async function loadBranches(project) {
  const same = !!project && project === branchesFor
  branchesFor = project
  if (!same) gitState.branches = []
  if (!project) return
  try {
    const list = await invoke('target_branches', { project })
    if (branchesFor !== project) return
    gitState.branches = list
  } catch (err) {
    if (branchesFor !== project) return
    // An empty list, like a folder outside git: the dialog then has nothing to
    // offer and its Run button stays disabled, which is honest. A list that
    // could not be read again is dropped rather than kept — it is no longer
    // known to be the repository's.
    console.error('[git] listing branches failed:', err)
    gitState.branches = []
  }
}
