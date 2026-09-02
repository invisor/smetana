<script setup>
/* The settings window: a real OS window of its own, not a modal, so it can be
   dragged out over the desktop and left beside whatever it is changing. It is
   the same bundle under `?view=settings` — the third branch in `App.vue`, beside
   the app and the gallery — which is also what makes this screen checkable in
   `npm run dev` with no Tauri behind it.

   **This window holds no settings store.** The main window is the only writer,
   because `settings_save` writes the whole resolved view — panel widths, the
   project map, the open tabs — and a second writer would post its own idea of
   all of that. So the traffic is three events (`stores/settings.js` describes
   them): it asks for the truth on opening, sends one edit at a time, and draws
   whatever the main window announces afterwards.

   An edit is applied here first and sent second. That is not optimism for its
   own sake: it is what makes a dropdown answer the person who used it in the
   same frame, and it is what makes this screen work in a browser at all, where
   nothing answers. The announcement that follows is the correction — a value the
   main window refuses comes straight back and overwrites what is drawn here. */
import { onMounted, onUnmounted, reactive, ref, watch, watchEffect } from 'vue'
import TabBar from '../components/shell/TabBar.vue'
import GeneralSettings from '../components/settings/GeneralSettings.vue'
import EditorSettings from '../components/settings/EditorSettings.vue'
import AgentSettings from '../components/settings/AgentSettings.vue'
import KanbanSettings from '../components/settings/KanbanSettings.vue'
import GitSettings from '../components/settings/GitSettings.vue'
import StorageSettings from '../components/settings/StorageSettings.vue'
import AboutSettings from '../components/settings/AboutSettings.vue'
import { EDITOR_FONT_DEFAULT, UI_FONT_DEFAULT, effectiveTheme } from '../appearance.js'
import { paintRoot, usePrefersDark } from './useAppearance.js'
import {
  readSharedSettings,
  sendSettingsPatch,
  watchSharedSettings
} from '../stores/settings.js'
import {
  announceWindowReady,
  appVersion,
  autostartState,
  openExternal,
  readAgentUsage,
  readCavemanState,
  requestCavemanInstall,
  setAutostart,
  watchActiveProject,
  watchBoardColumns,
  watchSettingsSection
} from '../stores/app.js'
/* Pure, no Vue and no DOM: what the Install button would type. The press
   arrives here as a bare event, the way `refresh` does, so the one place that
   knows a state's command is the module the group draws from. */
import { installCommand } from '../components/settings/caveman.js'
import { clearStorage, surveyStorage } from '../stores/attachments.js'
import { checkForUpdate, initUpdates, installUpdate, updatesState } from '../stores/updates.js'

/* The query string's two overrides, passed down rather than read here so that
   `App.vue` stays the one place that knows about them. They win over what the
   app window says, for this run only and never written back — the same
   precedence `DesktopApp` gives them, and the only way this window's own chrome
   can be looked at in the other theme and in compact. */
const props = defineProps({
  themeOverride: { type: String, default: null },
  densityOverride: { type: String, default: null },
  /* Which section to open on — `?tab=`, read in `App.vue` with the rest of the
     query string. Checked against `TABS` below rather than trusted: it comes
     off a URL, and Rust guards its shape without knowing the vocabulary. Named
     apart from the `tab` ref below on purpose: a prop and a setup binding of
     one name resolve by a precedence rule nobody should have to remember while
     reading the template. */
  initialTab: { type: String, default: null }
})

/* Everything this window can see and change, flat, in the shape the two windows
   speak in. The defaults are the shipped ones, so the window paints itself
   correctly in the moment before the first answer arrives rather than flashing
   a light theme at somebody working in the dark one. */
