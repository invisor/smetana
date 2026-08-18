/* The window's projects: the list's contents, which one is active, and moving
   between them. One of the front end's files that know about Tauri — along with
   tracker.js, settings.js and files.js.

   The truth about the list's contents lives in settings (only this interface
   writes them), the truth about the board lives in bd. Hence a switch has two
   halves: settings_load brings the layout, tracker_set_project brings the
   issues. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { listDir, setRoot } from './files.js'
import { measureStorage } from './notifications.js'
import { loadRepos } from './vcs.js'
import { flushPending, loadProjectLayout, settings } from './settings.js'
import { confirmUnsaved, resetTabs, restoreTabs } from './tabs.js'
import { loadSessions } from './terminals.js'
import { initBd, probeProjects, setProject } from './tracker.js'
import { basename } from '../paths.js'

/* Path → whether it holds a .beads. health says the same thing about the
   active project, but there is nowhere else to learn it about the other rows. */
const probes = reactive({})

/* Re-exported rather than defined here: this store was the first of three
   places to need it, which is how it came to live in one of them. It is one
   module now (src/paths.js) and this name is kept so its importers do not have
   to care where it moved to. */
export { basename }

export const projectRows = computed(() =>
  settings.openProjects.map((path) => ({
    path,
    name: basename(path),
    /* Until the probe comes back we assume a tracker is there: showing a
       warning and then taking it away is worse than showing it half a second
       later. */
    tracked: probes[path] ?? true
  }))
)

export const activePath = computed(() => settings.activeProject)

export async function refreshProbes() {
  const rows = await probeProjects([...settings.openProjects])
  for (const row of rows) probes[row.path] = row.tracked
}

/* setProject costs about two seconds, and nothing stops a person clicking a
   second row (or removing a project) while the first is in flight. Without this
   flag the answer that came back last would win — not the click that was last —
   and a shared finally would clear trackerState.switching in the middle of
   somebody else's trip. switchTo/addProject/removeProject check it first thing
   and silently do nothing if a move is already under way: the board shows a
   skeleton for that time anyway, the person can see work is happening, and a
   second attempt adds nothing. It is raised and cleared through try/finally so
   a failure (setProject throwing, say) does not lock the list forever. */
let moving = false

/* The shared part of a move: settings_load brings the new project's layout,
   tracker_set_project brings the issues. Callers are responsible for the moving
   flag themselves, and for flushPending() of the departing project's state
   before the call.

   Only the layout is taken from the disk (loadProjectLayout). The open list and
   the active project stay as this file made them: what lies on the disk at that
   moment is certainly the past, and a full loadSettings would bring a removed
   project back or erase one that was just added. */
async function moveTo(path) {
  settings.activeProject = path
  await loadProjectLayout(path)
  /* The old project's tree and buffers must not outlive it by a single frame.
     The tab list has already arrived from the new project's settings — that is
     what we restore. */
  resetTabs()
  setRoot(path)
  /* The Git panel, after `loadProjectLayout` and deliberately not from a
     watcher on the active project: which repository this project was left
     showing lives in its own settings entry, and a watcher fires on the
     assignment above — one microtask later, with the previous project's entry
     still in place. It would then pick a repository nobody chose here and write
     it over the remembered one a moment before the layout landed. Not awaited:
     nothing about the move waits on git. */
  loadRepos(path)
  if (path) {
    await listDir('')
    await Promise.all(settings.project.expanded.map((dir) => listDir(dir)))
    /* Before the tabs, and awaited, which is the one ordering in this function
       that is about another store's answer rather than about this one's work.
       `restoreTabs` repairs an `activeTab` of "terminal" when the project has no
       agent — sessions do not survive a restart — and it reads the session list
       to decide. The list is also loaded by the `activePath` watcher in
       DesktopApp.vue, but that is a race this may not lose: `terminal_list`
       queues behind whatever the terminal worker is already doing, and a spawn
       is about a second, while the two `files_list_dir` calls above are far
       quicker. Left unordered, switching to a project *while an agent starts*
       repairs it against the previous project's list — the new project opens on
       the board although its agent is live, and the repaired value is then
       written into its settings by the debounce. `onMounted` in DesktopApp.vue
       already orders these two the same way explicitly; this is the other
       caller. The duplicate request costs one round trip and nothing else —
       `loadSessions` guards against its own stale response — which is the same
       trade the duplicated `loadConfig` beside it already makes. */
    await loadSessions(path)
    await restoreTabs()
  }
  await setProject(path)
  refreshProbes()
  /* Last, and both halves of that matter. The store is weighed per project and
     the answer is written into that project's own settings entry, so this has
     to come after `loadProjectLayout` — before it, the number would be compared
     against, and written over, the layout of the project just left. And after
     `setProject`, because `attachments_survey` is answered against the tracker
     worker's idea of the active project: asked earlier it would answer about
     the old folder, which the guard in `notifications.js` would then throw
     away, costing the new project its measurement until the next window focus.
     Not awaited — nothing about the move waits on the bell. */
  measureStorage(path)
}

