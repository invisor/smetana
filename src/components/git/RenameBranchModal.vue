<script setup>
/* Giving a branch another name, from the row somebody right-clicked. One field,
   filled with the name the branch has now and selected the moment the window
   opens: the common case is a typo in a name that is otherwise right, so the
   whole of it is taken over by the first keystroke and there is nothing to
   clear by hand. The old name is named in the sentence above the field too,
   because the field stops showing it as soon as somebody types.

   Presentational like everything else in this panel: it is handed the branch
   and the list to check the new name against, and emits what was asked for.
   What may be typed and when the button lives is `branchName.js`, pure and
   tested, of the `gitActions.js` family — `renameError` and `canRename` rather
   than the pair beside them, because a rename has to pass over the branch it is
   about: that name is in the list, and `branchNameError` alone would open this
   window shouting "A branch with this name already exists." about the very
   branch the sentence above names.

   The unchanged name is deliberately not an error and draws no red line under
   the field. Nothing is wrong with it — it is the name the branch has — so what
   it does is hold the button, exactly as an empty field does in the dialog that
   cuts a branch.

   **git still decides.** The rule here refuses what git documents it refuses, so
   the button goes dead on the character that broke the name rather than the
   window closing onto a red block in the panel behind it — but `git branch -m`
   runs for real and a refusal nobody predicted comes back in git's own words
   under `GitPanel`'s "Git did not rename the branch". The rule is deliberately
   allowed to be narrower than git; it is not allowed to be wider.

   **The branch the repository is standing on can be renamed from here**, unlike
   deleted: `git branch -m` renames the branch HEAD is on and HEAD travels with
   the ref, so nothing about the tick refuses this window or the menu row that
   opens it. There is no confirmation either, which is the other place this
   parts company with the delete: a rename is undone by the opposite rename and
   loses nothing on the way.

   The window closes the moment `Rename` is pressed rather than waiting for git.
   That is the same shape as every other write here: the panel behind it goes
   inert with the spinner on the row, and if git refuses, the panel says so
   where it says the rest. */
import { computed, nextTick, ref, watch } from 'vue'
import Button from '../core/Button.vue'
import Input from '../core/Input.vue'
import Modal from '../overlays/Modal.vue'
import { canRename, renameError } from './branchName.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  /* What this dialog is called, in one place rather than two — the OS frame's
     caption is set from the same announcement that fills these props
     (`views/DialogWindow.vue`), so a title hardcoded below would be silently
     overridden by the announced one arriving as a fall-through attribute. The
     default is for a caller with no opinion, which is the gallery. */
  title: { type: String, default: 'Rename branch' },
  /* The branch being renamed — the row the menu was opened on, whole name and
     not the leaf the row draws. */
  from: { type: String, default: null },
  /* Every local branch, and only so a name another one already holds can be
     refused before git is asked. Nothing here draws them, and the branch named
     by `from` is passed over — `branchName.js` carries why. */
  branches: { type: Array, default: () => [] },
  /* `{ allowed, reason }` from `gitActions.js`, read live: a run can start
     while this window is open, and renaming a branch is a write like any
     other. Its sentence is what the line under the field then says. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  busy: { type: Boolean, default: false }
})

const emit = defineEmits(['close', 'rename'])

const name = ref('')
const field = ref(null)

/* Filled on opening rather than cleared, which is the whole difference from the
   dialog that cuts a branch: this one is about a name that already exists, and
   an empty field would make somebody type a name they were only editing. Reset
   on the way in and not on the way out, for that dialog's reason — the value is
   read while the window closes, since the pick is emitted first. */
watch(
  () => props.open,
  async (open) => {
    if (!open) return
    name.value = props.from ?? ''
    await nextTick()
    /* Selected and not merely focused: the first keystroke replaces the whole
       name, and a caret parked at one end would mean clearing it by hand.
       `Input` focuses inside `select` — a selection nobody has focused is
       invisible and the next key lands elsewhere. */
    field.value?.select()
  },
  { immediate: true }
)

/* The branch being renamed is not one of the names that are taken. */
const error = computed(() => renameError(name.value, props.from, props.branches))

/* One line under the field, and what it says is whichever refusal is about the
   thing a person can do something about. The name comes first: a run holding
   the repository is worth saying, but not over the top of "you cannot put a
   space in that". The unchanged name says nothing at all — it is not a mistake,
   and the dead button is the whole of the answer. */
const hint = computed(() => {
  if (error.value) return error.value
  if (props.actions?.allowed === false) return props.actions?.reason ?? null
  return null
})

const ready = computed(() =>
  canRename({
    name: name.value,
    from: props.from,
    branches: props.branches,
    allowed: props.actions?.allowed !== false,
    busy: props.busy
  })
)

const submit = () => {
  if (!ready.value) return
  emit('rename', { from: props.from, to: name.value.trim() })
}

const body = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  fontSize: 'var(--text-sm)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)'
}
/* A branch name is an identifier wherever it is drawn, prose around it or not —
   the same mono the rows of the list are in. */
const branchStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
}
/* The refusal, in the panel's own quiet idiom rather than in red: nothing has
   failed here, and the button beside it is already saying no. */
const hintStyle = computed(() => ({
  fontSize: 'var(--text-xs)',
  color: error.value ? 'var(--status-failed-fg)' : 'var(--text-muted)'
}))
</script>

<template>
  <Modal :open="open" :title="title" :closable="!busy" @close="$emit('close')">
    <div :style="body">
      <!-- The name it has now, said in prose because the field stops showing it
           the moment somebody types over it. -->
      <div>Renaming <span :style="branchStyle">{{ from }}</span>.</div>
      <!-- Enter submits, which is the whole keyboard this dialog needs: the
           listener is on the component and hears the input's own keydown on the
           way up, since attributes fall through to the wrapper rather than to
           the field. -->
      <Input
        ref="field"
        v-model="name"
        mono
        placeholder="feature/name"
        :invalid="Boolean(error)"
        @keydown.enter="submit"
      />
      <!-- The box is held whether or not there is anything in it, so nothing
           below it steps up the moment a name goes wrong. -->
      <div :style="[hintStyle, { minHeight: 'calc(var(--text-xs) * var(--leading-normal))' }]">
        {{ hint }}
      </div>
    </div>
    <template #footer>
      <Button variant="ghost" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!ready" @click="submit">Rename</Button>
    </template>
  </Modal>
</template>
