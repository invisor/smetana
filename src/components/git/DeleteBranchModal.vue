<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'

/* Deleting a branch, asked about first. Reached from a branch row's own menu,
   which is the only place it is offered.

   `DeleteTaskModal.vue` is the shape this follows — the same `Modal`, the same
   two buttons, the consequence stated rather than apologised for — and the one
   thing it does that no other confirm in this app does is **ask twice**. That
   is not a habit; it is the only way to offer `git branch -D` honestly.

   ## Three states in one window

   A press of Delete runs `git branch -d`, and git answers in one of three ways.

   It **takes it**, and the window closes: everything on the branch was already
   in the branch the repository is on, so nothing was lost and there is nothing
   left to say.

   It **refuses because the branch holds commits of its own**, and this window
   stays where it is and asks the harder question. The sentence changes to name
   what is about to be lost and the button becomes `Delete anyway`, which is
   `git branch -D`. That second press is a different act from the first and the
   label says so — a `force` checkbox offered up front was the version thrown
   away, because it puts the dangerous option in front of somebody who does not
   yet know they need it, and most of the time they never will.

   It **refuses for some other reason** — the branch is checked out in another
   worktree is the one that matters — and forcing would fail in exactly the same
   way. So git's own words are drawn, in the same mono block the Git panel draws
   a refusal in, and the only way out is Cancel: a `Delete anyway` there would be
   a button whose whole answer is the message already on screen.

   Which of the three it is, is decided in Rust by asking git a second question
   rather than by reading its first answer (`vcs_delete_branch`), and arrives
   here already decided. This file draws it and works nothing out. */
const props = defineProps({
  open: { type: Boolean, default: false },
  /* The whole branch name, which is the heading and the subject of every
     sentence here. Held by the caller as a name rather than as a row: the panel
     can refresh under an open window, and a name is what git is given. */
  branch: { type: String, default: '' },
  /* git declined the plain delete because this branch holds commits the current
     one does not. The second state, and the only one that offers a way
     forward. */
  notMerged: { type: Boolean, default: false },
  /* git's own words for a refusal `-D` will not fix. Drawn as it stands, and
     its presence is what takes the delete button off the footer altogether. */
  refusal: { type: String, default: '' },
  /* git is working. Both buttons go dead rather than the window closing: the
     call can fail, and this is the window the answer belongs over. */
  busy: { type: Boolean, default: false }
})

defineEmits(['close', 'confirm'])

/* The same words the app window announces for the OS frame — see
   `openDeleteBranch` in `DesktopApp.vue`, which has to say them too because
   nothing on the window's side of the wire knows what this dialog is called. */
const title = computed(() => `Delete ${props.branch}?`)

/* One sentence per state, and the description slot is the one place a person
   reads what a press will do. The first says what deleting a branch is and is
   not — the commits are not the branch, and a branch already merged is a label
   on history that stays exactly where it is. The second is the only sentence in
   this app that says work will be lost. */
const ASKING =
  'Git deletes the branch reference. The commits it points at stay in the repository as long as another branch or tag holds them.'
const LOSING =
  'This branch holds commits that are not in the branch this repository is on. Deleting it leaves nothing pointing at them, and there is no undo.'
const REFUSED = 'Git would not delete this branch, and forcing it would fail the same way.'

const description = computed(() => {
  if (props.refusal) return REFUSED
  return props.notMerged ? LOSING : ASKING
})

/* The branch's own name in the body, the way `DeleteTaskModal` puts the issue's
   title there: the heading is what a person checks they meant, and a name at
   the top of a frame is easy to read past. Mono, because it is an identifier. */
const branchStyle = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-mono)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
}

/* git's stderr, in the idiom the Git panel already draws one in — a failed-red
   title over a pre-wrapped mono block, `failureTitleStyle` and
   `failureTextStyle` in `GitPanel.vue`. The same pair rather than a third: a
   person who has seen one of these has seen all of them, and the words in it
   are git's either way. */
const refusalTitleStyle = {
  font: 'var(--weight-medium) var(--text-sm)/1 var(--font-sans)',
  color: 'var(--status-failed-fg)'
}
const refusalTextStyle = {
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-secondary)',
  whiteSpace: 'pre-wrap',
  overflowWrap: 'anywhere'
}
const refusalStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  marginTop: 'var(--space-4)'
}

/* The label carries the second press's whole meaning. "Delete anyway" is the
   only affordance saying that this one is not the one that was just refused. */
const confirmLabel = computed(() => {
  if (props.busy) return 'Deleting…'
  return props.notMerged ? 'Delete anyway' : 'Delete'
})
</script>

<template>
  <Modal
    :open="open"
    :closable="!busy"
    :title="title"
    :description="description"
    @close="$emit('close')"
  >
    <div :style="branchStyle">{{ branch }}</div>
    <!-- git's refusal, where there is one. It replaces the delete button rather
         than sitting beside it, which is the whole of what this state says: the
         way out of here is Cancel. -->
    <div v-if="refusal" :style="refusalStyle">
      <div :style="refusalTitleStyle">Git did not delete the branch</div>
      <div :style="refusalTextStyle">{{ refusal }}</div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <!-- `force` rides on the emit rather than being worked out by the app
           window, so the button a person pressed and the flag git is run with
           cannot come apart. -->
      <Button
        v-if="!refusal"
        variant="danger"
        :disabled="busy"
        @click="$emit('confirm', { force: notMerged })"
      >
        {{ confirmLabel }}
      </Button>
    </template>
  </Modal>
</template>
