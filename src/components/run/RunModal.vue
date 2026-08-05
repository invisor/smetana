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
import Icon from '../core/Icon.vue'
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
  /* The issue this one sits under, when it sits under one: `{ id, title,
     siblings }`, where `siblings` is how many other unfinished children it has.
     The board offers a run on every card, children included, so this dialog is
     where somebody finds out that the task in front of them is part of
     something larger — and it is the last cheap moment to find out.

     Deliberately not called an epic: bd's parent-child says nothing about the
     parent's type, and calling a `feature` with children an epic would be a
     plain untruth on screen. */
  partOf: { type: Object, default: null },
  /* What this dialog was left at last time in this project, or null. */
  remembered: { type: Object, default: null },
  /* False when the config declares no way to check a merged task. */
  liveCheckAvailable: { type: Boolean, default: true },
  /* What the worker refused with, if it did. */
  error: { type: String, default: '' },
  busy: { type: Boolean, default: false }
})

/* `rescope` re-aims this dialog at the epic without closing it: what has
   already been chosen in the fields below is still what somebody wants, and
   making them set it all again is a poor reward for taking the advice. */
const emit = defineEmits(['close', 'confirm', 'rescope'])

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

/* The scope can change under an open dialog — `rescope` is the one thing that
   does it — and solo is a single task's mode only. Left alone, the payload
   would go out as solo on an epic and Rust would refuse it, with the answer
   naming a field nobody could see was wrong. */
watch(soloAllowed, (allowed) => {
  if (!allowed && mode.value === 'solo') mode.value = 'auto'
})

const title = computed(() => {
  if (props.scope?.kind === 'task') return 'Run this task'
  /* Not "Run this epic": the scope takes an issue's children, and whether that
     issue is typed as one is bd's business and often nobody's. */
  if (props.scope?.kind === 'epic') return 'Run these tasks'
  return 'Run the queue'
})

/* "One other task of it is unfinished" reads as a fact about this run; a bare
   count next to a title reads as a badge. */
const siblings = computed(() => {
  const n = props.partOf?.siblings ?? 0
  return n === 1 ? 'One other task of it is unfinished' : `${n} other tasks of it are unfinished`
})

const description = computed(() =>
  props.scope?.id ? `${props.scope.id} — ${props.scope.title ?? ''}`.trim() : 'Everything ready on the board.'
)

/* Named out loud, because the dialog is the last place a wrong aim is
   cheap. Null means it has not been counted, and saying nothing is better
   than saying zero.

   Two sentences rather than one, because the count is not the priority's:
   nothing here has read anybody's priorities, and "12 ready tasks at or above
   P2" said it had. The count is what is in front of the run, the floor is what
   it will take out of it, and they are stated separately because that is what
   is actually known. */
const takes = computed(() => {
  if (props.count == null) return ''
  if (props.scope?.kind === 'task') return 'One task.'
  const n = props.count
  const what =
    props.scope?.kind === 'epic'
      ? `${n} task${n === 1 ? ' is' : 's are'} unfinished under it.`
      : `${n} task${n === 1 ? ' is' : 's are'} ready.`
  return `${what} It takes those at or above P${priority.value}.`
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

/* Advice, not a refusal — so it is drawn on a sunken surface rather than in a
   status colour, and it keeps its distance from the error line at the bottom.
   Nothing here stops the run: whether one task is worth taking on its own is
   a judgement about the epic, and the person is the only one holding it. */
const partOfStyle = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: 'var(--space-4)',
  padding: 'var(--space-4)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)'
}
const partOfTextStyle = {
  flex: 1,
  minWidth: 0,
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--text-secondary)'
}
const epicIdStyle = {
  font: 'var(--weight-medium) var(--text-xs)/1 var(--font-mono)',
  color: 'var(--text-primary)'
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
      <!-- Only while the scope is still the task. Taking the advice re-aims the
           dialog at the epic, and the note would then be describing something
           that has already happened — a caller that leaves `partOf` set through
           that is not made to be right about it. -->
      <div v-if="partOf && scope?.kind === 'task'" :style="partOfStyle">
        <Icon name="layers" :size="13" :style="{ color: 'var(--text-muted)', flex: 'none', marginTop: '1px' }" />
        <div :style="partOfTextStyle">
          Part of <span :style="epicIdStyle">{{ partOf.id }}</span> — {{ partOf.title }}.
          <template v-if="partOf.siblings > 0">
            {{ siblings }} — taking one on its own can leave the rest merged in half, and
            running them together goes in order.
          </template>
        </div>
        <Button v-if="partOf.siblings > 0" variant="secondary" size="sm" @click="$emit('rescope')">
          Run all of it
        </Button>
      </div>

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
