<script setup>
/* One agent. A driven session is the conversation panel itself - turns, markdown, the
   permission and AskUserQuestion cards answered through `session_answer`, the field that sends
   through `session_send` - with the paperclip left out (an attachment is a path on the desktop,
   which the bridge refuses).

   A terminal session is the other thing and is read-only: the last lines of what it wrote, as
   plain text, because a programme that draws a screen cannot be shown on one this size and the
   bridge lets the phone read output without taking the seat the desktop's own tab sits in.
   The two id spaces are separate counters on the desktop, which is why the entry says which
   one it is and nothing here guesses from the number. */
import { computed, onMounted, ref, watch } from 'vue'
import ConversationView from '@src/components/conversation/ConversationView.vue'
import Button from '@src/components/core/Button.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import StatusBadge from '@src/components/status/StatusBadge.vue'
import { drivenSessions } from '@src/stores/conversation.js'
import { terminalState, toUiState } from '@src/stores/terminals.js'
import { invoke } from '../bridge/core.js'
import { linesOf } from '../terminalText.js'

const props = defineProps({
  id: { type: Number, required: true },
  driven: { type: Boolean, default: true }
})

const emit = defineEmits(['close'])

const lines = ref([])
const loading = ref(false)
const failed = ref('')

const session = computed(() => terminalState.sessions.find((s) => s.id === props.id))
const state = computed(() => (session.value ? toUiState(session.value) : null))

async function read() {
  if (props.driven) return
  loading.value = true
  try {
    lines.value = linesOf(await invoke('terminal_attach', { id: props.id }))
    failed.value = ''
  } catch (err) {
    failed.value = typeof err === 'string' ? err : 'The output could not be read.'
  } finally {
    loading.value = false
  }
}

onMounted(read)
/* The desktop says when a session's state moves (`terminal:state`); its output has just
   changed in the same breath often enough that reading again is the right answer. */
watch(() => session.value?.state, read)

/* A conversation closed on another device leaves the list (`session:removed`); the screen that
   was showing it has nothing left to show and gives the person back to the Agents tab. */
watch(
  () => props.driven && drivenSessions.value.some((s) => s.id === props.id),
  (present, was) => {
    if (was && !present) emit('close')
  }
)

const page = { flex: 1, minHeight: 0, display: 'flex', flexDirection: 'column' }
const bar = {
  flex: '0 0 auto',
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  padding: 'var(--space-3) var(--space-4)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const note = {
  flex: 1,
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-muted)'
}
const output = {
  flex: 1,
  minHeight: 0,
  margin: 0,
  padding: 'var(--space-4)',
  overflow: 'auto',
  background: 'var(--surface-sunken)',
  color: 'var(--text-primary)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere'
}
</script>

<template>
  <div :style="page">
    <!-- A long press fires `contextmenu` on Android and the rows would emit to nobody. -->
    <ConversationView v-if="driven" :session-id="id" :attachable="false" :selection-menu="false" />
    <template v-else>
      <div :style="bar">
        <StatusBadge v-if="state" :status="state" size="sm" />
        <span :style="note">Read-only. The terminal can be used on the desktop.</span>
        <Button size="sm" variant="secondary" icon="refresh-cw" :disabled="loading" @click="read">Refresh</Button>
      </div>
      <EmptyState v-if="failed" icon="triangle-alert" tone="error" title="Could not read the output" :description="failed" />
      <EmptyState v-else-if="!lines.length && !loading" icon="terminal" title="No output yet" />
      <pre v-else :style="output">{{ lines.join('\n') }}</pre>
    </template>
  </div>
</template>
