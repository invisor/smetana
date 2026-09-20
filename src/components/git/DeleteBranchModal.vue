<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import { failureTextStyle, failureTitleStyle } from './failureStyle.js'

/* Deleting a branch's second question, and only the second one now: an
   ordinary delete is asked and answered in the branch row's own menu
   (`components/git/branchMenu.js`, `BranchList.vue`) exactly as the file
   tree's own Delete already asked one there, and this window opens only once
   that plain `git branch -d` has already been refused because the branch holds
   commits of its own.

   `DeleteTaskModal.vue` is the shape this follows — the same `Modal`, the same
   two buttons, the consequence stated rather than apologised for. It does not
   ask twice any more; the row's own confirm is what bought this window down to
   the one question left, `Delete anyway`, which is `git branch -D`. A `force`
   checkbox offered up front was the version thrown away, and stays thrown
   away — it puts the dangerous option in front of somebody who does not yet
   know they need it, and most of the time they never will, which is exactly
   why the row's own plain delete is what they meet first and this window only
   what a plain delete could not do.

   ## Two states in one window

   A press of `Delete anyway` runs `git branch -D`, and git answers in one of
   two ways here — the outcome where a plain delete would have taken it never
   reaches this window at all, since the row's own confirm already carries it.

   It **takes it**, and the window closes.

   It **refuses for some other reason** — the branch is checked out in another
   worktree is the one that matters — and forcing again would fail in exactly
   the same way. So git's own words are drawn, in the same mono block the Git
   panel draws a refusal in, and the only way out is Cancel: a second
   `Delete anyway` there would be a button whose whole answer is the message
   already on screen.

   Which of the two it is, is decided in Rust (`vcs_delete_branch`) and arrives
   here already decided. This file draws it and works nothing out. */
const props = defineProps({
  open: { type: Boolean, default: false },
  /* The whole branch name, which is the heading and the subject of every
     sentence here. Held by the caller as a name rather than as a row: the panel
     can refresh under an open window, and a name is what git is given. */
  branch: { type: String, default: '' },
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
   reads what a press will do. `LOSING` is the only sentence in this app that
   says work will be lost, and it is the ordinary description here now — the
   window would not be open at all if the branch's commits were not the reason
   git already refused a plain delete. */
const LOSING =
  'This branch holds commits that are not in the branch this repository is on. Deleting it leaves nothing pointing at them, and there is no undo.'
const REFUSED = 'Git would not delete this branch, and forcing it would fail the same way.'

const description = computed(() => (props.refusal ? REFUSED : LOSING))

/* The branch's own name in the body, the way `DeleteTaskModal` puts the issue's
   title there: the heading is what a person checks they meant, and a name at
   the top of a frame is easy to read past. Mono, because it is an identifier. */
const branchStyle = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-mono)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
}

/* Only the box is this file's. The failed-red title and the pre-wrapped mono
   under it are `failureStyle.js`'s, shared with the two blocks `GitPanel.vue`
   draws: a person who has seen one of these has seen all of them, and the words
   in it are git's either way. */
const refusalStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  marginTop: 'var(--space-4)'
}

/* The label carries this press's whole meaning: "Delete anyway" is the only
   affordance saying that this is not the plain delete the row already tried. */
const confirmLabel = computed(() => (props.busy ? 'Deleting…' : 'Delete anyway'))
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
      <div :style="failureTitleStyle">Git did not delete the branch</div>
      <div :style="failureTextStyle">{{ refusal }}</div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <!-- `force` rides on the emit rather than being worked out by the app
           window, and it is always `true` now: the one question left in this
           window is the one that only `git branch -D` can answer. -->
      <Button
        v-if="!refusal"
        variant="danger"
        :disabled="busy"
        @click="$emit('confirm', { force: true })"
      >
        {{ confirmLabel }}
      </Button>
    </template>
  </Modal>
</template>