const view = reactive({
  theme: 'dark',
  density: 'comfortable',
  uiFontSize: UI_FONT_DEFAULT,
  editorFontSize: EDITOR_FONT_DEFAULT,
  /* The editor's other field: whether a line longer than the pane wraps. Off,
     the same as `EditorSettings::default()` in Rust and `defaults()` in
     `stores/settings.js` — the three copies have to agree, or this window draws
     the switch in the opposite position for the moment before the first answer
     arrives. Off is also today's behaviour exactly. */
  editorWordWrap: false,
  agent: 'claude',
  /* BCP-47 ids, every one of them mirroring Rust's `en` — the same
     shipped-defaults reasoning the four above carry. */
  agentLanguage: 'en',
  taskLanguage: 'en',
  commitLanguage: 'en',
  reportLanguage: 'en',
  /* The person's own standing instruction, empty as it ships — see
     `agentPrompt` in `stores/settings.js` for why empty is the whole of
     today's behaviour rather than a placeholder. */
  agentPrompt: '',
  /* How compressed an agent's answers are. Shipped `off`, the same as
     `settings/model.rs` and `defaults()` in `stores/settings.js` — the copies
     have to agree, or this window draws a level the app is not using for the
     moment before the first answer arrives, and in a browser under
     `?view=settings` for good.

     The global level and nothing else. A project's own override stood beside it
     here and is edited in the project settings window now
     (`components/run/ProjectSettingsModal.vue`), which took the one per-project
     field back off this contract: every field this window speaks in is about
     the machine again, which is what the window is for. */
  cavemanLevel: 'off',
  /* The board's four, flat in the same message the rest ride in — see
     `toShared` in `stores/settings.js`. Shipped as today's board exactly, for
     the same reason the agent and the languages above are shipped values: this
     window paints itself correctly in the moment before the first answer
     arrives. */
  kanbanColumns: 'all',
  kanbanAlwaysShow: [],
  kanbanInterval: 'all',
  kanbanUnlimited: [],
  /* The Git tab's two: whether the Git panel goes to a remote by itself, and
     whether a run removes each task's worktree once it is merged and closed.
     Both shipped on, the same as `settings/model.rs`, `stores/settings.js` and
     the component's own prop defaults — the copies of each have to agree, or a
     switch draws the opposite of what the app is doing for the moment before
     the first answer arrives. */
  gitAutoFetch: true,
  gitRemoveWorktrees: true,
  /* The Agents tab's two run limits, the percentages at which a run holds
     itself back. Shipped as today's behaviour exactly — the same numbers as
     `settings/model.rs`, `stores/settings.js` and the component's own prop
     defaults, and those copies have to agree or the tab draws a threshold the
     app is not using for the moment before the first answer arrives.

     `0` is off, and it is a number rather than a `null` on purpose: `adopt()`
     below skips a field whose value is `null`, so an `Option` here would mean a
     threshold turned off in the app window never reached this screen and the
     old number stood until the window was reopened. Do not "improve" either end
     into one.  */
  subscriptionPauseAt: 90,
  subscriptionReducedAt: 75,
  /* Whether the main window opens where it was left. Shipped on, the same as
     `settings/model.rs` and `stores/settings.js`, for the reason the switch
     above it carries. */
  restoreGeometry: true,
  /* Whether the app asks about a newer version by itself. Shipped on, the same
     as `settings/model.rs`, `stores/settings.js` and the component's own prop
     default — the four copies have to agree, or this window draws the switch in
     the position opposite to what the app is doing, and in a browser under
     `?view=settings` it draws it that way for good, since no answer ever
     comes. */
  updatesAutoCheck: true,
  /* Which sound each announcement makes. Shipped as `settings/model.rs` and
     `stores/settings.js` ship them — the three copies of these defaults have to
     agree, or this window draws a sound the app is not playing for the moment
     before the first answer arrives. */
  notificationRunFinished: 'sound-1',
  notificationNeedsAttention: 'sound-2',
  /* Whether those two sounds wait until the main window is in the background.
     Shipped on, the same as `settings/model.rs`, `stores/settings.js` and the
     component's own prop default — the four copies have to agree, or this
     window draws the switch in the position opposite to what the app is doing
     for the moment before the first answer arrives. */
  notificationOnlyWhenUnfocused: true,
  /* Whether a finished run opens its report. Shipped on, the same as
     `settings/model.rs`, `stores/settings.js` and the component's own prop
     default — the four copies have to agree, or this window draws the switch in
     the position opposite to what the app is doing for the moment before the
     first answer arrives. */
  notificationShowReport: true
})
const FIELDS = Object.keys(view)

