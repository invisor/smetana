<script setup>
import { computed, defineAsyncComponent, ref } from 'vue'
import DesktopApp from './views/DesktopApp.vue'
import SettingsWindow from './views/SettingsWindow.vue'
import { loadSettings, settings } from './stores/settings.js'
import { effectiveTheme } from './appearance.js'
import { usePrefersDark } from './views/useAppearance.js'

// Dev-only harness, code-split so it never lands in the product bundle.
const Gallery = defineAsyncComponent(() => import('./views/Gallery.vue'))

/* Theme and density are the template's two props. The stored settings decide
   them; the query string still overrides both for one run, so both designed
   themes and both densities stay reachable in dev (?theme=light&density=compact)
   without adding chrome the design does not have. An override is never written
   back — one visit to the dev server must not repaint the app forever. */
const params = new URLSearchParams(window.location.search)
/* Three views over one bundle. `settings` is the second OS window (Rust opens
   it as `index.html?view=settings`, see `src-tauri/src/window.rs`) and it is a
   branch here rather than a build of its own for the same reason the gallery is:
   one front end, one set of tokens, one place a component can break. */
const view = params.get('view')
const gallery = ref(view === 'gallery')
const settingsWindow = ref(view === 'settings')

const override = (name, allowed) => (allowed.includes(params.get(name)) ? params.get(name) : null)
const themeOverride = override('theme', ['dark', 'light'])
const densityOverride = override('density', ['comfortable', 'compact'])

/* The gallery is a component harness and the settings window holds no store of
   its own: both render straight away. The app waits for the file — a few
   milliseconds — rather than painting the default theme and flipping. */
const standalone = gallery.value || settingsWindow.value
const ready = ref(standalone)
if (!standalone) {
  loadSettings().then(() => {
    ready.value = true
  })
}

/* `system` is not a stored colour: it means "ask the machine", and the answer
   changes while the app is running. The settings window resolves its own, since
   it draws from the main window's announcements rather than from this store —
   so this listener is not created there at all. Two `matchMedia` subscriptions
   in one window, one of them feeding a computed nothing renders, is a leak of
   the quiet kind. */
const prefersDark = settingsWindow.value ? ref(false) : usePrefersDark()
const theme = computed(
  () =>
    themeOverride ??
    (gallery.value ? 'dark' : effectiveTheme(settings.appearance.theme, prefersDark.value))
)
const density = computed(
  () => densityOverride ?? (gallery.value ? 'comfortable' : settings.appearance.density)
)
</script>

<template>
  <!-- The overrides go in rather than the resolved values: the settings window
       decides its own theme from what the app window tells it, and these two say
       "a person asked for this one instead, for this run". Without them its own
       chrome — the tab strip, the scrolling body, the column — could not be seen
       in compact or in the other theme at all, and that is the only check this
       project has. -->
  <SettingsWindow
    v-if="settingsWindow"
    :theme-override="themeOverride"
    :density-override="densityOverride"
  />
  <Gallery v-else-if="gallery" :theme="theme" :density="density" />
  <DesktopApp v-else-if="ready" :theme="theme" :density="density" />
</template>
