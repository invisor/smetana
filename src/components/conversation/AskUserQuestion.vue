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
   holds the parsing and the answer-building rules, pure and tested there for
   the reason every file in this family is out of its component: a `.vue`
   file is the one thing no runner here can reach. This component's own state
   is which option is selected (by index — see `toggle`'s own header) and
   what a person typed per question; what it still decides on its own is
   plainer but is a rule nonetheless — `setCustom` below is the one place
   that says typing clears a selection.

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
import { buildAnswers, isComplete, parseQuestions, selectedLabels, toggle } from './askUserQuestion.js'

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

const questions = ref([])

/* Per question: the **indices** of the options currently chosen, and
   whatever a person typed instead. Indices rather than labels, because
   `parseQuestions` defaults a missing `label` to `''`, and two options that
   both lost theirs would otherwise be the same value as far as selection is
   concerned — picking one would toggle both. Rebuilt whenever the questions
   themselves change — a fresh `AskUserQuestion` call is a fresh form, never
   the last one's state showing through a new set of options — which in
   practice means every time, since `ConversationView.vue` keys this
   component on the permission's own id. */
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

function isSelected(qi, oi) {
  return selected[qi]?.includes(oi) ?? false
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
   moved out for the opposite reason, carrying an edge case worth a test. */
function setCustom(qi, text) {
  custom[qi] = text
  if (text) selected[qi] = []
}

const complete = ref(false)
watch([questions, selected, custom], () => {
  complete.value = isComplete(questions.value, selectedLabels(questions.value, selected), custom)
}, { immediate: true, deep: true })

function send() {
  emit('answer', 'allow', buildAnswers(questions.value, selectedLabels(questions.value, selected), custom))
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

/* `alignItems: 'flex-start'` rather than `center`: below about 330px the
   sentence wraps to two lines, and centring the icon against the *block*
   put the triangle floating between them rather than sitting on the first
   line it is announcing. Flex-start pins it to the top, level with the
   first line, whether the sentence wraps or not. */
const head = {
  display: 'flex',
  alignItems: 'flex-start',
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

/* Sans, sentence case, no transform: `header` is the agent's own prose — a
   short label it wrote, not an identifier and not this system's own copy —
   and CLAUDE.md's rule is flat about both halves of that. Uppercase mono is
   also the silhouette `StatusBadge` reserves for a status; spending it on
   arbitrary agent text would put a badge's own idiom on a string that has
   nothing to do with status. Weight and size are what set it apart from
   `questionText` below instead of a transform or a second ink this card has
   no colour budget left for. */
const questionHeader = {
  font: 'var(--weight-medium) var(--text-xs)/var(--leading-snug) var(--font-sans)'
}

const questionText = {
  font: 'var(--weight-regular) var(--text-sm)/var(--leading-normal) var(--font-sans)'
}

const optionsList = { display: 'flex', flexDirection: 'column', gap: 'var(--space-3)' }

/* One option, drawn by hand rather than through `Button.vue`: that component
   is one line, centred and fixed-height, and an option here carries a label
   and a wrapping description on the line under it — auto-height, since
   nothing here declares one.

   **The border is always the same width, `var(--border-w-strong)`, and only
   its colour ever moves — never absent, only invisible**, the same device
   `sm-prose.css`'s copy control uses (`.claude/rules/conversation-panel.md`,
   "The copy button is always visible") and for the identical reason: with no
   height declared, `box-sizing: border-box` has nothing to absorb a wider
   border into, so a version that swapped `--border-w` for `--border-w-strong`
   on hover or on selection grew the box by the difference — 2px taller, the
   label a pixel lower, and the next question's first option shifted under
   the pointer, which in a `multiSelect` question is exactly where the next
   click was going.

   **Three colours for three states, and the middle one is a genuine colour
   change on hover** — `core/interactive.js`'s "never a colour change" rule
   is written for a control on the app's own neutral surface ladder
   (`--surface` → `--surface-hover` → `--surface-active`), and this card has
   no such ladder to step on: its ground is `c.fg`, a saturated fill, and
   stepping *that* would mean a second, brighter status hue with nothing in
   this design system to draw it from. What meets the rule's actual purpose —
   a control in a dense list cannot jump — is the reserved border alone: rest
   is `c.border`, the same status's own dimmer step (`statusColors`'s
   three-tier ramp, already computed above as `c` and already read for the
   card's own fill; nothing new is spent), present but quiet, so an option
   reads as bounded before anyone points at it; hover is `var(--surface-raised)`,
   the card's full ink, a clearly stronger ring than rest; and a selected
   option carries no border of its own — `transparent`, since the inverted
   fill already says what a border would, the same ink-on-paper idiom the
   primary button uses elsewhere in this system. */
function optionStyle(qi, oi, hovered) {
  const on = isSelected(qi, oi)
  const borderColor = on ? 'transparent' : hovered ? 'var(--surface-raised)' : c.border
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
    border: `var(--border-w-strong) solid ${borderColor}`,
    borderRadius: 'var(--radius-3)',
    font: 'inherit',
    cursor: 'default',
    transition: 'var(--transition-control)'
  }
}

const optionLabel = { font: 'var(--weight-medium) var(--text-sm)/var(--leading-snug) var(--font-sans)' }
const optionDescription = { font: 'var(--weight-regular) var(--text-xs)/var(--leading-normal) var(--font-sans)' }

/* The free-text field has to read as a different kind of thing from an
   option at a glance, and colour is not available to spend on it — both a
   selected option and `Input.vue`'s own box resolve to the identical
   `var(--surface-raised)` fill, `--radius-3` corner and full width, so in
   the `Platforms` question with `Windows` chosen the card drew three
   indistinguishable rounded boxes in a row, the last of which answers
   nothing. A hairline above it separates it from the option list as its own
   zone, a caption gives it the label an option never carries, and the
   `pencil` prefix inside the field itself (`Input`'s own `prefix` slot) says
   "type here" before anyone reads a word — three token-only cues, none of
   them a hue. */
const customRow = {
  display: 'flex',
  flexDirection: 'column',
  gap: 'var(--space-2)',
  paddingTop: 'var(--space-3)',
  borderTop: 'var(--border-w) solid var(--surface-raised)'
}

const customLabel = { font: 'var(--weight-regular) var(--text-xs)/var(--leading-snug) var(--font-sans)' }

const actions = { display: 'flex', flexWrap: 'wrap', gap: 'var(--space-3)' }

/* Tracked per option rather than through `useInteractive` — that composable
   is written for one control per component instance, and this card draws up
   to sixteen of them (four questions, up to four options apiece). A single
   hovered key is all a pointer can be over at once, and it is built from the
   same `(qi, oi)` pair selection is, for the reason `selected` itself is
   indexed rather than labelled. */
const hoveredKey = ref(null)
const keyOf = (qi, oi) => `${qi}:${oi}`
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
            v-for="(option, oi) in question.options"
            :key="oi"
            type="button"
            :style="optionStyle(qi, oi, hoveredKey === keyOf(qi, oi))"
            :aria-pressed="isSelected(qi, oi)"
            @mouseenter="hoveredKey = keyOf(qi, oi)"
            @mouseleave="hoveredKey = null"
            @click="toggleOption(qi, oi)"
          >
            <span :style="optionLabel">{{ option.label }}</span>
            <span v-if="option.description" :style="optionDescription">{{ option.description }}</span>
          </button>
        </div>
        <div :style="customRow">
          <div :style="customLabel">Or, in your own words</div>
          <Input
            :model-value="custom[qi]"
            placeholder="Type an answer"
            size="sm"
            @update:model-value="setCustom(qi, $event)"
          >
            <template #prefix><Icon name="pencil" :size="12" /></template>
          </Input>
        </div>
      </div>
    </div>
    <div :style="actions">
      <Button size="sm" variant="primary" :disabled="!complete" @click="send">Send answer</Button>
      <Button size="sm" variant="secondary" @click="decline">Decline to answer</Button>
    </div>
  </div>
</template>