/* Whether the app window has spoken. The disk read below is a fall-back for the
   moment before it does — and the two can land in either order, so a disk
   answer that arrives second must not overwrite the newer truth: the main
   window's copy can be up to a debounce ahead of the file. */
const heard = ref(false)

/* Whether anything at all has landed in `view` — an announcement, the disk
   read, or an edit made here — as opposed to `heard`, which is about the app
   window alone and stays false for the disk. The difference matters to exactly
   one reader, the subscription probe below: until this is true, every field in
   `view` is the shipped painting default rather than an answer, and a question
   asked out of one names a guess. */
const adopted = ref(false)

/* Whether somebody has typed in the standing instruction. From that moment this
   window's copy of that **one** field is the truth and an announcement out of
   the app window does not touch it.

   It is the only field here that needs saying, because it is the only free-text
   control in the window: everything else is a dropdown, a switch or a button,
   where an announcement re-setting the value already on screen is invisible. A
   text field is not so forgiving. Every keystroke goes out as a patch, the app
   window applies it and then `announce()`s the whole state back — and an
   announcement carrying "abc", landing after somebody has typed "abcd", would
   rewind `view.agentPrompt`, put the caret back at the end and send the rewound
   text on to the store and the disk. Characters would be lost for good, and only
   on a busy machine, which is the worst way for it to happen.

   Safe because nothing else can write this field: the app window has no control
   for it, so an announcement can only ever be carrying this window's own words
   back. Before the first keystroke announcements are taken as normal.

   That "only our own words back" is exactly the argument for deleting this
   guard, and it is the wrong one, because of what the other half of the loop
   costs: `announce()` rebuilds the whole shared object and broadcasts it once
   per keystroke, so typing a long instruction is the moment the app window's
   queue is busiest, and the lag that makes an echo stale is at its most likely
   precisely while somebody is typing. The guard makes that inert, which is why
   the chattiness is affordable — remove it on the reasoning above and the
   defect comes back at the volume that produces it. */
let promptEdited = false

const adopt = (state, fromApp) => {
  if (!state) return
  if (fromApp) heard.value = true
  adopted.value = true
  for (const field of FIELDS) {
    if (!(field in state) || state[field] == null) continue
    if (field === 'agentPrompt' && fromApp && promptEdited) continue
    view[field] = state[field]
  }
}

/* One edit: applied here, then sent. Never the whole object — the message is
   what changed, and sending every field would let a stale copy of one overwrite
   an edit somebody made in the app window in between. */
const change = (patch) => {
  adopt(patch, false)
  sendSettingsPatch(patch)
}

/* The standing instruction, which goes out per keystroke like every other edit
   here and, unlike them, closes the door behind it. Sent immediately rather than
   on a debounce of its own on purpose: the disk write is already debounced 400 ms
   in the app window, so a debounce here would buy nothing but a window that can
   be closed with the last few characters still in a timer. */
const changeAgentPrompt = (text) => {
  promptEdited = true
  change({ agentPrompt: text })
}

let stopWatching = null
let stopSections = null
let stopColumns = null
let stopProject = null
let stopUpdates = null
const version = ref(null)

/* The update machine, and the fourth part of this window that is not a setting:
   nothing about it reaches `settings.json` and `FIELDS` above deliberately does
   not name it. It is asked of Rust directly, the way the Storage numbers and the
   login item are, and for a reason sharper than either — the state is the app's
   and not this window's, so a download started before this window opened is
   already going and has to be drawn as such.

   Read on mounting rather than on opening the About tab, unlike Storage and the
   subscription probe. Two reasons, and the first is decisive: the answer is a
   subscription as much as a read, and an event that arrives while somebody is
   on the General tab has to be there when they walk over to About. The second
   is that it costs nothing — a lock and three fields, with no worker queue and
   no process behind it. */
