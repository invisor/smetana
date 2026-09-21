/* AskUserQuestion's own rules, pulled out of the component for the reason
   every file in this family is: a `.vue` file is the one thing no runner in
   this repository can reach.

   The wire's shape, from `session::model::EventKind::Permission`'s `input` —
   the tool call's own arguments, untouched by Rust — is the Agent SDK's own
   documented one: one to four `questions`, each carrying a `question`, a
   `header`, two to four `options` (`label` and `description`) and
   `multiSelect`. This is the one place in the front end reading a tool's raw
   JSON argument rather than a value this app minted, so every field here is
   read defensively rather than trusted.

   What travels back is `answers`, keyed by the question's own text — a
   person's typed words only when `isOther` permits them, the selected options joined by a
   comma otherwise. `toggle`, `selectedLabels`, `buildAnswers` and
   `isComplete` are the rules `AskUserQuestion.vue` calls before it will let
   a press through; what stays in the component is plainer — which question
   a click was on, and a small `watch` on the typed field that clears
   whatever was selected. */

/** The one tool this whole family exists for. */
export const ASK_USER_QUESTION_TOOL = 'AskUserQuestion'

export const isAskUserQuestion = (tool) => tool === ASK_USER_QUESTION_TOOL

/* `input.questions`, defensively. A missing or malformed field degrades to an
   empty list or an empty string rather than throwing — this is the one place
   in the front end reading a tool's raw argument rather than a value this
   app minted itself, and a malformed one must draw an empty card rather than
   take the panel down with it. */
export function parseQuestions(input) {
  const list = Array.isArray(input?.questions) ? input.questions : []
  return list.map((raw) => ({
    /* Codex app-server identifies answers by id; Claude's established wire
       contract has no such field and therefore remains keyed by question. */
    ...(typeof raw?.id === 'string'
      ? { id: raw.id, isOther: raw?.isOther === true, isSecret: raw?.isSecret === true }
      : {}),
    question: typeof raw?.question === 'string' ? raw.question : '',
    header: typeof raw?.header === 'string' ? raw.header : '',
    multiSelect: raw?.multiSelect === true,
    options: Array.isArray(raw?.options)
      ? raw.options.map((option) => ({
          label: typeof option?.label === 'string' ? option.label : '',
          description: typeof option?.description === 'string' ? option.description : ''
        }))
      : []
  }))
}

/* What clicking one option does to the identities selected for its question
   — one of this task's own acceptance criteria, and so pulled out here
   rather than left in the component: a `multiSelect` question allows more
   than one option at once, an ordinary one allows exactly one, and clicking
   a chosen option deselects it either way. `AskUserQuestion.vue` is the one
   thing in this repository no test can reach, and an edit that made
   single-select accumulate or multi-select replace would ship with both
   gates green and reach the agent as one label where four were chosen — the
   bug this task exists to fix, one layer up.

   Generic over what `id` is, deliberately: the tests below pass labels, the
   one real caller passes an option's own **index**, and this function does
   not care which — it only ever compares `id` for equality and never reads
   through it. The component keys by index rather than by label because
   `parseQuestions` can default two options to the same empty label, which a
   label-keyed selection would toggle as one; `selectedLabels` below is
   where an index is turned back into the label the wire actually wants. */
export function toggle(selected, id, multiSelect) {
  const current = selected ?? []
  if (multiSelect) {
    return current.includes(id) ? current.filter((chosen) => chosen !== id) : [...current, id]
  }
  return current.includes(id) ? [] : [id]
}

/* The one place an option's **index** and its **label** meet. A question's
   selection is kept by index in `AskUserQuestion.vue` (`toggle`'s own header
   says why), but `buildAnswers` and `isComplete` below take the wire's own
   vocabulary — the chosen labels — so this is the boundary, moved out here
   rather than left as a `.vue` method for the same reason `toggle` was: it
   is short, but it is still a rule, including what happens at its two edges.
   `''` is the answer for an option whose own label defaulted to empty
   (`parseQuestions` again) and for an index that does not name an option at
   all — past the end of `options`, which cannot happen from a click on a
   real button but is worth answering rather than throwing on, since nothing
   here has minted `options` and nothing guarantees its shape. */
export function selectedLabels(questions, selectedOptionIndices) {
  return questions.map((question, qi) =>
    (selectedOptionIndices[qi] ?? []).map((oi) => question.options[oi]?.label ?? '')
  )
}

/* The one string a single question's answer becomes on the wire: a person's
   own words when they wrote any, the selected options joined by a comma
   otherwise — the only join in this file, since a single-select answer is
   one label and needs none. A custom answer always wins over a selection:
   the two are mutually exclusive in the component, but this function is the
   one place that decides it, so a caller cannot get the two out of step. */
export function formatAnswer(selected, custom) {
  const text = String(custom ?? '').trim()
  if (text) return text
  return (selected ?? []).join(', ')
}

/* `answers`, keyed by each question's own text as the wire requires. Built
   fresh each time rather than accumulated, and a question with nothing to
   say is left out of it entirely — a placeholder empty string would be a
   answer nobody gave, and `isComplete` below is what stops this from ever
   being sent half built. */
export function buildAnswers(questions, selectedByIndex, customByIndex) {
  const answers = {}
  questions.forEach((q, i) => {
    const text = formatAnswer(selectedByIndex[i], customByIndex[i])
    if (text) answers[q.id || q.question] = text
  })
  return answers
}

/* Whether every question has something to send — the gate on the card's own
   Send button. The agent asked every question in one call, so this panel
   answers all of them in one call too, rather than sending a partial
   `answers` for a call that named four. */
export function isComplete(questions, selectedByIndex, customByIndex) {
  if (questions.length === 0) return false
  return questions.every((_, i) => formatAnswer(selectedByIndex[i], customByIndex[i]).length > 0)
}
