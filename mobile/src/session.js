/* The shell's glue: which stores run, and what the page reads when the link to
   the desktop comes up.

   The stores are the desktop front end's own (../../src/stores/), run unchanged
   over the bridge. This file only decides *when* they read: the first time the
   link is usable they start; after every reconnect they read their snapshots
   again, because missed deltas are not replayed. */
import { reactive } from 'vue'
import { initTracker, resync, trackerState, isLockIssue } from '@src/stores/tracker.js'
import { invoke } from './bridge/core.js'
import { whenOnline } from './bridge/index.js'
import { projectName, taskCount, saveSummary, loadSummary, forgetSummary } from './summary.js'
import { unpair } from './bridge/index.js'

export const home = reactive({ project: null, tasks: null, loadedAt: null })

function remember() {
  home.tasks = taskCount(trackerState.issues.values(), isLockIssue)
  home.loadedAt = Date.now()
  saveSummary(localStorage, { project: home.project, tasks: home.tasks })
}

async function readProject() {
  const settings = await invoke('settings_load', { project: null })
  home.project = projectName(settings.activeProject)
}

/* The stores are started once per app lifetime. `initTracker` subscribes to events, and a
   second call would subscribe again: every delta would be applied twice. Pairing again after
   an unpair, in the same session, therefore reads the snapshot again and does not start over. */
let started = false

export function wireStores() {
  const cached = loadSummary(localStorage)
  if (cached) {
    home.project = cached.project
    home.tasks = cached.tasks
    home.loadedAt = cached.savedAt
  }
  whenOnline(async () => {
    if (started) await resync()
    else {
      started = true
      await initTracker()
    }
    await readProject()
    remember()
  })
}

/* Counts again after a delta; the screen calls it when `generation` moves. */
export function refreshSummary() {
  remember()
}

/* Forgets the pairing and everything read from that desktop, so a different desktop paired next
   is never shown the previous one's project and count. */
export async function unpairAndForget() {
  await unpair()
  forgetSummary(localStorage)
  home.project = null
  home.tasks = null
  home.loadedAt = null
}