const updateRefusal = ref(null)

/* Which columns the active project's board has, for the Kanban tab's two lists.
   Not a setting and not on the settings contract: it is announced by the app
   window through `stores/app.js`, which is also where the reason it cannot come
   from Rust is written down. An empty list is an ordinary state — no project
   open, or nobody has answered yet — and the tab says so rather than drawing a
   gap. */
const boardColumns = ref([])

/* Which project the app window has open, announced by it the way the board
   columns are and for the same reason (`stores/app.js`): it is not a setting
   and has no business on the settings contract. Two things on the Agents tab
   want it — the Caveman group's Install button, which opens a terminal and so
   needs there to be somewhere to open one, and the state read below, since one
   of the four states caveman can be in is the skill in this repository alone.

   `null` is the ordinary state rather than a gap: every project closed, or no
   app window to answer at all, which is what a browser under `?view=settings`
   is. */
const activeProject = ref(null)

onMounted(async () => {
  try {
    stopWatching = await watchSharedSettings((state) => adopt(state, true))
  } catch (err) {
    console.warn('[settings-window] no app window to follow:', err)
  }
  try {
    /* The app window pressing "open the settings on Storage" while this window
       is already open. A name this build does not know is ignored rather than
       drawn: the person keeps the tab they were reading. */
    stopSections = await watchSettingsSection((name) => {
      const asked = known(name)
      if (asked) tab.value = asked
    })
  } catch (err) {
    console.warn('[settings-window] no app window to hear from:', err)
  }
  try {
    stopColumns = await watchBoardColumns((columns) => {
      boardColumns.value = Array.isArray(columns) ? columns : []
    })
  } catch (err) {
    console.warn('[settings-window] no app window to hear the board columns from:', err)
  }
  try {
    stopProject = await watchActiveProject((path) => {
      activeProject.value = typeof path === 'string' && path ? path : null
    })
  } catch (err) {
    console.warn('[settings-window] no app window to hear the active project from:', err)
  }
  /* And only now: this window is listening, and the section its URL named is
     the one `tab` was seeded with at setup. A gear pressed on a section while
     this window was still loading is handed over on this call and lands as an
     ordinary `settings:show` through the subscription above. The order is the
     whole of it — before the subscription the event would be lost, and before
     the seeding the URL's section, which is the older of the two, would be
     drawn over the newer one. `stores/app.js` carries the argument. */
  announceWindowReady()
  try {
    const stored = await readSharedSettings()
    /* `promptEdited` as well as `heard`, because this call passes
       `fromApp = false` and the guard in `adopt` would let the file's value
       through. It takes an app window that never answers and somebody typing
       before this awaited read resolves, which three `await listen(...)` calls
       and a millisecond of disk make very unlikely — but the flag claims to be
       the whole story about that field, and this is the road round it. */
    if (!heard.value && !promptEdited) adopt(stored, false)
  } catch (err) {
    console.warn('[settings-window] the settings could not be read:', err)
  }
  version.value = await appVersion()
  stopUpdates = await initUpdates()
})

onUnmounted(() => {
  stopWatching?.()
  stopSections?.()
  stopColumns?.()
  stopProject?.()
  stopUpdates?.()
})

/* This window paints itself: it is a separate webview with its own document
   root, so the app window's attributes reach nothing here. `system` is resolved
   against the machine and follows it live, which is the whole reason
   `usePrefersDark` is a listener rather than a reading. */
const prefersDark = usePrefersDark()
watchEffect(() => {
  paintRoot(document.documentElement, {
    theme: props.themeOverride ?? effectiveTheme(view.theme, prefersDark.value),
    density: props.densityOverride ?? view.density,
    uiFontSize: view.uiFontSize,
    editorFontSize: view.editorFontSize
  })
})

/* `pinned` is the tab kind with no close button and a sans label — which is
   exactly what a settings tab is. The centre's file tabs are the other kinds. */
