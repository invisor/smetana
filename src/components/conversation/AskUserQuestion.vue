<script setup>
/* `AskUserQuestion`'s own card, beside `PermissionRequest.vue` and never
   inside it: that one draws a tool's name and a one-line detail behind two
   buttons, and this tool has neither — up to four questions, each with a
   short header, its own prose and two to four options that carry their own
   description, some of them answerable with more than one choice at once.

   **This used to draw the same saturated `statusColors('needs-you')` fill
   `PermissionRequest.vue` still does, stretched over the whole card, and that
   was the defect this port fixes (smetana-ndm9).** One row and two buttons
   can carry a loud fill; two questions, up to eight options and two fields
   cannot — on a saturated ground nothing inside can be emphasised, so an
   option separated from the background by a border alone and "chosen"
   degraded to a slightly thicker one, the weakest possible signal for the
   most important state on the screen. A section of the design handoff written
   for exactly this card draws the fix. The handoff committed under
   `docs/design_handoff_conversation_panel/` (`README.md`, `markup-contract.md`,
   `reference.html`, `sm-prose-turns.css`) predates this feature and carries no
   section for it; this one reached the port outside the repository and is
   not committed anywhere, so its reasoning is written down in full in
   `.claude/rules/conversation-panel.md` rather than left as a path to
   follow: the container becomes an ordinary raised
   card, the same surface every other block in this panel sits on, and the
   whole loud budget moves to one chip in the header — colour, the system's
   triangle silhouette and the word, never colour alone. Every rule that
   paints it lives in `sm-prose.css` section 13, the same fourth styling
   exception the rest of this panel already spends (CLAUDE.md, Styling) —
   this file only emits the markup that section asks for and the state that
   decides which of it is drawn.

   **This card is not itself a row of the journal, and every selector in that
   section is written as a descendant of `.sm-prose`.** `ConversationView.vue`
   draws it in the panel's foot, under the composer, outside the scrolling
   viewport that carries the class — so the root here is its own `.sm-prose`
   wrapper around `section[data-ask]`, with that wrapper's own `padding` and
   `gap` neutralised through the computed style below: both are written for a
   column of turns and would double the inset `ConversationView.vue`'s own
   `questionPad` already spends around this card.

   The wire is `session::model::EventKind::Permission`'s `input` — the tool
   call's own arguments, untouched — and `askUserQuestion.js` beside this file
   holds the parsing and the answer-building rules, pure and tested there for
   the reason every file in this family is out of its component: a `.vue`
   file is the one thing no runner here can reach. This component's own state
   is which option is selected (by index — see `toggle`'s own header), what a
   person typed per question, and, new in this port, its own `state` —
   `pending` until a press, then `answered` or `declined` for good; it stops
   emitting after that, and the settled render is section 13's own, not a
   second component.

   **A custom answer always wins over a selection, for one question at a
   time.** Choosing an option clears whatever was typed for it and typing
   clears whatever was chosen — the wire takes one string per question, and a
   person is always free to answer in their own words rather than pick from
   the options the agent offered (the Design section of smetana-63kn is
   explicit about this). `formatAnswer` in `askUserQuestion.js` is the one
   place that resolves the two into that single string, so this file only has
   to keep them from being edited at the same time.

   **A real radiogroup gives up one thing a hand-drawn one had: clicking an
   already-chosen single-select option no longer deselects it.** `toggle` in
   `askUserQuestion.js` still supports that (both branches, tested there), but
   a native `<input type="radio">` never fires a change event for a click on
   the option already checked — that is true of every radiogroup on every
   platform, not a gap this file leaves open — so the component only ever
   calls `toggleOption` from a real `change`, and a single-select question can
   no longer be emptied by re-clicking its own answer. Typing a custom answer
   still clears it, which is the way out the contract actually asks for.
   `multiSelect`'s checkboxes keep the old behaviour: a `change` fires on
   every click regardless of the box's previous state. */
import { computed, onBeforeUnmount, onMounted, ref, reactive, useId, watch } from 'vue'
import { formatElapsedClock } from './elapsed.js'
import {
  ASK_USER_QUESTION_TOOL,
  buildAnswers,
  formatAnswer,
  isComplete,
  parseQuestions,
  selectedLabels,
  toggle
} from './askUserQuestion.js'

const props = defineProps({
  /* `Permission::input` off the wire — the raw arguments of the
     `AskUserQuestion` call, parsed by `askUserQuestion.js` rather than here. */
  input: { type: Object, default: () => ({}) },
  /* The permission event's own `at`, RFC 3339 — `session::model::Event`'s
     timestamp, threaded straight through by `ConversationView.vue`. Optional:
     a fixture built by hand (`Gallery.vue`'s settled columns) has no live
     event behind it and draws no clock at all rather than a fabricated one. */
  askedAt: { type: String, default: null }
})

