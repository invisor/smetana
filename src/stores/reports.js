/* The Reports tab's list — every run report `.smetana/reports/` holds for the
   active project. One of the files in this directory that know Tauri exists;
   components see a reactive object and nothing else, which is why this is a
   store rather than a helper next to the component that draws it.

   Read-only, and it stays that way: the worker reads the documents, this asks
   for the list, and nothing in this app writes one from here — a run's own
   loop is the only writer, in `runs::service::write_report`.

   No watcher, on purpose, for the same reason `sessions.js` carries one: the
   read is a walk of a folder and a parse of every document in it, which costs
   nothing next to a live subscription's own lifecycle and error reporting.
   The list is read again when the tab is opened and when a run of this
   project ends while it is open — both `DesktopApp.vue`'s doing, the same
   shape `sessions.js`'s own header argues for its tab. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export const reportsState = reactive({
  rows: [],
  /* Whose reports these are, and the project of the last call — one field
     serving both, the same shape `sessionsState.project` carries and for the
     same reason: the panel has to be able to tell "this project has none"
     from "nobody has asked yet". */
  project: null,
  /* True between the call and its answer. What it buys is the same thing it
     buys `sessionsState.loading`: the empty state is a sentence claiming the
     disk holds nothing, and drawing it before the first answer has landed
     would be a claim made before anybody looked. */
  loading: false
})

/* The project's reports, read again.

   The same two rules `sessions.js`'s `loadSessionHistory` carries, borrowed
   from `git.js` for the same reason there: a list belonging to *another*
   project goes the moment this one is asked about, and a list belonging to
   *this* project is left alone while it is read again, so re-opening the tab
   does not blink the column empty and back.

   And the guard against its own stale answer: two calls can be in flight with
   no ordering guarantee on which invoke resolves first, so the last *call*
   wins rather than the last answer.

   There is no error state to draw, and that is the contract rather than an
   oversight: `run_reports` answers a missing folder, an unreadable file and a
   name it does not recognise with fewer rows, never with a refusal. Reaching
   the catch means the call itself failed — the console gets it, the tab shows
   the empty state, and nothing is invented. */
export async function loadReports(project) {
  if (reportsState.project !== project) reportsState.rows = []
  reportsState.project = project
  if (!project) {
    reportsState.loading = false
    return
  }
  reportsState.loading = true
  try {
    const rows = await invoke('run_reports', { project })
    if (reportsState.project !== project) return
    reportsState.rows = rows ?? []
  } catch (err) {
    if (reportsState.project !== project) return
    console.error('[reports] listing failed:', err)
    reportsState.rows = []
  } finally {
    if (reportsState.project === project) reportsState.loading = false
  }
}
