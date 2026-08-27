<script setup>
/* Cutting a branch from the row somebody right-clicked. One name, one switch,
   and the branch it starts at named in the sentence above them — never assumed,
   because this dialog is reached from any row in the list and the row it was
   reached from is the whole of what makes the answer different.

   Presentational like everything else in this panel: it is handed the branch to
   cut from and the list to check the name against, and emits what was asked
   for. What may be typed and when the button lives is `branchName.js`, pure and
   tested, of the `gitActions.js` family.

   **git still decides.** The rule here refuses what git documents it refuses, so
   the button goes dead on the character that broke it rather than the dialog
   closing onto a red block in the panel behind it — but the command runs for
   real and a refusal nobody predicted comes back in git's own words, where every
   other refusal in this panel is drawn. The rule is deliberately allowed to be
   narrower than git; it is not allowed to be wider.

   The dialog closes the moment `Create` is pressed rather than waiting for git.
   That is the same shape as every other write here: the panel behind it goes
   inert with the spinner on the row this branch is being cut from, and if git
   refuses, the panel says so where it says the rest. A dialog held open over a
   spinner would be a second place to report the same thing. */
import { computed, ref, watch, nextTick } from 'vue'
import Button from '../core/Button.vue'
import Checkbox from '../core/Checkbox.vue'
import Input from '../core/Input.vue'
import Modal from '../overlays/Modal.vue'
import { branchNameError, canCreate } from './branchName.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  /* What this dialog is called, in one place rather than two.

     It matters because this dialog is a window of its own: the OS frame's
     caption is set from the same announcement that fills these props
     (`views/DialogWindow.vue`), so a title hardcoded below would have been
     silently overridden by the announced one arriving as a fall-through
     attribute — agreeing today, and disagreeing the first time somebody renamed
     one of them, with the frame and this dialog's own `aria-label` then saying
     different things and nothing to say so.

     The default is for a caller that has no opinion, which is the gallery. */
  title: { type: String, default: 'New branch' },
  /* The branch the new one starts at — the row the menu was opened on. */
  from: { type: String, default: null },
  /* Every local branch, and only so a name already taken can be refused before
     git is asked. Nothing here draws them. */
  branches: { type: Array, default: () => [] },
  /* `{ allowed, reason }` from `gitActions.js`, read live: a run can start
     while this dialog is open, and creating a branch is a write like any
     other. Its sentence is what the line under the field then says. */
  actions: { type: Object, default: () => ({ allowed: true, reason: null }) },
  busy: { type: Boolean, default: false }
})

const emit = defineEmits(['close', 'create'])

const name = ref('')
/* On by default, which is what `git switch -c` does and what somebody cutting a
   branch to work on it wants. It is a switch rather than a settled decision
   because the other case is real — cutting a branch off `main` to keep a point
   in history, without leaving what you are doing — and it is exactly the case
   nobody would find if the dialog decided for them. Not remembered between
   openings: the default is the common answer, and a checkbox that quietly kept
   the rare one would be a dialog that behaves differently on a day you have
   forgotten why. */
const switchTo = ref(true)
const field = ref(null)

/* Reset on opening rather than on closing: the values are read while it closes
   — the pick is emitted first — and a dialog cleared on the way out would race
   its own answer. */
watch(
  () => props.open,
  async (open) => {
    if (!open) return
    name.value = ''
    switchTo.value = true
    await nextTick()
    field.value?.focus()
  },
  { immediate: true }
)

const error = computed(() => branchNameError(name.value, props.branches))

/* One line under the field, and what it says is whichever refusal is about the
   thing a person can do something about. The name comes first: a run holding
   the repository is worth saying, but not over the top of "you cannot put a
   space in that". */
const hint = computed(() => {
  if (error.value) return error.value
  if (props.actions?.allowed === false) return props.actions?.reason ?? null
  return null
})

const ready = computed(() =>
  canCreate({
    name: name.value,
    branches: props.branches,
    allowed: props.actions?.allowed !== false,
    busy: props.busy
  })
)

const submit = () => {
  if (!ready.value) return
  emit('create', { name: name.value.trim(), from: props.from, switch: switchTo.value })
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
      <!-- The start point, named. A detached HEAD reaches this dialog too — the
           row it came from is still a branch — so there is always something to
           name here. -->
      <div>Cut from <span :style="branchStyle">{{ from }}</span>.</div>
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
      <!-- The box is held whether or not there is anything in it, so the
           checkbox below does not step up the moment a name goes wrong. -->
      <div :style="[hintStyle, { minHeight: 'calc(var(--text-xs) * var(--leading-normal))' }]">
        {{ hint }}
      </div>
      <Checkbox
        :model-value="switchTo"
        label="Switch to it"
        @update:model-value="switchTo = $event"
      />
    </div>
    <template #footer>
      <Button variant="ghost" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!ready" @click="submit">Create</Button>
    </template>
  </Modal>
</template>
