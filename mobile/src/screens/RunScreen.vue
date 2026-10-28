<script setup>
/* The form that starts a run: the desktop's "Run the queue" window, open as soon as the screen is.

   It is opened from the Ready column's play (the whole queue) or from a card's sheet (`scope`,
   `{ kind: 'task' | 'epic', id }`, the desktop's `cardScope`). The fields are filled in from the
   project's configuration, its branches and what the desktop remembered (./runStart.js has the
   rule), Cancel drops the edits, and Run sends the one `run_start`. A run that is already going
   over the queue greys Run with the desktop's own reason. `started` is said once a run has
   begun, so the screen that opened this one can move on to the Agents list, where its row is.

   The project is the one in front. The run stores follow it through `loadProject`, so the only
   thing read here is its branches, which `loadProject` does not load. */
import { computed, ref, watch } from 'vue'
import Button from '@src/components/core/Button.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import Input from '@src/components/core/Input.vue'
import Select from '@src/components/core/Select.vue'
import Switch from '@src/components/core/Switch.vue'
import Textarea from '@src/components/core/Textarea.vue'
import { scopeBusyReason, scopeLabel } from '@src/components/run/runScopes.js'
import { runsState, startRun } from '@src/stores/runs.js'
import { gitState, loadBranches } from '@src/stores/git.js'
import { boardColumns } from '@src/stores/tracker.js'
import { settings } from '@src/stores/settings.js'
import { branchHint, branchOptions } from '@src/components/run/branchChoice.js'
import { NOTES_MAX } from '@src/components/run/runNotes.js'
import {
  FINDINGS_LABEL,
  FINDINGS_NOTE,
  LIVE_CHECK_LABEL,
  LIVE_CHECK_NONE_NOTE,
  MODES,
  MODE_NOTE,
  NOTES_NOTE,
  NOTES_PLACEHOLDER,
  PARALLEL,
  PAUSE_NOTE,
  PRIORITIES,
  QUEUE_DESCRIPTION,
  canRun,
  hasPause,
  liveCheckAvailable,
  openForm,
  pauseErrors,
  runPayload,
  startBlock
} from '../runStart.js'

const props = defineProps({
  /* What the run is about: `{ kind: 'queue' }` or a card's scope. */
  scope: { type: Object, required: true },
  /* The card's title, drawn beside the scope. */
  title: { type: String, default: '' }
})
const emit = defineEmits(['started', 'cancel'])

const path = computed(() => settings.activeProject)

watch(
  path,
  (project) => {
    if (project) loadBranches(project).catch((err) => console.warn('[companion] branches not read:', err))
  },
  { immediate: true }
)

const isQueue = computed(() => props.scope.kind === 'queue')

const live = computed(() => runsState.project === path.value)

const block = computed(() => (live.value ? startBlock(runsState.config) : 'Reading the run configuration.'))
const busyReason = computed(() => (live.value ? scopeBusyReason(props.scope, runsState.runs) : ''))

/* The form: its fields' values (see ./runStart.js), `null` until the configuration is read. */
const form = ref(null)
const remembered = computed(() => settings.project.runSettings)
const sources = () => ({ config: runsState.config, branches: gitState.branches, remembered: remembered.value })

/* A native select has no groups of the desktop's kind, so a partial branch carries its note in the row. */
const branchRows = computed(() =>
  branchOptions(gitState.branches)
    .filter((o) => !o.header)
    .map((o) => ({ value: o.value, label: o.note ? `${o.label} (${o.note})` : o.label }))
)
/* The branch list often lands after the form opens; a form still without a branch takes it then. */
watch(
  () => gitState.branches,
  () => {
    if (form.value && !form.value.branch) form.value.branch = openForm(sources()).branch
  }
)
const hint = computed(() => (form.value ? branchHint(gitState.branches, form.value.branch) : ''))
const errors = computed(() => (form.value ? pauseErrors(form.value) : {}))
const noLiveCheck = computed(() => !liveCheckAvailable(runsState.config))
const payload = computed(() =>
  form.value && live.value ? runPayload(form.value, sources(), isQueue.value ? undefined : props.scope) : null
)
const canPress = computed(() => Boolean(form.value) && canRun(form.value) && !busyReason.value && !starting.value)

/* How many tasks the queue would take. */
const ready = computed(() => boardColumns.value.find((c) => c.status === 'ready')?.tasks.length)

const starting = ref(false)
const startFailure = ref('')

/* The form opens as soon as the project's configuration is read, once, so that nothing the
   person typed is dropped when the configuration changes under them. */
watch(
  [live, block],
  () => {
    if (!form.value && live.value && !block.value) {
      startFailure.value = ''
      form.value = openForm(sources())
    }
  },
  { immediate: true }
)

async function start() {
  if (!live.value || !payload.value || busyReason.value || starting.value) return
  starting.value = true
  startFailure.value = ''
  try {
    await startRun(path.value, payload.value)
    emit('started')
  } catch (err) {
    startFailure.value = typeof err === 'string' ? err : err?.message || 'The run did not start.'
  } finally {
    starting.value = false
  }
}

