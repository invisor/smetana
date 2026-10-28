<script setup>
/* The phone's settings. For now one group, About: what this build is, which desktop it talks to,
   and the way to forget that desktop. The desktop's own settings that the phone copies, and the
   ones it will be able to override, come later.

   Unpairing takes two taps (../unpairStep.js is the rule): the first turns the button into a
   question, Confirm calls `unpairAndForget` and `App.vue` then shows the pairing screen, since
   the bridge no longer says it is paired. */
import { computed, ref } from 'vue'
import Button from '@src/components/core/Button.vue'
import SettingsGroup from '@src/components/settings/SettingsGroup.vue'
import SettingsRow from '@src/components/settings/SettingsRow.vue'
import { bridge } from '../bridge/status.js'
import { desktopMinor, isOnline } from '../bridge/states.js'
import { unpairAndForget } from '../session.js'
import { ASKED, IDLE, confirms, nextUnpairStep } from '../unpairStep.js'

const desktop = computed(() => (isOnline(bridge.status) ? `protocol 1.${desktopMinor(bridge.status)}` : 'Not connected'))

const step = ref(IDLE)
const busy = ref(false)
const failure = ref('')

async function press(kind) {
  if (busy.value) return
  const sure = confirms(step.value, kind)
  step.value = nextUnpairStep(step.value, kind)
  if (!sure) return
  busy.value = true
  failure.value = ''
  try {
    await unpairAndForget()
  } catch (err) {
    failure.value = 'This phone could not be unpaired.'
    console.warn('[companion] could not unpair:', err)
  } finally {
    busy.value = false
  }
}

const page = { padding: 'var(--space-2) var(--space-5) var(--space-6)' }
const value = { font: 'var(--weight-regular) var(--text-sm)/1 var(--font-mono)', color: 'var(--text-secondary)' }
const pair = { display: 'flex', gap: 'var(--space-3)' }
const failed = {
  margin: 'var(--space-4) 0 0',
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--status-failed-fg)'
}
</script>

<template>
  <div :style="page">
    <SettingsGroup label="About">
      <SettingsRow label="Companion" control-width="12ch">
        <span :style="value">{{ bridge.version || '-' }}</span>
      </SettingsRow>
      <SettingsRow label="Desktop" control-width="18ch">
        <span :style="value">{{ desktop }}</span>
      </SettingsRow>
      <SettingsRow
        label="Unpair this phone"
        description="Forgets this desktop. Pair again from the desktop's Devices tab."
        control-width="auto"
      >
        <span v-if="step === ASKED" :style="pair">
          <Button variant="ghost" :disabled="busy" @click="press('cancel')">Cancel</Button>
          <Button variant="danger" :disabled="busy" @click="press('confirm')">Confirm</Button>
        </span>
        <Button v-else variant="danger" @click="press('unpair')">Unpair</Button>
      </SettingsRow>
    </SettingsGroup>
    <p v-if="failure" :style="failed">{{ failure }}</p>
  </div>
</template>
