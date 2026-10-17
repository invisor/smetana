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
import { initRuns, loadConfig, loadRun, runsState } from '../../src/stores/runs.js'
import { initConversation, adoptRunSession } from '../../src/stores/conversation.js'
import { initTerminals, loadSessions } from '../../src/stores/terminals.js'
import { loadBranches } from '../../src/stores/git.js'
import { resync, setProject, trackerState } from '../../src/stores/tracker.js'
import { invoke } from './bridge/core.js'
import { applyDesktopSettings } from './desktopSettings.js'

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
  applyDesktopSettings(settings, stored)
  return stored
}

/* What belongs to one project and is read for it: the sessions, the configuration, the runs.
   A run that is being driven as a conversation is adopted, as the desktop does on `run:state`,
   so that its session is in the Agents list. */
export async function loadProject(project) {
  if (!project) return
  /* The run stores hold one project at a time. While the Runs screen is showing another one
     (a run notification named it), moving them back to this project would leave the screen
     naming one project over the other's runs and configuration, so they are left alone. */
  const runsElsewhere = runsPin !== null && runsPin !== project
  await Promise.all([
    loadSessions(project),
    runsElsewhere ? null : loadConfig(project),
    runsElsewhere
      ? null
      : loadRun(project).then(() => {
          for (const run of runsState.runs) {
            if (run.transport === 'driven' && run.session != null) adoptRunSession(run.project, run.session)
          }
        })
  ])
}

/* The project the Runs screen is showing, while it is showing one; `null` otherwise. */
let runsPin = null

/* The Runs screen is gone: the run stores follow the active project again, and are brought to
   it now if they were left on another. */
export function unpinRuns() {
  const was = runsPin
  runsPin = null
  if (was !== null && was !== settings.activeProject) {
    loadProject(settings.activeProject).catch((err) => console.warn('[companion] could not read the project:', err))
  }
}

/* The link is up: the first time, and after every reconnect. The tracker has been read by the
   caller (session.js); this reads the rest. */
export async function startWorkspace() {
  await readDesktop()
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
  /* A Runs screen on another project is read again too, or it would be stale after the
     phone was in the background. */
  if (runsPin !== null && runsPin !== settings.activeProject) await loadRunsFor(runsPin)
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

/* The runs screen for a project that is not the one in front (a notification named it). The
   run stores keep one project at a time, so this is the same load the active project gets. */
export async function loadRunsFor(project) {
  runsPin = project
  await Promise.all([loadConfig(project), loadRun(project), loadBranches(project)])
}
