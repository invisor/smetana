<script setup>
/* A task is filed by an agent: the person says what they want, an agent reads the repository and
   writes the task, and the conversation with it is where the person is taken - it may well ask
   before it files. The agent is started with the intent the desktop's "New task" window starts
   it with (./newTask.js), as a driven session, so the conversation is the panel's own.

   The fields are that window's, one under another at the width of the screen: the words, the
   pictures, the type, the priority and the three stages with their cascade. The controls are the
   native `Select`, not `Dropdown`, whose popover is positioned in window coordinates.

   Only a conversation can be started from a phone: a terminal session is a PTY on the
   workstation (`terminal_create` is not on the bridge). When the desktop's conversation panel is
   off, or the agent it would use cannot be driven, this says so and offers nothing. */
import { computed, ref, watch } from 'vue'
import Button from '@src/components/core/Button.vue'
import Select from '@src/components/core/Select.vue'
import Textarea from '@src/components/core/Textarea.vue'
import AttachmentStrip from '@src/components/kanban/AttachmentStrip.vue'
import { STAGES, cascade } from '@src/components/kanban/taskStages.js'
import { attachFiles, attachmentsState, clearAttachments, removeAttachment } from '@src/stores/attachments.js'
import { canDrive, conversationState, startConversation } from '@src/stores/conversation.js'
import { effectiveAgents, settings } from '@src/stores/settings.js'
import { agentLabel } from '@src/stores/agents.js'
import { bridge } from '../bridge/status.js'
import { PRIORITIES, STAGE_DEFAULT, TYPES, canAttach, canStart, newTaskIntent } from '../newTask.js'

const emit = defineEmits(['started'])

const text = ref('')
const issueType = ref('auto')
const priority = ref('auto')
const brainstorm = ref(STAGE_DEFAULT)
const spec = ref(STAGE_DEFAULT)
const plan = ref(STAGE_DEFAULT)
const busy = ref(false)
const attaching = ref(false)
const picker = ref(null)

const drivable = computed(() => canDrive(effectiveAgents.value.agent))
const refusal = computed(() => (conversationState.lastError?.session === null ? conversationState.lastError.text : ''))
const attachable = computed(() => canAttach(bridge.status))
const stages = computed(() => cascade(brainstorm.value, spec.value, plan.value))

/* A child opens on Auto whenever its parent moves, as in the desktop window. */
watch(brainstorm, () => {
  spec.value = STAGE_DEFAULT
  plan.value = STAGE_DEFAULT
})
watch(spec, () => {
  plan.value = STAGE_DEFAULT
})

async function choose(event) {
  const input = event.target
  const files = [...(input.files ?? [])]
  input.value = ''
  if (!files.length) return
  attaching.value = true
  try {
    await attachFiles(files)
  } finally {
    attaching.value = false
  }
}

function reset() {
  text.value = ''
  issueType.value = 'auto'
  priority.value = 'auto'
  brainstorm.value = STAGE_DEFAULT
  spec.value = STAGE_DEFAULT
  plan.value = STAGE_DEFAULT
  clearAttachments()
}

async function start() {
  if (!canStart({ text: text.value, busy: busy.value, attaching: attaching.value }) || !settings.activeProject) return
  busy.value = true
  conversationState.lastError = null
  try {
    const intent = newTaskIntent({
      text: text.value,
      issueType: issueType.value,
      priority: priority.value,
      brainstorm: brainstorm.value,
      spec: spec.value,
      plan: plan.value,
      images: attachable.value ? attachmentsState.items.map((item) => item.path) : []
    })
    const id = await startConversation(settings.activeProject, intent)
    if (id != null) {
      reset()
      emit('started', id)
    }
  } finally {
    busy.value = false
  }
}

const full = { width: '100%' }
const page = { padding: 'var(--space-5)', display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }
const lead = {
  margin: 0,
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)'
}
const muted = { ...lead, color: 'var(--text-muted)' }
const failure = { ...lead, color: 'var(--status-failed-fg)' }
const errorLine = { ...failure, font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)' }
const label = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
const field = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }
const images = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }
const attachRow = { display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }
const hidden = { display: 'none' }
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
      <div :style="field">
        <div :style="label">Task</div>
        <Textarea v-model="text" :rows="8" placeholder="What should the task say?" />
      </div>
      <div v-if="attachable" :style="images">
        <div :style="attachRow">
          <div :style="label">Images</div>
          <Button size="sm" icon="paperclip" :disabled="busy || attaching" @click="picker?.click()">Attach</Button>
          <input ref="picker" type="file" accept="image/*" multiple :style="hidden" @change="choose" />
        </div>
        <AttachmentStrip :items="attachmentsState.items" :disabled="busy" @remove="removeAttachment" />
        <p v-if="attachmentsState.lastError" :style="errorLine">{{ attachmentsState.lastError }}</p>
      </div>
      <p v-else :style="muted">Attaching pictures needs a newer desktop app.</p>
      <div :style="field">
        <div :style="label">Type</div>
        <Select v-model="issueType" :options="TYPES" :style="full" />
      </div>
      <div :style="field">
        <div :style="label">Priority</div>
        <Select v-model="priority" :options="PRIORITIES" :style="full" />
      </div>
      <div :style="field">
        <div :style="label">Brainstorming</div>
        <Select v-model="brainstorm" :options="STAGES" :style="full" />
      </div>
      <div :style="field">
        <div :style="label">Spec</div>
        <Select
          :model-value="stages.spec.value"
          :options="STAGES"
          :disabled="!stages.spec.interactive"
          :style="full"
          @update:model-value="spec = $event"
        />
      </div>
      <div :style="field">
        <div :style="label">Plan</div>
        <Select
          :model-value="stages.plan.value"
          :options="STAGES"
          :disabled="!stages.plan.interactive"
          :style="full"
          @update:model-value="plan = $event"
        />
      </div>
      <p v-if="refusal" :style="failure">{{ refusal }}</p>
      <Button variant="primary" :disabled="!canStart({ text, busy, attaching })" @click="start">
        {{ busy ? 'Starting the agent' : 'Start the agent' }}
      </Button>
    </template>
  </div>
</template>
