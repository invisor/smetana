<script setup>
import { computed, onMounted, watch, watchEffect } from 'vue'
import { bridge, initStatus, screenFor } from './bridge/index.js'
import { wireStores, home, refreshSummary } from './session.js'
import { initPushTarget } from './pushTarget.js'
import { trackerState } from '@src/stores/tracker.js'
import { effectiveTheme } from '@src/appearance.js'
import { usePrefersDark } from '@src/views/useAppearance.js'
import PairScreen from './screens/PairScreen.vue'
import HomeScreen from './screens/HomeScreen.vue'
import UnreachableScreen from './screens/UnreachableScreen.vue'
import UpdateScreen from './screens/UpdateScreen.vue'
import RevokedScreen from './screens/RevokedScreen.vue'
import StatusScreen from './screens/StatusScreen.vue'

/* The phone follows the system theme and has no settings of its own yet; the
   desktop's density and font choices are not read, because the phone does not
   read the desktop's settings file beyond the active project. */
const params = new URLSearchParams(window.location.search)
const prefersDark = usePrefersDark()
const theme = computed(() =>
  ['dark', 'light'].includes(params.get('theme')) ? params.get('theme') : effectiveTheme('system', prefersDark.value)
)

watchEffect(() => {
  const root = document.documentElement
  root.setAttribute('data-theme', theme.value)
  root.setAttribute('data-density', 'comfortable')
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
  <HomeScreen
    v-else-if="screen === 'home'"
    :project="home.project"
    :tasks="home.tasks"
    :banner="showBanner"
    :store-url="bridge.status.store_url"
  />
  <StatusScreen v-else :loading="screen === 'loading'" />
</template>
