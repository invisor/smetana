<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'

/* A parked task on its way back to Ready, asked about first.

   Inline in `DesktopApp.vue` until the dialogs became windows of their own, and
   moved here word for word: the host draws a component and nothing else, so a
   dialog written into a view had to become one before it could move.

   The questions themselves are quoted rather than summarised. This is the one
   moment somebody decides whether they matter, and a dialog that only said
   "there are questions" would send them to the card to find out — which is the
   decision they came here to make. Three ways out, and the recommended one
   last, where every other dialog in this app puts the action it expects. */
const props = defineProps({
  open: { type: Boolean, default: false },
  /* The issue's id, which is the whole of the heading. An id and not the issue,
     for the reason `DeleteTaskModal` carries: the dialog names what the board
     holds now, not what it held when a menu was opened. */
  id: { type: String, default: '' },
  /* The issue's own title. Not called `title` for the reason spelled out in
     `DeleteTaskModal`: in a window of its own that name belongs to the OS
     frame's caption. */
  taskTitle: { type: String, default: '' },
  /* What is still unanswered, one string per question, already picked out of
     the issue's notes by the caller. A parked task with no note is an ordinary
     outcome — somebody can park one by hand — and this dialog says so in prose
     rather than drawing an empty list. */
  questions: { type: Array, default: () => [] }
})

defineEmits(['close', 'confirm', 'resolve'])

/* The same words the app window announces for the OS frame — see `openReady` in
   `DesktopApp.vue`. */
const title = computed(() => `Move ${props.id} to ready with the question unanswered?`)

const description = computed(() =>
  props.questions.length
    ? 'An agent parked this because it could not settle something on its own. Moving it to ready puts it back in the queue, and whoever takes it next meets the same question.'
    : 'An agent parked this and left no note saying why. Moving it to ready puts it back in the queue, and whatever stopped the last agent is still there.'
)

const taskTitleStyle = {
  font: 'var(--weight-medium) var(--text-md)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  textWrap: 'pretty'
}
/* The parked questions, quoted verbatim. Prose rather than a table for the
   reason the inspector's own notes section carries: a note is somebody's
   sentence, and a row would promise a field it is not. The triangle beside each
   is `status/status.js`'s glyph for parked, so the dialog and the card the
   person came from say the same thing the same way. */
const questionListStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  marginTop: 'var(--space-4)'
}
const questionStyle = {
  display: 'flex',
  gap: 'var(--space-4)',
  alignItems: 'flex-start',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
}
const questionGlyphStyle = {
  flex: 'none',
  display: 'flex',
  marginTop: '2px',
  color: 'var(--attn-loud)'
}
</script>

<template>
  <Modal :open="open" :title="title" :description="description" @close="$emit('close')">
    <div :style="taskTitleStyle">{{ taskTitle }}</div>
    <div v-if="questions.length" :style="questionListStyle">
      <div v-for="(question, i) in questions" :key="i" :style="questionStyle">
        <span :style="questionGlyphStyle"><Icon name="triangle-alert" :size="14" /></span>
        <span>{{ question }}</span>
      </div>
    </div>
    <template #footer>
      <Button variant="ghost" @click="$emit('close')">Cancel</Button>
      <Button variant="secondary" @click="$emit('confirm')">Move anyway</Button>
      <Button variant="primary" @click="$emit('resolve')">Answer questions</Button>
    </template>
  </Modal>
</template>