/* `decision`, `'allow'` or `'deny'` (`session::model::Decision`'s wire
   words); `answers`, only for an allow, keyed by each question's own text —
   `undefined` for a decline, which the store already reads as no answers at
   all. Fired once: `state` moves out of `pending` in the same call and every
   later press is refused. */
const emit = defineEmits(['answer'])

/* Unique per instance, because option ids and radiogroup names are shared
   between the hidden `<input>` and its `<label for>` — the same reason
   `CommandPalette.vue` mints one, and the same failure mode without it: the
   gallery draws this card more than once on one page. */
const uid = `sm-ask-${useId()}`
const optionId = (qi, oi) => `${uid}-q${qi}-o${oi}`
const groupName = (qi) => `${uid}-q${qi}`

const questions = ref([])

/* Per question: the **indices** of the options currently chosen, and
   whatever a person typed instead. Indices rather than labels, because
   `parseQuestions` defaults a missing `label` to `''`, and two options that
   both lost theirs would otherwise be the same value as far as selection is
   concerned — picking one would toggle both. Rebuilt whenever the questions
   themselves change — a fresh `AskUserQuestion` call is a fresh form, never
   the last one's state showing through a new set of options — which in
   practice means every time, since `ConversationView.vue` keys this
   component on the permission's own id. Neither is cleared once the card
   settles: the settled render reads the very same selection back to draw
   which row was chosen. */
const selected = reactive([])
const custom = reactive([])

/* `pending` until a press, then `answered` or `declined` for good — section
   13's own three words, `data-state` on `section[data-ask]` unchanged. A
   fresh call resets it exactly as it resets the selection: a new question is
   never drawn settled from the state a previous one reached. */
const state = ref('pending')

function reset(parsed) {
  questions.value = parsed
  selected.length = 0
  custom.length = 0
  parsed.forEach(() => {
    selected.push([])
    custom.push('')
  })
  state.value = 'pending'
}
reset(parseQuestions(props.input))
watch(() => props.input, (next) => reset(parseQuestions(next)))

function isSelected(qi, oi) {
  return selected[qi]?.includes(oi) ?? false
}

/* A declined ask shows no chosen row at all, whatever was tentatively picked
   before the decline — refusing to answer discards the draft, the same
   reading `reference.html`'s own settle script takes. An answered one shows
   exactly what `selected`/`custom` still hold, unchanged since the press.

   This is also what the template's `:checked` binds to, rather than
   `isSelected` directly: the hidden input stays in the markup once settled
   (disabled, never removed), and a declined ask must not go on reporting a
   tentative pick as checked through that branch while `label[data-chosen]`
   says otherwise on the other — one source of truth for both. */
function isChosen(qi, oi) {
  return state.value !== 'declined' && isSelected(qi, oi)
}

/* The multiSelect/single-select/deselect rule itself is `toggle` in
   `askUserQuestion.js`, pure and tested there — this is one of this task's
   own acceptance criteria, and a `.vue` file is the one thing no runner here
   can reach. It is generic over what identifies an option, and it is handed
   an index here rather than a label for `isSelected`'s own reason. What is
   left here is plain state plumbing: which question, and the mutual
   exclusion with the typed answer. */
function toggleOption(qi, oi) {
  selected[qi] = toggle(selected[qi], oi, questions.value[qi].multiSelect)
  // A selection and a typed answer are mutually exclusive for one question —
  // see this file's own header for why picking an option clears the field.
  custom[qi] = ''
}

/* The one rule left in this component rather than in `askUserQuestion.js`:
   typing an answer clears whatever was selected for the same question — the
   mutual exclusion `toggleOption` above keeps the other way round. It stays
   here because it is exactly this short; `selectedLabels`, imported above,
   moved out for the opposite reason, carrying an edge case worth a test.

   This used to be the freeform field's own `@input` handler, called with
   `$event.target.value` — a `:value`/`@input` pair, which is not `v-model`
   and carries none of its guarantees. Vue's own `vModelText` ignores an
   `input` event while `el.composing` is true and re-syncs once
   `compositionend` fires, and a handler that writes `custom[qi]` straight
   from every `input` event has no such guard: an IME mid-composition — this
   app ships twelve languages — could see its own pre-edit buffer cleared or
   reordered by a write landing between keystrokes the composition has not
   settled yet. The template now binds the field with a plain `v-model` on
   `custom[qi]` directly, which is a valid assignment target on a `reactive`
   array and gets the same compiled guard any other text input in this tree
   would; this watcher is only the side effect v-model does not carry —
   clearing a question's selection the moment its typed answer becomes
   non-empty, checked for every question rather than tracked by index, since
   a `deep` watch on the whole array does not say which of them changed. */
