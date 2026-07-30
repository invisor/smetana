<script setup>
import { defineAsyncComponent, ref } from 'vue'
import DesktopApp from './views/DesktopApp.vue'

// Dev-only harness, code-split so it never lands in the product bundle.
const Gallery = defineAsyncComponent(() => import('./views/Gallery.vue'))

/* Theme and density are the template's two props. Reading them from the query
   string keeps both designed themes and both densities reachable in dev
   (?theme=light&density=compact) without adding chrome the design does not have. */
const params = new URLSearchParams(window.location.search)
const theme = ref(params.get('theme') === 'light' ? 'light' : 'dark')
const density = ref(params.get('density') === 'compact' ? 'compact' : 'comfortable')
const gallery = ref(params.get('view') === 'gallery')
</script>

<template>
  <Gallery v-if="gallery" :theme="theme" :density="density" />
  <DesktopApp v-else :theme="theme" :density="density" />
</template>
