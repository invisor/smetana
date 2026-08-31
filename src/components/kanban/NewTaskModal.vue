<script setup>
import { computed, nextTick, onBeforeUnmount, ref, watch } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Dropdown from '../core/Dropdown.vue'
import Textarea from '../core/Textarea.vue'
import Icon from '../core/Icon.vue'
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
  error: { type: String, default: '' },
  /* The issue this task is a follow-up to, or null for an ordinary filing:
     `{ id, title }`, read from the store by the caller rather than copied off
     the card that opened the menu — a card's copy may be a delta behind.

     The title is here and does not cross to Rust. The agent reads the issue
     itself; this is for the person, who may well have opened the menu on the
     wrong card and has no other way to find out. */
  parent: { type: Object, default: null },
  /* A draft this window is being reopened with, or null for an ordinary
     opening. The app window keeps one per project when a project switch closes
     this window, and hands it back when the person returns — `taskDraft.js`
     beside this file says what the record is and when one may come back.

     Read once, when the dialog opens: a live value would refill the fields
     under somebody's hands, since the app window announces its props again on
     every change. */
  draft: { type: Object, default: null },
  /* Whether the pictures that draft names are still being read back into the
     list above. The host owns both the list and this, for the same reason it
     owns `dragging`. See `reportDraft` for the one thing it decides. */
  restoringImages: { type: Boolean, default: false }
})

const emit = defineEmits(['close', 'submit', 'attach', 'files', 'remove', 'draft'])

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
   happened to change.

   Both stand down while a kept draft is being put back: seeding all three
   stages in one tick would otherwise fire both and throw two of the three
   away, which is the one trap in restoring a draft at all. */

/* Whether the fields are being filled from a kept draft rather than typed by
   somebody. Dropped a tick after the seeding, once the two watchers below have
   run and returned early. */
const seeding = ref(false)

watch(brainstorm, () => {
  if (seeding.value) return
  spec.value = DEFAULT_STAGE
  plan.value = DEFAULT_STAGE
})
watch(spec, () => {
  if (seeding.value) return
  plan.value = DEFAULT_STAGE
})

const valid = computed(() => text.value.trim().length > 0)

/* What the app window is told, so that a window closed by a project switch does
   not take the person's words with it. The same record `submit` sends, with the
   parent whole rather than as an id — the title is drawn here and the app window
   is the side that has it — and with the images as paths, never as bytes: the
   bytes would be megabytes of base64 through an IPC event on every keystroke,
   and two copies of every picture resident at once.

   Debounced, so a burst of typing is one message rather than one per keystroke.
   The cost of that is stated where it is paid: somebody who types a sentence and
   switches project inside a quarter of a second has genuinely kept nothing, and
   the app window's notice reads accordingly. */
const DRAFT_DEBOUNCE = 250
let draftTimer = null

const reportDraft = () => {
  clearTimeout(draftTimer)
  draftTimer = setTimeout(() => {
    /* A closing dialog resets its fields, and the report of that reset is not
       news about a draft — it is the window being cleared on the way out. */
    if (!props.open) return
    emit('draft', {
      text: text.value,
      issue_type: issueType.value === 'auto' ? null : issueType.value,
      priority: priority.value === 'auto' ? null : Number(priority.value),
      /* The kept list while the pictures are still arriving, and the real one
         after. Each restored picture is a round trip, so a report sent in the
         middle of that would hand back a draft with half its images — narrowing
         the record this window was rebuilt from, and losing those paths for good
         if the project changed a moment later. Once the host says it has
         finished, what the list holds is the truth, including a picture cleared
         from the Storage tab in between and one the person took out. */
      images: props.restoringImages
        ? (props.draft?.images ?? [])
        : props.attachments.map((item) => item.path),
      /* What the screen says, not what the refs hold — the same reading
         `submit` sends, so a restored dialog cannot come back promising a
         stage nobody asked for. */
      brainstorm: brainstorm.value,
      spec: stages.value.spec.value,
      plan: stages.value.plan.value,
      parent: props.parent ?? null
    })
  }, DRAFT_DEBOUNCE)
}

/* The images are watched by their paths rather than by the array: the store
   pushes onto the list it already handed over, so the array's identity never
   changes when a picture is added, and a watcher on it would never fire. */