const TABS = [
  { id: 'general', label: 'General', kind: 'pinned' },
  { id: 'editor', label: 'Editor', kind: 'pinned' },
  { id: 'agents', label: 'Agents', kind: 'pinned' },
  { id: 'kanban', label: 'Kanban', kind: 'pinned' },
  /* Between Kanban and Storage rather than at the end: the five before it are
     settings and Storage is the one tab that is not, so a sixth section of
     settings belongs on this side of that line. */
  { id: 'git', label: 'Git', kind: 'pinned' },
  { id: 'storage', label: 'Storage', kind: 'pinned' },
  { id: 'about', label: 'About', kind: 'pinned' }
]

/* The list of sections is this file's, and this is the only place a name is
   checked against it — Rust builds the URL and guards its shape, never its
   vocabulary, so a section this build has never heard of opens on General
   rather than on an empty body. Whoever asked for it pressed a button, so the
   worst outcome is landing one tab away from what they meant. */
const known = (name) => (TABS.some((entry) => entry.id === name) ? name : null)

/* Which section a caller asked for, in the two forms that can reach this
   window. `?tab=` is how a window being built hears about it — passed in as a
   prop, the way `?theme=` and `?density=` are, so `App.vue` stays the one place
   that reads the query string; the event is how an already-open window hears,
   since opening it a second time focuses it and never reloads the URL. Unlike
   the appearance overrides this one is not a standing override: it names where
   to start, and a person is free to walk away from it. */
const tab = ref(known(props.initialTab) ?? 'general')

/* The Storage tab, and the one part of this window that is not a setting at
   all: it asks the back end two questions of its own rather than reading the
   state the app window announces. That is not a second writer of
   `settings.json` — nothing here reaches that file. The store is the app's own
   data directory, Rust owns it, and both calls are answered against the tracker
   worker's idea of the active project, so this window never names a project or
   a path of its own.

   Read on opening the tab rather than on mounting the window: the answer costs
   a queue behind the tracker worker, which may be two seconds into a bd call,
   and the other four tabs have no use for it. */
const storage = reactive({ survey: null, busy: false, error: null, cleaned: null })

const readStorage = async () => {
  storage.busy = true
  /* What a previous press did is news about that press, and coming back to the
     tab later is a fresh look at the store rather than a repeat of it. */
  storage.cleaned = null
  try {
    storage.survey = await surveyStorage()
    storage.error = null
  } catch (err) {
    storage.error = err.message
  } finally {
    storage.busy = false
  }
}

/* The press. What goes is decided in Rust, against the board and inside the
   active project's own folder — this function hands over no path and no name,
   which is what keeps the button unable to reach another project's images.
   The command answers with fresh numbers, so the screen corrects itself with no
   second round trip. */
const clear = async () => {
  storage.busy = true
  storage.error = null
  try {
    const cleaned = await clearStorage()
    storage.cleaned = cleaned
    storage.survey = cleaned.survey
  } catch (err) {
    /* A refusal leaves the numbers exactly as they were: they still describe
       the store, since nothing was deleted. */
    storage.error = err.message
  } finally {
    storage.busy = false
  }
}

/* What is left of the agent's subscription, and the third part of this window
   that is not a setting: nothing about it reaches `settings.json` either. It is
   a question put to the harness itself, through Rust, and the answer belongs to
   the minute it was given in.

   Read on opening the Agents tab, the way the Storage numbers and the login
   item are read on opening theirs — and here the argument is stronger than
   theirs, because the probe is somebody else's CLI with a minute's ceiling over
   it. Asking on mounting the window would start that process for everybody who
   came to change the theme. Asking only on a press was the other rejected
   option, and it is the original complaint back again: the block is empty at
   first glance. There is no timer either, deliberately — this window can be
   open for hours and an allowance moves in hours and days.

   The reading is cleared at the start of every read rather than left on screen
   under "Reading…", and that is what makes the agent dropdown correct: switch
   Claude to Codex and the block must stop talking about the previous agent
   before it knows anything about the new one.

   **The agent is named in the question**, out of what this window is showing,
   and never left to Rust to read off the disk. This window's edits reach
   `settings.json` through the app window's 400 ms debounce, so the file is
   behind what is on screen here for as long as it takes one edit to settle —
   and a probe is up to sixty seconds. Asking without naming an agent therefore
   answers about the agent somebody has just switched away from, every time
   rather than occasionally.

   **Until this window has adopted anything, it names nobody**, and that is the
   same defect one door over. `view.agent` starts as the shipped `claude`, a
   painting default so the window does not flash the wrong picker for a frame,
   and it is replaced inside `onMounted` after an awaited call — while the
   `watch` below runs synchronously during setup. So a window built on
   `?tab=agents` would ask about `claude` whatever the file says, and nothing
   would ask again once the real value arrived. Passing `null` there is not a
   gap: nothing has been edited yet, so the file genuinely is the authority, and
   `wanted`'s `None` branch is exactly that case.

   The guard is a sequence number and not the `busy` flag alone: a change of
   agent has to supersede a probe already out, so two can be in flight at once
   and only the newest may be drawn. Without it, an answer about the agent
   somebody has just switched away from would land last and win. */