const section = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  padding: 'var(--space-5)'
}
const text = {
  margin: 0,
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)'
}
const scopeStyle = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }
const cardTitle = { ...text, color: 'var(--text-primary)', overflowWrap: 'anywhere' }
const actions = { display: 'flex', gap: 'var(--space-3)', justifyContent: 'flex-end' }
const body = { display: 'flex', flexDirection: 'column', gap: 'var(--space-6)' }
const row = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }
const labelStyle = { fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', fontFamily: 'var(--font-sans)' }
const noteStyle = { fontSize: 'var(--text-xs)', color: 'var(--text-muted)', lineHeight: 'var(--leading-normal)' }
const errorStyle = { fontSize: 'var(--text-xs)', lineHeight: 'var(--leading-normal)', color: 'var(--status-failed-fg)' }
const pairStyle = { display: 'flex', gap: 'var(--space-4)' }
const pairCellStyle = { display: 'flex', flexDirection: 'column', gap: 'var(--space-2)', flex: 1, minWidth: 0 }
const takesStyle = { fontSize: 'var(--text-xs)', color: 'var(--text-secondary)', fontFamily: 'var(--font-sans)' }
const full = { width: '100%' }
</script>

<template>
  <div>
    <EmptyState v-if="!path" icon="folder" title="No project is open" description="Open a project on the desktop." />
    <template v-else>
      <section :style="section">
        <p v-if="block || !form" :style="text">{{ block || 'Reading the run configuration.' }}</p>
        <div v-else :style="body">
          <template v-if="!isQueue">
            <span :style="takesStyle">{{ scopeLabel(scope) }}</span>
            <span v-if="title" :style="cardTitle">{{ title }}</span>
          </template>
          <span v-else :style="takesStyle">{{ QUEUE_DESCRIPTION }}</span>

          <div :style="row">
            <span :style="labelStyle">Merge into</span>
            <Select v-model="form.branch" :options="branchRows" :disabled="starting" :style="full" />
            <span v-if="hint" :style="noteStyle">{{ hint }}</span>
          </div>

          <div :style="row">
            <span :style="labelStyle">How it works</span>
            <Select v-model="form.mode" :options="MODES" :disabled="starting" :style="full" />
            <span :style="noteStyle">{{ MODE_NOTE[form.mode] }}</span>
          </div>

          <div :style="row">
            <span :style="labelStyle">How many at once</span>
            <Select v-model="form.parallel" :options="PARALLEL" :disabled="starting" :style="full" />
          </div>

          <div v-if="hasPause(form)" :style="row">
            <span :style="labelStyle">Pause between batches, min</span>
            <div :style="pairStyle">
              <div :style="pairCellStyle">
                <span :style="noteStyle">From</span>
                <Input v-model="form.pauseMin" type="number" mono :invalid="Boolean(errors.min)" :disabled="starting" />
              </div>
              <div :style="pairCellStyle">
                <span :style="noteStyle">To</span>
                <Input v-model="form.pauseMax" type="number" mono :invalid="Boolean(errors.max)" :disabled="starting" />
              </div>
            </div>
            <span v-if="errors.min || errors.max" :style="errorStyle">{{ errors.min || errors.max }}</span>
            <span v-else :style="noteStyle">{{ PAUSE_NOTE }}</span>
          </div>

          <div :style="row">
            <span :style="labelStyle">Take tasks</span>
            <Select v-model="form.priority" :options="PRIORITIES" :disabled="starting" :style="full" />
          </div>

          <div :style="row">
            <Switch v-model="form.liveCheck" :disabled="starting || noLiveCheck" :label="LIVE_CHECK_LABEL" />
            <span v-if="noLiveCheck" :style="noteStyle">{{ LIVE_CHECK_NONE_NOTE }}</span>
          </div>

          <div :style="row">
            <Switch v-model="form.fileFindings" :disabled="starting" :label="FINDINGS_LABEL" />
            <span :style="noteStyle">{{ FINDINGS_NOTE }}</span>
          </div>

          <div :style="row">
            <span :style="labelStyle">Notes for the agent</span>
            <Textarea
              v-model="form.notes"
              :rows="3"
              :maxlength="NOTES_MAX"
              :disabled="starting"
              :placeholder="NOTES_PLACEHOLDER"
            />
            <span :style="noteStyle">{{ NOTES_NOTE }}</span>
          </div>

          <span v-if="isQueue && typeof ready === 'number'" :style="takesStyle">
            {{ ready === 1 ? 'One task is ready.' : `${ready} tasks are ready.` }}
          </span>
          <span v-if="busyReason" :style="takesStyle">{{ busyReason[0].toUpperCase() + busyReason.slice(1) }}.</span>
          <span v-if="startFailure" :style="errorStyle">{{ startFailure }}</span>

          <div :style="actions">
            <Button variant="ghost" :disabled="starting" @click="emit('cancel')">Cancel</Button>
            <Button variant="primary" :disabled="!canPress" @click="start">{{ starting ? 'Starting…' : 'Run' }}</Button>
          </div>
        </div>
      </section>
    </template>
  </div>
</template>
