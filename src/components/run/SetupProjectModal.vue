<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'

/* Shown when a project is added and has no .smetana/project.toml, and again
   whenever somebody asks for the setup from the right-click menu on the
   project's own row — which is where that second route now lives, the run
   dialog having carried it until the row grew a menu to hold it. It states what
   will happen before anything happens: a session starts, a folder is read, and
   a file appears in the person's repository. None of that should arrive
   unannounced — adding a project to a list is otherwise a read. */
const props = defineProps({
  open: { type: Boolean, default: false },
  name: { type: String, default: '' },
  /* True when .smetana/project.toml is already there — set up once and being
     set up again, or damaged and being repaired. The words change with it and
     that is the whole reason the prop exists: the first-run copy promises a
     file will appear, which to somebody re-running this over a file they wrote
     by hand reads as a warning that it is about to be replaced. It is not.
     The setup skill reads the file and updates it, keeping what it already
     gets right, and that promise belongs on screen next to the button that
     starts it. */
  existing: { type: Boolean, default: false },
  busy: { type: Boolean, default: false }
})

defineEmits(['close', 'confirm'])

const title = computed(() => (props.existing ? 'Set this project up again?' : 'Set this project up?'))

/* "Already has a run configuration", never "is already set up for runs": this
   dialog is reached over a damaged file as often as over a working one, and a
   project whose config will not parse is not set up for anything. What is true
   in both cases is that a file is there — which is also the fact the promise
   below rests on. */
const description = computed(() => {
  const subject = props.name || 'This project'
  return props.existing
    ? `${subject} already has a run configuration.`
    : `${subject} has no run configuration yet.`
})

/* The reassurance under the first paragraph, and it is a different one in each
   case: the first run has nothing to lose, so what it owes is that the setup
   stays inside the file it named; a second run is over somebody's work, so what
   it owes is that questions come before changes. */
const note = computed(() =>
  props.existing
    ? 'It will ask about anything it cannot work out, and you can review the file before any run uses it.'
    : 'Nothing else is changed, and you can review the file before any run uses it.'
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
    :title="title"
    :description="description"
    @close="$emit('close')"
  >
    <div :style="body">
      <div v-if="existing">
        An agent will look through the folder again — its repositories, their manifests and
        scripts — and update <span :style="pathStyle">.smetana/project.toml</span> with what has
        changed. What the file already gets right is kept: it is updated, not rewritten.
      </div>
      <div v-else>
        An agent will look through the folder — its repositories, their manifests and scripts —
        and write what it finds to <span :style="pathStyle">.smetana/project.toml</span>.
        It will ask about anything the folder does not answer.
      </div>
      <div>{{ note }}</div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="busy" @click="$emit('confirm')">
        {{ busy ? 'Starting…' : existing ? 'Set up again' : 'Set up' }}
      </Button>
    </template>
  </Modal>
</template>