const usage = reactive({ reading: null, busy: false, error: null })
let asked = 0

const readUsage = async (agent = adopted.value ? view.agent : null) => {
  const mine = (asked += 1)
  usage.busy = true
  usage.reading = null
  usage.error = null
  try {
    const reading = await readAgentUsage(agent)
    if (mine !== asked) return
    usage.reading = reading
  } catch (err) {
    if (mine !== asked) return
    usage.error = err.message
  } finally {
    if (mine === asked) usage.busy = false
  }
}

/* How caveman stands on this machine, and the fifth part of this window that is
   not a setting: nothing about it reaches `settings.json`. The machine's own
   four files are the whole of the truth (`src-tauri/src/caveman.rs`), so it is
   asked rather than remembered — a copy of ours would disagree with the disk
   the first time somebody ran `caveman enable` outside this app, with no way to
   tell which half was stale.

   Read on opening the Agents tab, the way the Storage numbers, the login item
   and the subscription probe are read on opening theirs. Not on mounting the
   window: everybody who comes to change the theme would otherwise be asking
   about somebody else's installer. It is re-read when the project changes too,
   since one of the four states is about a repository rather than about the
   machine — but only while this tab is the one on screen, for the reason it is
   not read on mounting.

   The project travels as the empty string when there is none, which is what the
   command is asked with in a browser and with every project closed. Rust joins
   the skill path onto it and finds nothing, which is exactly right: the three
   states that can be true with no project open are facts about the machine and
   need no path at all.

   The reading is cleared at the start of every read, the way the subscription
   block's is, so a stale answer about the previous project cannot sit under a
   sentence about the new one; and the guard is a sequence number for that same
   reason. `readCavemanState` never rejects — a browser is a `debug` line and an
   `absent` — so there is no error branch to draw. */
const caveman = reactive({ reading: null, busy: false })
let askedCaveman = 0

const readCaveman = async () => {
  const mine = (askedCaveman += 1)
  caveman.busy = true
  caveman.reading = null
  try {
    const reading = await readCavemanState(activeProject.value ?? '')
    if (mine !== askedCaveman) return
    caveman.reading = reading
  } finally {
    if (mine === askedCaveman) caveman.busy = false
  }
}

/* The Install press. Nothing is opened from here: the terminal belongs to the
   app window, which has the project and the tab row, so what goes is the
   command and the app window types it — without a newline, which is the whole
   point of the button. A state with no command cannot reach this, since the row
   is not drawn at all in the other two states. */
const installCaveman = () => {
  const command = installCommand(caveman.reading)
  if (command) requestCavemanInstall(command)
}

/* The project changed under an open window. Only while the Agents tab is the
   one on screen: the read is cheap, but asking on behalf of a tab nobody is
   looking at is the habit this window does not have. */
watch(activeProject, () => {
  if (tab.value === 'agents') readCaveman()
})

/* An agent is chosen: the edit goes where every edit on this window goes, and
   the block is asked again, since it is about whoever would answer now. */
const chooseAgent = (id) => {
  change({ agent: id })
  readUsage(id)
}

