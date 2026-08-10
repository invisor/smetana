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
import { onMounted, onUnmounted, reactive, ref, watchEffect } from 'vue'
import TabBar from '../components/shell/TabBar.vue'
import GeneralSettings from '../components/settings/GeneralSettings.vue'
import EditorSettings from '../components/settings/EditorSettings.vue'
import AgentSettings from '../components/settings/AgentSettings.vue'
import AboutSettings from '../components/settings/AboutSettings.vue'
import { EDITOR_FONT_DEFAULT, UI_FONT_DEFAULT, effectiveTheme } from '../appearance.js'
import { paintRoot, usePrefersDark } from './useAppearance.js'
import {
  readSharedSettings,
  sendSettingsPatch,
  watchSharedSettings
} from '../stores/settings.js'
import { appVersion, openExternal } from '../stores/app.js'

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
    theme: effectiveTheme(view.theme, prefersDark.value),
    density: view.density,
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
  { id: 'about', label: 'About', kind: 'pinned' }
]
const tab = ref('general')

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
   control, and a label a metre away from its dropdown is unreadable. */
const columnStyle = { maxWidth: '640px', margin: '0 auto' }
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
        <AboutSettings v-else :version="version" @open="openExternal" />
      </div>
    </div>
  </div>
</template>
