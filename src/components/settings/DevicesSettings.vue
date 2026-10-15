<script setup>
/* The Devices tab: pairing a phone with this desktop, and the phones already
   paired.

   Four things on one screen, in the order a person meets them. A button that
   makes a pairing code; the code itself, while there is one; the question "this
   phone wants to connect", while one is asking; and the list of phones that may
   connect, each with the one control that matters on it, Revoke. Under them the
   relay the phones go through, which is the only thing here that is a setting.

   **Nothing is trusted because it was scanned.** The code only gets a phone as
   far as asking. The confirmation is the part that lets it in, which is why it
   is drawn over the code, in the place a person is already looking, and why its
   second sentence says what to check.

   Presentational, like every component here: handed values, emitting what was
   pressed. The window talks to Rust, so this renders in `?view=gallery` with
   nothing behind it. */
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import Input from '../core/Input.vue'
import SettingsGroup from './SettingsGroup.vue'
import SettingsRow from './SettingsRow.vue'
import {
  DEFAULT_RELAY_URL,
  countdownLine,
  lastContactLine,
  pairedLine,
  platformLabel,
  qrCode,
  relayProblem,
  requestDetail,
  requestLine
} from './devices.js'

const props = defineProps({
  /* The paired phones, in the shape `stores/remote.js` hands over. */
  devices: { type: Array, default: () => [] },
  /* The string the code carries, or `null` when no pairing is going on. */
  ticket: { type: String, default: null },
  /* Whole seconds the code has left. Counted by the window, which owns the
     clock, so this stays a function of its props. */
  remaining: { type: Number, default: 0 },
  /* The phone asking to be let in, `{ deviceName, platform }`, or `null`. */
  request: { type: Object, default: null },
  /* The browser's clock in milliseconds, for "last seen". Handed in for the
     reason `remaining` is. */
  now: { type: Number, default: 0 },
  /* Something is in flight: a code being made, an answer being sent. */
  busy: { type: Boolean, default: false },
  /* What the last press was refused with, in one readable line. */
  error: { type: String, default: null },
  /* Mirrors `RemoteSettings::default()` in Rust, `defaults().remote` in
     `stores/settings.js` and `view` in `SettingsWindow.vue`: all three import
     it from `devices.js` rather than spell it again. */
  relayUrl: { type: String, default: DEFAULT_RELAY_URL }
})

const emit = defineEmits(['begin', 'cancel', 'decide', 'revoke', 'update:relayUrl'])

const code = computed(() => (props.ticket && props.remaining > 0 ? qrCode(props.ticket) : null))
const expired = computed(() => props.ticket !== null && props.remaining <= 0)
const asking = computed(() => props.request !== null)

/* The relay field is edited as text and committed whole, on Enter or when the
   focus leaves. Committing per keystroke would put `https://r` in the file on
   the way to `https://relay.example.org`, and the endpoint reads that file the
   next time a code is made. A value that fails `relayProblem` is never sent: it
   stays in the field with the reason under it, because Rust would replace it
   with the default without a word. */
const draft = ref(props.relayUrl)
watch(
  () => props.relayUrl,
  (value) => {
    draft.value = value
  }
)
const problem = computed(() => (draft.value.trim() === props.relayUrl ? null : relayProblem(draft.value)))
const commit = () => {
  const text = draft.value.trim()
  if (text === props.relayUrl || relayProblem(text) !== null) return
  emit('update:relayUrl', text)
}
onBeforeUnmount(commit)

/* Under the row's description rather than in its control column: an address is
   long, and that column is a few characters wide beside a description that
   asks for the rest. */
const fieldStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4) 0'
}
const panelStyle = {
  display: 'flex',
  flexDirection: 'column',
  alignItems: 'center',
  gap: 'var(--space-4)',
  padding: 'var(--space-5)',
  margin: 'var(--space-4) 0',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-4)'
}
/* Always paper under the modules, in both themes — see `tokens/qr.css`. */
const matStyle = {
  background: 'var(--qr-paper)',
  padding: 'var(--space-4)',
  borderRadius: 'var(--radius-3)',
  lineHeight: 0
}
const codeStyle = { width: '30ch', maxWidth: '100%', height: 'auto', display: 'block' }
const lineStyle = {
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)',
  textAlign: 'center'
}
const titleStyle = {
  color: 'var(--text-primary)',
  font: 'var(--weight-medium) var(--text-ui-size)/var(--leading-snug) var(--font-sans)',
  textAlign: 'center'
}
const buttonsStyle = { display: 'flex', gap: 'var(--space-4)' }
const deviceStyle = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  padding: 'var(--space-4) 0',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const deviceTextStyle = { flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-2)' }
const nameStyle = {
  color: 'var(--text-primary)',
  font: 'var(--weight-medium) var(--text-ui-size)/var(--leading-snug) var(--font-sans)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const metaStyle = {
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
const iconStyle = { color: 'var(--text-secondary)', display: 'flex', flex: '0 0 auto' }
const emptyStyle = {
  padding: 'var(--space-4) 0',
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
const errorStyle = {
  marginTop: 'var(--space-4)',
  color: 'var(--status-failed-fg)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
</script>

<template>
  <div>
    <SettingsRow
      label="Connect a phone"
      description="Pair the Smetana app on your phone with this desktop. Each phone has to be confirmed here before it can connect."
      control-width="22ch"
    >
      <Button
        v-if="props.ticket === null"
        variant="primary"
        icon="qr-code"
        :disabled="props.busy"
        @click="emit('begin')"
      >
        Show pairing code
      </Button>
      <Button v-else variant="secondary" icon="x" @click="emit('cancel')">Cancel pairing</Button>
    </SettingsRow>

    <!-- A phone has presented a valid code and is waiting. It takes the place
         of the code: the code has been spent, and showing it still would invite
         a second scan of something that no longer works. -->
    <div v-if="asking" :style="panelStyle" role="alertdialog" aria-label="Pairing request">
      <span :style="iconStyle"><Icon name="smartphone" :size="24" /></span>
      <span :style="titleStyle">{{ requestLine(props.request) }}</span>
      <span :style="lineStyle">{{ requestDetail(props.request) }}</span>
      <div :style="buttonsStyle">
        <Button variant="primary" icon="check" :disabled="props.busy" @click="emit('decide', true)">
          Accept
        </Button>
        <Button variant="secondary" icon="x" :disabled="props.busy" @click="emit('decide', false)">
          Reject
        </Button>
      </div>
    </div>
    <div v-else-if="props.ticket !== null" :style="panelStyle">
      <div v-if="code" :style="matStyle">
        <svg
          :viewBox="`-4 -4 ${code.size + 8} ${code.size + 8}`"
          :style="codeStyle"
          shape-rendering="crispEdges"
          role="img"
          aria-label="Pairing code"
        >
          <rect x="-4" y="-4" :width="code.size + 8" :height="code.size + 8" :style="{ fill: 'var(--qr-paper)' }" />
          <path :d="code.path" :style="{ fill: 'var(--qr-ink)' }" />
        </svg>
      </div>
      <span :style="lineStyle">
        {{ expired ? countdownLine(0) : 'Open Smetana on your phone and scan this code.' }}
      </span>
      <span v-if="!expired" :style="lineStyle">{{ countdownLine(props.remaining) }}</span>
      <Button v-else variant="secondary" icon="qr-code" :disabled="props.busy" @click="emit('begin')">
        Show a new code
      </Button>
    </div>
    <p v-if="props.error" :style="errorStyle">{{ props.error }}</p>

    <SettingsGroup label="Paired phones">
      <div v-if="props.devices.length === 0" :style="emptyStyle">No phones are paired.</div>
      <div v-for="device in props.devices" :key="device.key" :style="deviceStyle">
        <span :style="iconStyle"><Icon name="smartphone" /></span>
        <div :style="deviceTextStyle">
          <span :style="nameStyle">{{ device.name }}</span>
          <span :style="metaStyle">
            {{ [platformLabel(device.platform), lastContactLine(device, props.now), pairedLine(device)].filter(Boolean).join(' · ') }}
          </span>
        </div>
        <Button variant="danger" size="sm" icon="trash-2" @click="emit('revoke', device.key)">
          Revoke
        </Button>
      </div>
    </SettingsGroup>

    <SettingsGroup label="Relay">
      <SettingsRow
        label="Relay address"
        description="Phones reach this desktop through a relay, which carries encrypted traffic it cannot read. It does see which devices talk and when. Changes apply the next time you show a pairing code."
        control-width="0"
      />
      <div :style="fieldStyle">
        <Input
          v-model="draft"
          mono
          :invalid="problem !== null"
          placeholder="https://relay.example.org"
          @keydown.enter="commit"
          @focusout="commit"
        />
        <span v-if="problem" :style="{ ...metaStyle, color: 'var(--status-failed-fg)' }">{{ problem }}</span>
      </div>
    </SettingsGroup>
  </div>
</template>
