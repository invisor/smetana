<script setup>
import { computed, onMounted, watch, watchEffect } from 'vue'
import { bridge, initStatus, screenFor } from './bridge/index.js'
import { wireStores, home, refreshSummary } from './session.js'
import { initPushTarget } from './pushTarget.js'
import { trackerState } from '@src/stores/tracker.js'
import { settings } from '@src/stores/settings.js'
import { phoneLook } from './look.js'
import { usePrefersDark } from '@src/views/useAppearance.js'
import PairScreen from './screens/PairScreen.vue'
import Workspace from './screens/Workspace.vue'
import UnreachableScreen from './screens/UnreachableScreen.vue'
import UpdateScreen from './screens/UpdateScreen.vue'
import RevokedScreen from './screens/RevokedScreen.vue'
import StatusScreen from './screens/StatusScreen.vue'

/* The phone draws the desktop's theme and density, which `desktopSettings.js` copies into the
   settings store (`look.js` is the rule: the desktop's `system` is the phone's own system theme).
   `?theme=` and `?density=` are for the browser, where the four looks have to be checked by eye
   (CLAUDE.md, Commands). The font size is not the desktop's to set. */
const params = new URLSearchParams(window.location.search)
const prefersDark = usePrefersDark()
const look = computed(() => phoneLook(settings.appearance, prefersDark.value, params))

watchEffect(() => {
  const root = document.documentElement
  root.setAttribute('data-theme', look.value.theme)
  root.setAttribute('data-density', look.value.density)
  root.setAttribute('data-window-chrome', 'none')
})

wireStores()
onMounted(() => {
  initPushTarget().catch((err) => console.warn('[companion] push target unavailable:', err))
  initStatus().catch((err) => {
    console.warn('[companion] could not read the connection state:', err)
    bridge.paired = false
  })
})

watch(() => trackerState.generation, refreshSummary)

const screen = computed(() => screenFor(bridge.paired, bridge.status))
const showBanner = computed(
  () => bridge.status.state === 'update-recommended' && !bridge.dismissedRecommended
)
</script>

<template>
  <PairScreen v-if="screen === 'pair'" />
  <UpdateScreen v-else-if="screen === 'update'" :store-url="bridge.status.store_url" />
  <RevokedScreen v-else-if="screen === 'revoked'" />
  <UnreachableScreen
    v-else-if="screen === 'unreachable'"
    :last-seen="bridge.status.last_seen"
    :project="home.project"
    :tasks="home.tasks"
  />
  <Workspace
    v-else-if="screen === 'home'"
    :banner="showBanner"
    :store-url="bridge.status.store_url"
  />
  <StatusScreen v-else :loading="screen === 'loading'" />
</template>
