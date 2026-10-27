/* The shell's glue: which stores run, and what the page reads when the link to
   the desktop comes up.

   The stores are the desktop front end's own (../../src/stores/), run unchanged
   over the bridge. This file only decides *when* they read: the first time the
   link is usable they start; after every reconnect they read their snapshots
   again, because missed deltas are not replayed. */
import { reactive, watch } from 'vue'
import { initTracker, resync, trackerState, isLockIssue } from '@src/stores/tracker.js'
import { settings } from '@src/stores/settings.js'
import { bridge, isOnline, whenOnline } from './bridge/index.js'
import { startWorkspace } from './workspace.js'
import { projectName, taskCount, saveSummary, loadSummary, forgetSummary, savedLook } from './summary.js'
import { unpair } from './bridge/index.js'

export const home = reactive({ project: null, tasks: null, loadedAt: null })

function remember() {
  home.tasks = taskCount(trackerState.issues.values(), isLockIssue)
  home.loadedAt = Date.now()
  saveSummary(localStorage, {
    project: home.project,
    tasks: home.tasks,
    look: { theme: settings.appearance.theme, density: settings.appearance.density }
  })
}

/* The stores are started once per app lifetime. `initTracker` subscribes to events, and a
   second call would subscribe again: every delta would be applied twice. Pairing again after
   an unpair, in the same session, therefore reads the snapshot again and does not start over. */
let started = false

export function wireStores() {
  /* The store's default is `dark`; until the desktop has answered, the phone follows its own
     system theme, which is what `system` means here. */
  settings.appearance.theme = 'system'
  const cached = loadSummary(localStorage)
  if (cached) {
    home.project = cached.project
    home.tasks = cached.tasks
    home.loadedAt = cached.savedAt
    /* The last look the desktop had, so a launch with the desktop out of reach opens in it. */
    const look = savedLook(cached)
    if (look) {
      settings.appearance.theme = look.theme
      settings.appearance.density = look.density
    }
  }
  /* A look that changed while online is kept for the next launch. Offline nothing is saved: the
     copy would be stamped as read just now. */
  watch(
    () => [settings.appearance.theme, settings.appearance.density],
    () => {
      if (isOnline(bridge.status) && typeof home.tasks === 'number') remember()
    }
  )
  whenOnline(async () => {
    if (started) await resync()
    else {
      started = true
      await initTracker()
    }
    /* The rest of what the screens draw. A failure here is the working screens' to show; the
       summary below is what the unreachable screen keeps, and it needs only the tracker. */
    try {
      await startWorkspace()
    } catch (err) {
      console.warn('[companion] the working screens could not be read:', err)
    }
    home.project = projectName(settings.activeProject)
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
  settings.appearance.theme = 'system'
  settings.appearance.density = 'comfortable'
}
