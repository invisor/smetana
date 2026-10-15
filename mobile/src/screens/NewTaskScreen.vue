<script setup>
/* A task is filed by an agent: the person says what they want, an agent reads the repository and
   writes the task, and the conversation with it is where the person is taken - it may well ask
   before it files. The agent is started with the intent the desktop's "New task" window starts
   it with (./newTask.js), as a driven session, so the conversation is the panel's own.

   Only a conversation can be started from a phone: a terminal session is a PTY on the
   workstation (`terminal_create` is not on the bridge). When the desktop's conversation panel is
   off, or the agent it would use cannot be driven, this says so and offers nothing. */
import { computed, ref } from 'vue'
import Button from '@src/components/core/Button.vue'
import Textarea from '@src/components/core/Textarea.vue'
import { canDrive, conversationState, startConversation } from '@src/stores/conversation.js'
import { effectiveAgents, settings } from '@src/stores/settings.js'
import { agentLabel } from '@src/stores/agents.js'
import { canFile, newTaskIntent } from '../newTask.js'

const emit = defineEmits(['started'])

const text = ref('')
const busy = ref(false)

const drivable = computed(() => canDrive(effectiveAgents.value.agent))
const refusal = computed(() => (conversationState.lastError?.session === null ? conversationState.lastError.text : ''))

async function start() {
  if (busy.value || !canFile(text.value) || !settings.activeProject) return
  busy.value = true
  conversationState.lastError = null
  try {
    const id = await startConversation(settings.activeProject, newTaskIntent(text.value))
    if (id != null) {
      text.value = ''
      emit('started', id)
    }
  } finally {
    busy.value = false
  }
}

const page = { padding: 'var(--space-5)', display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }
const lead = {
  margin: 0,
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)'
}
const failure = { ...lead, color: 'var(--status-failed-fg)' }
</script>

<template>
  <div :style="page">
    <p :style="lead">
      Describe the task. An agent files it, and you can talk to it while it does.
    </p>
    <p v-if="!settings.activeProject" :style="failure">No project is open on the desktop.</p>
    <p v-else-if="!drivable" :style="failure">
      {{ agentLabel(effectiveAgents.agent) }} cannot be started as a conversation. Turn the conversation
      panel on in the desktop's settings, or choose an agent it can drive.
    </p>
    <template v-else>
      <Textarea v-model="text" :rows="8" placeholder="What should the task say?" />
      <p v-if="refusal" :style="failure">{{ refusal }}</p>
      <Button variant="primary" :disabled="busy || !canFile(text)" @click="start">
        {{ busy ? 'Starting the agent' : 'Start the agent' }}
      </Button>
    </template>
  </div>
</template>
