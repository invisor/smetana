<script setup>
/* The Server tab: which `smetanad` this device uses, the one on this machine or one somewhere else
   paired by a code.

   Three things on one screen, in the order a person meets them. What is chosen now and how the
   connection is going; the way to choose a remote server (paste the code `smetanad pair --kind
   desktop` printed, press the button, confirm at the server); and the way back. Between the
   button and the switch there is one question, and only when it applies: the server on this machine
   has work in it, so the person says whether it is left running or stops once that work has
   finished. Nothing is ended by switching - not here and not by the stop, which waits.

   Presentational, like every component here: handed values, emitting what was pressed. The window
   talks to the shell (`stores/connection.js`), so this renders in `?view=gallery` with nothing
   behind it. The rules - what a code may look like, what the choice is called - are
   `serverChoice.js`. */
import { computed } from 'vue'
import Button from '../core/Button.vue'
import Input from '../core/Input.vue'
import SettingsGroup from './SettingsGroup.vue'
import SettingsRow from './SettingsRow.vue'
import {
  LEAVE_RUNNING,
  STOP_WHEN_FINISHED,
  WAITING_FOR_CONFIRMATION,
  askSentence,
  stateLine,
  targetLine,
  ticketProblem
} from './serverChoice.js'

const props = defineProps({
  /* The shell's `{ kind: 'local' }` or `{ kind: 'remote', node, relay }`. */
  target: { type: Object, default: () => ({ kind: 'local' }) },
  /* The connection store's value, for the state phrase. */
  connection: { type: Object, default: () => ({}) },
  /* The text of the code field. */
  ticket: { type: String, default: '' },
  /* The local server's live work when the switch is waiting on an answer about it, else null. */
  ask: { type: Array, default: null },
  /* A switch is under way. Pairing waits for a person at the server, so this can be long. */
  busy: { type: Boolean, default: false },
  /* What the last press was refused with, in one readable line, and what it left to say. */
  error: { type: String, default: null },
  note: { type: String, default: '' }
})

const emit = defineEmits(['update:ticket', 'use-remote', 'choose', 'cancel-ask', 'use-local'])

const remote = computed(() => props.target?.kind === 'remote')
const problem = computed(() => ticketProblem(props.ticket))
const canSwitch = computed(() => props.ticket.trim() !== '' && problem.value === null && !props.busy)

const textStyle = {
  color: 'var(--text-secondary)',
  font: 'var(--weight-regular) var(--text-label-size)/var(--leading-normal) var(--font-sans)'
}
const fieldStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4) 0'
}
const panelStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  padding: 'var(--space-5)',
  margin: 'var(--space-4) 0',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-4)'
}
const buttonsStyle = { display: 'flex', gap: 'var(--space-4)', flexWrap: 'wrap' }
const errorStyle = { ...textStyle, color: 'var(--status-failed-fg)', marginTop: 'var(--space-4)' }
</script>

<template>
  <div>
    <SettingsGroup label="In use">
      <SettingsRow label="Server" :description="targetLine(props.target)" control-width="22ch">
        <span :style="textStyle">{{ stateLine(props.connection) }}</span>
      </SettingsRow>
      <SettingsRow
        v-if="remote"
        label="Go back to this machine"
        description="The server on this machine keeps running while you use another, and is started again if it has stopped."
        control-width="22ch"
      >
        <Button variant="secondary" icon="laptop" :disabled="props.busy" @click="emit('use-local')">
          Use this machine
        </Button>
      </SettingsRow>
    </SettingsGroup>

    <SettingsGroup label="Use a server on another machine">
      <SettingsRow
        label="Pairing code"
        description="On the server run smetanad pair --kind desktop, paste what it prints here, press the button and confirm at the server. Projects, agents and runs then live there."
        control-width="0"
      />
      <div :style="fieldStyle">
        <Input
          :model-value="props.ticket"
          mono
          :invalid="problem !== null"
          :disabled="props.busy"
          placeholder="smetana2:…"
          @update:model-value="emit('update:ticket', $event)"
          @keydown.enter="canSwitch && emit('use-remote')"
        />
        <span v-if="problem" :style="{ ...textStyle, color: 'var(--status-failed-fg)' }">{{ problem }}</span>
      </div>

      <!-- Only when the server being left has work in it. The two answers are both safe: neither
           ends anything. -->
      <div v-if="props.ask" :style="panelStyle" role="alertdialog" aria-label="The server on this machine is busy">
        <span :style="textStyle">{{ askSentence(props.ask) }}</span>
        <div :style="buttonsStyle">
          <Button variant="primary" :disabled="props.busy" @click="emit('choose', false)">{{ LEAVE_RUNNING }}</Button>
          <Button variant="secondary" :disabled="props.busy" @click="emit('choose', true)">
            {{ STOP_WHEN_FINISHED }}
          </Button>
          <Button variant="ghost" :disabled="props.busy" @click="emit('cancel-ask')">Cancel</Button>
        </div>
      </div>
      <div v-else :style="buttonsStyle">
        <Button variant="primary" icon="server" :disabled="!canSwitch" @click="emit('use-remote')">
          {{ props.busy ? 'Pairing…' : 'Use this server' }}
        </Button>
      </div>
      <p v-if="props.busy" :style="{ ...textStyle, marginTop: 'var(--space-4)' }">{{ WAITING_FOR_CONFIRMATION }}</p>
      <p v-if="props.note" :style="{ ...textStyle, marginTop: 'var(--space-4)' }">{{ props.note }}</p>
      <p v-if="props.error" :style="errorStyle">{{ props.error }}</p>
    </SettingsGroup>
  </div>
</template>
