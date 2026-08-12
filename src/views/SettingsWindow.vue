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
import StorageSettings from '../components/settings/StorageSettings.vue'
import AboutSettings from '../components/settings/AboutSettings.vue'
import { EDITOR_FONT_DEFAULT, UI_FONT_DEFAULT, effectiveTheme } from '../appearance.js'
import { paintRoot, usePrefersDark } from './useAppearance.js'
import {
  readSharedSettings,
  sendSettingsPatch,
  watchSharedSettings
} from '../stores/settings.js'
import { appVersion, openExternal } from '../stores/app.js'
import { clearStorage, surveyStorage } from '../stores/attachments.js'

/* The query string's two overrides, passed down rather than read here so that
   `App.vue` stays the one place that knows about them. They win over what the
   app window says, for this run only and never written back — the same
   precedence `DesktopApp` gives them, and the only way this window's own chrome
   can be looked at in the other theme and in compact. */
const props = defineProps({
  themeOverride: { type: String, default: null },
  densityOverride: { type: String, default: null }
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
  agent: 'claude'
})
const FIELDS = Object.keys(view)

/* Whether the app window has spoken. The disk read below is a fall-back for the
   moment before it does — and the two can land in either order, so a disk
   answer that arrives second must not overwrite the newer truth: the main
   window's copy can be up to a debounce ahead of the file. */
const heard = ref(false)

const adopt = (state, fromApp) => {
  if (!state) return
  if (fromApp) heard.value = true
  for (const field of FIELDS) {
    if (field in state && state[field] != null) view[field] = state[field]
  }
}

/* One edit: applied here, then sent. Never the whole object — the message is
   what changed, and sending all five fields would let a stale copy of one
   overwrite an edit somebody made in the app window in between. */
const change = (patch) => {
  adopt(patch, false)
  sendSettingsPatch(patch)
}

let stopWatching = null
const version = ref(null)

onMounted(async () => {
  try {
    stopWatching = await watchSharedSettings((state) => adopt(state, true))
  } catch (err) {
    console.warn('[settings-window] no app window to follow:', err)
  }
  try {
    const stored = await readSharedSettings()
    if (!heard.value) adopt(stored, false)
  } catch (err) {
    console.warn('[settings-window] the settings could not be read:', err)
  }
  version.value = await appVersion()
})

onUnmounted(() => stopWatching?.())

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
  { id: 'storage', label: 'Storage', kind: 'pinned' },
  { id: 'about', label: 'About', kind: 'pinned' }
]
const tab = ref('general')

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

watch(
  tab,
  (which) => {
    if (which === 'storage' && !storage.busy) readStorage()
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
          @update:theme="change({ theme: $event })"
          @update:ui-font-size="change({ uiFontSize: $event })"
        />
        <EditorSettings
          v-else-if="tab === 'editor'"
          :font-size="view.editorFontSize"
          @update:font-size="change({ editorFontSize: $event })"
        />
        <AgentSettings
          v-else-if="tab === 'agents'"
          :agent="view.agent"
          @update:agent="change({ agent: $event })"
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