/* The login item, and the other part of this window that is not a setting:
   nothing about it reaches `settings.json`, and `FIELDS` above deliberately
   does not name it. The operating system's own list is the truth
   (`src-tauri/src/autostart.rs`), so it is asked rather than remembered — and
   asked on opening the General tab rather than on mounting the window, the way
   the Storage numbers are, since a list somebody may have edited outside the
   app is stale the moment it is cached.

   `supported: false` until an answer arrives, so the row opens disabled and
   settles into being live: the other way round it would offer a press for the
   length of a round trip and then take it away. */
const autostart = reactive({ supported: false, enabled: false })

const readAutostart = async () => {
  Object.assign(autostart, await autostartState())
}

/* Applied here and sent second, the way an edit to a setting is — a switch that
   waited for the operating system before moving would read as a dead control —
   but corrected by what Rust read *back* rather than by an announcement, since
   there is no other window in this conversation. A registration the system
   declined therefore returns the switch by itself. */
const toggleAutostart = async (enabled) => {
  autostart.enabled = enabled
  Object.assign(autostart, await setAutostart(enabled))
}

watch(
  tab,
  (which) => {
    if (which === 'storage' && !storage.busy) readStorage()
    if (which === 'general') readAutostart()
    /* Guarded by `busy` the way Storage is, and for a longer reason: walking
       off this tab and back while a minute-long probe is out must not start a
       second one against the same harness. */
    if (which === 'agents' && !usage.busy) readUsage()
    /* Guarded by `busy` like the two above, though this one is four file reads
       rather than somebody else's CLI: walking off the tab and back should not
       queue a second answer behind the first. */
    if (which === 'agents' && !caveman.busy) readCaveman()
  },
  { immediate: true }
)

/* The two presses on the About tab. Checking never fails — Rust answers with
   the state that stopped it — so there is nothing to catch and nothing to say.

   Installing is the opposite: every way it can decline is a refusal carrying its
   reason, the run gate above all, and on success it does not return in any
   useful sense because the app is on its way out. The refusal is cleared at the
   start of the press rather than left standing, so a second press after a run
   has ended does not read as having been refused again. */
const check = () => {
  updateRefusal.value = null
  checkForUpdate()
}

const install = async () => {
  updateRefusal.value = null
  try {
    await installUpdate()
  } catch (err) {
    updateRefusal.value = err
  }
}

const rootStyle = {
  display: 'flex',
  flexDirection: 'column',
  height: '100vh',
  background: 'var(--canvas)',
  color: 'var(--text-primary)',
  fontFamily: 'var(--font-sans)',
  fontSize: 'var(--text-md)',
  overflow: 'hidden'
}
const bodyStyle = {
  flex: 1,
  minHeight: 0,
  overflowY: 'auto',
  padding: 'var(--space-5) var(--space-6) var(--space-7)'
}
/* One column, and not the window's whole width: settings rows are a label and a
   control, and a label a metre away from its dropdown is unreadable.

   Measured in `ch` like the prose on the About tab, and for the same reason: a
   ceiling in pixels would stay put while the type inside it grew, so the app
   -wide font size would be spent on wrapping rather than on legibility. Two
   measures in one window in two units, only one of which followed the font, was
   the state before this. */
const columnStyle = { maxWidth: '88ch', margin: '0 auto' }
</script>