watch(custom, () => {
  questions.value.forEach((_, qi) => {
    if (custom[qi] && selected[qi]?.length) selected[qi] = []
  })
}, { deep: true })

const complete = ref(false)
watch([questions, selected, custom], () => {
  complete.value = isComplete(questions.value, selectedLabels(questions.value, selected), custom)
}, { immediate: true, deep: true })

/* The footer's own hint — how many of the questions already have something
   to send. Built off the same pure functions `complete` is, not a second
   rule: a question counts once `formatAnswer` gives it a non-empty string,
   exactly what gates `Send answer` for the whole card. */
const answeredCount = computed(() => {
  const labels = selectedLabels(questions.value, selected)
  return questions.value.filter((_, i) => formatAnswer(labels[i], custom[i]).length > 0).length
})

function send() {
  if (state.value !== 'pending') return
  emit('answer', 'allow', buildAnswers(questions.value, selectedLabels(questions.value, selected), custom))
  state.value = 'answered'
}

function decline() {
  if (state.value !== 'pending') return
  emit('answer', 'deny')
  state.value = 'declined'
}

/* The header's own clock — ticking only while the ask is still `pending`,
   the same "still going" idiom `TurnResult.vue` uses and for the identical
   reason: the elapsed time is the liveness signal, not decoration, so it
   stops rather than freezing the instant a press settles the card. Nothing
   here fabricates a start time — no `askedAt` draws no `<time>` at all,
   which is the gallery's own settled columns (hand-written markup, no live
   event behind them). */
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
  timer = setInterval(() => { now.value = Date.now() }, 1000)
}

onMounted(() => {
  if (props.askedAt && state.value === 'pending') startTicking()
})
onBeforeUnmount(stopTicking)
watch(state, (next) => {
  if (props.askedAt && next === 'pending') startTicking()
  else stopTicking()
})

const elapsedLabel = computed(() => {
  if (!props.askedAt) return ''
  return formatElapsedClock(Math.max(0, now.value - Date.parse(props.askedAt)))
})

/* `needs you` while pending; the state's own word once settled — the exact
   text `reference.html`'s own settle script writes, kept for the reason
   everything else in this port matches the handoff rather than "fixing" it. */
const markLabel = computed(() => (state.value === 'pending' ? 'needs you' : state.value))

/* The neutralising style this file's own header explains: `.sm-prose`'s own
   `padding`/`gap` are written for a column of turns, and this card is not
   one — `ConversationView.vue`'s `questionPad` already insets it, and the
   journal's own turn gap has nothing here to space against, since this
   wrapper holds exactly one child. */
const root = { padding: 0, gap: 0 }
</script>

<template>
  <div class="sm-prose" :style="root">
    <section data-ask :data-state="state">
      <header>
        <span data-mark>{{ markLabel }}</span>
        <span data-tool :data-count="questions.length > 1 ? questions.length : undefined">{{ ASK_USER_QUESTION_TOOL }}</span>
        <time v-if="elapsedLabel">{{ elapsedLabel }}</time>
      </header>

      <div v-for="(question, qi) in questions" :key="qi" data-question>
        <h6 v-if="question.header">{{ question.header }}</h6>
        <p>{{ question.question }}</p>

        <ul
          data-options
          :role="question.multiSelect ? 'group' : 'radiogroup'"
          :aria-label="question.header || question.question"
        >
          <li v-for="(option, oi) in question.options" :key="oi">
            <input
              v-if="question.multiSelect"
              type="checkbox"
              :id="optionId(qi, oi)"
              :checked="isChosen(qi, oi)"
              :disabled="state !== 'pending'"
              @change="toggleOption(qi, oi)"
            >
            <input
              v-else
              type="radio"
              :id="optionId(qi, oi)"
              :name="groupName(qi)"
              :checked="isChosen(qi, oi)"
              :disabled="state !== 'pending'"
              @change="toggleOption(qi, oi)"
            >
            <label :for="optionId(qi, oi)" :data-chosen="isChosen(qi, oi) ? '' : undefined">
              <span :data-ring="question.multiSelect ? undefined : ''" :data-box="question.multiSelect ? '' : undefined"></span>
              <strong>{{ option.label }}</strong>
              <span v-if="option.description" data-why>{{ option.description }}</span>
            </label>
          </li>
        </ul>

        <label v-if="state === 'pending' && question.isOther !== false" data-own>
          <span>Or, in your own words</span>
          <input :type="question.isSecret ? 'password' : 'text'" v-model="custom[qi]" placeholder="Type an answer">
        </label>
      </div>

      <footer v-if="state === 'pending'">
        <button type="button" data-send :disabled="!complete" @click="send">Send answer</button>
        <button type="button" data-decline @click="decline">Decline to answer</button>
        <span data-hint>{{ answeredCount }} of {{ questions.length }} answered</span>
      </footer>
    </section>
  </div>
</template>
