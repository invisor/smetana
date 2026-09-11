<script setup>
/* The agent's activity, one element and three moments —
   `markup-contract.md` section 6. The strip mounts the instant a message is
   sent, at `waiting`, and is replaced in place by `done` or `failed` when the
   turn closes; nothing else in the column moves when it does, which is the
   whole reason this used to be three things (a spinner somewhere, this
   component's own receipt line, an error row) and is now one.

   **This absorbed `TurnResult`'s old, narrower job** — the closing receipt,
   mono and quiet — rather than sitting beside it: `journal.js`'s fold now
   opens this element at `turn-start` and only ever closes the one already
   standing, so a second component for the receipt would draw a line nothing
   in the journal still asks for. The component keeps the file's old name
   because the receipt is still most of what it draws; what changed is that it
   now also draws the wait before one and the failure instead of one.

   `state` is `journal.js`'s own `waiting` / `done` / `failed`, the exact words
   `data-activity` takes — no translation, because there is nothing to
   translate: this and the fold agree on the vocabulary by construction, `row.
   state` bound straight through.

   **Waiting does not spin.** The mark beside it fades on `--dur-pulse` —
   `sm-prose.css` section 11 — never blinking and never turning; the ticking
   `<time>` is the strip's actual claim that something is still alive, and the
   mark is decoration under it rather than a second, competing signal.
   `prefers-reduced-motion` silences the fade globally (`tokens/motion.css`)
   and leaves the clock running, because the clock is not motion for its own
   sake — it is the one question this element answers.

   **The two clocks are deliberately not the same clock.** `done`'s `<time>`
   is `elapsed.js`'s `formatDuration`, the wire's own `ms` read to a tenth of a
   second — a number worth that precision once, after the fact. `waiting` and
   `failed` read `formatElapsedClock` instead, whole seconds spelled the way a
   clock somebody is watching move is spelled: `4s`, `2m 14s`. `waiting` is the
   only one of the three actually ticking — `startedAt` plus a one-second
   interval, torn down the moment `state` stops being `waiting` so nothing
   here keeps a timer alive under a turn that has already closed. `failed` has
   already stopped, so its `ms` — `journal.js`'s own `elapsedSince`, the gap
   between the `turn-start` that opened the turn and the `error` that closed
   it, since `EventKind::Error` carries no duration of its own — is read once
   and never ticks again.

   **Failed is the one place this strip takes a saturated colour**, because
   failed is a status rather than a mood: `--status-failed-fg`, from
   `sm-prose.css`, and the mark becomes a square there — the system's own
   silhouette for a stopped thing, in place of the pill everything alive
   draws.

   The `.sm-prose` wrapper is interim, for the reason every other conversation
   component under this heading carries the same one: the contract's own root
   is the journal `ConversationView.vue` draws, and that landing is
   smetana-e3mc's, not this task's. */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { formatDuration, formatElapsedClock } from './elapsed.js'

const props = defineProps({
  /* `waiting`, `done` or `failed` — `journal.js`'s own three words, and the
     exact ones `data-activity` takes. */
  state: { type: String, required: true },
  /* `waiting`'s own sentence — "Claude Code is thinking" and the like. Unused
     by the other two states. */
  label: { type: String, default: '' },
  /* `waiting`'s own clock face: the ISO stamp its `turn-start` carried, which
     is all a ticking clock can be built from — nothing pure can know "now". */
  startedAt: { type: String, default: null },
  /* `failed`'s own sentence — the worker's own words for what went wrong.
     Unused by the other two states. */
  text: { type: String, default: '' },
  tokensIn: { type: Number, default: 0 },
  tokensOut: { type: Number, default: 0 },
  /* Dollars, or `null` where the harness said nothing. `done` only. */
  costUsd: { type: Number, default: null },
  /* `done`'s own duration, in milliseconds off the wire. `failed`'s own is a
     different field below — that one has no wire duration to read, only the
     gap `journal.js` already measured between the two timestamps. */
  ms: { type: Number, default: 0 }
})

/* The one clock that ticks. A `ref` rather than a computed off `Date.now()`
   directly, because nothing calls a computed back on its own — something has
   to ask it to run again once a second, and this is that something. */
const now = ref(Date.now())
let timer = null

function stopTicking() {
  if (timer != null) {
    clearInterval(timer)
    timer = null
  }
}

function startTicking() {
  stopTicking()
  now.value = Date.now()
  timer = setInterval(() => {
    now.value = Date.now()
  }, 1000)
}

onMounted(() => {
  if (props.state === 'waiting') startTicking()
})
onBeforeUnmount(stopTicking)
watch(
  () => props.state,
  (state) => {
    if (state === 'waiting') startTicking()
    else stopTicking()
  }
)

const waitingElapsedMs = computed(() => {
  if (!props.startedAt) return 0
  return Math.max(0, now.value - Date.parse(props.startedAt))
})

/* `done`'s own line: what the turn spent, in the mono voice every number a
   person did not choose is drawn in here — moved verbatim off the old
   `TurnResult.vue`. A cost of `null` is omitted rather than drawn as `$0`:
   there is a difference between a turn that was free and a harness that did
   not say, and `$0.000` under a turn that ran for four seconds would be the
   panel inventing the first of them. */
const doneLine = computed(() => {
  const parts = [`${props.tokensIn} in`, `${props.tokensOut} out`]
  if (props.costUsd != null) parts.push(`$${props.costUsd.toFixed(3)}`)
  return parts.join(' · ')
})

const primaryText = computed(() => {
  if (props.state === 'waiting') return props.label
  if (props.state === 'failed') return props.text
  return doneLine.value
})

const timeText = computed(() => {
  if (props.state === 'waiting') return formatElapsedClock(waitingElapsedMs.value)
  if (props.state === 'failed') return formatElapsedClock(props.ms)
  return formatDuration(props.ms)
})
</script>

<template>
  <div class="sm-prose">
    <div :data-activity="state" :role="state === 'waiting' ? 'status' : undefined">
      <span v-if="state !== 'done'" data-mark></span>
      <span>{{ primaryText }}</span>
      <time>{{ timeText }}</time>
    </div>
  </div>
</template>
