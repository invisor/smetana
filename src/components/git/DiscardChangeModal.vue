<script setup>
import { computed } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import { failureTextStyle, failureTitleStyle } from './failureStyle.js'

/* Throwing away what one file has that the last commit does not, asked about
   first. Reached from the last row of a change's context menu, which is the
   only place it is offered.

   `DeleteTaskModal.vue` is the shape — the same `Modal`, the same two buttons,
   the consequence stated rather than apologised for — and `DeleteBranchModal.vue`
   beside it is where the refusal block comes from. What it does **not** borrow
   from that one is the second question: a refused delete has a way forward and
   a refused discard has none, so git's words here are the end of the window
   rather than the middle of it.

   ## The sentence is chosen by what git says the change is

   Three of them, and the split is by what the person loses rather than by how
   many words it takes to say so.

   A path the last commit does not have — untracked, or added and staged — is
   **deleted**, and nothing anywhere holds a copy: no commit, no reflog, no
   stash. That is the only sentence here that says a thing will stop existing.

   A path the commit has and the working tree does not is **restored**, which is
   the one row on this menu where the word "discard" undoes a deletion rather
   than making one. Saying "will be lost" over it would be describing the
   opposite of what is about to happen.

   Everything else — a modification, a rename, a copy, a type change, staged or
   not — loses the difference between the working tree and the commit, and there
   is no undo because this app has no stash and git keeps nothing for an
   uncommitted change.

   **The kind is drawn and never decided here.** It is `ChangeKind` through
   serde, exactly as `changeStatus.js` and `changeMenu.js` read it, and what git
   is actually run with is decided in Rust by asking git — `vcs_discard`'s own
   header says why a kind read off a `git status` minutes old may not be true by
   the time the button is pressed. So a sentence that turns out to have been
   about the wrong shape of change costs a word, never a wrong write. */
const props = defineProps({
  open: { type: Boolean, default: false },
  /* The change's path, relative to its repository exactly as `vcs_status`
     reports it — with the trailing slash an untracked directory record carries,
     since `vendor/` says a folder where `vendor` would read as a file. Held by
     the caller as a path rather than as the change, for `DeleteBranchModal`'s
     reason: the panel can refresh under an open window, and a path is what git
     is given. */
  path: { type: String, default: '' },
  /* What git says the change is — `modified`, `added`, `deleted`, `renamed`,
     `untracked`, and whatever else `ChangeKind` grows. An unknown one takes the
     third sentence, which is the honest thing to say about a change this
     component has never heard of: something is different from the commit and it
     is about to stop being. */
  kind: { type: String, default: '' },
  /* git's own words for a refusal. Drawn as it stands, and its presence is what
     takes the Discard button off the footer altogether — there is no forcing
     this and nothing else to try, so the only way out is Cancel. */
  refusal: { type: String, default: '' },
  /* git is working. Both buttons go dead rather than the window closing: the
     call can fail, and this is the window the answer belongs over. */
  busy: { type: Boolean, default: false }
})

defineEmits(['close', 'confirm'])

/* The same words the app window announces for the OS frame — see
   `openDiscardChange` in `DesktopApp.vue`, which has to say them too because
   nothing on the window's side of the wire knows what this dialog is called.

   The path is not in it, unlike the two branch windows' headings: it is in the
   sentence below and again in the body, and a third copy in a frame caption
   would be the one place it is most likely to be clipped. */
const TITLE = 'Discard changes?'

/* Which paths the commit does not have. `added` is in the list because a staged
   new file is exactly as unrecoverable as an untracked one — the index is not
   history, and nothing puts back a blob no commit points at. */
const NEVER_COMMITTED = ['untracked', 'added']

const description = computed(() => {
  if (NEVER_COMMITTED.includes(props.kind)) {
    return `${props.path} will be deleted. It was never committed, so nothing can bring it back.`
  }
  if (props.kind === 'deleted') return `${props.path} will be restored from the last commit.`
  return `Every change to ${props.path} since the last commit will be lost. There is no undo.`
})

/* The path's own line in the body, the way `DeleteBranchModal` puts the branch
   there and `DeleteTaskModal` the issue's title: the sentence above is what a
   person reads and this is what they check they meant. Mono, because it is an
   identifier, and `anywhere` because a path in a repository several folders
   deep is longer than 440 pixels of dialog. */
const pathStyle = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-snug) var(--font-mono)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
}

/* Only the box is this file's. The failed-red title and the pre-wrapped mono
   under it are `failureStyle.js`'s, shared with `DeleteBranchModal.vue` and the
   two blocks `GitPanel.vue` draws: a person who has seen one of these has seen
   all of them, and the words in it are git's either way. */
const refusalStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  marginTop: 'var(--space-4)'
}

const confirmLabel = computed(() => (props.busy ? 'Discarding…' : 'Discard'))
</script>

<template>
  <Modal
    :open="open"
    :closable="!busy"
    :title="TITLE"
    :description="description"
    @close="$emit('close')"
  >
    <div :style="pathStyle">{{ path }}</div>
    <!-- git's refusal, where there is one. It replaces the Discard button
         rather than sitting beside it, which is the whole of what this state
         says: the way out of here is Cancel. -->
    <div v-if="refusal" :style="refusalStyle">
      <div :style="failureTitleStyle">Git did not discard the changes</div>
      <div :style="failureTextStyle">{{ refusal }}</div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button v-if="!refusal" variant="danger" :disabled="busy" @click="$emit('confirm')">
        {{ confirmLabel }}
      </Button>
    </template>
  </Modal>
</template>