<template>
  <div :style="rootStyle">
    <TabBar :tabs="TABS" :active-id="tab" @select="tab = $event" />
    <div :style="bodyStyle">
      <!-- No heading over the panel: the tab a person just pressed is a
           centimetre above it, and repeating its word there spent a line saying
           nothing — on About it put "About" over "Smetana". -->
      <div :style="columnStyle">
        <GeneralSettings
          v-if="tab === 'general'"
          :theme="view.theme"
          :ui-font-size="view.uiFontSize"
          :autostart-supported="autostart.supported"
          :autostart-enabled="autostart.enabled"
          :restore-geometry="view.restoreGeometry"
          :updates-auto-check="view.updatesAutoCheck"
          :notification-run-finished="view.notificationRunFinished"
          :notification-needs-attention="view.notificationNeedsAttention"
          :notification-only-when-unfocused="view.notificationOnlyWhenUnfocused"
          :notification-show-report="view.notificationShowReport"
          @update:theme="change({ theme: $event })"
          @update:ui-font-size="change({ uiFontSize: $event })"
          @update:autostart-enabled="toggleAutostart($event)"
          @update:restore-geometry="change({ restoreGeometry: $event })"
          @update:updates-auto-check="change({ updatesAutoCheck: $event })"
          @update:notification-run-finished="change({ notificationRunFinished: $event })"
          @update:notification-needs-attention="change({ notificationNeedsAttention: $event })"
          @update:notification-only-when-unfocused="change({ notificationOnlyWhenUnfocused: $event })"
          @update:notification-show-report="change({ notificationShowReport: $event })"
        />
        <EditorSettings
          v-else-if="tab === 'editor'"
          :font-size="view.editorFontSize"
          :word-wrap="view.editorWordWrap"
          @update:font-size="change({ editorFontSize: $event })"
          @update:word-wrap="change({ editorWordWrap: $event })"
        />
        <!-- `notificationShowReport` is handed to two tabs out of this one
             view. General owns the switch; Agents only reads it, to say why the
             Report language row is drawn and cannot be used. -->
        <AgentSettings
          v-else-if="tab === 'agents'"
          :agent="view.agent"
          :agent-language="view.agentLanguage"
          :task-language="view.taskLanguage"
          :commit-language="view.commitLanguage"
          :report-language="view.reportLanguage"
          :agent-prompt="view.agentPrompt"
          :subscription-pause-at="view.subscriptionPauseAt"
          :subscription-reduced-at="view.subscriptionReducedAt"
          :show-report="view.notificationShowReport"
          :caveman="caveman.reading"
          :caveman-level="view.cavemanLevel"
          :project-open="Boolean(activeProject)"
          :usage="usage.reading"
          :busy="usage.busy"
          :error="usage.error"
          @update:agent="chooseAgent($event)"
          @update:agent-language="change({ agentLanguage: $event })"
          @update:task-language="change({ taskLanguage: $event })"
          @update:commit-language="change({ commitLanguage: $event })"
          @update:report-language="change({ reportLanguage: $event })"
          @update:agent-prompt="changeAgentPrompt($event)"
          @update:subscription-pause-at="change({ subscriptionPauseAt: $event })"
          @update:subscription-reduced-at="change({ subscriptionReducedAt: $event })"
          @update:caveman-level="change({ cavemanLevel: $event })"
          @install="installCaveman()"
          @refresh="readUsage()"
        />
        <KanbanSettings
          v-else-if="tab === 'kanban'"
          :columns="view.kanbanColumns"
          :always-show="view.kanbanAlwaysShow"
          :interval="view.kanbanInterval"
          :unlimited="view.kanbanUnlimited"
          :board-columns="boardColumns"
          @update:columns="change({ kanbanColumns: $event })"
          @update:always-show="change({ kanbanAlwaysShow: $event })"
          @update:interval="change({ kanbanInterval: $event })"
          @update:unlimited="change({ kanbanUnlimited: $event })"
        />
        <GitSettings
          v-else-if="tab === 'git'"
          :auto-fetch="view.gitAutoFetch"
          :remove-worktrees="view.gitRemoveWorktrees"
          @update:auto-fetch="change({ gitAutoFetch: $event })"
          @update:remove-worktrees="change({ gitRemoveWorktrees: $event })"
        />
        <StorageSettings
          v-else-if="tab === 'storage'"
          :survey="storage.survey"
          :busy="storage.busy"
          :error="storage.error"
          :cleaned="storage.cleaned"
          @clear="clear"
        />
        <AboutSettings
          v-else
          :version="version"
          :update-state="updatesState.state"
          :update-refusal="updateRefusal"
          @open="openExternal"
          @check="check"
          @install="install"
        />
      </div>
    </div>
  </div>
</template>
