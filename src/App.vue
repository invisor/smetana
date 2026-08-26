<script setup>
import { computed, defineAsyncComponent, ref } from 'vue'
import DesktopApp from './views/DesktopApp.vue'
import SettingsWindow from './views/SettingsWindow.vue'
import CompareWindow from './views/CompareWindow.vue'
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
/* Four views over one bundle. `settings` and `compare` are the app's other two
   OS windows (Rust opens them as `index.html?view=settings` and
   `index.html?view=compare`, see `src-tauri/src/window.rs`) and both are
   branches here rather than builds of their own for the same reason the gallery
   is: one front end, one set of tokens, one place a component can break. */
const view = params.get('view')
const gallery = ref(view === 'gallery')
const settingsWindow = ref(view === 'settings')
const compareWindow = ref(view === 'compare')

const override = (name, allowed) => (allowed.includes(params.get(name)) ? params.get(name) : null)
const themeOverride = override('theme', ['dark', 'light'])
const densityOverride = override('density', ['comfortable', 'compact'])
/* Which section the settings window opens on, when something asked for one —
   `settings_window_open` puts it here. Passed through untouched: the list of
   sections belongs to that window, and checking a name against it here would be
   the same closed list written out twice. */
const settingsTab = params.get('tab')
/* Which repository and which branch the compare window is aimed at —
   `compare_window_open` percent-encoded both into the URL it built. Passed
   through untouched, the way the section above is: what a repository path and a
   branch name may hold is git's business and not this file's, and an already
   open window is re-aimed by an event rather than by a URL. */
const compareRepo = params.get('repo')
const compareBranch = params.get('branch')

/* The gallery is a component harness, and neither of the two other windows
   holds this store: all three render straight away. The app waits for the
   file — a few milliseconds — rather than painting the default theme and
   flipping. */
const standalone = gallery.value || settingsWindow.value || compareWindow.value
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
const prefersDark = settingsWindow.value || compareWindow.value ? ref(false) : usePrefersDark()
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
    :initial-tab="settingsTab"
  />
  <!-- The compare window, on the pair its URL names. Its overrides go in for
       the reason the settings window's do: it paints its own root from what the
       app window announces, and without these two its chrome — the header, the
       file list, the diff's captions — could not be looked at in compact or in
       the other theme at all. -->
  <CompareWindow
    v-else-if="compareWindow"
    :theme-override="themeOverride"
    :density-override="densityOverride"
    :repo="compareRepo"
    :branch="compareBranch"
  />
  <Gallery v-else-if="gallery" :theme="theme" :density="density" />
  <DesktopApp v-else-if="ready" :theme="theme" :density="density" />
</template>
