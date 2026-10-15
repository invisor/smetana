<script setup>
import { computed, ref } from 'vue'
import Frame from './Frame.vue'
import Button from '@src/components/core/Button.vue'
import Input from '@src/components/core/Input.vue'
import Icon from '@src/components/core/Icon.vue'
import { pair } from '../bridge/index.js'
import { scanTicket } from '../scan.js'

/* One thing to do here: get the code from the desktop's Devices tab into the
   phone. The camera is the way; the field is for a simulator with no camera, and
   for pasting a code that was sent over rather than shown. */
const pasted = ref('')
const busy = ref(false)
const error = ref('')

async function submit(ticket) {
  busy.value = true
  error.value = ''
  try {
    await pair(ticket)
  } catch (err) {
    error.value = typeof err === 'string' ? err : (err?.message ?? 'Pairing failed.')
  } finally {
    busy.value = false
  }
}

async function scan() {
  error.value = ''
  try {
    const ticket = await scanTicket()
    if (ticket) await submit(ticket)
  } catch (err) {
    error.value = typeof err === 'string' ? err : (err?.message ?? 'The camera could not be opened.')
  }
}

const headingStyle = computed(() => ({
  margin: 0,
  fontSize: 'var(--text-lg)',
  fontWeight: 'var(--weight-medium)'
}))
const bodyStyle = computed(() => ({
  margin: 0,
  color: 'var(--text-secondary)',
  fontSize: 'var(--text-ui-size)',
  lineHeight: 'var(--leading-normal)'
}))
const errorStyle = computed(() => ({
  margin: 0,
  color: 'var(--status-failed-fg)',
  fontSize: 'var(--text-sm)'
}))
const rowStyle = computed(() => ({ display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }))
</script>

<template>
  <Frame>
    <Icon name="smartphone" :size="28" />
    <h1 :style="headingStyle">Connect to your workstation</h1>
    <p :style="bodyStyle">
      On the desktop, open Settings, then Devices, then Connect a phone. Scan the code it shows.
    </p>
    <Button variant="primary" size="lg" icon="qr-code" full-width :disabled="busy" @click="scan">
      Scan code
    </Button>
    <div :style="rowStyle">
      <p :style="bodyStyle">Or paste the code.</p>
      <Input v-model="pasted" mono placeholder="smetana1:…" :disabled="busy" />
      <Button full-width :disabled="busy || !pasted.trim()" @click="submit(pasted.trim())">
        Connect
      </Button>
    </div>
    <p v-if="busy" :style="bodyStyle">Waiting for the desktop to confirm.</p>
    <p v-if="error" :style="errorStyle">{{ error }}</p>
  </Frame>
</template>
