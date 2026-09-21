<script setup>
/* Two files behind one Save, and the second one is new: `[defaults]` in the
   project's own `.smetana/project.toml`, the run configuration a person turns
   between runs rather than discovers by looking at the folder, and — below it
   — whether this project keeps its own agent table in `settings.json` rather
   than following the root one everybody else's project follows. Everything
   else in `project.toml` — the repositories, the gate lists, the preflight,
   the merge hazards — stays the setup agent's, and `SetupProjectModal.vue`
   beside this is the window that starts it. The two files disagree about
   *when* an edit is saved — `project.toml` is committed and travels to
   everybody working in the repository, `settings.json` is this machine's own
   and the settings window writes it on every keystroke — and this dialog
   settles that by having one button mean the same thing for both: nothing
   reaches either file until Save is pressed, `.claude/rules/settings.md`
   carries why the second file is edited here at all rather than in the
   settings window.

   **The fields go with an explicit Save**, and the rules behind both groups
   are pulled out of this file rather than kept in it: a `.vue` is the one
   thing no test in this repository can reach, so the bounds, the branch list,
   "has anything changed" and the sentence that stands in for the defaults
   fields live in `projectDefaults.js`, and the seeding, the equality check
   and the single Save's own rule live in `projectAgents.js` beside it. */
import { computed, ref, watch } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Input from '../core/Input.vue'
import Select from '../core/Select.vue'
import Switch from '../core/Switch.vue'
import SettingsGroup from '../settings/SettingsGroup.vue'
import SettingsRow from '../settings/SettingsRow.vue'
import AgentRoleRows from '../settings/AgentRoleRows.vue'
import {
  CONFIG_FILE,
  branchOptions,
  configNotice,
  isDirty,
  offersDefaults,
  validateDraft
} from './projectDefaults.js'
import {
  OWN_AGENTS_DESCRIPTION,
  OWN_AGENTS_LABEL,
  canSaveProject,
  sameAgentTable,
  seedFromGlobal
} from './projectAgents.js'

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
  error: { type: String, default: '' },
  /* The project's own stored block, whole, or `null` for "the root table,
     entirely" — `settings.project.agents`, unpacked no further than the app
     window's read of it. This dialog only ever opens on the active project
     (`.claude/rules/runs.md`'s own note on the menu item that opens it), which
     is what lets the window write `settings.project.agents` directly on Save
     rather than resolving a project of its own. */
  agents: { type: Object, default: null },
  /* The root's own three fields, read for one purpose only: seeding the
     switch's draft the moment somebody turns it on. Not `effectiveAgents` —
     that would seed a project's own block from itself the moment this dialog
     reopened one, which is not what turning the switch on means. */
  rootAgent: { type: String, default: 'claude' },
  rootModel: { type: String, default: '' },
  rootAgentRoles: {
    type: Object,
    default: () => ({
      tasks: { agent: '', model: '' },
      code: { agent: '', model: '' },
      runLead: { agent: '', model: '' },
      reviewBranch: { agent: '', model: '' }
    })
  }
})

const emit = defineEmits(['close', 'save'])

/* A local copy, seeded when the window opens rather than bound to the prop.
   `RunModal.vue` does the same and for the same reason: props arrive again
   whenever the app window re-announces them — a `busy` flag, a branch list that
   has just landed — and a field rebuilt under somebody mid-edit loses what they
   typed. */
const draft = ref({ ...props.defaults })
/* The Agents group's own draft, seeded the same way and on the same watch:
   `seedFromGlobal` rather than a bare spread, since a project's stored block
   may carry fewer than four roles and this copy has to be safe to edit
   without touching the prop underneath it. `null` stays `null` — a project
   with no block opens with the switch off. */
const agentsDraft = ref(props.agents ? seedFromGlobal(props.agents) : null)
watch(
  () => props.open,
  (open) => {
    if (!open) return
    draft.value = { ...props.defaults }
    agentsDraft.value = props.agents ? seedFromGlobal(props.agents) : null
  },
  { immediate: true }
)

/* Turning the switch on seeds from the *root* table — the same one the
   Agents tab of Settings edits — and turning it off drops the draft whole:
   the design's own rule is that the posted-then-abandoned seed is not kept
   around for next time, so a project's block is either the whole table or
   nothing at all. */
const toggleOwnAgents = (on) => {
  agentsDraft.value = on
    ? seedFromGlobal({ agent: props.rootAgent, model: props.rootModel, agentRoles: props.rootAgentRoles })
    : null
}

