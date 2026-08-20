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
import Tooltip from '../core/Tooltip.vue'
import { needsCutting, pickBranch } from './branchChoice.js'

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
  /* `[defaults].max_parallel_tasks` from the project's config, which this field
     is filled from and which the choice then overrides for this one run — the
     file is not rewritten. */
  defaultParallel: { type: Number, default: 3 },
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
  /* Why the machine cannot carry out a browser live check — nothing installed
     to drive one, or something else already driving it — and '' when it can.
     One string rather than a flag and a label: the tooltip's words and "is it
     blocked" are the same fact, and two values could disagree.

     A different question from `liveCheckAvailable`, which is about what the
     project declares. Both can block the toggle and they never say the same
     thing, so the note under the switch stays exactly what it was and this one
     is a tooltip on the switch itself. The rule producing it is
     browserTools.js, where a test can reach it. */
  liveCheckBlocked: { type: String, default: '' },
  /* The parser's own complaint about .smetana/project.toml, or '' when the file
     reads. This dialog is where a damaged configuration is said out loud, and
     it is the only place: the board offers no other route, and the project row
     carries a mark and a menu, neither of which has the room to name a section
     — the menu offers the way out of the state, not a reading of it. The
     message is the parser's verbatim: it already points at the key and the
     line, which is more than a summary written here would say. */
  configError: { type: String, default: '' },
  /* What the worker refused with, if it did. */
  error: { type: String, default: '' },
  busy: { type: Boolean, default: false }
})

/* `rescope` re-aims this dialog at the epic without closing it: what has
   already been chosen in the fields below is still what somebody wants, and
   making them set it all again is a poor reward for taking the advice.

   There was a "Set up again" here as well, and it is gone: re-running the setup
   agent is an action on a project, and it now sits with the project's other two
   in the right-click menu on its row. A dialog about one run was never the
   place to keep the only route to it. */
const emit = defineEmits(['close', 'confirm', 'rescope'])

/* Nothing here can be answered while the configuration cannot be read: the
   defaults every field is filled from come out of that file, so the values on
   screen would be this component's own fallbacks presented as the project's.
   The fields are disabled rather than hidden — a dialog that emptied itself
   would leave the notice floating over nothing and say less, not more, about
   what is wrong. The parallel row below goes the other way under solo and is
   not drawn at all, and the difference is what each state is about: there the
   field has no meaning in that mode, while here every field still means what it
   says and only its value would be untrustworthy. */
const broken = computed(() => props.configError !== '')
const locked = computed(() => props.busy || broken.value)

/* Solo means the agent does the work itself instead of delegating, which is
   coherent for one task and not for a queue or an epic. The same rule lives in
   RunSettings::validate on the Rust side, and it lives there rather than only
   here for the reason recorded next to it: a dialog gets rewritten. */
const soloAllowed = computed(() => props.scope?.kind === 'task')

/* A priority floor is only a question where the run picks its own work. Where
   somebody pointed at a task or an epic the work is already named, and the
   floor could only take it away — which is what it did, silently: a P4 task run
   from its card under the default P2 floor stopped at once with "the queue is
   empty", about the task the person had just chosen. So the field is not shown,
   and the payload carries null rather than a number nobody reads —
   RunSettings::validate refuses one that comes anyway. */
const hasFloor = computed(() => props.scope?.kind === 'queue')

const MODES = [
  { value: 'auto', label: 'Autopilot' },
  { value: 'supervised', label: 'Crew' },
  { value: 'solo', label: 'Solo' }
]
const modes = computed(() => (soloAllowed.value ? MODES : MODES.filter((m) => m.value !== 'solo')))

const PRIORITIES = [0, 1, 2, 3, 4].map((p) => ({ value: p, label: `P${p} and better` }))

/* What stops a run going wider is the subscription and the machine, not this
   app, so the ceiling is a wide one — and it is a closed list rather than a
   number field because there is nothing here worth typing. The same range is
   in RunSettings::validate, which is what refuses one that arrives anyway. */
const PARALLEL_MAX = 8
const PARALLEL = Array.from({ length: PARALLEL_MAX }, (_, i) => ({
  value: i + 1,
  label: i === 0 ? 'One at a time' : `${i + 1} at once`
}))

