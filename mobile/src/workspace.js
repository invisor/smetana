/* What the working screens need read, and when.

   The screens draw the desktop front end's own stores, run unchanged over the bridge. This file
   is the phone's half of "init": which stores are started, what is read for the project in
   front, and what is read again when the link comes back or the person returns to the app.

   The stores here are started by their `init*` functions and read by the `load*` ones the
   desktop's `DesktopApp.vue` calls. What is not called is as deliberate as what is: `loadSettings`
   (it starts a watch that writes the settings back), `initSettings`, `initProjects`, anything
   that touches files or git. The commands this reaches are the allowed list (src/allowed.js),
   which tests/workspace.test.js runs the whole of this against.

   The phone follows the desktop's project. There is no event that says the desktop has
   switched, so the project is read again whenever the phone has a reason to look: a
   reconnect, the page becoming visible, a pull on the Projects screen. */
import { reactive } from 'vue'
import { settings } from '../../src/stores/settings.js'
import { initAgents } from '../../src/stores/agents.js'
import { initRuns, loadConfig, loadRun } from '../../src/stores/runs.js'
import { initConversation } from '../../src/stores/conversation.js'
import { initTerminals, loadSessions } from '../../src/stores/terminals.js'
import { resync, setProject, trackerState } from '../../src/stores/tracker.js'
import { invoke } from './bridge/core.js'
import { listen } from './bridge/event.js'
import { bridge } from './bridge/status.js'
import { desktopMinor } from './bridge/states.js'
import { LOOK_MINOR, applyDesktopSettings } from './desktopSettings.js'

export const workspace = reactive({
  /* The first read has landed: the screens can stop saying "reading". */
  ready: false,
  /* A switch in flight, for the Projects screen. */
  switching: null,
  error: null
})

let started = false

/* The settings the desktop holds, for the project asked about (`null` for the one it has
   open). */
export async function readDesktop(project = null) {
  const stored = await invoke('settings_load', { project })
  applyDesktopSettings(settings, stored, { minor: desktopMinor(bridge.status) })
  return stored
}

/* Whether the phone has asked to hear `settings:changed`. Asked once, and only of a desktop that
   announces it: below `LOOK_MINOR` the core would refuse the subscription, and nothing is sent
   there. The core subscribes again after a reconnect, so once is enough. */
let hearingSettings = false

function hearSettings() {
  if (hearingSettings || desktopMinor(bridge.status) < LOOK_MINOR) return
  hearingSettings = true
  /* The payload is not folded in: the phone reads `settings_load` again, the same read a
     reconnect does, which is what keeps one way of getting the desktop's settings in. */
  listen('settings:changed', () => {
    refreshFromDesktop().catch((err) => console.warn('[companion] the settings could not be read again:', err))
  }).catch((err) => {
    hearingSettings = false
    console.warn('[companion] could not listen for settings changes:', err)
  })
}

/* What belongs to one project and is read for it: the sessions, the configuration, the runs.
   The conversation a run drives reaches the Agents list through `session_list` and
   `session:state`, not through anything read here. */
export async function loadProject(project) {
  if (!project) return
  await Promise.all([loadSessions(project), loadConfig(project), loadRun(project)])
}

/* The link is up: the first time, and after every reconnect. The tracker has been read by the
   caller (session.js); this reads the rest. */
export async function startWorkspace() {
  await readDesktop()
  hearSettings()
  if (!started) {
    started = true
    await Promise.all([initAgents(), initRuns(), initConversation(), initTerminals()])
  }
  await loadProject(settings.activeProject)
  workspace.ready = true
}

/* The person is looking again. What the desktop shows may have moved while the phone was
   away: its project, so the board is read again when that differs. */
export async function refreshFromDesktop() {
  const before = settings.activeProject
  await readDesktop()
  if (settings.activeProject !== before) await resync()
  await loadProject(settings.activeProject)
}

/* Switching the project here switches the desktop's too: `tracker_set_project` is the one
   active project of the worker, and the epic decided the phone follows it rather than keeping
   one of its own. The page's own state is brought to the new project after the answer. */
export async function chooseProject(path) {
  if (!path || workspace.switching) return false
  workspace.switching = path
  try {
    await setProject(path)
    if (trackerState.lastError) return false
    await readDesktop(path)
    settings.activeProject = path
    await loadProject(path)
    return true
  } finally {
    workspace.switching = null
  }
}
