<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'

/* Shown once, when a project is added and has no .smetana/project.toml. It
   states what will happen before anything happens: a session starts, a folder
   is read, and a file appears in the person's repository. None of that should
   arrive unannounced — adding a project to a list is otherwise a read. */
const props = defineProps({
  open: { type: Boolean, default: false },
  name: { type: String, default: '' },
  busy: { type: Boolean, default: false }
})

defineEmits(['close', 'confirm'])

const description = computed(() =>
  props.name
    ? `${props.name} has no run configuration yet.`
    : 'This project has no run configuration yet.'
)

const body = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)'
}
const pathStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
}
</script>

<template>
  <Modal
    :open="open"
    :closable="!busy"
    title="Set this project up?"
    :description="description"
    @close="$emit('close')"
  >
    <div :style="body">
      <p>
        An agent will look through the folder — its repositories, their manifests and scripts —
        and write what it finds to <span :style="pathStyle">.smetana/project.toml</span>.
        It will ask about anything the folder does not answer.
      </p>
      <p>Nothing else is changed, and you can review the file before any run uses it.</p>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="busy" @click="$emit('confirm')">
        {{ busy ? 'Starting…' : 'Set up' }}
      </Button>
    </template>
  </Modal>
</template>