const mode = ref('auto')
const branch = ref('')
const priority = ref(2)
const parallel = ref(3)
const createBranch = ref(false)
const liveCheck = ref(true)
const fileFindings = ref(true)

/* Whether the branch in the field is somebody's answer or this dialog's guess.
   Set in exactly one place — the handler under the control — which is why the
   control is not on `v-model` here: through `v-model` a fill and a choice are
   the same assignment, and nothing downstream could tell them apart. */
const branchChosen = ref(false)

/* Remembered, else the project's default, else the first in the list. The rule
   itself is in branchChoice.js, where a test can reach it. */
const fillBranch = () => {
  branch.value = pickBranch(props.branches, props.remembered?.targetBranch, props.defaultBranch)
  /* Derived, never forced. This said `false` and its comment said why —
     "everything this fills with is a branch that exists, `pickBranch` offers
     nothing else, so there is never anything to create" — and that stopped
     being true the moment a branch could exist in three repositories out of
     four. `pickBranch` will happily fill the field with a remembered
     `release/7` that two of them lack, and forcing the flag false then sends
     the run out telling the agent not to cut it: a `provisioning` STOP in the
     first batch, in the one mode where nobody was going to be asked anything.

     `branchChosen` is untouched by all this and still guards a late fill from
     overwriting a choice somebody has already made — see `chooseBranch`. */
  createBranch.value = branch.value !== '' && needsCutting(props.branches, branch.value)
}

/* Somebody's own answer, and the one thing the late fill below will not touch
   afterwards. `create` needs no handler of its own: the control only raises it
   while naming a branch, which is a choice by definition and arrives with a
   choice of `modelValue` in the same breath. */
const chooseBranch = (name) => {
  branch.value = name
  branchChosen.value = true
}

/* The list is often not there when the dialog opens: `openRun` shows the dialog
   and only then goes to disk for the branches. So the fill on opening can run
   against nothing, and this is what runs it again once they land. Without it the
   field opened on "Pick a branch" every time and Run stayed disabled until
   somebody picked one by hand, which left the remembered branch, the config
   default and the fall back to the most recent branch all dead at once
   (smetana-6gs, smetana-o8r).

   Only while nobody has chosen. An answer arriving late must never overwrite a
   person's own pick — that is what `branchChosen` is for, and why it cannot be
   set by anything but the control. */
watch(
  [() => props.branches, () => props.defaultBranch, () => props.remembered],
  () => {
    if (props.open && !branchChosen.value) fillBranch()
  }
)

/* Filled on opening rather than on mounting: the dialog is kept in the tree
   and reopened, and its fields have to be what this project remembers now, not
   what the last project remembered. */
watch(
  () => props.open,
  (open) => {
    if (!open) return
    const kept = props.remembered ?? {}
    /* A fresh dialog: whatever was picked by hand last time was for that run,
       and the three steps above decide this one. */
    branchChosen.value = false
    fillBranch()
    const keptMode = kept.mode ?? 'auto'
    mode.value = keptMode === 'solo' && !soloAllowed.value ? 'auto' : keptMode
    priority.value = kept.minPriority ?? props.defaultPriority
    /* Straight from the config every time, and never from what was chosen last
       run: the choice lives one run by design, and the config is the project's
       standing answer. Clamped because nothing constrains the file's number to
       this list — an out-of-range default would otherwise leave the field
       showing a value it does not have. */
    parallel.value = Math.min(Math.max(props.defaultParallel || 1, 1), PARALLEL_MAX)
    liveCheck.value = props.liveCheckAvailable && !props.liveCheckBlocked && (kept.liveCheck ?? true)
    fileFindings.value = kept.fileFindings ?? true
  },
  { immediate: true }
)

/* The machine's answer arrives after the dialog does, exactly as the branch list
   does and for the same reason — `openRun` shows the dialog and only then goes
   to disk. So the fill above runs against a blocking reason that is not there
   yet, and this is what switches the toggle off when one lands (smetana-6gs and
   smetana-o8r are the same defect one field over).

   No `branchChosen` here, and the difference from the branch field is the whole
   of why: a branch is a choice between things that all work, so a late answer
   must not overwrite a person's own pick. This is a statement that the thing
   cannot be done at all, and somebody having switched it on a moment earlier
   does not make a browser appear. It wins over a choice, every time. */
