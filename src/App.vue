<script setup>
import { computed, defineAsyncComponent, ref } from 'vue'
import DesktopApp from './views/DesktopApp.vue'
import { loadSettings, settings } from './stores/settings.js'

// Dev-only harness, code-split so it never lands in the product bundle.
const Gallery = defineAsyncComponent(() => import('./views/Gallery.vue'))

/* Theme and density are the template's two props. The stored settings decide
   them; the query string still overrides both for one run, so both designed
   themes and both densities stay reachable in dev (?theme=light&density=compact)
   without adding chrome the design does not have. An override is never written
   back — one visit to the dev server must not repaint the app forever. */
const params = new URLSearchParams(window.location.search)
const gallery = ref(params.get('view') === 'gallery')

const override = (name, allowed) => (allowed.includes(params.get(name)) ? params.get(name) : null)
const themeOverride = override('theme', ['dark', 'light'])
const densityOverride = override('density', ['comfortable', 'compact'])

/* The gallery is a component harness: it neither reads nor writes settings,
   so it renders straight away. The app waits for the file — a few
   milliseconds — rather than painting the default theme and flipping. */
const ready = ref(gallery.value)
if (!gallery.value) {
  loadSettings().then(() => {
    ready.value = true
  })
}

const theme = computed(() => themeOverride ?? (gallery.value ? 'dark' : settings.appearance.theme))
const density = computed(
  () => densityOverride ?? (gallery.value ? 'comfortable' : settings.appearance.density)
)
</script>

<template>
  <Gallery v-if="gallery" :theme="theme" :density="density" />
  <DesktopApp v-else-if="ready" :theme="theme" :density="density" />
</template>
