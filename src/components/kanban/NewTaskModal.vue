<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Dropdown from '../core/Dropdown.vue'
import Textarea from '../core/Textarea.vue'
import AttachmentStrip from './AttachmentStrip.vue'
import { cascade, DEFAULT_STAGE, STAGES } from './taskStages.js'

const props = defineProps({
  open: { type: Boolean, default: false },
  busy: { type: Boolean, default: false },
  /* The column the dialog was opened from: it decides where the card lands. */
  status: { type: String, default: null },
  /* The images already attached, owned by the caller — a drop is a window
     event rather than this dialog's, so the list cannot live in here. */
  attachments: { type: Array, default: () => [] },
  /* True while something is being dragged over the window. */
  dragging: { type: Boolean, default: false },
  /* What attaching was refused with, if it was. */
  error: { type: String, default: '' }
})

const emit = defineEmits(['close', 'submit', 'attach', 'files', 'remove'])

/* The types and priorities are the ones bd understands, each behind an Auto
   that leaves the choice to the agent — which has read the text of the task,
   as nothing in this app has. Auto travels as null rather than as the word:
   a field that is either a value bd knows or nothing at all cannot reach Rust
   carrying a type bd would reject. */
const AUTO = { value: 'auto', label: 'Auto' }
const TYPES = [AUTO, 'task', 'bug', 'feature', 'chore', 'epic', 'decision']
const PRIORITIES = [
  AUTO,
  { value: '0', label: 'P0 · highest' },
  { value: '1', label: 'P1' },
  { value: '2', label: 'P2' },
  { value: '3', label: 'P3' },
  { value: '4', label: 'P4 · lowest' }
]

/* The three stages of the work before a task is filed — whether the agent
   talks it through, writes the design the discussion produced, and writes the
   implementation plan — all offering the same three positions. Auto leaves the
   judgement to the agent: nothing here has read the text, and guessing from
   the length of a title would be wrong in both directions.

   Which of them a person may touch, and what a disabled one shows, is
   `taskStages.js` — the rule lives outside this file because no test in this
   repository can reach a `.vue`. */

/* One field, not a title and a description: bd wants a title, but writing one
   is the agent's job — it is the only party here that has read what the person
   wrote, and the filing skill says how this project wants a title worded. */
const text = ref('')
const issueType = ref('auto')
const priority = ref('auto')
const brainstorm = ref(DEFAULT_STAGE)
/* What was last chosen for each, which is not what is drawn: under a parent
   that is not On the control shows the parent's own position instead. */
const spec = ref(DEFAULT_STAGE)
const plan = ref(DEFAULT_STAGE)

const stages = computed(() => cascade(brainstorm.value, spec.value, plan.value))

/* A child opens on Auto whenever its parent moves, rather than coming back
   carrying a choice made under a different parent: turning Brainstorming off
   and on again is a fresh decision about the spec, not a return to an old one.
   Resetting the spec cascades into the plan through the second watcher, and
   the plan is reset here as well so the chain never depends on the spec having
   happened to change. */
watch(brainstorm, () => {
  spec.value = DEFAULT_STAGE
  plan.value = DEFAULT_STAGE
})
watch(spec, () => {
  plan.value = DEFAULT_STAGE
})

const valid = computed(() => text.value.trim().length > 0)

const intro = computed(() =>
  props.status
    ? `An agent files it, in ${String(props.status).replace(/-/g, ' ')}.`
    : 'An agent files it.'
)

const submit = () => {
  if (!valid.value || props.busy) return
  emit('submit', {
    text: text.value.trim(),
    issue_type: issueType.value === 'auto' ? null : issueType.value,
    priority: priority.value === 'auto' ? null : Number(priority.value),
    /* Paths, not thumbnails: what the agent is handed, and what it has to
       write into the issue, is where the file is. */
    images: props.attachments.map((item) => item.path),
    /* What the screen says, not what the refs hold: a stage under a parent
       that is not On is settled by that parent, and sending the remembered
       choice instead would ask for a spec nobody can see asked for. */
    brainstorm: brainstorm.value,
    spec: stages.value.spec.value,
    plan: stages.value.plan.value
  })
}

/* Cmd+V, and it is the gesture the whole feature is for: the main case is a
   screenshot sitting on the clipboard.

   On document rather than on the dialog, because the paste target is whatever
   holds the caret — the textarea when someone is typing, the body when nobody
   is — and only the document sees both. It is registered while the dialog is
   open and taken off the moment it closes, so a paste into the editor behind
   it never reaches here. */
const onPaste = (event) => {
  const clipboard = event.clipboardData
  /* Two ways in, because WebKit uses both. A screenshot off the system
     clipboard lands in `files`, which is the main case; an image copied out of
     a web page sometimes arrives only through `items`, with `files` empty. The
     one route the spec says this feature is pointless without does not get to
     depend on which of the two the browser happened to take. */
  let files = [...(clipboard?.files ?? [])]
  if (!files.length) {
    files = [...(clipboard?.items ?? [])]
      .filter((item) => item.kind === 'file')
      .map((item) => item.getAsFile())
      .filter(Boolean)
  }
  if (!files.length) return
  /* Text pastes are left alone entirely: preventDefault here would swallow the
     ordinary Cmd+V into the field above. */
  event.preventDefault()
  emit('files', files)
}

