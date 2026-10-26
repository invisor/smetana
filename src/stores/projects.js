/* The window's projects: the list's contents, which one is active, and moving
   between them. One of the front end's files that know about Tauri — along with
   tracker.js, settings.js and files.js.

   The truth about the list's contents lives in settings (only this interface
   writes them), the truth about the board lives in bd. Hence a switch has two
   halves: settings_load brings the layout, tracker_set_project brings the
   issues. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { listDir, setRoot } from './files.js'
import { measureStorage } from './notifications.js'
import { loadRepos } from './vcs.js'
import { flushPending, loadProjectLayout, settings } from './settings.js'
import { confirmUnsaved, dirtyPaths, resetTabs, restoreTabs } from './tabs.js'
import { loadSessions } from './terminals.js'
import { initBd, probeProjects, setProject } from './tracker.js'
import { basename, dirname } from '../paths.js'
import { refusalOf } from './connection.js'

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

/* A switch the phone made while a move was under way. Never dropped: the
   worker has already gone to that folder, and a window that did not follow
   would sit on one project with the board of another. It is kept as `{ path }`
   (the path may be `null`) and settled by `release`, the one place that ends a
   move. Only the last one matters — each is the phone's latest word. */
let pendingRemote = null

/* True while an unsaved-edits question is up. The question can stay up for hours
   on an unattended desktop, and while it does no `setProject` is in flight, so a
   phone's switch that arrives meanwhile has left the worker on the phone's
   folder with nothing to bring it back: `followRemoteProject` puts it back at
   once. Every `confirmUnsaved` in a move goes through `askUnsaved`. */
let asking = false

async function askUnsaved() {
  asking = true
  try {
    return await confirmUnsaved()
  } finally {
    asking = false
  }
}

/* Ends a move, and settles what the phone said during it. Every `finally` that
   clears `moving` calls this and never assigns the flag itself, so no road out
   of a move can forget the announcement. The follow runs on its own turn: it
   asks the worker for things, and the caller is on its way out. */
function release() {
  moving = false
  if (!pendingRemote) return
  const { path } = pendingRemote
  pendingRemote = null
  followRemoteProject(path).catch((err) => {
    console.error("[projects] could not follow the phone's project switch:", err)
  })
}

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
    /* Started here and awaited below, which is deliberate on both ends.
       `restoreTabs` repairs an `activeTab` of "terminal" when the project has no
       agent — sessions do not survive a restart — and it reads the session list
       to decide, so it may not run before that list is the *new* project's. Left
       unordered it is a race this loses often enough to matter: `terminal_list`
       queues behind whatever the terminal worker is already doing and a spawn is
       about a second, while the directory reads below are far quicker — so
       switching to a project *while an agent starts* would repair it against the
       project just left, opening it on the board although its agent is live and
       writing that over its settings on the next debounce. `onMounted` in
       DesktopApp.vue orders the same two calls explicitly; this is the other
       caller.

       Started before the directory reads rather than after them because the
       ordering this needs is against `restoreTabs` alone, and nothing above that
       line reads the session list. Awaiting it here instead would serialise the
       whole switch — `setProject` and its tracker call included — behind that
       same queued second, and `switchTo` ignores clicks while a move is up, so
       the window in which the project list stops responding would grow by it.
       `loadSessions` sets `terminalState.project` synchronously, before its
       first await, so the guard that drops a stale answer is armed the moment
       this is called rather than when it is awaited.

       The list is loaded a second time by the `activePath` watcher in
       DesktopApp.vue. That duplicate costs one round trip and nothing else —
       `loadSessions` guards against its own stale response — which is the same
       trade the duplicated `loadConfig` beside it already makes. */
    const sessions = loadSessions(path)
    /* The root first and the expanded folders only if it answered — the rule is
       `refreshDirs`' and the reason is the same one: a project folder that is
       gone answers `notFound` for every directory inside it, and each of those
       would be folded out of `expanded` as if somebody had deleted it. */
    if (await listDir('')) {
      await Promise.all(settings.project.expanded.map((dir) => listDir(dir)))
    }
    await sessions
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
    if (!(await askUnsaved())) return
    await flushPending()
    await moveTo(path)
  } finally {
    release()
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

/* Where the folder panel opens, for the reason set out at length over
   `pickImages` in attachments.js: with no `defaultPath` macOS starts it in
   Recents, which is every application's files at once — the Photos library,
   other applications' containers, network mounts — and an unsandboxed app is
   charged for every preview the panel builds there. The same shape of answer,
   with a different starting point: the folder of the last choice made here, and
   before there is one, the folder the current project sits in, since a person's
   projects usually sit next to each other.

   Not `settings.json`, again: this resets to the project's parent when the app
   restarts, and that is a reasonable place to start from. */
let lastPickedDir = null

/* Returns the added path on every success, and null on every cancellation —
   the picker closed with nothing chosen, a move was already under way, or the
   person declined to lose unsaved changes. The caller (DesktopApp's
   onAddProject) tells "added" from "cancelled" by this value alone, so a bare
   return here would read as a cancellation. */
export async function addProject() {
  if (moving) return null
  let picked = null
  try {
    picked = await open({
      directory: true,
      multiple: false,
      title: 'Add project',
      /* `undefined` and not `null`: with no project open there is nothing to
         take a parent of, and the option has to be absent rather than empty —
         which puts the panel back exactly where it was before this. */
      defaultPath: lastPickedDir ?? dirname(settings.activeProject) ?? undefined
    })
  } catch (err) {
    console.error('[projects] picking a folder failed:', err)
    return null
  }
  if (!picked) return null
  /* The folder the choice was made in, and deliberately the parent of what the
     panel handed back rather than of the root `projectRoot` climbs to: that
     folder is what was on screen, and where the tracked root turns out to be is
     a different question. Only on a choice — a cancelled panel returns above
     with the memory untouched. */
  lastPickedDir = dirname(picked) ?? lastPickedDir
  return addProjectAt(picked)
}

/* The second half of adding a project, from a folder that has already been chosen: by the
   operating system's panel on this machine (`addProject` above), or in the server-side folder
   browser when the server is on another one (`listServerFolders`, below). Same contract: the added
   path on success, null on any cancellation. */
export async function addProjectAt(picked) {
  if (moving) return null
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
    if (path !== settings.activeProject && !(await askUnsaved())) return null
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
    release()
  }
}