/* The move. The order matters: first we flush the departing project's state,
   then we fetch the new one's layout, and only then do we ask for the board —
   that costs about two seconds, and the screen shows a skeleton all the while. */
export async function switchTo(path) {
  if (path === settings.activeProject || moving) return
  moving = true
  try {
    if (!(await confirmUnsaved())) return
    await flushPending()
    await moveTo(path)
  } finally {
    moving = false
  }
}

/* Pick a subfolder of a tracked repository and its root is what opens. We
   normalize exactly once, before the path reaches the list: the key in
   settings, the row in the list and the worker's directory have to be one and
   the same path. If there is nothing tracked above, the path stays as it is and
   the person is offered bd init right in it. */
async function projectRoot(path) {
  try {
    return await invoke('project_root', { path })
  } catch (err) {
    console.error('[projects] could not determine the project root:', err)
    return path
  }
}

/* Returns the added path on every success, and null on every cancellation —
   the picker closed with nothing chosen, a move was already under way, or the
   person declined to lose unsaved changes. The caller (DesktopApp's
   onAddProject) tells "added" from "cancelled" by this value alone, so a bare
   return here would read as a cancellation. */
export async function addProject() {
  if (moving) return null
  let picked = null
  try {
    picked = await open({ directory: true, multiple: false, title: 'Add project' })
  } catch (err) {
    console.error('[projects] picking a folder failed:', err)
    return null
  }
  if (!picked) return null
  const path = await projectRoot(picked)
  /* The dialog itself takes as long as it takes — a move may have started and
     finished while the person was picking a folder, so the flag is checked
     again after it returns, not only on the way into the function. */
  if (moving) return null

  moving = true
  try {
    /* We only ask where there will be a move: if the chosen project is already
       active, the tabs stay put and the question would be about nothing. It
       still comes before flushPending: the departing project's state must not
       land on disk before the person has decided whether to move at all. */
    if (path !== settings.activeProject && !(await confirmUnsaved())) return null
    /* First we flush the departing project's state and only then change the
       list: a write started after the row was added would carry the old
       project's state under the new list. The order here must not depend on
       whether the debounce timer managed to start — flushPending is responsible
       for that itself. */
    await flushPending()
    if (!settings.openProjects.includes(path)) settings.openProjects.push(path)
    if (path === settings.activeProject) return path
    await moveTo(path)
    return path
  } finally {
    moving = false
  }
}

/* Removal from the list. The project's state stays in the settings file and
   comes back if it is opened again. The next project becomes active, or the
   previous one for the last row; an emptied list leaves the window without a
   project, and that is a normal state. */
export async function removeProject(path) {
  if (moving) return
  moving = true
  try {
    /* The row is already gone — nothing to ask about and nothing to write. */
    if (!settings.openProjects.includes(path)) return
    /* The question is only about a move. Removing an inactive row does not
       touch the tabs at all, and "Don't save" would erase the current project's
       edits for nothing. And still before flushPending — for the same reason as
       in switchTo. */
    if (path === settings.activeProject && !(await confirmUnsaved())) return
    /* As in switchTo: the departing project's state has to land on disk before
       the list changes, otherwise the four-hundred-millisecond debounce would
       overwrite an unsaved edit with an already-shortened list. */
    await flushPending()
    /* The index is taken after the awaits: the question takes as long as it
       takes, and the row's position may have moved by then. */
    const at = settings.openProjects.indexOf(path)
    if (at === -1) return
    settings.openProjects.splice(at, 1)
    delete probes[path]

    if (path !== settings.activeProject) return

    const next = settings.openProjects[at] ?? settings.openProjects[at - 1] ?? null
    await moveTo(next)
  } finally {
    moving = false
  }
}

/* bd init in the active folder. We swallow the error: Toast has already shown
   it through trackerState.lastError. */
export async function initActive() {
  try {
    await initBd()
    await refreshProbes()
  } catch {
    /* the message already sits in trackerState.lastError */
  }
}

/* The first run: settings brought an active project that is not in the list
   yet (it was found from the working directory). We put it in — and the list
   stops being empty for good. */
export function adoptInitialProject() {
  const active = settings.activeProject
  if (active && !settings.openProjects.includes(active)) settings.openProjects.push(active)
  refreshProbes()
}
