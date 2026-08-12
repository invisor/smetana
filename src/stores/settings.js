/* The app's settings in the front end. Components see an ordinary reactive
   object; this file is the only one that knows about Tauri, the disk and the
   schema version — the way tracker.js is the only one that knows about the
   tracker.

   The difference from the tracker is fundamental: there the truth is outside,
   in bd, and the store catches up with it through deltas. Here the truth is in
   this object — only this interface changes the settings, and Rust is
   responsible for the schema and the disk. */
import { nextTick, reactive, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { emit, listen } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
/* Pure, no Vue and no DOM: what the theme control offers and what a font size
   is allowed to be. Imported so the settings window and this store cannot
   disagree about which values are legal. */
import { EDITOR_FONT_DEFAULT, THEME_CHOICES, UI_FONT_DEFAULT, clampFont } from '../appearance.js'
/* tabs.js imports settings.js and we import it back: the cycle is closed but
   harmless. Both sides touch each other only inside functions, and by the time
   of the first call both modules have been evaluated. */
import { confirmUnsaved } from './tabs.js'
/* A pure module with no Vue and no DOM — the panel width rules and their
   defaults. Imported so that these two numbers do not end up with two copies in
   the front end. */
import { LEFT_DEFAULT, RIGHT_DEFAULT } from '../views/panelWidths.js'

/* The defaults mirror the ones in Rust. With no back end (a browser) or after
   a failed read, the app still has to open looking a known way. */
const defaults = () => ({
  /* `uiFontSize` scales the whole type scale rather than naming one size — see
     `fontVars` in `../appearance.js` — and the editor's own size sits in its own
     section because it is pinned rather than scaled. Both mirror Rust's
     `UI_FONT_DEFAULT` / `EDITOR_FONT_DEFAULT`. */
  appearance: { theme: 'dark', density: 'comfortable', uiFontSize: UI_FONT_DEFAULT },
  editor: { fontSize: EDITOR_FONT_DEFAULT },
  layout: {
    leftCollapsed: false,
    rightCollapsed: false,
    leftWidth: LEFT_DEFAULT,
    rightWidth: RIGHT_DEFAULT
  },
  /* Which agent the app starts — the Agents tab of the settings window is what
     changes it, through `applyPatch` below. The defaults here and in Rust have
     to agree, the same as appearance and layout do. */
  agent: 'claude',
  openProjects: [],
  activeProject: null,
  project: {
    sideTab: 'files',
    activeTab: 'kanban',
    selectedTask: null,
    selectedPath: null,
    expanded: [],
    openTabs: [],
    previewTab: null,
    /* The board's columns in the order a person dragged them to. Empty means
       "never rearranged", and the board then draws bd's own order. */
    columnOrder: [],
    /* What the run dialog was last set to here — null until somebody runs
       something. Listed rather than left out, and the difference is not
       cosmetic: applySection is Object.assign(target, defaults, stored), so a
       key missing from this object is a key the defaults layer cannot clear.
       Switching projects would then leave the previous project's remembered
       target branch in place unless Rust happened to send an explicit null,
       and it would be prefilled in the one dialog whose whole job is being the
       last cheap place to notice a run aimed at the wrong thing. */
    runSettings: null,
    /* The highest attachment-storage threshold this project has been warned
       about, in MiB — null until it has been warned about any. The one thing
       the notification bell keeps between runs, and per project because the
       folder it is about is (`stores/notifications.js`). Listed here for the
       reason `runSettings` above spells out: a key missing from this object is
       a key the defaults layer cannot clear, and switching projects would carry
       the previous one's number across — which would silence a warning for a
       folder nobody has ever been warned about. */
    storageWarnedMib: null,
    usedAt: null
  }
})

/* Exported for the browser mock: it answers with these same defaults, and a
   second copy of them must not exist in the project. */
export { defaults }

export const settings = reactive(defaults())

/* A write costs a trip to the disk, and a panel changes dozens of times during
   one drag. We accumulate and write once, when the stream settles. */
const SAVE_DELAY = 400
/* How long we wait for the write while closing. Longer, and a wedged IPC turns
   into a window that will not close; that is worse than one lost last edit. */
const CLOSE_FLUSH_LIMIT = 2000
let timer = null
let watching = false
let closing = false

/* Writes are chained: there is never more than one in flight at a time. Rust
   writes through a temp file and a rename, and two overlapping writes would
   race for order — the second could land on disk before the first. */
let chain = Promise.resolve()

function scheduleSave() {
  clearTimeout(timer)
  timer = setTimeout(flush, SAVE_DELAY)
}

function flush() {
  timer = null
  /* A reactive proxy does not survive the trip across the IPC: we send a plain
     object. structuredClone is not allowed here — the build target is es2021.
     The snapshot is taken now rather than at send time: the state would move on
     while sitting in the queue. */
  const snapshot = JSON.parse(JSON.stringify(settings))
  chain = chain.then(() =>
    invoke('settings_save', { settings: snapshot }).catch((err) => {
      console.error('[settings] save failed:', err)
    })
  )
  return chain
}

/* Sends what is pending right away and returns a promise that shows when the
   disk has caught up with the state.

   Awaiting a tick is mandatory: the settings watcher is deferred to a
   microtask, so whoever changed a field and called us in the same synchronous
   block (as the projects store does before changing the list) would find the
   timer not yet set. Without the wait, "there is nothing to flush" would merely
   mean "it has not happened yet", and the departing project's edit would land
   on disk later — already carrying somebody else's list.

   The promise always resolves: nextTick is a microtask, not a trip outwards, so
   both the window-close handler with its two-second ceiling and beforeunload
   get what they used to get, only one tick later. */
export async function flushPending() {
  await nextTick()
  if (timer) {
    clearTimeout(timer)
    return flush()
  }
  return chain
}

/* Closing the window — the close button, Cmd+W, the system "close window"
   menu — goes through this handler. Quitting with Cmd+Q is a different matter:
   on macOS that can end the process without ever delivering a per-window close
   request to the webview, and nobody has verified this case here. Only the
   SAVE_DELAY debounce (400 ms) covers it — if an edit falls inside it and the
   handler never saw the Cmd+Q, it may not reach the disk. For the closes that
   do reach us we ask Tauri to hold them: we flush the write and destroy the
   window ourselves.

   What is promised here: the window will close. A repeat request is ignored,
   the wait for the write is capped by CLOSE_FLUSH_LIMIT, and destroy is called
   even after a failed write. What is not promised: that the edit reaches the
   disk — if the back end stays silent for two seconds the window closes anyway
   and the edit is lost. */
async function closeAfterFlush() {
  if (closing) return
  closing = true
  /* The question about unsaved files comes before the settings flush, not
     inside its two-second ceiling: settings may be lost, somebody's work may
     not. A failing handler has no right to lock the window: we assume closing
     is allowed and carry on. */
  let mayClose = true
  try {
    mayClose = await confirmUnsaved()
  } catch (err) {
    console.error('[settings] the unsaved-work question did not work out:', err)
  }
  if (!mayClose) {
    closing = false
    return
  }
  try {
    await Promise.race([
      flushPending(),
      new Promise((resolve) => setTimeout(resolve, CLOSE_FLUSH_LIMIT))
    ])
  } catch (err) {
    console.error('[settings] the write on close failed:', err)
  }
  try {
    await getCurrentWindow().destroy()
  } catch (err) {
    /* A failure here must not lock the window forever: we clear the flag so
       the next close request reaches this handler again instead of being
       silently swallowed by the re-entrancy guard. The write has finished by
       now (successfully or not), so a second pass will not repeat the flush. */
    closing = false
    console.error('[settings] closing the window failed:', err)
  }
}

/* In a browser there is no Tauri window: getCurrentWindow throws synchronously,
   before it even gets to the subscription. That is a normal mode, not a
   breakage — there is nothing to intercept in a browser, the window simply does
   not exist for it, and the little that can be saved there is already covered
   by the fallback beforeunload; hence debug rather than error here — a red line
   on every page load would hide real errors. The subscription (the second
   catch) is a different matter: that is real Tauri, and if onCloseRequested
   rejects (an ACL, say) that is a signal, not the norm — so the level there
   stays as it was. */
function watchClose() {
  try {
    getCurrentWindow()
      .onCloseRequested((event) => {
        event.preventDefault()
        closeAfterFlush()
      })
      .catch((err) => console.warn('[settings] window close not intercepted:', err))
  } catch (err) {
    console.debug('[settings] window close not intercepted (a browser, there is no window):', err)
  }
}

/* Sections are merged in place rather than replaced wholesale: the view
   captures settings.layout and settings.project by reference once, at creation,
   and replacing the object would leave it reading and writing something the
   settings no longer hold — the new project's layout would never appear on
   screen, and everything the person changed afterwards would never reach the
   disk.

   The argument order matters: the defaults come before the stored values, so a
   project that is not in the map starts from a clean state instead of wearing
   the previous one's fields. */
function applySection(target, fallback, stored) {
  Object.assign(target, fallback, stored)
}

/* The read at startup: it applies everything — appearance, layout, the project
   list and the active project. Called once; switching projects goes for the
   layout through loadProjectLayout, because the front end knows the list's
   contents, not the disk. */
export async function loadSettings() {
  try {
    const stored = await invoke('settings_load', { project: null })
    const base = defaults()
    applySection(settings.appearance, base.appearance, stored.appearance)
    applySection(settings.editor, base.editor, stored.editor)
    applySection(settings.layout, base.layout, stored.layout)
    applySection(settings.project, base.project, stored.project)
    settings.openProjects = stored.openProjects ?? []
    settings.activeProject = stored.activeProject ?? null
    settings.agent = stored.agent ?? base.agent
  } catch (err) {
    console.error('[settings] the read failed, taking the defaults:', err)
  }

  /* Watching starts only after the load: otherwise the layout of the values
     just read would go back to disk on its own as a "change". */
  if (!watching) {
    watch(settings, scheduleSave, { deep: true })
    window.addEventListener('beforeunload', flushPending)
    watchClose()
    watching = true
  }
  return settings
}

/* ---- the settings window ------------------------------------------------

   The settings window is a second webview over the same settings file, and the
   rule that keeps the two from destroying each other is that **the main window
   stays the only writer**. `settings_save` writes the whole resolved view — the
   panel widths, the project map, the open tabs — so a settings window calling it
   would post its own idea of everything else, and whichever write landed last
   would win. The window that has the whole picture is the one that saves it.

   So the traffic is three events and nothing else:

   - `settings:hello` — the settings window has opened and wants the truth;
   - `settings:state` — the main window's answer, and its announcement after any
     change: the flat five fields the settings window draws;
   - `settings:apply` — one edit, from the settings window to the main window.

   The payload is flat rather than a slice of the settings tree, because it is a
   message and not the settings: three of its five fields live in three different
   sections, and a nested shape would invite somebody to send a whole section and
   quietly blank the fields they left out. */
export const SETTINGS_APPLY = 'settings:apply'
export const SETTINGS_STATE = 'settings:state'
export const SETTINGS_HELLO = 'settings:hello'

/* Both the store's live values and a raw `settings_load` answer go through here,
   so the settings window cannot be shown one shape by the main window and
   another by the disk. Missing sections take the defaults: a file written before
   these fields existed is the ordinary case, not an error. */
function toShared(source) {
  const base = defaults()
  const appearance = { ...base.appearance, ...source.appearance }
  const editor = { ...base.editor, ...source.editor }
  return {
    theme: appearance.theme,
    density: appearance.density,
    uiFontSize: appearance.uiFontSize,
    editorFontSize: editor.fontSize,
    agent: source.agent ?? base.agent
  }
}

/* What the settings window draws, taken from this window's live state — which
   may be newer than the disk, since a save is 400 ms behind. */
export const sharedSettings = () => toShared(settings)

/* One edit from the settings window, landing in the main window's state — from
   where the ordinary debounce carries it to disk. Nothing else in the app writes
   these fields, so this is the whole of the settings screen's power.

   Every field is checked, and a field that fails its check is skipped rather
   than reset: this arrives as an event, and an event is not a response to
   anything — a malformed one must cost nothing. The fallbacks are the values
   already held, not the shipped defaults, for the same reason.

   `agent` is the exception that proves the rule: the list of agent ids lives in
   `agents::IDS` and Rust is the only party that holds it, so anything non-empty
   travels and an id nobody ships is dropped on the way to the file — which is
   exactly what `Settings::validate` already does for a hand-edited one. */
export function applyPatch(patch) {
  if (!patch || typeof patch !== 'object') return
  if (THEME_CHOICES.some((choice) => choice.value === patch.theme)) {
    settings.appearance.theme = patch.theme
  }
  if ('uiFontSize' in patch) {
    settings.appearance.uiFontSize = clampFont(patch.uiFontSize, settings.appearance.uiFontSize)
  }
  if ('editorFontSize' in patch) {
    settings.editor.fontSize = clampFont(patch.editorFontSize, settings.editor.fontSize)
  }
  if (typeof patch.agent === 'string' && patch.agent) {
    settings.agent = patch.agent
  }
}

function announce() {
  emit(SETTINGS_STATE, sharedSettings()).catch((err) => {
    console.error('[settings] telling the settings window failed:', err)
  })
}

let bridged = false

/* The main window's half. Called once, from the app view: from here on an edit
   made in the settings window is an ordinary change to this object, and the
   watcher already installed by loadSettings takes it to disk. */
export async function initSettingsBridge() {
  if (bridged) return
  bridged = true
  try {
    await listen(SETTINGS_APPLY, (event) => {
      applyPatch(event.payload)
      /* Announced rather than assumed: the window that sent the edit applied it
         to its own copy a moment ago, and this is what corrects it when a value
         was refused — otherwise a rejected size would sit on screen looking
         chosen. */
      announce()
    })
    await listen(SETTINGS_HELLO, announce)
  } catch (err) {
    /* A browser, or an ACL. The app is fully usable without the settings
       window; nothing else depends on these subscriptions. */
    console.warn('[settings] the settings window bridge did not start:', err)
    bridged = false
  }
}

/* The settings window's half. It holds no store of its own — these three
   functions are the whole of its contact with the settings. */
export function sendSettingsPatch(patch) {
  return emit(SETTINGS_APPLY, patch).catch((err) => {
    console.error('[settings] sending the change to the app window failed:', err)
  })
}

export async function watchSharedSettings(onState) {
  const stop = await listen(SETTINGS_STATE, (event) => onState(event.payload))
  /* Asked for after the subscription, never before: the answer is an event too,
     and a hello sent first could be answered into a window that is not listening
     yet.

     Not awaited, and that is the point of the ordering here: the subscription
     already exists and has to reach the caller whatever the hello does. Awaiting
     it meant a rejected hello threw past the `return`, leaving a live listener
     nobody held the way to stop. A hello that never went is a window drawing the
     file's values instead of this window's, which is the fall-back it already
     has. */
  emit(SETTINGS_HELLO, null).catch((err) => {
    console.warn('[settings] the app window was not asked for the current values:', err)
  })
  return stop
}

/* The disk, read directly, for the moment before the main window has answered —
   and for `npm run dev`, where there is no main window to answer at all. A read
   and never a write: the one-writer rule is about `settings_save`. */
export async function readSharedSettings() {
  const stored = await invoke('settings_load', { project: null })
  return toShared(stored ?? {})
}

/* One project's layout — and only that. This is how the projects store picks
   up a new project's state on a switch, without restarting the app. This
   function deliberately does not touch the open list, the active project or the
   appearance: their truth lives in the front end, and an answer from the disk
   is certainly the past (the move starts before the debounce manages to write
   the new list). Taking the list from there would bring back a project that was
   just removed, or lose one that was just added. */
export async function loadProjectLayout(project) {
  /* No projects are left. There is nowhere for a layout to come from, and
     settings_load with no argument would answer about the active project from
     the file, that is, about the project that was just closed: we set the
     defaults. */
  if (!project) {
    Object.assign(settings.project, defaults().project)
    return
  }
  try {
    const stored = await invoke('settings_load', { project })
    applySection(settings.project, defaults().project, stored.project)
  } catch (err) {
    console.error('[settings] reading the project layout failed:', err)
  }
}
