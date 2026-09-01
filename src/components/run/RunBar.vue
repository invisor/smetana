<script setup>
/* The run's segment in the status footer: where it has got to, and the one
   control it has.

   A stopped run stays here until the project changes or another starts. The
   reason it stopped is the thing somebody came back to read, and the unhappy
   endings need different responses — a single word for all of them would send
   people to the wrong place.

   One segment per run, and a paused one may be drawn without its words. The
   subscription is one per machine, so the sentence about it is one per footer:
   `limitVoice.js` next door picks which run says it, and the rest keep the pause
   glyph and their own Stop button and say nothing. Not `v-if`-ed away, because
   that button belongs to that run — see the module's own header.

   The run that speaks also carries "Run anyway", which releases every run alive
   at that moment from its pause threshold until each of them ends. It is absent
   where the pause is a hold on a spent allowance (`state.spent`): the button
   would work and the session it let through would die at once. */
import { computed } from 'vue'
import Icon from '../core/Icon.vue'
import IconButton from '../core/IconButton.vue'
import { TONE, endingDetail, stopReason } from './stopReason.js'

const props = defineProps({
  /* The whole Run from the worker, or null when nothing has been started. */
  run: { type: Object, default: null },
  busy: { type: Boolean, default: false },
  /* Whether this segment is the one that writes the sentence about the
     subscription limit. Decided by `limitVoice.js` out of the whole list, which
     is knowledge no single segment has.

     `true` by default, so a bar drawn on its own — the gallery, a footer with
     one run in it — reads exactly as it always has. The default is the
     single-run case rather than a caller's obligation. */
  speaks: { type: Boolean, default: true }
})

defineEmits(['stop', 'release'])

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

/* The one state that may be drawn without words: a second and a third run
   paused on the same reading of the same subscription. The glyph stays, the
   Stop button stays, and the sentence goes to whoever `limitVoice.js` picked. */
const mute = computed(() => paused.value && !props.speaks)

/* Offered on a threshold and refused on a hold, which is `usage::held`'s answer
   riding in on the state — the two pauses are otherwise identical, both with a
   percentage and a reset in them. On the muted segments too: the release is one
   press for every run, so a second button beside a silent segment would be a
   second way to do the thing the first one already did. */
const releasable = computed(() => paused.value && props.speaks && !state.value?.spent)

const label = computed(() => {
  if (!state.value) return ''
  if (mute.value) return ''
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
  if (mute.value) return ''
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
    <!-- Dropped rather than left empty when there is nothing to say: the row's
         own `gap` sits on either side of it, so a blank span would space the
         glyph and the buttons of a muted segment twice as far apart as a
         speaking one. -->
    <span v-if="label" :style="{ whiteSpace: 'nowrap' }">{{ label }}</span>
    <span v-if="detail && !reason.bare" :style="detailStyle">{{ detail }}</span>
    <!-- The label says what stopping actually does rather than "Stop the run":
         the wrapper `Tooltip` that used to say it is gone, since `IconButton`
         draws its own now and two would draw two panels. It is the accessible
         name as well as the hint, and both are better for the precision — the
         button does not stop the run where it stands, it stops it after the
         batch in flight. -->
    <!-- Beside Stop and before it, in the reading order of the sentence it
         answers: the bar has just said the run is paused, and this is the other
         thing that can be done about it. `play` is the direct pair of the
         `pause` glyph already drawn at the head of the segment. -->
    <IconButton
      v-if="releasable"
      icon="play"
      label="Run anyway"
      size="sm"
      :disabled="busy"
      @click="$emit('release')"
    />
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
