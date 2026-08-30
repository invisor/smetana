<script setup>
/* The run's segment in the status footer: where it has got to, and the one
   control it has.

   A stopped run stays here until the project changes or another starts. The
   reason it stopped is the thing somebody came back to read, and the unhappy
   endings need different responses — a single word for all of them would send
   people to the wrong place. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import { TONE, endingDetail, stopReason } from './stopReason.js'

const props = defineProps({
  /* The whole Run from the worker, or null when nothing has been started. */
  run: { type: Object, default: null },
  busy: { type: Boolean, default: false }
})

defineEmits(['stop'])

const state = computed(() => props.run?.state ?? null)
const over = computed(() => state.value?.kind === 'stopped')
const paused = computed(() => state.value?.kind === 'paused')

/* What an ending says, in what colour, under what glyph: `stopReason.js`, pure
   and next door, because a table of which endings read as failures is worth a
   test and no test in this repository can reach a `.vue`. */
const reason = computed(() => stopReason(state.value?.reason?.kind))

/* The finished run and the ones that stopped short differ by silhouette, not
   only by colour — the rule the status palette keeps everywhere else in this
   system. A pause is a third silhouette for the same reason: it is neither
   working nor over, and the glyph is the fastest way to tell it from both. */
const glyph = computed(() => {
  /* No fallback here: `stopReason` answers with a glyph for every ending it
     knows and for every one it does not, so a default written at this call site
     would be a second copy of a decision that lives next door. */
  if (over.value) return reason.value.icon
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
  /* What the ending has to say for itself, and the branch only when it has
     nothing: "The agent is waiting for an answer" without the question sends
     somebody to the terminal to find out what for, and "Could not start"
     without the tool that was not found sends them nowhere at all. The order
     is `endingDetail`'s, next door and pinned by its own tests. */
  if (over.value) return endingDetail(state.value.reason, branch.value)
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
  /* An ending that names no tone of its own is drawn as the ordinary one: red
     is a claim about what happened, and an omission has not made it. */
  return reason.value.tone ?? TONE.quiet
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
    <!-- The label says what stopping actually does rather than "Stop the run":
         the wrapper `Tooltip` that used to say it is gone, since `IconButton`
         draws its own now and two would draw two panels. It is the accessible
         name as well as the hint, and both are better for the precision — the
         button does not stop the run where it stands, it stops it after the
         batch in flight. -->
    <IconButton
      v-if="!over"
      icon="square"
      :label="run.stopping ? 'Stopping after this batch' : 'Stop after this batch'"
      size="sm"
      :disabled="busy || run.stopping"
      @click="$emit('stop')"
    />
  </div>
</template>
