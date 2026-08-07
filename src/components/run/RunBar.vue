<script setup>
/* The run's segment in the scope bar: where it has got to, and the one control
   it has.

   A stopped run stays here until the project changes or another starts. The
   reason it stopped is the thing somebody came back to read, and the unhappy
   endings need different responses — a single word for all of them would send
   people to the wrong place. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import Tooltip from '../core/Tooltip.vue'

const props = defineProps({
  /* The whole Run from the worker, or null when nothing has been started. */
  run: { type: Object, default: null },
  busy: { type: Boolean, default: false }
})

defineEmits(['stop'])

const state = computed(() => props.run?.state ?? null)
const over = computed(() => state.value?.kind === 'stopped')
const paused = computed(() => state.value?.kind === 'paused')

/* Loud only where a person has to do something. A run that finished its queue
   is the ordinary ending and gets the quiet treatment; one that stopped because
   nothing moved, or because the harness kept failing, is the reason this bar
   is worth a colour at all. */
const REASONS = {
  queue_empty: { text: 'Done — nothing left to take', loud: false, icon: 'check' },
  cancelled: { text: 'Stopped', loud: false },
  /* Quiet, like the stop button and for the same reason: a person did this on
     purpose and there is nothing here to fix. Loudness is not what it owes
     them — the sentence is. "Mid-batch" is the whole of it: a stop lets the
     batch in flight finish, while removing the session killed it where it
     stood, so there are worktrees left half-done for the next run's recovery
     phase to pick up. The person reading this line is deciding whether to go
     and look, and nothing else on screen will tell them to.

     `bare` keeps the branch suffix off this one line: "…was removed into
     staging" is a garden path, and the branch is the least of what somebody
     needs at that moment. */
  session_removed: {
    text: 'Stopped mid-batch — its agent session was removed',
    loud: false,
    bare: true
  },
  no_progress: { text: 'Stuck — a whole batch changed nothing', loud: true },
  max_iterations: { text: 'Stopped after too many batches', loud: true },
  unreadable: { text: 'Stopped — the tracker could not be read', loud: true },
  crashed: { text: 'Stopped — the agent kept failing', loud: true },
  preflight: { text: 'Could not start', loud: true }
}

const reason = computed(() => {
  const kind = state.value?.reason?.kind
  /* An unknown reason is an ordinary outcome, not a crash: this front end may
     be older than the worker. It says so plainly rather than drawing nothing. */
  return REASONS[kind] ?? { text: kind ? `Stopped — ${kind.replace(/_/g, ' ')}` : 'Stopped', loud: true }
})

/* The finished run and the ones that stopped short differ by silhouette, not
   only by colour — the rule the status palette keeps everywhere else in this
   system. A pause is a third silhouette for the same reason: it is neither
   working nor over, and the glyph is the fastest way to tell it from both. */
const glyph = computed(() => {
  if (over.value) return reason.value.icon ?? 'square'
  return paused.value ? 'pause' : 'play'
})

const label = computed(() => {
  if (!state.value) return ''
  switch (state.value.kind) {
    case 'preflight':
      return 'Bringing the project up'
    case 'deciding':
      return 'Reading the board'
    case 'working':
      return `Batch ${(props.run.batches ?? 0) || 1}`
    /* Named as the subscription's and not as an error: nothing failed, and
       nobody is being asked to do anything. The percentage is here rather than
       in the detail because it is the whole of what happened. */
    case 'paused':
      return `Paused — subscription limit reached (${state.value.pct}%)`
    default:
      return reason.value.text
  }
})

const branch = computed(() =>
  props.run?.settings?.target_branch ? `into ${props.run.settings.target_branch}` : ''
)

/* The whole point of a cooperative stop is visible here or nowhere: pressing
   stop does not end the batch in flight, and a bar that went on saying "Batch
   3" would read as the button having done nothing. */
const detail = computed(() => {
  if (over.value) return branch.value
  /* While paused the branch is the least of what somebody needs: they came to
     find out when this picks up again, and the harness's own sentence about the
     reset is the only thing that answers it. Without one, say that the run is
     still asking rather than leave the line bare — silence there reads as a
     hang, which is the very thing making the pause a state was meant to
     prevent. */
  if (paused.value) return state.value.resets ? `resets ${state.value.resets}` : 're-checking every 10 min'
  if (props.run?.stopping) return 'stopping after this batch'
  /* A batch running smaller than was asked for has nothing else on screen to
     explain it, and "why is it only doing two" is a question somebody would
     otherwise take to the tracker. Joined rather than interpolated: a project
     with no target branch would otherwise open the line on a bare separator. */
  if (props.run?.reduced != null) {
    return [branch.value, `fewer tasks, ${props.run.reduced}% used`].filter(Boolean).join(' · ')
  }
  return branch.value
})

const tone = computed(() => {
  if (!over.value) return 'var(--text-primary)'
  return reason.value.loud ? 'var(--status-failed-fg)' : 'var(--text-secondary)'
})

const style = computed(() => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  minWidth: 0,
  color: tone.value,
  font: 'var(--weight-regular) var(--text-xs)/1 var(--font-mono)'
}))

const detailStyle = {
  color: 'var(--text-muted)',
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap'
}
</script>

<template>
  <div v-if="run" :style="style">
    <Icon :name="glyph" :size="11" />
    <span :style="{ whiteSpace: 'nowrap' }">{{ label }}</span>
    <span v-if="detail && !reason.bare" :style="detailStyle">{{ detail }}</span>
    <Tooltip v-if="!over" :label="run.stopping ? 'Stopping after this batch' : 'Stop after this batch'" title="">
      <IconButton
        icon="square"
        label="Stop the run"
        size="sm"
        :disabled="busy || run.stopping"
        @click="$emit('stop')"
      />
    </Tooltip>
  </div>
</template>
