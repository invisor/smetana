<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'

/* Deleting a task, asked about first. Reached from a card's own menu and from
   the Task & details header, which send the same payload.

   It was written inline in `DesktopApp.vue` until the dialogs became windows of
   their own: the host draws a component and nothing else, so the two dialogs
   that had none needed one before they could move. Nothing about the words or
   the shape changed on the way — this is the same `Modal`, the same
   description, the same two buttons.

   What it says is the consequence rather than an apology, which is what this
   system asks a destructive confirm to say: bd rewrites the references and the
   dependants are left without their dependency. The issue's own title is in the
   body because that is what somebody reads to check they are about to delete
   the thing they meant — the id in the heading is not something a person
   recognises on sight. */
const props = defineProps({
  open: { type: Boolean, default: false },
  /* The issue's id, which is the whole of the heading. Held by the caller as an
     id rather than as the issue: deletion is irreversible and the board can
     move under an open dialog, so what is drawn is read from the store by id at
     the moment it is drawn. */
  id: { type: String, default: '' },
  /* The issue's own title, and deliberately not called `title`. In a window of
     its own the props this dialog draws from arrive announced, and `title` is
     the one name already spoken for in that vocabulary: it is what the OS frame
     is captioned with (`views/DialogWindow.vue`). A prop of that name here
     would be the frame's caption drawn in the body, in place of the very line a
     person is checking. */
  taskTitle: { type: String, default: '' },
  /* bd is deleting. Both buttons go dead rather than the dialog closing: the
     call can fail, and the message belongs over the dialog that asked. */
  busy: { type: Boolean, default: false }
})

defineEmits(['close', 'confirm'])

/* The same words the app window announces for the OS frame — see `openDelete`
   in `DesktopApp.vue`, which has to say them too because nothing on the window's
   side of the wire knows what this dialog is called. */
const title = computed(() => `Delete ${props.id}?`)

const DESCRIPTION =
  'bd deletes the issue outright and rewrites references to it in whatever was linked to it. Anything that depended on this issue is left without the dependency. There is no undo.'

const taskTitleStyle = {
  font: 'var(--weight-medium) var(--text-md)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  textWrap: 'pretty'
}
</script>

<template>
  <Modal
    :open="open"
    :closable="!busy"
    :title="title"
    :description="DESCRIPTION"
    @close="$emit('close')"
  >
    <div :style="taskTitleStyle">{{ taskTitle }}</div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="danger" :disabled="busy" @click="$emit('confirm')">
        {{ busy ? 'Deleting…' : 'Delete' }}
      </Button>
    </template>
  </Modal>
</template>
