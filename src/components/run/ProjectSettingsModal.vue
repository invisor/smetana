<script setup>
/* `[defaults]` in the project's own `.smetana/project.toml` — the one part of a
   run configuration a person turns between runs rather than discovers by
   looking at the folder. Everything else in that file — the repositories, the
   gate lists, the preflight, the merge hazards — stays the setup agent's, and
   `SetupProjectModal.vue` beside this is the window that starts it.

   **The fields go with an explicit Save**, which the settings window's
   save-as-you-type deliberately is not: this file is committed and travels to
   everybody working in the repository, so a keystroke here has to be a
   decision, where the app's own file on this machine is the other case.

   The rules are `projectDefaults.js`'s and not this file's: a `.vue` is the one
   thing no test in this repository can reach, so the bounds, the branch list,
   "has anything changed" and the sentence that stands in for the fields all
   live outside it. */
import { computed, ref, watch } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Input from '../core/Input.vue'
import Select from '../core/Select.vue'
import {
  CONFIG_FILE,
  branchOptions,
  configNotice,
  isDirty,
  offersDefaults,
  validateDraft
} from './projectDefaults.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  /* A draft in `projectDefaults.js`'s shape, already through `draftFrom`, so
     the four keys are always here whether or not the file carried them. */
  defaults: { type: Object, default: () => ({}) },
  /* `target_branches`' answer, `{name, missing_in}` apiece — the same list the
     run dialog's own branch field is filled from. */
  branches: { type: Array, default: () => [] },
  /* `project_config`'s own word for what this project's file is: `ok`, `missing`
     or `broken`, kept whole the way `stores/runs.js` keeps it. It decides one
     thing here — whether the fields and Save are drawn at all, or one sentence
     in their place — and `projectDefaults.js` owns that decision.

     `ok` by default, which is what this component drew before it could be told
     anything else. It is inert in the app: `DialogWindow.vue` mounts a guest
     only once the app window has announced its props, so the state is always a
     real one by the time anything is drawn. */
  configState: { type: String, default: 'ok' },
  busy: { type: Boolean, default: false },
  /* The command's own refusal, shown rather than swallowed: "the file will not
     parse" is the one thing the person has to read to know what to do next. */
  error: { type: String, default: '' }
})

const emit = defineEmits(['close', 'save'])

/* A local copy, seeded when the window opens rather than bound to the prop.
   `RunModal.vue` does the same and for the same reason: props arrive again
   whenever the app window re-announces them — a `busy` flag, a branch list that
   has just landed — and a field rebuilt under somebody mid-edit loses what they
   typed. */
const draft = ref({ ...props.defaults })
watch(
  () => props.open,
  (open) => {
    if (!open) return
    draft.value = { ...props.defaults }
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

/* Whether there is a form at all. With no parsed file there is nothing to fill
   in and nothing to save, so the four rows and the Save button go together and
   one sentence takes their place — greying each field in turn was the other
   answer and is more code for the same meaning, with four disabled controls
   saying nothing about why. */
const fields = computed(() => offersDefaults(props.configState))
const notice = computed(() => configNotice(props.configState))

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
      <div v-if="fields" :style="introStyle">
        Stored in <span :style="pathStyle">{{ CONFIG_FILE }}</span>. Everything else in that
        file is the setup agent's.
      </div>
      <!-- And what stands in their place when there is no file to fill them
           from. Every word of it is `projectDefaults.js`'s, in two halves with
           the path between them, because the path is an identifier and is set
           in mono like every other path this app puts in front of somebody. -->
      <div v-else :style="introStyle">
        {{ notice.lead }} <span :style="pathStyle">{{ CONFIG_FILE }}</span> {{ notice.tail }}
      </div>

      <!-- The file's own four, drawn together or not at all: with no parsed
           file there is nothing to put in them, and four disabled controls say
           less than the one sentence above. -->
      <template v-if="fields">
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
      </template>

      <span v-if="error" :style="errorStyle">{{ error }}</span>
    </div>
    <template #footer>
      <!-- "Close" and not "Cancel" where there is nothing to cancel: with no
           parsed file there are no fields and nothing has been typed, and a
           button reading Cancel over one sentence promises to undo something. -->
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">
        {{ fields ? 'Cancel' : 'Close' }}
      </Button>
      <Button v-if="fields" variant="primary" :disabled="!canSave" @click="$emit('save', draft)">
        {{ busy ? 'Saving…' : 'Save' }}
      </Button>
    </template>
  </Modal>
</template>
