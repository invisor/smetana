<script setup>
/* What a run is about to do, before it does it.

   The last point at which somebody can see they aimed at the wrong thing —
   hence the line at the bottom naming what will actually be taken. Everything
   here is a choice for this run only; what does not change from run to run
   lives in .smetana/project.toml and is never repeated on screen. */
import { computed, ref, watch } from 'vue'
import Modal from '../overlays/Modal.vue'
import Button from '../core/Button.vue'
import Dropdown from '../core/Dropdown.vue'
import BranchSelect from './BranchSelect.vue'
import Switch from '../core/Switch.vue'

const props = defineProps({
  open: { type: Boolean, default: false },
  /* { kind: 'queue' | 'task' | 'epic', id, title }. `queue` carries neither. */
  scope: { type: Object, default: () => ({ kind: 'queue' }) },
  /* How much is in front of it, for the line at the bottom. Null while it is
     still being counted, which reads as nothing rather than as zero. */
  count: { type: Number, default: null },
  branches: { type: Array, default: () => [] },
  /* The project's own defaults, from the config. */
  defaultBranch: { type: String, default: '' },
  defaultPriority: { type: Number, default: 2 },
  /* What this dialog was left at last time in this project, or null. */
  remembered: { type: Object, default: null },
  /* False when the config declares no way to check a merged task. */
  liveCheckAvailable: { type: Boolean, default: true },
  /* What the worker refused with, if it did. */
  error: { type: String, default: '' },
  busy: { type: Boolean, default: false }
})

const emit = defineEmits(['close', 'confirm'])

/* Solo means the agent does the work itself instead of delegating, which is
   coherent for one task and not for a queue or an epic. The same rule lives in
   RunSettings::validate on the Rust side, and it lives there rather than only
   here for the reason recorded next to it: a dialog gets rewritten. */
const soloAllowed = computed(() => props.scope?.kind === 'task')

const MODES = [
  { value: 'auto', label: 'On its own' },
  { value: 'supervised', label: 'With a lead' },
  { value: 'solo', label: 'Plain, one task' }
]
const modes = computed(() => (soloAllowed.value ? MODES : MODES.filter((m) => m.value !== 'solo')))

const PRIORITIES = [0, 1, 2, 3, 4].map((p) => ({ value: p, label: `P${p} and better` }))

const mode = ref('auto')
const branch = ref('')
const priority = ref(2)
const createBranch = ref(false)
const liveCheck = ref(true)
const fileFindings = ref(true)

/* Filled on opening rather than on mounting: the dialog is kept in the tree
   and reopened, and its fields have to be what this project remembers now, not
   what the last project remembered. */
watch(
  () => props.open,
  (open) => {
    if (!open) return
    const kept = props.remembered ?? {}
    /* A remembered branch that no longer exists is dropped in silence rather
       than shown as an option that would fail: the branch list is the truth
       here, and a stale name in settings is not worth a warning. */
    const wanted = [kept.targetBranch, props.defaultBranch].find(
      (name) => name && props.branches.includes(name)
    )
    branch.value = wanted ?? props.branches[0] ?? ''
    createBranch.value = false
    const keptMode = kept.mode ?? 'auto'
    mode.value = keptMode === 'solo' && !soloAllowed.value ? 'auto' : keptMode
    priority.value = kept.minPriority ?? props.defaultPriority
    liveCheck.value = props.liveCheckAvailable && (kept.liveCheck ?? true)
    fileFindings.value = kept.fileFindings ?? true
  },
  { immediate: true }
)

const title = computed(() => {
  if (props.scope?.kind === 'task') return 'Run this task'
  if (props.scope?.kind === 'epic') return 'Run this epic'
  return 'Run the queue'
})

const description = computed(() =>
  props.scope?.id ? `${props.scope.id} — ${props.scope.title ?? ''}`.trim() : 'Everything ready on the board.'
)

/* Named out loud, because the dialog is the last place a wrong aim is
   cheap. Null means it has not been counted, and saying nothing is better
   than saying zero. */
const takes = computed(() => {
  if (props.count == null) return ''
  if (props.scope?.kind === 'task') return 'One task.'
  const n = props.count
  const what = props.scope?.kind === 'epic' ? 'child' : 'ready task'
  return `${n} ${what}${n === 1 ? '' : 's'} at or above P${priority.value}.`
})

const confirm = () => {
  emit('confirm', {
    scope:
      props.scope?.kind === 'queue'
        ? { kind: 'queue' }
        : { kind: props.scope.kind, id: props.scope.id },
    mode: mode.value,
    target_branch: branch.value,
    create_target: createBranch.value,
    min_priority: priority.value,
    live_check: liveCheck.value,
    file_findings: fileFindings.value
  })
}

const body = { display: 'flex', flexDirection: 'column', gap: 'var(--space-6)' }
const row = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }
const labelStyle = {
  fontSize: 'var(--text-xs)',
  color: 'var(--text-secondary)',
  fontFamily: 'var(--font-sans)'
}
const noteStyle = {
  fontSize: 'var(--text-xs)',
  color: 'var(--text-muted)',
  lineHeight: 'var(--leading-normal)'
}
const takesStyle = {
  fontSize: 'var(--text-xs)',
  color: 'var(--text-secondary)',
  fontFamily: 'var(--font-mono)'
}
const errorStyle = {
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--status-failed-fg)'
}
</script>

<template>
  <Modal :open="open" :closable="!busy" :title="title" :description="description" @close="$emit('close')">
    <div :style="body">
      <div :style="row">
        <span :style="labelStyle">Merge into</span>
        <BranchSelect
          v-model="branch"
          v-model:create="createBranch"
          :branches="branches"
          :disabled="busy"
        />
      </div>

      <div :style="row">
        <span :style="labelStyle">How it works</span>
        <Dropdown v-model="mode" :options="modes" :disabled="busy" />
        <span :style="noteStyle">
          {{
            mode === 'auto'
              ? 'Nothing is asked. Anything it cannot resolve is parked with a reason.'
              : mode === 'supervised'
                ? 'It keeps going on its own and asks when something needs deciding.'
                : 'It does the work itself instead of delegating, and asks freely.'
          }}
        </span>
      </div>

      <div :style="row">
        <span :style="labelStyle">Take tasks</span>
        <Dropdown v-model="priority" :options="PRIORITIES" :disabled="busy" />
      </div>

      <div :style="row">
        <Switch
          v-model="liveCheck"
          :disabled="busy || !liveCheckAvailable"
          label="Check each task for real before closing it"
        />
        <span v-if="!liveCheckAvailable" :style="noteStyle">
          This project declares no way to check a merged task, so tasks close on a green merge.
        </span>
      </div>

      <div :style="row">
        <Switch v-model="fileFindings" :disabled="busy" label="File what it finds along the way" />
        <span :style="noteStyle">
          New tasks go to deferred and wait for you — a run never picks up its own findings.
        </span>
      </div>

      <span v-if="takes" :style="takesStyle">{{ takes }}</span>
      <span v-if="error" :style="errorStyle">{{ error }}</span>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="busy || !branch" @click="confirm">
        {{ busy ? 'Starting…' : 'Run' }}
      </Button>
    </template>
  </Modal>
</template>
