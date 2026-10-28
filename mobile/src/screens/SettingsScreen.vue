<script setup>
/* The phone's settings: Appearance and Kanban, which this phone can set for itself instead of
   taking the desktop's (`../phoneSettings.js`; `Same as desktop` is the default and follows it),
   and About: what this build is, which desktop it talks to, and the way to forget that desktop.

   Unpairing takes two taps (../unpairStep.js is the rule): the first turns the button into a
   question, Confirm calls `unpairAndForget` and `App.vue` then shows the pairing screen, since
   the bridge no longer says it is paired. */
import { computed, ref } from 'vue'
import Button from '@src/components/core/Button.vue'
import Select from '@src/components/core/Select.vue'
import SettingsGroup from '@src/components/settings/SettingsGroup.vue'
import SettingsRow from '@src/components/settings/SettingsRow.vue'
import { bridge } from '../bridge/status.js'
import { desktopMinor, isOnline } from '../bridge/states.js'
import { unpairAndForget } from '../session.js'
import { phone, setPhoneSetting } from '../workspace.js'
import {
  COLUMN_OPTIONS,
  DENSITY_OPTIONS,
  FONT_OPTIONS,
  INTERVAL_OPTIONS,
  THEME_OPTIONS
} from '../phoneSettings.js'
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

const full = { width: '100%' }
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
    <SettingsGroup label="Appearance">
      <SettingsRow label="Theme" control-width="22ch">
        <Select :model-value="phone.theme" :options="THEME_OPTIONS" :style="full" @update:model-value="setPhoneSetting('theme', $event)" />
      </SettingsRow>
      <SettingsRow label="Density" control-width="22ch">
        <Select :model-value="phone.density" :options="DENSITY_OPTIONS" :style="full" @update:model-value="setPhoneSetting('density', $event)" />
      </SettingsRow>
      <SettingsRow label="Font size" description="This phone only." control-width="22ch">
        <Select :model-value="phone.uiFontSize" :options="FONT_OPTIONS" :style="full" @update:model-value="setPhoneSetting('uiFontSize', Number($event))" />
      </SettingsRow>
    </SettingsGroup>
    <SettingsGroup label="Kanban">
      <SettingsRow label="Columns" control-width="22ch">
        <Select :model-value="phone.columns" :options="COLUMN_OPTIONS" :style="full" @update:model-value="setPhoneSetting('columns', $event)" />
      </SettingsRow>
      <SettingsRow label="Show tasks from" control-width="22ch">
        <Select :model-value="phone.interval" :options="INTERVAL_OPTIONS" :style="full" @update:model-value="setPhoneSetting('interval', $event)" />
      </SettingsRow>
    </SettingsGroup>
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
