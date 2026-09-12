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
   person's typed words when there are any, the selected options joined by a
   comma otherwise. `buildAnswers` and `isComplete` are the two rules
   `ConversationView.vue` calls before it will let a press through; the
   component itself only tracks which option is selected and what a person
   typed, both of them plain state with no rule of their own. */

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
    if (text) answers[q.question] = text
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
