<script setup>
/* `AskUserQuestion`'s own card, beside `PermissionRequest.vue` and never
   inside it: that one draws a tool's name and a one-line detail behind two
   buttons, and this tool has neither — up to four questions, each with a
   short header, its own prose and two to four options that carry their own
   description, some of them answerable with more than one choice at once. A
   permission card fed this call's `detail` alone drew a yellow strip with a
   title and nothing under it, because `agents::claude::tool_detail` has one
   line to give and this tool needs a form.

   The wire is `session::model::EventKind::Permission`'s `input` — the tool
   call's own arguments, untouched — and `askUserQuestion.js` beside this file
   is the whole of the parsing and the answer-building rule, pure and tested
   there for the reason every file in this family is out of its component: a
   `.vue` file is the one thing no runner here can reach. This component's own
   state is only which option is selected and what a person typed per
   question, both plain and neither a rule of its own.

   **A custom answer always wins over a selection, for one question at a
   time.** Choosing an option clears whatever was typed for it and typing
   clears whatever was chosen — the wire takes one string per question, and a
   person is always free to answer in their own words rather than pick from
   the options the agent offered (the Design section of smetana-63kn is
   explicit about this). `formatAnswer` in `askUserQuestion.js` is the one
   place that resolves the two into that single string, so this file only has
   to keep them from being edited at the same time.

   **Drawn at the same `loud` weight as `PermissionRequest.vue`, and for the
   same reason**: the harness is holding this same tool call open and the
   session is `needs-you` until somebody answers, whichever card is on
   screen. `statusColors('needs-you')` and `STATUS_GLYPH` are the identical
   pair that card uses, so the two read as one vocabulary rather than two —
   only the shape of what is inside the frame differs. */
import { reactive, ref, watch } from 'vue'
import Button from '../core/Button.vue'
import Icon from '../core/Icon.vue'
import Input from '../core/Input.vue'
import { STATUS_GLYPH, statusColors } from '../status/status.js'
import { buildAnswers, isComplete, parseQuestions } from './askUserQuestion.js'

const props = defineProps({
  /* `Permission::input` off the wire — the raw arguments of the
     `AskUserQuestion` call, parsed by `askUserQuestion.js` rather than here. */
  input: { type: Object, default: () => ({}) }
})

/* `decision`, `'allow'` or `'deny'` (`session::model::Decision`'s wire
   words); `answers`, only for an allow, keyed by each question's own text —
   `undefined` for a decline, which the store already reads as no answers at
   all. */
const emit = defineEmits(['answer'])

const questions = ref(parseQuestions(props.input))

/* Per question: the labels currently chosen, and whatever a person typed
   instead. Rebuilt whenever the questions themselves change — a fresh
   `AskUserQuestion` call is a fresh form, never the last one's state showing
   through a new set of options — which in practice means every time, since
   `ConversationView.vue` keys this component on the permission's own id. */
const selected = reactive([])
const custom = reactive([])

function reset(parsed) {
  questions.value = parsed
  selected.length = 0
  custom.length = 0
  parsed.forEach(() => {
    selected.push([])
    custom.push('')
  })
}
reset(parseQuestions(props.input))
watch(() => props.input, (next) => reset(parseQuestions(next)))

function isSelected(qi, label) {
  return selected[qi]?.includes(label) ?? false
}

function toggleOption(qi, label) {
  const question = questions.value[qi]
  const current = selected[qi] ?? []
  if (question.multiSelect) {
    selected[qi] = current.includes(label)
      ? current.filter((chosen) => chosen !== label)
      : [...current, label]
  } else {
    selected[qi] = current.includes(label) ? [] : [label]
  }
  // A selection and a typed answer are mutually exclusive for one question —
  // see this file's own header for why picking an option clears the field.
  custom[qi] = ''
}

function setCustom(qi, text) {
  custom[qi] = text
  if (text) selected[qi] = []
}

const complete = ref(false)
watch([questions, selected, custom], () => {
  complete.value = isComplete(questions.value, selected, custom)
}, { immediate: true, deep: true })

function send() {
  emit('answer', 'allow', buildAnswers(questions.value, selected, custom))
}

function decline() {
  emit('answer', 'deny')
}

/* Reserved, so this is a constant rather than a computed — `needs-you`
   always resolves to the same four token references, the pair
   `PermissionRequest.vue` already draws its own card from. */
const c = statusColors('needs-you')
const glyph = STATUS_GLYPH[c.key]

