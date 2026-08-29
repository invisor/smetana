<script setup>
/* The two doors out of a conflicted working tree.

   It opens the moment a merge or a rebase answers `conflict`, and it is a
   dialog rather than a panel section because there is nothing else to be doing
   until it is answered: the repository is mid-operation, and every other write
   in the panel is refused while it stands.

   **There is no third door and no dismiss.** No close button (`closable` is
   false), and `overlays/Modal.vue` listens for neither Escape nor a press on
   the scrim, so nothing else has to be suppressed here. That is deliberate: a
   conflicted tree behind a closed dialog is a state this panel promises to show
   and cannot — nothing in the app draws a conflicted file, and the changes list
   would say `C` beside three rows with no way back to what caused it.

   The app cannot resolve a conflict either: there is no merge editor here and
   this epic does not add one. So the two doors are the only two things it can
   honestly offer — put the tree back exactly as it was, or hand the tree, as
   git left it, to an agent. Everything git wrote into those files stays where
   it is for the second one; the session is started by the caller, which is also
   what switches to the agents tab.

   Presentational like the rest of `components/git/`: handed the record and
   emitting which door was taken, so it renders in `?view=gallery` with no back
   end behind it. */
import { computed } from 'vue'
import Button from '../core/Button.vue'
import Modal from '../overlays/Modal.vue'
/* git's own stderr: machine output, shown exactly as git wrote it, because
   the person reading it knows git. The two lines were written out here and
   in `GitPanel.vue` alike, in the same words, which is what `failureStyle.js`
   now holds once. */
import { failureTextStyle, failureTitleStyle } from './failureStyle.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  /* `merge` or `rebase` — `vcs::model::OpKind` as it crosses the wire. It
     decides the whole of what this dialog says, because the two operations put
     the branches on opposite sides of the sentence. */
  op: { type: String, default: 'merge' },
  /* The repository's absolute path. Named even in a project of one, because
     the one that matters is a project of five and a dialog that named the
     repository only sometimes would be one nobody reads. */
  repo: { type: String, default: '' },
  /* The branch this repository was on, read before the operation: a rebase
     stopped on a conflict leaves HEAD detached, so afterwards there is no
     branch to ask about. Null where the list had not landed yet. */
  ours: { type: String, default: null },
  /* The branch being brought in, or the one being rebased onto. */
  theirs: { type: String, default: '' },
  /* Every path git left unmerged, repository-relative. All of them are drawn:
     the number is what somebody weighs the two doors with, and a list cut off
     at three would hide exactly the case where aborting is the sane answer. */
  files: { type: Array, default: () => [] },
  /* An abort is in flight. Both doors go inert: git is already working in this
     tree, and starting an agent in a tree being put back is the one press that
     could genuinely lose something. */
  busy: { type: Boolean, default: false },
  /* Git's refusal of the abort, `{ kind, message }`. Drawn inside the dialog
     because the dialog has no dismiss — a message put in the panel behind it
     would be one nobody could see. */
  error: { type: Object, default: null }
})

defineEmits(['resolve', 'abort'])

const title = computed(() =>
  props.op === 'rebase' ? 'Rebase stopped on a conflict' : 'Merge stopped on a conflict'
)

const bodyStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)'
}

/* Identifiers in mono, prose in sans — so a branch called `main` is told apart
   from the word main in the sentence around it. */
const idStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
}

/* The paths, capped and scrolling rather than pushing the buttons off the
   bottom of the window: a rebase across a rename can leave dozens. Every one
   of them is in there — what is capped is the height, not the list. */
const filesStyle = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  maxHeight: 'calc(var(--row-h) * 8)',
  overflow: 'auto',
  padding: 'var(--space-4)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)'
}
const fileStyle = {
  font: 'var(--weight-regular) var(--text-xs)/1.4 var(--font-mono)',
  color: 'var(--status-failed-fg)',
  overflowWrap: 'anywhere'
}

const count = computed(() =>
  props.files.length === 1 ? '1 file is conflicted' : `${props.files.length} files are conflicted`
)
</script>

<template>
  <Modal :open="open" :closable="false" :title="title" :width="480">
    <div :style="bodyStyle">
      <!-- Which operation, in the branches' own order: a merge brings `theirs`
           into `ours`, a rebase moves `ours` onto `theirs`, and a sentence that
           got that backwards would send an agent the wrong way round. The
           branch this repository was on can be unknown — a repository whose
           list had not landed — and the sentence says the operation either
           way rather than inventing a name for it. -->
      <div>
        <template v-if="op === 'rebase'">
          Git stopped rebasing
          <span v-if="ours" :style="idStyle">{{ ours }}</span>
          <template v-else>this branch</template>
          onto <span :style="idStyle">{{ theirs }}</span>.
        </template>
        <template v-else>
          Git stopped merging <span :style="idStyle">{{ theirs }}</span>
          <template v-if="ours">
            into <span :style="idStyle">{{ ours }}</span>
          </template>.
        </template>
        Nothing has been committed.
      </div>
      <!-- Which repository, said in prose so the path is not a line of mono
           floating on its own: a project of five repositories is the case this
           dialog has to be unambiguous in, and a project of one still reads. -->
      <div>In <span :style="idStyle">{{ repo }}</span>.</div>
      <div>{{ count }}:</div>
      <div :style="filesStyle">
        <div v-for="file in files" :key="file" :style="fileStyle">{{ file }}</div>
      </div>
      <!-- What each button does to the tree, because the difference is the
           whole decision and neither label has room for it. -->
      <div>
        An agent works in the tree exactly as git left it and finishes the
        {{ op }}. Aborting puts the repository back where it was.
      </div>
      <div v-if="error" :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }">
        <div :style="failureTitleStyle">Git did not abort</div>
        <div :style="failureTextStyle">{{ error.message }}</div>
      </div>
    </div>
    <template #footer>
      <Button variant="secondary" :disabled="busy" @click="$emit('abort')">
        {{ busy ? 'Aborting…' : 'Abort' }}
      </Button>
      <Button variant="primary" :disabled="busy" @click="$emit('resolve')">
        Resolve with an agent
      </Button>
    </template>
  </Modal>
</template>
