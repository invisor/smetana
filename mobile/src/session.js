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
import { projectName, taskCount, saveSummary, loadSummary } from './summary.js'

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

export function wireStores() {
  const cached = loadSummary(localStorage)
  if (cached) {
    home.project = cached.project
    home.tasks = cached.tasks
    home.loadedAt = cached.savedAt
  }
  whenOnline(async ({ again }) => {
    if (again) await resync()
    else await initTracker()
    await readProject()
    remember()
  })
}

/* Counts again after a delta; the screen calls it when `generation` moves. */
export function refreshSummary() {
  remember()
}
