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
