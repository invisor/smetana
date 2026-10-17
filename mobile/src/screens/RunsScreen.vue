<script setup>
/* The runs of a project: the ones going now with the way to stop them, and the one thing the
   phone starts.

   Starting is the desktop's "Run the queue" window, folded away behind a button: pressing it
   fills the fields in again (from the project's configuration, its branches and what the desktop
   remembered - ./runStart.js has the rule), Cancel folds it and drops the edits, and Run sends
   the one `run_start`. A run that is already going over the queue greys Run with the desktop's
   own reason. Stopping is the desktop's bar button: cooperative, after the batch in flight.

   `project` is the one a notification named, else the one in front. The run stores hold one
   project at a time, so it is loaded again whenever it changes. */
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import Button from '@src/components/core/Button.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import Input from '@src/components/core/Input.vue'
import Select from '@src/components/core/Select.vue'
import Switch from '@src/components/core/Switch.vue'
import Textarea from '@src/components/core/Textarea.vue'
import RunBar from '@src/components/run/RunBar.vue'
import { scopeBusyReason, scopeLabel } from '@src/components/run/runScopes.js'
import { runsState, startRun, stopRun } from '@src/stores/runs.js'
import { gitState } from '@src/stores/git.js'
import { boardColumns } from '@src/stores/tracker.js'
import { settings } from '@src/stores/settings.js'
import { basename } from '@src/paths.js'
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
import { loadRunsFor, unpinRuns } from '../workspace.js'

const props = defineProps({ project: { type: String, default: null } })

const path = computed(() => props.project ?? settings.activeProject)

watch(
  path,
  (project) => {
    if (project) loadRunsFor(project).catch((err) => console.warn('[companion] runs not read:', err))
  },
  { immediate: true }
)

/* The run stores can only be trusted for the project they hold. If they drift from this
   screen's, read them again. */
watch(
  () => runsState.project,
  (held) => {
    if (path.value && held !== path.value) loadRunsFor(path.value).catch((err) => console.warn('[companion] runs not read:', err))
  }
)
onBeforeUnmount(unpinRuns)

const live = computed(() => runsState.project === path.value)
const runs = computed(() => (live.value ? [...runsState.runs].reverse() : []))

const block = computed(() => (live.value ? startBlock(runsState.config) : 'Reading the run configuration.'))
const busyReason = computed(() => (live.value ? scopeBusyReason({ kind: 'queue' }, runsState.runs) : ''))

/* The form: `null` while folded, else the values of its fields (see ./runStart.js). */
const form = ref(null)
watch(path, () => (form.value = null))
const remembered = computed(() => (path.value === settings.activeProject ? settings.project.runSettings : null))
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
const payload = computed(() => (form.value && live.value ? runPayload(form.value, sources()) : null))
const canPress = computed(() => Boolean(form.value) && canRun(form.value) && !busyReason.value && !starting.value)

/* How many tasks the queue would take, when the board on the page is this project's. */
const ready = computed(() =>
  path.value === settings.activeProject
    ? boardColumns.value.find((c) => c.status === 'ready')?.tasks.length
    : undefined
)

const starting = ref(false)
const stopping = ref(null)
const failure = ref('')
const startFailure = ref('')

function openIt() {
  startFailure.value = ''
  form.value = openForm(sources())
}

async function start() {
  if (!live.value || !payload.value || busyReason.value || starting.value) return
  starting.value = true
  startFailure.value = ''
  try {
    await startRun(path.value, payload.value)
    form.value = null
  } catch (err) {
    startFailure.value = typeof err === 'string' ? err : err?.message || 'The run did not start.'
  } finally {
    starting.value = false
  }
}

async function stop(run) {
  stopping.value = run.token
  failure.value = ''
  try {
    await stopRun(run.token)
  } catch (err) {
    failure.value = typeof err === 'string' ? err : err?.message || 'The run could not be stopped.'
  } finally {
    stopping.value = null
  }
}

const section = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-4)',
  padding: 'var(--space-5)',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const heading = {
  margin: 0,
  font: 'var(--weight-medium) var(--text-2xs)/1 var(--font-mono)',
  letterSpacing: 'var(--tracking-caps)',
  textTransform: 'uppercase',
  color: 'var(--text-muted)'
}
const text = {
  margin: 0,
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)',
  color: 'var(--text-secondary)'
}
const failed = { ...text, color: 'var(--status-failed-fg)' }
const card = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 'var(--space-4)',
  padding: 'var(--space-4)',
  background: 'var(--surface-raised)',
  border: 'var(--border-w) solid var(--border)',
  borderRadius: 'var(--radius-3)'
}
const scope = { font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)', color: 'var(--text-muted)' }
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
        <h2 :style="heading">{{ basename(path) }} - running now</h2>
        <p v-if="!runs.length" :style="text">No run is going.</p>
        <div v-for="run in runs" :key="run.token" :style="card">
          <div :style="{ display: 'flex', flexDirection: 'column', gap: 'var(--space-3)', minWidth: 0 }">
            <RunBar :run="run" :can-release="false" :busy="stopping === run.token" @stop="stop(run)" />
            <span :style="scope">{{ scopeLabel(run.settings?.scope) }}</span>
          </div>
        </div>
        <p v-if="failure" :style="failed">{{ failure }}</p>
      </section>

      <section :style="section">
        <h2 :style="heading">{{ form ? 'Run the queue' : 'Start a run' }}</h2>
        <p v-if="block" :style="text">{{ block }}</p>
        <Button v-else-if="!form" variant="secondary" icon="play" @click="openIt">Start a run</Button>
        <div v-else :style="body">
          <span :style="takesStyle">{{ QUEUE_DESCRIPTION }}</span>

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

          <span v-if="typeof ready === 'number'" :style="takesStyle">
            {{ ready === 1 ? 'One task is ready.' : `${ready} tasks are ready.` }}
          </span>
          <span v-if="busyReason" :style="takesStyle">{{ busyReason[0].toUpperCase() + busyReason.slice(1) }}.</span>
          <span v-if="startFailure" :style="errorStyle">{{ startFailure }}</span>

          <div :style="actions">
            <Button variant="ghost" :disabled="starting" @click="form = null">Cancel</Button>
            <Button variant="primary" :disabled="!canPress" @click="start">{{ starting ? 'Starting…' : 'Run' }}</Button>
          </div>
        </div>
      </section>
    </template>
  </div>
</template>
