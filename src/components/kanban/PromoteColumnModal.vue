<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import { promoteTitle, taskCount } from './promoteTitle.js'

/* The confirm behind the deferred header's button. What it exists to say is the
   count: moving a whole column into the queue has no undo — putting a task back
   is one issue at a time in the inspector — so the number is the entire content
   of the question, and it is in the title rather than buried in the body.

   The count is a snapshot the caller took at the moment of the press, not a
   live reading of the column. The board can change under an open dialog, and a
   number that moved between being read and being confirmed would describe a set
   nobody agreed to. */
const props = defineProps({
  open: { type: Boolean, default: false },
  count: { type: Number, default: 0 },
  /* Writing is under way. Each issue costs about two seconds, so a column of
     twenty is most of a minute — long enough that the dialog owes a person the
     progress rather than a spinner. */
  busy: { type: Boolean, default: false },
  /* How many have landed so far, which is progress while `busy` and the first
     half of the report afterwards. */
  moved: { type: Number, default: 0 },
  /* How many did not, or null while nothing has been attempted. A run that
     lost none closes the dialog rather than reporting a zero, so a number here
     always means there is something to say. */
  failed: { type: Number, default: null }
})

defineEmits(['close', 'confirm'])

/* The heading is `promoteTitle.js`'s and not this file's, because the OS frame
   of the window this dialog is drawn in has to carry the same sentence and is
   captioned from the other side of the wire (`DesktopApp.vue`, `openPromote`).
   A rule written out in both places would go quietly wrong in the frame. */
const title = computed(() =>
  promoteTitle({ count: props.count, moved: props.moved, failed: props.failed })
)

/* Three things this can be saying, and only one of them at a time: what is
   about to happen, how far it has got, and what became of it. */
const body = computed(() => {
  if (props.failed != null) {
    return `${taskCount(props.failed)} could not be moved. The board shows the ones that did — nothing was rolled back.`
  }
  if (props.busy) return `Moved ${props.moved} of ${props.count}…`
  return 'They move to ready and nothing else happens — no run starts. There is no undo: putting one back is one issue at a time in the inspector.'
})

const bodyStyle = {
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)'
}
</script>

<template>
  <Modal :open="open" :closable="!busy" :title="title" @close="$emit('close')">
    <div :style="bodyStyle">{{ body }}</div>
    <template #footer>
      <!-- After a partial failure there is nothing left to confirm: the writes
           have all been attempted and pressing again would ask bd to move what
           already moved. The way out is the only button. -->
      <template v-if="failed != null">
        <Button variant="primary" @click="$emit('close')">Close</Button>
      </template>
      <template v-else>
        <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
        <Button variant="primary" :disabled="busy" @click="$emit('confirm')">
          {{ busy ? 'Moving…' : 'Move to ready' }}
        </Button>
      </template>
    </template>
  </Modal>
</template>
