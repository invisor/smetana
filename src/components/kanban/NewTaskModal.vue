<script setup>
import { computed, ref, watch } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Dropdown from '../core/Dropdown.vue'
import Textarea from '../core/Textarea.vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  busy: { type: Boolean, default: false },
  /* The column the dialog was opened from: it decides where the card lands. */
  status: { type: String, default: null }
})

const emit = defineEmits(['close', 'submit'])

/* The types and priorities are the ones bd understands, each behind an Auto
   that leaves the choice to the agent — which has read the text of the task,
   as nothing in this app has. Auto travels as null rather than as the word:
   a field that is either a value bd knows or nothing at all cannot reach Rust
   carrying a type bd would reject. */
const AUTO = { value: 'auto', label: 'Auto' }
const TYPES = [AUTO, 'task', 'bug', 'feature', 'chore', 'epic', 'decision']
const PRIORITIES = [
  AUTO,
  { value: '0', label: 'P0 · highest' },
  { value: '1', label: 'P1' },
  { value: '2', label: 'P2' },
  { value: '3', label: 'P3' },
  { value: '4', label: 'P4 · lowest' }
]

/* Whether the agent talks the task through before filing it. Auto leaves the
   judgement to the agent: nothing here has read the text, and guessing from
   the length of a title would be wrong in both directions. */
const BRAINSTORM = [
  { value: 'auto', label: 'Auto' },
  { value: 'on', label: 'On' },
  { value: 'off', label: 'Off' }
]

/* One field, not a title and a description: bd wants a title, but writing one
   is the agent's job — it is the only party here that has read what the person
   wrote, and the filing skill says how this project wants a title worded. */
const text = ref('')
const issueType = ref('auto')
const priority = ref('auto')
const brainstorm = ref('auto')

const valid = computed(() => text.value.trim().length > 0)

const intro = computed(() =>
  props.status
    ? `An agent files it, in ${String(props.status).replace(/-/g, ' ')}.`
    : 'An agent files it.'
)

const submit = () => {
  if (!valid.value || props.busy) return
  emit('submit', {
    text: text.value.trim(),
    issue_type: issueType.value === 'auto' ? null : issueType.value,
    priority: priority.value === 'auto' ? null : Number(priority.value),
    brainstorm: brainstorm.value
  })
}

/* We do not clear in submit(): if the write fails, the user has to see their
   own text rather than an empty field — the reset follows the outcome, not the
   fact of submitting. The parent closes the dialog both on success and on
   cancel; on a failed write it stays open, so a reset on "open -> false" covers
   both cases that should clear the form and never the one that should not. */
watch(
  () => props.open,
  (isOpen) => {
    if (isOpen) return
    text.value = ''
    issueType.value = 'auto'
    priority.value = 'auto'
    brainstorm.value = 'auto'
  }
)

const fields = { display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }
const row = { display: 'flex', gap: 'var(--space-4)' }
const label = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)',
  marginBottom: 'var(--space-3)'
}
const field = { flex: 1, minWidth: 0 }
</script>

<template>
  <Modal :open="open" :closable="!busy" title="New task" :description="intro" @close="$emit('close')">
    <div :style="fields">
      <div>
        <div :style="label">Task</div>
        <Textarea v-model="text" :rows="5" placeholder="What needs doing, and anything the agent should know" />
      </div>
      <div :style="row">
        <div :style="field">
          <div :style="label">Type</div>
          <Dropdown v-model="issueType" :options="TYPES" />
        </div>
        <div :style="field">
          <div :style="label">Priority</div>
          <Dropdown v-model="priority" :options="PRIORITIES" />
        </div>
        <div :style="field">
          <div :style="label">Brainstorming</div>
          <Dropdown v-model="brainstorm" :options="BRAINSTORM" />
        </div>
      </div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!valid || busy" @click="submit">
        {{ busy ? 'Creating…' : 'Create' }}
      </Button>
    </template>
  </Modal>
</template>