/* ---- a server on another machine ---------------------------------------- */

/* The folders under `path` on the server (its home folder when `path` is null): `{ root, path,
   parent, entries: [{ name, path, repo, tracker }], truncated, repo, tracker }`. The server
   refuses anything that is not under its root; the browser never composes a path of its own, it
   only passes back ones the server listed. Rejects with the server's words. */
export async function listServerFolders(path = null) {
  try {
    return await invoke('folders_list', { path })
  } catch (err) {
    throw new Error(refusalOf(err))
  }
}

/* `git clone` on the server into `parent/name` (the address's last segment when `name` is empty).
   Answers the new folder's path. The server runs git with no terminal, so a repository that wants
   credentials the server does not have fails in git's own words, which come back here. */
export async function cloneOnServer({ url, parent, name = null }) {
  try {
    return (await invoke('project_clone', { url, parent, name: name || null })).path
  } catch (err) {
    throw new Error(refusalOf(err))
  }
}

/* The desktop's half of a switch made on the phone. The phone's
   `tracker_set_project` moves the tracker worker and nothing else, so the
   desktop announces the new folder here (`remote:project-switched`) and the
   window follows by the road a click on the list takes: `moveTo`, with the same
   question about unsaved edits. A folder not on the list is put on it, as
   `addProject` does.

   No loop: the window's own move calls the `tracker_set_project` command, which
   announces nothing — only a call that came over the bridge does — and a path
   that is already active is not a move at all.

   When the window does not follow (the person kept their unsaved edits, or the
   phone emptied the tracker with `null`) the worker is put back on the window's
   project, because a board that answers for one folder under another's header is
   the fault this exists to prevent. An announcement that arrives during another
   move is kept in `pendingRemote` and replayed by `release` when that move ends,
   so it is never lost. */
export async function followRemoteProject(raw) {
  if (moving) {
    pendingRemote = { path: raw }
    /* A question is up, so nothing is putting the worker right: do it now. */
    if (asking) await setProject(settings.activeProject)
    return
  }
  /* The root, as `addProject` takes it: a subfolder or a trailing slash must not
     become a second row. Compared with the active project only after this. */
  const path = raw ? await projectRoot(raw) : null
  /* A move may have started while the root was being asked for. */
  if (moving) {
    pendingRemote = { path }
    return
  }
  moving = true
  try {
    if (path && path !== settings.activeProject) {
      /* The question can stay up for hours on an unattended desktop, and until it
         is answered the board must not show the phone's project under this
         window's header. So with edits to lose the worker goes back first, and
         moves to the phone's project only once the person agrees. (The simpler
         alternative, leaving the switching skeleton up, would hide the board for
         the same hours.) With nothing to lose there is no question and no wait. */
      if (dirtyPaths.value.length) await setProject(settings.activeProject)
      if (await askUnsaved()) {
        await flushPending()
        if (!settings.openProjects.includes(path)) settings.openProjects.push(path)
        await moveTo(path)
      }
      return
    }
    /* `null`, or the window's own project. The worker is where the phone put it,
       which for a subfolder of this project is the subfolder, so it is the raw
       path that is compared: the window's project is what the worker must hold. */
    if (raw !== settings.activeProject) await setProject(settings.activeProject)
  } finally {
    release()
  }
}

/* Listens for the announcement above. Answers with the way to stop. */
export function initProjectFollow() {
  return listen('remote:project-switched', (event) => followRemoteProject(event.payload?.path ?? null))
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
    if (path === settings.activeProject && !(await askUnsaved())) return
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
    release()
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