const onAgentRole = ({ role, pair }) => {
  if (!agentsDraft.value) return
  agentsDraft.value = role
    ? { ...agentsDraft.value, agentRoles: { ...agentsDraft.value.agentRoles, [role]: pair } }
    : { ...agentsDraft.value, agent: pair.agent, model: pair.model }
}

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
const defaultsDirty = computed(() => isDirty(draft.value, props.defaults))
const defaultsValid = computed(() => Object.keys(errors.value).length === 0)
const agentsDirty = computed(() => !sameAgentTable(agentsDraft.value, props.agents ?? null))

/* Whether there is a form at all. With no parsed file there is nothing to fill
   in and nothing to save on that half, so the four rows go together with one
   sentence in their place — greying each field in turn was the other answer
   and is more code for the same meaning, with four disabled controls saying
   nothing about why. The Agents group is unaffected: it is drawn and may be
   saved whatever state `project.toml` is in. */
const fields = computed(() => offersDefaults(props.configState))
const notice = computed(() => configNotice(props.configState))

/* One rule for one Save over two files — `projectAgents.js`'s own, so the
   dialog and its tests read the identical rule. */
const canSave = computed(() =>
  canSaveProject({
    busy: props.busy,
    fields: fields.value,
    defaultsDirty: defaultsDirty.value,
    defaultsValid: defaultsValid.value,
    agentsDirty: agentsDirty.value
  })
)
/* `defaults` on the payload is `null` when the file was never parsed or the
   draft never moved — the design's own rule for what "nothing to save" means
   on that half — and `agents` is the draft whole, `null` included, since the
   store's own field is what a `null` there means. */
const savePayload = computed(() => ({
  defaults: fields.value && defaultsDirty.value ? draft.value : null,
  agents: agentsDraft.value
}))

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
  <!-- The width is read only outside a dialog window — inside one `Modal`
       takes the whole frame, which is already the registry's number. It is
       here so that `?view=gallery` draws this dialog at the width it has in
       the app, and it has to agree with `project-settings` in
       `views/dialogRegistry.js`: 560, the settings window's own column,
       because `AgentRoleRows` below asks for a `38ch` control column that
       was verified at that width and not at `Modal`'s 440 default. -->
  <Modal
    :open="open"
    :closable="!busy"
    title="Project settings"
    description="What a run in this project starts from."
    :width="560"
    @close="$emit('close')"
  >
    <div :style="body">
      <!-- The file the four fields below are stored in, named where somebody
           can find the rest of the settings this form does not offer, and
           named as *theirs* rather than the dialog's — since the Agents group
           further down is a different file, `settings.json`, the one thing
           this dialog must not leave somebody unsure of is which switch
           writes to which. An identifier, so mono. -->
      <div v-if="fields" :style="introStyle">
        These four fields are stored in <span :style="pathStyle">{{ CONFIG_FILE }}</span>.
        Everything else in that file is the setup agent's.
      </div>
      <!-- And what stands in their place when there is no file to fill them
           from. Every word of it is `projectDefaults.js`'s, in two halves with
           the path between them, because the path is an identifier and is set
           in mono like every other path this app puts in front of somebody.
           Read narrowly — "nothing here to fill in", not "nothing here" — it
           is a sentence about that one file too, and the Agents group below
           is drawn and may be saved whatever state it names. -->
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

      <!-- The second file, and drawn whatever state the first one is in:
           which harness and models this project uses is a fact about this
           machine's `settings.json`, not about `project.toml`, so a broken or
           missing project file has no bearing on whether this group may be
           edited or saved. -->
      <SettingsGroup label="Agents">
        <SettingsRow :label="OWN_AGENTS_LABEL" :description="OWN_AGENTS_DESCRIPTION">
          <Switch
            :model-value="agentsDraft !== null"
            :disabled="busy"
            @update:model-value="toggleOwnAgents($event)"
          />
        </SettingsRow>
        <AgentRoleRows
          v-if="agentsDraft"
          :agent="agentsDraft.agent"
          :model="agentsDraft.model"
          :agent-roles="agentsDraft.agentRoles"
          :disabled="busy"
          @update:agent-role="onAgentRole($event)"
        />
      </SettingsGroup>
    </div>
    <template #footer>
      <!-- Always Cancel now: the Agents group above is drawn and may be
           edited whatever state `project.toml` is in, so there is always
           something on this screen a press of the ghost button could be
           undoing. -->
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="!canSave" @click="$emit('save', savePayload)">
        {{ busy ? 'Saving…' : 'Save' }}
      </Button>
    </template>
  </Modal>
</template>