/* The caret goes into the field the dialog is for. Imperatively and after a
   tick, the way `Dropdown` and `BranchSelect` do it, rather than through the
   `autofocus` attribute: the dialog is inserted long after the page loaded, and
   an autofocus candidate arriving then is one the document has already stopped
   collecting.
   `$el` because the child's single root is the textarea itself.

   It is also the half of Cmd+V that this dialog controls: a paste is delivered
   to whatever holds the caret, so opening with the caret nowhere would leave
   the images row's main gesture resting on the webview's goodwill. */
const taskField = ref(null)

watch(
  () => props.open,
  async (isOpen) => {
    if (isOpen) {
      document.addEventListener('paste', onPaste)
      await nextTick()
      /* preventScroll because focusing an element scrolls whatever contains it
         into view, and this dialog is not always the only thing on the page:
         in `?view=gallery` two of them stand open inside one long scrolling
         column. Nothing to gain from the scroll either way — the field is on
         screen already, the dialog having just opened over everything. */
      taskField.value?.$el?.focus({ preventScroll: true })
      return
    }
    document.removeEventListener('paste', onPaste)
    /* We do not clear in submit(): if the write fails, the user has to see their
       own text rather than an empty field — the reset follows the outcome, not the
       fact of submitting. The parent closes the dialog both on success and on
       cancel; on a failed write it stays open, so a reset on "open -> false" covers
       both cases that should clear the form and never the one that should not.
       The attachments belong to the parent and are cleared there, on the same
       event and for the same reason. */
    text.value = ''
    issueType.value = 'auto'
    priority.value = 'auto'
    brainstorm.value = DEFAULT_STAGE
    spec.value = DEFAULT_STAGE
    plan.value = DEFAULT_STAGE
  },
  { immediate: true }
)

/* A dialog unmounted while open would otherwise leave the listener behind on
   the document, and every paste in the app after it would reach a component
   nobody can see. */
onBeforeUnmount(() => document.removeEventListener('paste', onPaste))

const fields = { display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }
const row = { display: 'flex', gap: 'var(--space-4)' }
const label = {
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)',
  marginBottom: 'var(--space-3)'
}
const field = { flex: 1, minWidth: 0 }
/* The same label, on a row with the button instead of above a field, so the
   gap below it belongs to the row rather than to the label. */
const labelInline = { ...label, marginBottom: 'var(--space-0)' }

/* The images block: the button, the hint that names the other two gestures,
   and the thumbnails under them. The hint is the only place a person learns
   that pasting and dropping work at all — neither leaves a mark on the screen
   until it is used. */
const images = { display: 'flex', flexDirection: 'column', gap: 'var(--space-4)' }
const attachRow = { display: 'flex', alignItems: 'center', gap: 'var(--space-4)' }
const hint = computed(() => ({
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  /* While something is over the window the same line says so, rather than a
     second element appearing and pushing the dialog about under the pointer. */
  color: props.dragging ? 'var(--text-primary)' : 'var(--text-muted)'
}))
const errorStyle = {
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--status-failed-fg)'
}
</script>

<template>
  <Modal :open="open" :closable="!busy" title="New task" :description="intro" @close="$emit('close')">
    <div :style="fields">
      <div>
        <div :style="label">Task</div>
        <Textarea
          ref="taskField"
          v-model="text"
          :rows="5"
          placeholder="What needs doing, and anything the agent should know"
        />
      </div>
      <div :style="images">
        <div :style="attachRow">
          <div :style="labelInline">Images</div>
          <Button size="sm" icon="paperclip" :disabled="busy" @click="$emit('attach')">Attach</Button>
          <span :style="hint">{{ dragging ? 'Drop them anywhere' : 'or paste, or drop them on the window' }}</span>
        </div>
        <AttachmentStrip :items="attachments" :disabled="busy" @remove="$emit('remove', $event)" />
        <span v-if="error" :style="errorStyle">{{ error }}</span>
      </div>
      <div :style="row">
        <div :style="field">
          <div :style="label">Type</div>
          <Dropdown v-model="issueType" :options="TYPES" />
        </div>
        <div :style="field">
          <div :style="label">Priority</div>
          <Dropdown v-model="priority" :options="PRIORITIES" />
        </div>
        <div :style="field">
          <div :style="label">Brainstorming</div>
          <Dropdown v-model="brainstorm" :options="STAGES" />
        </div>
      </div>
      <!-- A second row rather than five fields across one: the modal's width
           divided five ways is too narrow to read, and the empty third cell
           keeps every field one column wide. Neither of these is on v-model,
           because what a person chose and what the control shows are different
           facts here — a disabled stage draws its parent's position. -->
      <div :style="row">
        <div :style="field">
          <div :style="label">Spec</div>
          <Dropdown
            :model-value="stages.spec.value"
            :options="STAGES"
            :disabled="!stages.spec.interactive"
            @update:model-value="spec = $event"
          />
        </div>
        <div :style="field">
          <div :style="label">Plan</div>
          <Dropdown
            :model-value="stages.plan.value"
            :options="STAGES"
            :disabled="!stages.plan.interactive"
            @update:model-value="plan = $event"
          />
        </div>
        <div :style="field" />
      </div>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!valid || busy" @click="submit">
        {{ busy ? 'Creating…' : 'Create' }}
      </Button>
    </template>
  </Modal>
</template>
