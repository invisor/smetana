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
/* Pure, no Vue and no DOM: what the board's two view settings may be, and the
   shape of the two column lists beside them. Imported so this store and the
   settings tab cannot disagree about which values are legal — and so that what
   the tab offers stays a subset of what Rust accepts, since a value Rust
   refuses would lose itself on the next save with nothing on screen to say so. */
import {
  COLUMN_MODES,
  INTERVALS,
  KANBAN_DEFAULTS,
  columnNames
} from '../components/kanban/boardView.js'
/* Pure, no Vue and no DOM: the closed list of notification sounds and the two
   shipped ones. Imported for the reason `boardView.js` above it is — so this
   store and the settings tab cannot disagree about which values are legal, and
   so what the tab offers stays a subset of what Rust accepts. */
import { NOTIFICATION_DEFAULTS, isSound } from '../sounds.js'

/* The defaults mirror the ones in Rust. With no back end (a browser) or after
   a failed read, the app still has to open looking a known way. */
const defaults = () => ({
  /* `uiFontSize` scales the whole type scale rather than naming one size — see
     `fontVars` in `../appearance.js` — and the editor's own size sits in its own
     section because it is pinned rather than scaled. Both mirror Rust's
     `UI_FONT_DEFAULT` / `EDITOR_FONT_DEFAULT`. */
  appearance: { theme: 'dark', density: 'comfortable', uiFontSize: UI_FONT_DEFAULT },
  /* `wordWrap` off is today's behaviour to the letter — long lines scroll
     sideways — which is the argument the `kanban` defaults carry, and
     deliberately not `git.autoFetch`'s: wrapping shows itself on the first file
     opened, so shipping it on would re-lay somebody's editor out unasked. The
     copies of this default in `EditorSettings::default()` (Rust) and in `view`
     (`views/SettingsWindow.vue`) have to agree with this one. */
  editor: { fontSize: EDITOR_FONT_DEFAULT, wordWrap: false },
  /* What the app may do to a person's repositories without asking each time.
     `autoFetch` is whether this app opens a socket by itself — on window focus,
     throttled, and silently — to find out whether a branch has commits waiting
     for it. `removeWorktrees` is whether a run sweeps up each task's checkout
     once it is merged and closed; nothing in this app runs `git worktree` at
     all, so what that one reaches is a line of the run prompt the lead reads.
     Both global rather than per project for the reason `GitSettings` in Rust
     records: they are facts about a connection and a person, not about a
     repository, and that file carries the same two defaults. `removeWorktrees`
     ships on because that is today's behaviour exactly. A section missing there
     is a section this window cannot draw. */
  git: { autoFetch: true, removeWorktrees: true },
  /* What the main window does with the size and position it was left at.
     `restoreGeometry` off stops it being put back and never stops it being
     saved — `src-tauri/src/window.rs` holds that half — so switching it back on
     a week later opens the window where it was last left rather than at the
     size in the configuration. Global rather than per project for the reason
     `WindowSettings` in Rust records: there is one main window, and it is a
     fact about a person's screen. Shipped on, because that is today's
     behaviour exactly, and Rust carries the same default. */
  window: { restoreGeometry: true },
  /* How the board is drawn — which columns get a slot and how far back a card
     is worth looking at. Global rather than per project, for the reason
     `KanbanSettings` in Rust records, and shipped as today's board exactly:
     every column, every task. The rule these four feed is
     `components/kanban/boardView.js`. */
  kanban: { ...KANBAN_DEFAULTS, alwaysShow: [], unlimited: [] },
  /* What the app says when a run ends or an agent stops to ask: which sound
     each of the two announcements makes, or `off` for none, whether those
     sounds wait until nobody is looking, and whether a finished run's report is
     put in front of the person at all. Global rather than per project for the
     reason `NotificationSettings` in Rust records — a noise is a fact about a
     person and a room, and wanting the document or not is a habit of reading.
     Rust holds the same four defaults, and a section missing here is a section
     the settings window cannot draw.

     The two booleans are written out here rather than taken from
     `NOTIFICATION_DEFAULTS`: that constant lives in `sounds.js`, which is the
     closed list of sounds and the two shipped ones, and neither a boolean about
     a document nor one about when to make a noise has any business in it.
     `showReport` ships `true` because that is today's behaviour — somebody
     updating the app must not find that reports have silently stopped arriving
     — and `onlyWhenUnfocused` ships `true` for the opposite reason, named in
     `sounds.js`: a sound is for the person who is not looking, so this is a
     change to what the app does and a deliberate one. */
  notifications: { ...NOTIFICATION_DEFAULTS, onlyWhenUnfocused: true, showReport: true },
  layout: {
    leftCollapsed: false,
    rightCollapsed: false,
    /* Whether the project rail is drawn beside the left panel. Per window,
       beside the two widths, and not per project: it is a preference about this
       window's chrome, and a rail that appeared and vanished as somebody moved
       between projects would be a layout that shifted under a click. `Layout`
       in Rust carries the same default. */
    railOpen: true,
    leftWidth: LEFT_DEFAULT,
    rightWidth: RIGHT_DEFAULT,
    /* How the Git panel's three sections are folded, and how tall two of them
       were dragged to, in rows — `null` for "never dragged", which is a state
       and not a stand-in for a number: until there is a drag a section follows
       its own content. Global rather than per project for the reason
       `GitSections` in Rust records, and the rule that reads it is
       `components/git/sectionHeights.js`.

       Rust always sends this whole, so `applySection`'s Object.assign replacing
       it wholesale is the intended behaviour rather than a hazard. */
    gitSections: {
      reposRows: null,
      branchRows: null,
      reposOpen: true,
      changesOpen: true,
      branchesOpen: true
    }
  },
  /* Which agent the app starts — the Agents tab of the settings window is what
     changes it, through `applyPatch` below. The defaults here and in Rust have
     to agree, the same as appearance and layout do. */
  agent: 'claude',
  /* The language a session talks to the person in, the language the prose of a
     bd issue it writes is in, the language it writes a git commit message in,
     and the language the prose of a run's report is in — BCP-47 ids, every one
     of them at the root beside `agent` because which language somebody wants to
     be spoken to in is a habit of theirs rather than a property of a
     repository. The list of legal ids is `agents::LANGUAGES` in Rust; these
     defaults mirror its `en`. For `commitLanguage` that `en` is also today's
     behaviour to the letter: the commit-message prompt asked for English
     outright before the setting existed. `reportLanguage` moves the prose a
     run's lead writes into its batch file and nothing else in the document —
     `agents/prompt.rs` holds the whole of that watershed. */
  agentLanguage: 'en',
  taskLanguage: 'en',
  commitLanguage: 'en',
  reportLanguage: 'en',
  openProjects: [],
  activeProject: null,
  project: {
    sideTab: 'files',
    activeTab: 'kanban',
    selectedTask: null,
    /* The last three tasks somebody looked at here, newest first. Listed rather
       than left out, for the reason `runSettings` below spells out: applySection
       is Object.assign(target, defaults, stored), so a key missing from this
       object is a key the defaults layer cannot clear, and switching projects
       would leave one project's recents standing under another project's board.

       Written by a watch on `selectedTask` in `DesktopApp.vue` rather than by
       the command palette, so "recent" means every task somebody opened — from
       the board, from a run's claimed list, from anywhere — and not only the
       ones they found by searching. */
    recentTasks: [],
    selectedPath: null,
    /* Which repository the Git panel is showing, by absolute path — null until
       somebody has opened that tab in this project. Listed here for the reason
       `runSettings` below spells out: a key missing from this object is a key
       the defaults layer cannot clear, so switching projects would leave the
       previous project's repository in place, and the panel would ask git about
       a folder belonging to a project nobody is looking at. */
    selectedRepo: null,
    expanded: [],
    /* Which branch folders the Git panel has unfolded. **Null and empty are
       different states**, which is why the default is null and not `[]`: null
       is "nobody has chosen here" and unfolds the folder the current branch is
       in, while an empty list is somebody having folded them all. Written out
       whole on the first press — `branchTree.js` holds both halves of that. */
    branchFolders: null,
    openTabs: [],
    previewTab: null,
    /* The board's columns in the order a person dragged them to. Empty means
       "never rearranged", and the board then draws bd's own order. */
    columnOrder: [],
    /* The centre's tab row in the order somebody dragged it into, by tab id and
       without the pinned ones — the board and the Agent tab do not move. Empty
       means "never rearranged", and the row then draws the order it grew in.
       Beside `openTabs` rather than instead of it: that list is the set of files
       to open again, and this one is a sequence naming diffs and shell tabs too,
       ids that die with the app. `components/shell/tabOrder.js` is the rule that
       reconciles the two. */
    tabOrder: [],
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
    applySection(settings.git, base.git, stored.git)
    applySection(settings.window, base.window, stored.window)
    applySection(settings.kanban, base.kanban, stored.kanban)
    applySection(settings.notifications, base.notifications, stored.notifications)
    applySection(settings.layout, base.layout, stored.layout)
    applySection(settings.project, base.project, stored.project)
    settings.openProjects = stored.openProjects ?? []
    settings.activeProject = stored.activeProject ?? null
    settings.agent = stored.agent ?? base.agent
    settings.agentLanguage = stored.agentLanguage ?? base.agentLanguage
    settings.taskLanguage = stored.taskLanguage ?? base.taskLanguage
    settings.commitLanguage = stored.commitLanguage ?? base.commitLanguage
    settings.reportLanguage = stored.reportLanguage ?? base.reportLanguage
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
     change: the flat set of fields the settings window draws, which is whatever
     `toShared` below builds;
   - `settings:apply` — one edit, from the settings window to the main window.

   The payload is flat rather than a slice of the settings tree, because it is a
   message and not the settings: its fields come from several different sections
   of that tree, and a nested shape would invite somebody to send a whole section
   and quietly blank the fields they left out. */
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
  const kanban = { ...base.kanban, ...source.kanban }
  const git = { ...base.git, ...source.git }
  /* Deliberately not `window`: that name is the global object, and shadowing it
     inside this function would take `window.addEventListener` and every other
     use of it in this module out of reach for whoever edits here next. */
  const windowSection = { ...base.window, ...source.window }
  const notifications = { ...base.notifications, ...source.notifications }
  return {
    theme: appearance.theme,
    density: appearance.density,
    uiFontSize: appearance.uiFontSize,
    editorFontSize: editor.fontSize,
    /* Flat beside `editorFontSize`, and for the reason every flat field here
       is: the payload is a message rather than a slice of the settings tree. */
    editorWordWrap: editor.wordWrap,
    /* Flat, like `editor.fontSize` above and for the same reason: the payload
       is a message rather than a slice of the settings tree, and a nested
       `kanban` would invite somebody to send the whole section and quietly
       blank the two fields they left out. */
    kanbanColumns: kanban.columns,
    kanbanAlwaysShow: kanban.alwaysShow,
    kanbanInterval: kanban.interval,
    kanbanUnlimited: kanban.unlimited,
    /* Flat for the same reason the four above it are. */
    gitAutoFetch: git.autoFetch,
    gitRemoveWorktrees: git.removeWorktrees,
    /* Flat for the same reason, and the whole of what this window may change
       about the main window's geometry — where it is now is not a setting and
       never crosses this contract. */
    restoreGeometry: windowSection.restoreGeometry,
    /* Flat for the same reason, and named for the event rather than for the
       section: a `notifications` object in this message would invite somebody
       to send it whole and quietly blank the choice they left out. */
    notificationRunFinished: notifications.runFinished,
    notificationNeedsAttention: notifications.needsAttention,
    /* Flat beside the two sounds, and named for what it decides rather than for
       its section — the name `applyPatch` reads back, and the two spellings
       have to be the same word or the switch moves on screen, is dropped on
       arrival and reverts on the next open with nothing to say so. */
    notificationOnlyWhenUnfocused: notifications.onlyWhenUnfocused,
    /* Flat beside the two sounds, and for the same reason. Named for what it
       decides rather than for the section, since it is the whole of the report
       delivery policy — `components/run/reportDelivery.js` asks this and
       nothing else. */
    notificationShowReport: notifications.showReport,
    agent: source.agent ?? base.agent,
    agentLanguage: source.agentLanguage ?? base.agentLanguage,
    taskLanguage: source.taskLanguage ?? base.taskLanguage,
    commitLanguage: source.commitLanguage ?? base.commitLanguage,
    reportLanguage: source.reportLanguage ?? base.reportLanguage
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
   exactly what `Settings::validate` already does for a hand-edited one. The
   languages beside it are checked the same way and for the same reason:
   `agents::LANGUAGES` is Rust's list, so what is guarded here is the shape and
   not the vocabulary. */
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
  /* A switch, so the whole check is the type — the rule `gitAutoFetch` below
     records: an event is not a response to a request, so a malformed one leaves
     the previous value standing rather than falling back to the shipped
     default, and `false` is reachable because it is never coerced. */
  if (typeof patch.editorWordWrap === 'boolean') {
    settings.editor.wordWrap = patch.editorWordWrap
  }
  if (typeof patch.agent === 'string' && patch.agent) {
    settings.agent = patch.agent
  }
  if (typeof patch.agentLanguage === 'string' && patch.agentLanguage) {
    settings.agentLanguage = patch.agentLanguage
  }
  if (typeof patch.taskLanguage === 'string' && patch.taskLanguage) {
    settings.taskLanguage = patch.taskLanguage
  }
  if (typeof patch.commitLanguage === 'string' && patch.commitLanguage) {
    settings.commitLanguage = patch.commitLanguage
  }
  if (typeof patch.reportLanguage === 'string' && patch.reportLanguage) {
    settings.reportLanguage = patch.reportLanguage
  }
  /* The board's four. The two scalars are checked against the closed lists
     `boardView.js` holds — unlike `agent`, where Rust is the only party with
     the list, the vocabulary here is the front end's own rule and Rust merely
     validates the file against its copy of it. The two lists are cleaned rather
     than trusted: they end up filtering the board, and a number arriving in one
     would sit there matching no column for ever. */
  if (COLUMN_MODES.includes(patch.kanbanColumns)) {
    settings.kanban.columns = patch.kanbanColumns
  }
  if (INTERVALS.includes(patch.kanbanInterval)) {
    settings.kanban.interval = patch.kanbanInterval
  }
  if (Array.isArray(patch.kanbanAlwaysShow)) {
    settings.kanban.alwaysShow = columnNames(patch.kanbanAlwaysShow)
  }
  if (Array.isArray(patch.kanbanUnlimited)) {
    settings.kanban.unlimited = columnNames(patch.kanbanUnlimited)
  }
  /* A switch, so the whole check is the type: anything that is not a boolean is
     not an answer to this question and is skipped rather than coerced — `false`
     has to be reachable, and `Boolean(patch.gitAutoFetch)` would turn a
     malformed event into a deliberate-looking "off". */
  if (typeof patch.gitAutoFetch === 'boolean') {
    settings.git.autoFetch = patch.gitAutoFetch
  }
  /* The second switch on that tab, checked exactly the same way and for the
     same reason: `false` is the whole point of it, so coercion would turn a
     malformed event into a deliberate-looking answer either way. */
  if (typeof patch.gitRemoveWorktrees === 'boolean') {
    settings.git.removeWorktrees = patch.gitRemoveWorktrees
  }
  /* A switch too, checked exactly the way the two above it are and for the same
     reason: `false` is the whole point of this field, so anything that is not a
     boolean is skipped rather than coerced into a deliberate-looking "off". */
  if (typeof patch.restoreGeometry === 'boolean') {
    settings.window.restoreGeometry = patch.restoreGeometry
  }
  /* The two sounds. Checked against the closed list `sounds.js` holds — the
     same relationship the board's two scalars have with `boardView.js`: the
     vocabulary is the front end's own rule and Rust merely validates the file
     against its copy of it. A value that fails is skipped rather than
     normalised, so a malformed event leaves the previous choice standing. */
  if (isSound(patch.notificationRunFinished)) {
    settings.notifications.runFinished = patch.notificationRunFinished
  }
  if (isSound(patch.notificationNeedsAttention)) {
    settings.notifications.needsAttention = patch.notificationNeedsAttention
  }
  /* Two switches, both checked the way `restoreGeometry` above is and for the
     same reason: `false` is the whole point of either field, so anything that
     is not a boolean is skipped rather than coerced into a deliberate-looking
     "off". */
  if (typeof patch.notificationOnlyWhenUnfocused === 'boolean') {
    settings.notifications.onlyWhenUnfocused = patch.notificationOnlyWhenUnfocused
  }
  if (typeof patch.notificationShowReport === 'boolean') {
    settings.notifications.showReport = patch.notificationShowReport
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