watch(
  () => props.liveCheckBlocked,
  (blocked) => {
    if (blocked) liveCheck.value = false
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

   The count only, and never the floor beside it: nothing here has read
   anybody's priorities, so "12 ready tasks at or above P2" would say it had —
   and the two scopes that have no floor at all were being told about one. What
   the queue will take out of the count is the field above, in its own words. */
const takes = computed(() => {
  if (props.count == null) return ''
  if (props.scope?.kind === 'task') return 'One task.'
  const n = props.count
  return props.scope?.kind === 'epic'
    ? `${n} task${n === 1 ? ' is' : 's are'} unfinished under it.`
    : `${n} task${n === 1 ? ' is' : 's are'} ready.`
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
    min_priority: hasFloor.value ? priority.value : null,
    /* Null in solo, where the lead delegates to nobody — the same shape and the
       same reason as the floor above, and RunSettings::validate refuses a
       number that comes anyway. */
    max_parallel_tasks: mode.value === 'solo' ? null : parallel.value,
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

/* The same sunken slab the `partOf` advice sits on, and deliberately so: the
   two are the dialog's two ways of saying something before the fields, and
   giving this one a status-coloured surface of its own would make the pair read
   as different kinds of thing. The colour it does spend is the glyph and the
   headline — a red panel behind a wall of parser output is the loud budget gone
   on a paragraph nobody can act on directly. */
const configErrorStyle = {
  display: 'flex',
  alignItems: 'flex-start',
  gap: 'var(--space-4)',
  padding: 'var(--space-4)',
  background: 'var(--surface-sunken)',
  border: 'var(--border-w) solid var(--border-subtle)',
  borderRadius: 'var(--radius-3)'
}
const configErrorHeadStyle = {
  fontSize: 'var(--text-xs)',
  lineHeight: 'var(--leading-normal)',
  color: 'var(--status-failed-fg)'
}
/* `pre-wrap` because the message is the parser's own layout — the caret line
   under the offending key means nothing once its spaces collapse. It scrolls
   rather than growing: a file with a deep error produces a long complaint, and
   the dialog must not push its own footer off the screen.

   The ceiling is six of this text's own lines, written as its font size times
   its line height rather than as a multiple of a spacing token. Spacing is the
   one scale density rewrites and this text is not on it — `--text-xs` and
   `--leading-normal` are the same in both — so a spacing-based ceiling shrank
   under compact while the parser's message stayed the size it was, and the
   sentence naming the field dropped out of view entirely. Six lines is what the
   ordinary complaint measures: a header, the caret pair, the offending line,
   and that sentence. */
const configErrorTextStyle = {
  marginTop: 'var(--space-3)',
  maxHeight: 'calc(var(--text-xs) * var(--leading-normal) * 6)',
  overflow: 'auto',
  whiteSpace: 'pre-wrap',
  font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-mono)',
  color: 'var(--text-secondary)'
}
const configErrorPathStyle = {
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
/* Written once and read by both branches of the toggle below. The two differ in
   nothing but the tooltip around one of them, and a label copied twice is a
   label that ends up saying two things. */
const LIVE_CHECK_LABEL = 'Check each task for real before closing it'

/* Three reasons, one control: the dialog is locked, the project declares no way
   to check anything, or this machine has nothing to drive a browser with. The
   last two are separate props because they are separate sentences — see
   `liveCheckBlocked`. */
const liveCheckOff = computed(
  () => locked.value || !props.liveCheckAvailable || props.liveCheckBlocked !== ''
)

const takesStyle = {
  fontSize: 'var(--text-xs)',
  color: 'var(--text-secondary)',
  fontFamily: 'var(--font-sans)'
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
      <!-- First, and above the epic advice: everything below it is a choice
           about a run that cannot start, and the advice about running an epic
           whole is beside the point until the file reads. -->
      <div v-if="broken" :style="configErrorStyle">
        <Icon
          name="triangle-alert"
          :size="13"
          :style="{ color: 'var(--status-failed-fg)', flex: 'none', marginTop: '1px' }"
        />
        <div :style="{ flex: 1, minWidth: 0 }">
          <div :style="configErrorHeadStyle">
            <span :style="configErrorPathStyle">.smetana/project.toml</span> cannot be read, so
            nothing can be run here until it is fixed.
          </div>
          <div :style="configErrorTextStyle">{{ configError }}</div>
        </div>
      </div>

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
        <!-- Not `v-model`: see `chooseBranch`. What the control emits is a
             person's answer, and it has to stay one. -->
        <BranchSelect
          :model-value="branch"
          v-model:create="createBranch"
          :branches="branches"
          :disabled="locked"
          @update:model-value="chooseBranch"
        />
      </div>

      <div :style="row">
        <span :style="labelStyle">How it works</span>
        <Dropdown v-model="mode" :options="modes" :disabled="locked" />
        <span :style="noteStyle">
          {{
            mode === 'auto'
              ? 'Nothing is asked. Anything it cannot resolve is parked with a reason.'
              : mode === 'supervised'
                ? 'It takes a single batch, asks when something needs deciding, and stops once the batch is merged.'
                : 'It does the work itself instead of delegating, and asks freely.'
          }}
        </span>
      </div>

      <!-- Kept where the mode is, because it is the mode that decides whether
           it means anything: solo does the work itself, so there is nobody to
           spawn. The whole row goes rather than being disabled, and that
           reverses what stood here before: the shift it costs was the argument
           for leaving a dead field on screen, and it lost, because a field that
           can do nothing still takes its line and then owes a sentence under it
           saying why it is there — dropping the field drops the sentence too.
           A dialog that rebuilds itself on the spot is nothing new here either:
           `rescope` takes the "Part of ..." block away and Solo out of the mode
           list without closing anything. -->
      <div v-if="mode !== 'solo'" :style="row">
        <span :style="labelStyle">How many at once</span>
        <Dropdown v-model="parallel" :options="PARALLEL" :disabled="locked" />
      </div>

      <!-- The queue's alone: see `hasFloor`. -->
      <div v-if="hasFloor" :style="row">
        <span :style="labelStyle">Take tasks</span>
        <Dropdown v-model="priority" :options="PRIORITIES" :disabled="locked" />
      </div>

      <div :style="row">
        <!-- The tooltip is on the switch itself rather than in a note beside it,
             because what it says is about this control and not about the run:
             the project's own reason already has the note below, and a second
             paragraph under the same switch would read as one explanation
             contradicting itself.

             `Switch` is a `<label>` with spans and a `role="switch"` — never a
             native disabled `<input>` — so a disabled one still raises the
             pointer events this needs. `alignSelf` keeps the hover on the
             control and its label: the wrapper is a flex item in this column and
             would otherwise stretch across the dialog, putting a tooltip on
             empty space. -->
        <Tooltip
          v-if="liveCheckBlocked"
          :label="liveCheckBlocked"
          :style="{ alignSelf: 'flex-start' }"
        >
          <Switch v-model="liveCheck" :disabled="liveCheckOff" :label="LIVE_CHECK_LABEL" />
        </Tooltip>
        <Switch v-else v-model="liveCheck" :disabled="liveCheckOff" :label="LIVE_CHECK_LABEL" />
        <span v-if="!liveCheckAvailable" :style="noteStyle">
          This project declares no way to check a merged task, so tasks close on a green merge.
        </span>
      </div>

      <div :style="row">
        <Switch v-model="fileFindings" :disabled="locked" label="File what it finds along the way" />
        <span :style="noteStyle">
          New tasks go to deferred and wait for you — a run never picks up its own findings.
        </span>
      </div>

      <!-- Not while the file is unreadable: the count is true, but "12 tasks are
           ready" under a notice saying nothing can be run reads as a promise the
           dialog is in no position to keep. -->
      <span v-if="takes && !broken" :style="takesStyle">{{ takes }}</span>
      <span v-if="error" :style="errorStyle">{{ error }}</span>
    </div>
    <template #footer>
      <Button variant="ghost" :disabled="busy" @click="$emit('close')">Cancel</Button>
      <Button variant="primary" :disabled="locked || !branch" @click="confirm">
        {{ busy ? 'Starting…' : 'Run' }}
      </Button>
    </template>
  </Modal>
</template>
