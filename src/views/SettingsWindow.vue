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
  appVersion,
  autostartState,
  openExternal,
  readAgentUsage,
  setAutostart,
  watchBoardColumns,
  watchSettingsSection
} from '../stores/app.js'
import { clearStorage, surveyStorage } from '../stores/attachments.js'

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

const adopt = (state, fromApp) => {
  if (!state) return
  if (fromApp) heard.value = true
  adopted.value = true
  for (const field of FIELDS) {
    if (field in state && state[field] != null) view[field] = state[field]
  }
}

/* One edit: applied here, then sent. Never the whole object — the message is
   what changed, and sending every field would let a stale copy of one overwrite
   an edit somebody made in the app window in between. */
const change = (patch) => {
  adopt(patch, false)
  sendSettingsPatch(patch)
}

let stopWatching = null
let stopSections = null
let stopColumns = null
const version = ref(null)

/* Which columns the active project's board has, for the Kanban tab's two lists.
   Not a setting and not on the settings contract: it is announced by the app
   window through `stores/app.js`, which is also where the reason it cannot come
   from Rust is written down. An empty list is an ordinary state — no project
   open, or nobody has answered yet — and the tab says so rather than drawing a
   gap. */
const boardColumns = ref([])

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
    const stored = await readSharedSettings()
    if (!heard.value) adopt(stored, false)
  } catch (err) {
    console.warn('[settings-window] the settings could not be read:', err)
  }
  version.value = await appVersion()
})

onUnmounted(() => {
  stopWatching?.()
  stopSections?.()
  stopColumns?.()
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
  },
  { immediate: true }
)

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
          :show-report="view.notificationShowReport"
          :usage="usage.reading"
          :busy="usage.busy"
          :error="usage.error"
          @update:agent="chooseAgent($event)"
          @update:agent-language="change({ agentLanguage: $event })"
          @update:task-language="change({ taskLanguage: $event })"
          @update:commit-language="change({ commitLanguage: $event })"
          @update:report-language="change({ reportLanguage: $event })"
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
        <AboutSettings v-else :version="version" @open="openExternal" />
      </div>
    </div>
  </div>
</template>