watch(
  [
    text,
    issueType,
    priority,
    brainstorm,
    spec,
    plan,
    () => props.attachments.map((item) => item.path).join('\n'),
    /* The restore finishing is itself news: if every picture in it failed to
       come back, the list never changed and nothing above this line would fire,
       leaving the app window holding paths that no longer read. */
    () => props.restoringImages
  ],
  reportDraft
)

/* A kept draft put back into the fields, in one go and with the cascade above
   held off for the length of it. */
const seed = (draft) => {
  seeding.value = true
  text.value = draft.text ?? ''
  issueType.value = draft.issue_type ?? 'auto'
  priority.value = draft.priority == null ? 'auto' : String(draft.priority)
  brainstorm.value = draft.brainstorm ?? DEFAULT_STAGE
  spec.value = draft.spec ?? DEFAULT_STAGE
  plan.value = draft.plan ?? DEFAULT_STAGE
  nextTick(() => {
    seeding.value = false
  })
}

/* Under a parent the column is not the person's to know: a follow-up lands in
   Blocked or in Ready depending on what the parent is doing, so promising
   `in ready` would be a promise the dialog cannot keep. It names the parent
   instead, which is the more useful half anyway. */
const intro = computed(() => {
  if (props.parent) return `A follow-up to ${props.parent.id}. An agent files it.`
  return props.status
    ? `An agent files it, in ${String(props.status).replace(/-/g, ' ')}.`
    : 'An agent files it.'
})

const submit = () => {
  if (!valid.value || props.busy) return
  /* A window that is filing has nothing left to report. Without this a person
     who typed and pressed Create inside the debounce would have the pending
     report land *after* the submit — putting a draft of the task they just
     filed back into the app window's map, to reappear on the next switch. */
  clearTimeout(draftTimer)
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
    plan: stages.value.plan.value,
    /* The id only. The agent runs `bd show` for everything else, and the title
       drawn under the field never crosses to Rust. */
    parent: props.parent?.id ?? null
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
      /* Before the focus and before anything is awaited: the fields are what
         this window is, and a person who sees it empty for a frame and full the
         next has watched their own words arrive as if from somewhere else. */
      if (props.draft) seed(props.draft)
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
   nobody can see. The draft's timer goes the same way and for the same shape of
   reason: it would fire into a component that is no longer there. */
onBeforeUnmount(() => {
  document.removeEventListener('paste', onPaste)
  clearTimeout(draftTimer)
})

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

/* The note under the TASK field. Two lines: what the relationship is, and what
   follows from it. The first carries the id in mono, because it is an
   identifier and this system draws those in `--font-mono`; the title beside it
   is prose and is drawn as prose. */
const parentNote = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: 'var(--space-3)',
  marginTop: 'var(--space-3)',
  /* Said here rather than left to whatever the modal's body inherits: the glyph
     draws in `currentColor`, so the colour of the first line is the colour of
     the glyph, and the two have to be decided in one place. */
  color: 'var(--text-primary)'
}
/* The glyph belongs to the first line and not to the top of the block, so it is
   nudged down by the difference between its own 14px and the line box that
   `--text-xs` at `--leading-normal` makes. */
const parentGlyph = { marginTop: 'var(--space-1)' }
const parentLines = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  minWidth: 0
}
const parentHead = {
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-primary)',
  overflowWrap: 'anywhere'
}
const parentId = {
  font: 'var(--weight-medium) var(--text-xs)/var(--leading-normal) var(--font-mono)'
}
const parentWhy = {
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-muted)'
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
        <!-- Under the field rather than above it: the person came here to
             type, and the relationship is context for what they type, not a
             heading over it. It is not removable — Cancel and "+ New task" are
             two clicks and cost no state. -->
        <div v-if="parent" :style="parentNote">
          <Icon name="git-branch-plus" :size="14" :style="parentGlyph" />
          <div :style="parentLines">
            <span :style="parentHead">
              Blocked by <span :style="parentId">{{ parent.id }}</span> · {{ parent.title }}
            </span>
            <span :style="parentWhy">
              It waits in Blocked until that one is done, and merges where its work already is.
            </span>
          </div>
        </div>
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
