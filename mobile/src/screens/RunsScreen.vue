<script setup>
/* The runs of a project: the ones going now with the way to stop them, the one thing the phone
   starts, and the reports of the ones that are over.

   Starting is a choice of target branch and a press, and the press is confirmed in place with
   a sentence saying what it starts (./runStart.js has the rest of the settings, which are the
   ones the desktop's run window opens on). A run that is already going over the queue greys
   the start with the desktop's own reason. Stopping is the desktop's bar button: cooperative,
   after the batch in flight.

   `project` is the one a notification named, else the one in front. The run stores hold one
   project at a time, so it is loaded again whenever it changes. */
import { computed, ref, watch } from 'vue'
import Button from '@src/components/core/Button.vue'
import EmptyState from '@src/components/core/EmptyState.vue'
import Select from '@src/components/core/Select.vue'
import RunBar from '@src/components/run/RunBar.vue'
import { scopeBusyReason, scopeLabel } from '@src/components/run/runScopes.js'
import { runsState, startRun, stopRun } from '@src/stores/runs.js'
import { reportsState } from '@src/stores/reports.js'
import { gitState } from '@src/stores/git.js'
import { boardColumns } from '@src/stores/tracker.js'
import { settings } from '@src/stores/settings.js'
import { basename } from '@src/paths.js'
import { describeStart, runSettings, startBlock } from '../runStart.js'
import { reportRows } from '../reports.js'
import { loadRunsFor } from '../workspace.js'

const props = defineProps({ project: { type: String, default: null } })

const path = computed(() => props.project ?? settings.activeProject)

watch(
  path,
  (project) => {
    if (project) loadRunsFor(project).catch((err) => console.warn('[companion] runs not read:', err))
  },
  { immediate: true }
)

const live = computed(() => runsState.project === path.value)
const runs = computed(() => (live.value ? [...runsState.runs].reverse() : []))
const reports = computed(() => (reportsState.project === path.value ? reportRows(reportsState.rows) : []))

const block = computed(() => startBlock(runsState.config))
const busyReason = computed(() => scopeBusyReason({ kind: 'queue' }, runsState.runs))

/* The target branch: the person's pick, else the desktop's own rule (a pick of nothing). */
const picked = ref('')
watch(path, () => (picked.value = ''))
const remembered = computed(() => (path.value === settings.activeProject ? settings.project.runSettings : null))
const settingsToSend = computed(() =>
  runSettings({
    config: runsState.config,
    branches: gitState.branches,
    remembered: remembered.value,
    branch: picked.value
  })
)
const branchOptions = computed(() =>
  gitState.branches.map((b) => ({
    value: b.name,
    label: b.missing_in?.length ? `${b.name} (not in ${b.missing_in.join(', ')})` : b.name
  }))
)
/* How many tasks the queue would take, when the board on the page is this project's. */
const ready = computed(() =>
  path.value === settings.activeProject
    ? boardColumns.value.find((c) => c.status === 'ready')?.tasks.length
    : undefined
)

const confirming = ref(false)
const starting = ref(false)
const stopping = ref(null)
const failure = ref('')

async function start() {
  if (!settingsToSend.value || starting.value) return
  starting.value = true
  failure.value = ''
  try {
    await startRun(path.value, settingsToSend.value)
    confirming.value = false
  } catch (err) {
    failure.value = typeof err === 'string' ? err : err?.message || 'The run did not start.'
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
const actions = { display: 'flex', gap: 'var(--space-3)' }
const report = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  padding: 'var(--space-4) 0',
  borderBottom: 'var(--border-w) solid var(--border-subtle)'
}
const reportTitle = { font: 'var(--weight-medium) var(--text-sm)/var(--leading-snug) var(--font-sans)' }
const reportDate = {
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-muted)',
  fontVariantNumeric: 'tabular-nums'
}
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
        <h2 :style="heading">Start a run</h2>
        <p v-if="block" :style="text">{{ block }}</p>
        <template v-else>
          <p :style="text">Runs everything that is ready, with the settings the desktop's run window opens on.</p>
          <Select
            v-model="picked"
            :options="[{ value: '', label: 'Default branch' }, ...branchOptions]"
            :style="{ width: '100%' }"
          />
          <p v-if="!settingsToSend" :style="failed">No target branch could be found for this project.</p>
          <p v-else-if="busyReason" :style="text">{{ busyReason[0].toUpperCase() + busyReason.slice(1) }}.</p>
          <template v-else-if="confirming">
            <p :style="text">{{ describeStart(settingsToSend, ready) }}.</p>
            <div :style="actions">
              <Button variant="primary" :disabled="starting" @click="start">{{ starting ? 'Starting' : 'Start the run' }}</Button>
              <Button variant="ghost" :disabled="starting" @click="confirming = false">Cancel</Button>
            </div>
          </template>
          <Button v-else variant="secondary" icon="play" @click="confirming = true">Start a run</Button>
        </template>
      </section>

      <section :style="section">
        <h2 :style="heading">Reports</h2>
        <p v-if="!reports.length" :style="text">{{ reportsState.loading ? 'Reading the reports.' : 'No reports yet.' }}</p>
        <div v-for="item in reports" :key="item.key" :style="report">
          <span :style="reportTitle">{{ item.title }}</span>
          <span :style="reportDate">{{ item.date }} {{ item.time }}</span>
        </div>
      </section>
    </template>
  </div>
</template>
