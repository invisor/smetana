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
  /* **Git is working somewhere in this repository**, and not necessarily on
     this. It is the whole of what refuses the Discard button, because it is the
     whole of what the store refuses: `write()` in `stores/vcs.js` declines any
     call made while an operation is in flight, so a button left live under
     somebody's pull is a button that goes dead on the press with nothing said.
     It is the same fact that greys the menu row this window was opened from,
     and the two have to be one answer. */
  busy: { type: Boolean, default: false },
  /* **This discard**, in flight. Narrower than `busy` on purpose, and the split
     is the difference between refusing an act and taking away the way out: this
     is what dims Cancel, what makes Escape answer with a refusal, and what
     takes the cross off the frame — and none of the three may be spent on an
     operation this window is not about.

     A merge running under
     `WRITE_CEILING` has five minutes to finish, and there is no scrim here to
     stop somebody starting one — a discard dialog whose Cancel went grey for
     the length of it would be the one dialog in the app that can be held shut
     by something it has nothing to do with.

     While it is true, `busy` is true as well, so the Discard button is refused
     by the wider fact and reads its label off this one.

     **The third of those three is the desktop's rather than this page's, and
     nothing here draws it.** `closable` below reaches `overlays/Modal.vue`,
     which writes it into the `ref` `views/DialogWindow.vue` provides; that view
     carries the same `ref` out to `window::dialog_window_closable`, which dims
     the frame's button and refuses the close behind it. This comment claimed
     the cross before any of that existed and was untrue for a while
     (smetana-an3v), so it is worth saying where the claim is cashed: if that
     chain is ever cut, this sentence goes with it. */
  discarding: { type: Boolean, default: false }
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

/* `discarding` and not `busy`: the button says what **this** window is doing,
   and "Discarding…" over somebody else's merge would be a sentence about the
   wrong operation. Under that wider fact the button is simply refused, which is
   what a control with nothing to say says. */
const confirmLabel = computed(() => (props.discarding ? 'Discarding…' : 'Discard'))
</script>

<template>
  <!-- `closable` and Cancel follow `discarding`, the Discard button follows
       `busy`: the wider fact refuses the act, and only this window's own write
       may take away the way out of it. In a window of its own there is no cross
       here to govern — the frame's is the one `closable` reaches, by the road
       the prop's own comment names. -->
  <Modal
    :open="open"
    :closable="!discarding"
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
      <Button variant="ghost" :disabled="discarding" @click="$emit('close')">Cancel</Button>
      <Button v-if="!refusal" variant="danger" :disabled="busy" @click="$emit('confirm')">
        {{ confirmLabel }}
      </Button>
    </template>
  </Modal>
</template>
