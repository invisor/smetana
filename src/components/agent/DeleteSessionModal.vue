<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import {
  DELETE_SESSION_DESCRIPTION,
  DELETE_SESSION_TITLE,
  deleteSessionFacts
} from './sessionMenu.js'

/* Deleting a Claude Code transcript, asked about first. Reached from the menu
   on a session row and from nowhere else.

   **The one thing in this app that deletes a file the app did not make.** The
   file tree deletes inside the project, into the trash, and `attachments.js`
   deletes pictures this app copied there itself; a transcript is a person's own
   history, it lives under `~/.claude/projects`, it is in no repository, and it
   goes with `remove_file` because a cross-platform trash is a dependency rather
   than a line of code (`sessions/act.rs` carries that argument). So this dialog
   is not a courtesy — it is the whole of what stands between a menu row and a
   file nothing can bring back, which is why it names three facts about what is
   about to go rather than asking a yes/no about a noun.

   `DeleteTaskModal` is the shape this follows, down to the two buttons and the
   busy wording. What differs is the body: a task is recognised by its title,
   and one line is enough, while a session is recognised by what somebody opened
   it with and then checked against a path and a size — three transcripts of the
   same conversation on three branches look alike in every other respect. */
const props = defineProps({
  open: { type: Boolean, default: false },
  /* The whole record, unlike `DeleteTaskModal`'s bare id, and the difference is
     where the truth lives. A task is in `trackerState` and the dialog's host
     reads the current one by id at the moment it announces; a session is a row
     of a list read off disk when the tab was opened and never watched, so there
     is nothing to look it up in — the record the menu was opened over *is* what
     this app knows. It arrives whole so that a field added to `SessionSummary`
     reaches this dialog without a second prop.

     Null while nothing is being confirmed, which is the state a window that has
     not been told anything yet comes up in. */
  session: { type: Object, default: null },
  /* The delete is running. Both buttons go dead rather than the dialog closing:
     the call can fail — a transcript that has gone since the list was read is
     the ordinary way — and the message belongs over the dialog that asked. */
  busy: { type: Boolean, default: false }
})

defineEmits(['close', 'confirm'])

/* The caption, in one copy, called by both this heading and the announcement
   `DesktopApp.vue` hands to `set_title` — the `promoteTitle.js` shape, which
   the hazards list names as the one to follow, so the OS frame and the body of
   the window cannot come to say different things. */
const title = computed(() => DELETE_SESSION_TITLE)

/* What a person recognises the session by, and the reason it is above the three
   facts rather than among them: it is prose and they are identifiers, and this
   is the line somebody actually reads to check they are about to delete the
   conversation they meant. */
const opener = computed(() => props.session?.title ?? '')

const facts = computed(() => deleteSessionFacts(props.session))

const openerStyle = {
  font: 'var(--weight-medium) var(--text-md)/var(--leading-snug) var(--font-sans)',
  color: 'var(--text-primary)',
  textWrap: 'pretty'
}

/* The three facts, label over value. Not side by side: an absolute path under a
   440px dialog with a label column beside it would have about two hundred
   pixels left, and a path broken across four lines beside the word "Log path"
   is harder to check than one that has the width to itself. */
const factsStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  marginTop: 'var(--space-5)',
  paddingTop: 'var(--space-5)',
  borderTop: 'var(--border-w) solid var(--border-subtle)'
}
const labelStyle = {
  font: 'var(--weight-regular) var(--text-2xs)/1 var(--font-sans)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
/* Mono, all three of them: two are paths and the third is a measurement, and
   this system sets identifiers and numbers-about-things in `--font-mono`. */
const valueStyle = {
  marginTop: 'var(--space-2)',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-secondary)',
  overflowWrap: 'anywhere'
}
</script>

<template>
  <Modal
    :open="open"
    :closable="!busy"
    :title="title"
    :description="DELETE_SESSION_DESCRIPTION"
    @close="$emit('close')"
  >
    <div v-if="opener" :style="openerStyle">{{ opener }}</div>
    <div :style="factsStyle">
      <div v-for="fact in facts" :key="fact.label">
        <div :style="labelStyle">{{ fact.label }}</div>
        <div :style="valueStyle">{{ fact.value }}</div>
      </div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="danger" :disabled="busy" @click="$emit('confirm')">
        {{ busy ? 'Deleting…' : 'Delete' }}
      </Button>
    </template>
  </Modal>
</template>
