<script setup>
/* Editing `[defaults]` in the project's own `.smetana/project.toml`, which is
   the one part of a run configuration a person turns between runs rather than
   discovers by looking at the folder. Everything else in that file — the
   repositories, the gate lists, the preflight, the merge hazards — stays the
   setup agent's, and `SetupProjectModal.vue` beside this is the window that
   starts it.

   The rules are `projectDefaults.js`'s and not this file's: a `.vue` is the one
   thing no test in this repository can reach, so the bounds, the branch list
   and "has anything changed" live outside it.

   Explicit Cancel and Save, and deliberately not the settings window's
   save-as-you-type. That window writes the app's own file on this machine and
   can afford a keystroke to be a decision; this writes into a repository
   somebody else may be working in. */
import { computed, ref, watch } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Input from '../core/Input.vue'
import Select from '../core/Select.vue'
import { branchOptions, isDirty, validateDraft } from './projectDefaults.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  /* A draft in `projectDefaults.js`'s shape, already through `draftFrom`, so
     the four keys are always here whether or not the file carried them. */
  defaults: { type: Object, default: () => ({}) },
  /* `target_branches`' answer, `{name, missing_in}` apiece — the same list the
     run dialog's own branch field is filled from. */
  branches: { type: Array, default: () => [] },
  busy: { type: Boolean, default: false },
  /* The command's own refusal, shown rather than swallowed: "the file will not
     parse" is the one thing the person has to read to know what to do next. */
  error: { type: String, default: '' }
})

defineEmits(['close', 'save'])

/* A local copy, seeded when the window opens rather than bound to the prop.
   `RunModal.vue` does the same and for the same reason: props arrive again
   whenever the app window re-announces them — a `busy` flag, a branch list that
   has just landed — and a field rebuilt under somebody mid-edit loses what they
   typed. */
const draft = ref({ ...props.defaults })
watch(
  () => props.open,
  (open) => {
    if (open) draft.value = { ...props.defaults }
  },
  { immediate: true }
)

/* What a control hands back is a string, always, and the file's fields are
   whole numbers. An unparseable value is kept as it was typed rather than
   quietly turned into one: emptying a field to type another number must not put
   a number back under the cursor, and `validateDraft` refuses anything that is
   not whole and in range, which is what greys Save. */
const setNumber = (field, raw) => {
  const text = String(raw).trim()
  const value = text === '' ? text : Number(text)
  draft.value = { ...draft.value, [field]: Number.isFinite(value) ? value : text }
}

const errors = computed(() => validateDraft(draft.value))
const canSave = computed(
  () =>
    !props.busy &&
    isDirty(draft.value, props.defaults) &&
    Object.keys(errors.value).length === 0
)

/* Off the stored value rather than off the draft: what this keeps in the list
   is the branch the file names, and a branch nobody can pick any more must
   still be pickable back. */
const branchList = computed(() => branchOptions(props.branches, props.defaults.target_branch))

/* bd's priority scale is closed, so a select says so where a number field would
   not. The two ends are labelled because "higher priority is a lower number" is
   the one thing about this scale nobody guesses right. */
const PRIORITIES = [
  { value: '0', label: '0 — highest' },
  { value: '1', label: '1' },
  { value: '2', label: '2' },
  { value: '3', label: '3' },
  { value: '4', label: '4 — lowest' }
]

const body = { display: 'flex', flexDirection: 'column', gap: 'var(--space-6)' }
const introStyle = {
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)'
}
const row = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }
const labelStyle = {
  fontSize: 'var(--text-xs)',
  color: 'var(--text-secondary)',
  fontFamily: 'var(--font-sans)'
}
const pathStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
}
const errorStyle = {
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--status-failed-fg)'
}
</script>

<template>
  <Modal
    :open="open"
    :closable="!busy"
    title="Project settings"
    description="What a run in this project starts from."
    @close="$emit('close')"
  >
    <div :style="body">
      <!-- The file this edits, named where somebody can find the rest of the
           settings this form does not offer. An identifier, so mono. -->
      <div :style="introStyle">
        Stored in <span :style="pathStyle">.smetana/project.toml</span>. Everything else in that
        file is the setup agent's.
      </div>

      <div :style="row">
        <span :style="labelStyle">Target branch</span>
        <Select
          :model-value="draft.target_branch ?? ''"
          :options="branchList"
          :disabled="busy"
          @update:model-value="draft.target_branch = $event"
        />
      </div>

      <div :style="row">
        <span :style="labelStyle">Minimum priority</span>
        <Select
          :model-value="String(draft.min_priority ?? '')"
          :options="PRIORITIES"
          :disabled="busy"
          @update:model-value="setNumber('min_priority', $event)"
        />
        <span v-if="errors.min_priority" :style="errorStyle">{{ errors.min_priority }}</span>
      </div>

      <div :style="row">
        <span :style="labelStyle">Max parallel tasks</span>
        <Input
          type="number"
          :model-value="draft.max_parallel_tasks"
          :invalid="Boolean(errors.max_parallel_tasks)"
          :disabled="busy"
          @update:model-value="setNumber('max_parallel_tasks', $event)"
        />
        <span v-if="errors.max_parallel_tasks" :style="errorStyle">
          {{ errors.max_parallel_tasks }}
        </span>
      </div>

      <div :style="row">
        <span :style="labelStyle">Review passes</span>
        <Input
          type="number"
          :model-value="draft.review_passes"
          :invalid="Boolean(errors.review_passes)"
          :disabled="busy"
          @update:model-value="setNumber('review_passes', $event)"
        />
        <span v-if="errors.review_passes" :style="errorStyle">{{ errors.review_passes }}</span>
      </div>

      <span v-if="error" :style="errorStyle">{{ error }}</span>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!canSave" @click="$emit('save', draft)">
        {{ busy ? 'Saving…' : 'Save' }}
      </Button>
    </template>
  </Modal>
</template>