const card = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-5)',
  padding: 'var(--space-5)',
  background: c.fg,
  color: 'var(--surface-raised)',
  border: `var(--border-w) solid ${c.fg}`,
  borderRadius: 'var(--radius-4)',
  fontFamily: 'var(--font-sans)'
}

const head = {
  display: 'flex',
  alignItems: 'center',
  gap: 'var(--space-3)',
  font: 'var(--weight-medium) var(--text-sm)/var(--leading-snug) var(--font-sans)'
}

const questionsList = { display: 'flex', flexDirection: 'column', gap: 'var(--space-5)' }

/* A rule between two questions in the same call, drawn from the border weight
   this card already uses rather than a second colour: nothing here is
   allowed a colour this design system did not choose, and a hairline in the
   card's own ink is legible on both themes without one. The first question
   gets none — there is nothing above it to separate it from. */
const questionBlock = (i) => ({
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-3)',
  paddingTop: i > 0 ? 'var(--space-5)' : 0,
  borderTop: i > 0 ? 'var(--border-w) solid var(--surface-raised)' : 'none'
})

const questionHeader = {
  font: 'var(--weight-semibold) var(--text-xs)/var(--leading-snug) var(--font-mono)',
  textTransform: 'uppercase',
  letterSpacing: 'var(--tracking-tight)'
}

const questionText = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)'
}

const optionsList = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }

/* One option, drawn by hand rather than through `Button.vue`: that component
   is one line, centred and fixed-height, and an option here carries a label
   and a wrapping description on the line under it. Interaction is still a
   surface step and never a colour change on its own account (`core/
   interactive.js`'s own rule) — hover only thickens the border, selecting is
   the one state allowed to invert the card's own ink and fill, the same
   ink-on-paper idiom the primary button uses elsewhere in this system. */
function optionStyle(qi, label, hovered) {
  const on = isSelected(qi, label)
  return {
    display: 'flex',
    flexDirection: 'column',
    alignItems: 'flex-start',
    gap: 'var(--space-1)',
    width: '100%',
    textAlign: 'left',
    padding: 'var(--space-3) var(--space-4)',
    background: on ? 'var(--surface-raised)' : 'transparent',
    color: on ? c.fg : 'var(--surface-raised)',
    border: `${on || hovered ? 'var(--border-w-strong)' : 'var(--border-w)'} solid var(--surface-raised)`,
    borderRadius: 'var(--radius-3)',
    font: 'inherit',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}

const optionLabel = { font: 'var(--weight-medium) var(--text-sm)/var(--leading-snug) var(--font-sans)' }
const optionDescription = { font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)' }

const customRow = { display: 'flex' }

const actions = { display: 'flex', flexWrap: 'wrap', gap: 'var(--space-3)' }

/* Tracked per option rather than through `useInteractive` — that composable
   is written for one control per component instance, and this card draws up
   to sixteen of them (four questions, up to four options apiece). A single
   hovered key is all a pointer can be over at once. */
const hoveredKey = ref(null)
const keyOf = (qi, label) => `${qi}:${label}`
</script>

<template>
  <div data-attention="loud" :style="card">
    <div :style="head">
      <Icon :name="glyph" :size="13" :stroke-width="2.25" />
      <span>The agent needs an answer before it can continue.</span>
    </div>
    <div :style="questionsList">
      <div v-for="(question, qi) in questions" :key="qi" :style="questionBlock(qi)">
        <div v-if="question.header" :style="questionHeader">{{ question.header }}</div>
        <div :style="questionText">{{ question.question }}</div>
        <div :style="optionsList">
          <button
            v-for="option in question.options"
            :key="option.label"
            type="button"
            :style="optionStyle(qi, option.label, hoveredKey === keyOf(qi, option.label))"
            :aria-pressed="isSelected(qi, option.label)"
            @mouseenter="hoveredKey = keyOf(qi, option.label)"
            @mouseleave="hoveredKey = null"
            @click="toggleOption(qi, option.label)"
          >
            <span :style="optionLabel">{{ option.label }}</span>
            <span v-if="option.description" :style="optionDescription">{{ option.description }}</span>
          </button>
        </div>
        <div :style="customRow">
          <Input
            :model-value="custom[qi]"
            placeholder="Or answer in your own words"
            size="sm"
            @update:model-value="setCustom(qi, $event)"
          />
        </div>
      </div>
    </div>
    <div :style="actions">
      <Button size="sm" variant="primary" :disabled="!complete" @click="send">Send answer</Button>
      <Button size="sm" variant="secondary" @click="decline">Decline to answer</Button>
    </div>
  </div>
</template>
