<script setup>
/* The agents of the project in front: driven conversations and terminal sessions, newest
   first, each with its state. A row waiting on a person is drawn the way `status/status.js`
   says a loud thing is drawn - the triangle, the attention colour, the border - and it is the
   one thing here meant to catch the eye.

   A conversation opens in the panel. A terminal session opens read-only: the bridge lets the
   phone read its output and nothing else, and a screen-drawing programme is not usable on a
   phone anyway.

   "New agent" starts a bare conversation, which is the desktop's button of the same name. */
import { computed, onBeforeUnmount, ref } from 'vue'
import Button from '@src/components/core/Button.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import Icon from '@src/components/core/Icon.vue'
import StatusBadge from '@src/components/status/StatusBadge.vue'
import { STATUS_GLYPH } from '@src/components/status/status.js'
import { agentRows as terminalRows, forgetRestored, formatElapsed, removeSession } from '@src/stores/terminals.js'
import {
  canDrive,
  conversationState,
  closeConversation,
  drivenSessions,
  forget,
  startConversation,
  statusOf
} from '@src/stores/conversation.js'
import { effectiveAgents, settings } from '@src/stores/settings.js'
import IconButton from '@src/components/core/IconButton.vue'
import { closeActsFor } from '@src/components/agent/closeRow.js'
import { agentRowsFor, drivenRecords, entryFor, loud, resumeIntentFor } from '../agentList.js'

const emit = defineEmits(['open'])

/* The rows' clocks. The terminal store ticks its own; the driven ones are worked out here. */
const now = ref(Date.now())
const tick = setInterval(() => (now.value = Date.now()), 30000)
onBeforeUnmount(() => clearInterval(tick))

const rows = computed(() =>
  agentRowsFor(
    terminalRows.value,
    drivenRecords(drivenSessions.value, settings.activeProject, {
      statusOf,
      elapsed: (startedAt) => formatElapsed(now.value - startedAt)
    })
  )
)

const drivable = computed(() => canDrive(effectiveAgents.value.agent))
const busy = ref(false)

async function startAgent() {
  if (busy.value || !settings.activeProject) return
  busy.value = true
  conversationState.lastError = null
  try {
    const id = await startConversation(settings.activeProject, { kind: 'bare' })
    if (id != null) emit('open', { screen: 'conversation', id, driven: true })
  } finally {
    busy.value = false
  }
}

/* An offline row is a record, not a session: pressing it resumes the conversation by the
   desktop's own intent. With no driven road the desktop falls through to a PTY, which a phone
   cannot show, so the person is told and nothing starts. */
const resumeNote = ref('')

async function openRow(item) {
  const entry = entryFor(item)
  if (!entry.resume) {
    emit('open', entry)
    return
  }
  if (busy.value || !settings.activeProject) return
  resumeNote.value = ''
  if (!drivable.value) {
    resumeNote.value = 'Conversations cannot be resumed from here with the desktop\'s current settings.'
    return
  }
  busy.value = true
  conversationState.lastError = null
  try {
    const id = await startConversation(settings.activeProject, resumeIntentFor(item))
    if (id != null) emit('open', { screen: 'conversation', id, driven: true })
  } finally {
    busy.value = false
  }
}

/* The cross: the rule is shared with the desktop, and these are the same four verbs. */
function closeRow(item) {
  const act = closeActsFor(item)
  switch (act.kind) {
    case 'crew-root':
      closeConversation(act.root)
      return
    case 'crew-child':
      return
    case 'driven':
      closeConversation(act.session)
      forget(act.session)
      if (act.recorded) forgetRestored(act.recorded)
      return
    case 'offered':
      forgetRestored(act.conversation)
      return
    default:
      removeSession(act.id)
  }
}

const refusal = computed(() =>
  resumeNote.value || (conversationState.lastError?.session === null ? conversationState.lastError.text : '')
)

const bar = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  padding: 'var(--space-4) var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const barText = {
  flex: 1,
  margin: 0,
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-muted)'
}
const failure = { ...barText, color: 'var(--status-failed-fg)', padding: 'var(--space-4) var(--space-5) 0' }
const wrap = (item) => ({
  display: 'flex',
  alignItems: 'center',
  borderBottom: 'var(--border-w) solid var(--border-subtle)',
  borderLeft: `var(--border-w-strong) solid ${loud(item) ? 'var(--attn-loud)' : 'transparent'}`,
  opacity: item.restored ? 'var(--attn-quiet-opacity)' : undefined
})
const row = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-4)',
  flex: 1,
  minWidth: 0,
  minHeight: 'var(--control-h-lg)',
  padding: 'var(--space-4) var(--space-5)',
  border: 0,
  background: 'transparent',
  color: 'var(--text-primary)',
  textAlign: 'left',
  cursor: 'pointer'
}
const cross = { flex: '0 0 auto', marginRight: 'var(--space-3)' }
const caption = { flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: 'var(--space-2)' }
const label = {
  font: 'var(--weight-medium) var(--text-sm)/var(--leading-snug) var(--font-sans)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const ids = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
const elapsed = (state) => ({
  flex: '0 0 auto',
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: loud({ state }) ? 'var(--attn-loud)' : 'var(--text-muted)'
})
</script>

<template>
  <div>
    <div :style="bar">
      <p :style="barText">
        {{ drivable ? 'Conversations and terminal sessions of this project.' : 'Conversations cannot be started from here with the desktop\'s current settings.' }}
      </p>
      <Button size="sm" variant="secondary" icon="plus" :disabled="busy || !drivable || !settings.activeProject" @click="startAgent">
        New agent
      </Button>
    </div>
    <p v-if="refusal" :style="failure">{{ refusal }}</p>
    <EmptyState
      v-if="!rows.length"
      icon="bot"
      title="No agents"
      description="Start one here, or file a task from the board and talk to the agent that files it."
    />
    <div v-for="item in rows" :key="item.id" :style="wrap(item)">
      <button type="button" :data-attention="loud(item) ? 'loud' : undefined" :style="row" @click="openRow(item)">
        <Icon v-if="item.state === 'needs-you'" :name="STATUS_GLYPH['needs-you']" :size="16" :stroke-width="2.25" :style="{ color: 'var(--attn-loud)' }" />
        <span :style="caption">
          <span :style="label">{{ item.label ?? 'Agent' }}</span>
          <span v-if="item.tasks?.length" :style="ids">{{ item.tasks.join(', ') }}</span>
          <span v-else-if="item.id && typeof item.id === 'number'" :style="ids">terminal session</span>
        </span>
        <StatusBadge :status="item.state" size="sm" />
        <span :style="elapsed(item.state)">{{ item.elapsed }}</span>
      </button>
      <IconButton :style="cross" icon="x" label="Close agent" @click="closeRow(item)" />
    </div>
  </div>
</template>
