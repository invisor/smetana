<script setup>
import { computed, ref, watch } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Input from '../core/Input.vue'
import Select from '../core/Select.vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  busy: { type: Boolean, default: false },
  /* The column the dialog was opened from: it decides where the card lands. */
  status: { type: String, default: null }
})

const emit = defineEmits(['close', 'submit'])

// The types and priorities are the ones bd understands.
const TYPES = ['task', 'bug', 'feature', 'chore', 'epic', 'decision']
const PRIORITIES = [
  { value: '0', label: 'P0 · highest' },
  { value: '1', label: 'P1' },
  { value: '2', label: 'P2' },
  { value: '3', label: 'P3' },
  { value: '4', label: 'P4 · lowest' }
]

const title = ref('')
const issueType = ref('task')
const priority = ref('2')
const description = ref('')

const valid = computed(() => title.value.trim().length > 0)

const intro = computed(() =>
  props.status
    ? `Goes straight into the tracker, in ${String(props.status).replace(/-/g, ' ')}.`
    : 'Goes straight into the tracker.'
)

const submit = () => {
  if (!valid.value || props.busy) return
  emit('submit', {
    title: title.value.trim(),
    issue_type: issueType.value,
    priority: Number(priority.value),
    description: description.value.trim() || null
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
    title.value = ''
    issueType.value = 'task'
    priority.value = '2'
    description.value = ''
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
        <div :style="label">Title</div>
        <Input v-model="title" placeholder="What needs doing" />
      </div>
      <div :style="row">
        <div :style="field">
          <div :style="label">Type</div>
          <Select v-model="issueType" :options="TYPES" />
        </div>
        <div :style="field">
          <div :style="label">Priority</div>
          <Select v-model="priority" :options="PRIORITIES" />
        </div>
      </div>
      <div>
        <div :style="label">Description</div>
        <Input v-model="description" placeholder="Optional" />
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
