<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'

/* Shown instead of `SetupProjectModal` when a project is added and its folder
   holds nothing but housekeeping (`survey::is_empty` on the back end,
   `setupGate.js`'s `needsStart` here), and again from the tile's own menu
   while it stays that way. It says what will happen before anything happens —
   bd is initialised here, a session starts, and an agent asks before it
   creates anything — because adding a folder to a list is otherwise a read.

   A sibling of `SetupProjectModal.vue` rather than a second state of it: the
   two dialogs are about opposite premises — that one describes a folder with
   something in it, this one a folder with nothing — and folding them into one
   component would mean branching every sentence in it on which premise holds. */
const props = defineProps({
  open: { type: Boolean, default: false },
  name: { type: String, default: '' },
  busy: { type: Boolean, default: false }
})

defineEmits(['close', 'confirm'])

const description = computed(() => `${props.name || 'This folder'} is empty.`)

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
    title="Start a project here?"
    :description="description"
    @close="$emit('close')"
  >
    <div :style="body">
      <div>
        An agent will ask what you want to build here — what it is, which stack, what to call it —
        and only then lay the foundation: the first files, the first commit and the run
        configuration in <span :style="pathStyle">.smetana/project.toml</span>.
      </div>
      <div>
        bd is initialised in this folder first, so the board is live. Nothing else is created
        before you agree to it in the conversation.
      </div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="busy" @click="$emit('confirm')">
        {{ busy ? 'Starting…' : 'Start' }}
      </Button>
    </template>
  </Modal>
</template>
