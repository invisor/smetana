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

/* The local branches. Not guarded against a stale response the way loadHead is,
   and deliberately: this is called from opening a dialog, which cannot happen
   twice at once, and the dialog reads the list at that moment. Clearing first
   is what keeps a previous project's branches off the screen while the new
   ones are on their way. */
export async function loadBranches(project) {
  gitState.branches = []
  if (!project) return
  try {
    gitState.branches = await invoke('git_branches', { project })
  } catch (err) {
    // An empty list, like a folder outside git: the dialog then has nothing to
    // offer and its Run button stays disabled, which is honest.
    console.error('[git] listing branches failed:', err)
  }
}
