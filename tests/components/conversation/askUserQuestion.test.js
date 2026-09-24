import { afterEach, describe, expect, it } from 'vitest'
import { createApp, nextTick } from 'vue'
import AskUserQuestion from '../../../src/components/conversation/AskUserQuestion.vue'
import {
  ASK_USER_QUESTION_TOOL,
  buildAnswers,
  canSubmitCustomAnswer,
  formatAnswer,
  isAskUserQuestion,
  isComplete,
  parseQuestions,
  selectedLabels,
  toggle
} from '../../../src/components/conversation/askUserQuestion.js'

describe('isAskUserQuestion', () => {
  it('names the one tool this whole family exists for', () => {
    expect(ASK_USER_QUESTION_TOOL).toBe('AskUserQuestion')
    expect(isAskUserQuestion('AskUserQuestion')).toBe(true)
  })

  it('is false for every other tool, permission cards included', () => {
    expect(isAskUserQuestion('Bash')).toBe(false)
    expect(isAskUserQuestion(undefined)).toBe(false)
  })
})

describe('parseQuestions', () => {
  it('reads the wire shape whole: header, question, options and multiSelect', () => {
    const input = {
      questions: [
        {
          question: 'Which approach?',
          header: 'Approach',
          multiSelect: false,
          options: [
            { label: 'Rewrite', description: 'Touches every caller' },
            { label: 'Patch', description: 'Smaller, leaves the old path' }
          ]
        }
      ]
    }
    expect(parseQuestions(input)).toEqual([
      {
        question: 'Which approach?',
        header: 'Approach',
        multiSelect: false,
        options: [
          { label: 'Rewrite', description: 'Touches every caller' },
          { label: 'Patch', description: 'Smaller, leaves the old path' }
        ]
      }
    ])
  })

  it('reads up to four questions, each with its own options', () => {
    const input = {
      questions: [
        { question: 'A?', header: 'A', options: [{ label: 'x', description: '' }] },
        { question: 'B?', header: 'B', options: [{ label: 'y', description: '' }] },
        { question: 'C?', header: 'C', options: [{ label: 'z', description: '' }] },
        { question: 'D?', header: 'D', options: [{ label: 'w', description: '' }] }
      ]
    }
    expect(parseQuestions(input).map((q) => q.question)).toEqual(['A?', 'B?', 'C?', 'D?'])
  })

  it('degrades a malformed call to an empty list rather than throwing', () => {
    expect(parseQuestions(undefined)).toEqual([])
    expect(parseQuestions(null)).toEqual([])
    expect(parseQuestions({})).toEqual([])
    expect(parseQuestions({ questions: 'not a list' })).toEqual([])
  })

  it('defaults a missing field on one question rather than dropping the question', () => {
    expect(parseQuestions({ questions: [{}] })).toEqual([
      { question: '', header: '', multiSelect: false, options: [] }
    ])
  })

  it('drops an option with no real shape rather than throwing on it', () => {
    const parsed = parseQuestions({ questions: [{ question: 'Q?', options: [null, { label: 'ok' }] }] })
    expect(parsed[0].options).toEqual([
      { label: '', description: '' },
      { label: 'ok', description: '' }
    ])
  })
})

describe('toggle', () => {
  it('replaces whatever was chosen for an ordinary, single-select question', () => {
    expect(toggle(['Rewrite'], 'Patch', false)).toEqual(['Patch'])
  })

  it('adds a second label for a multiSelect question rather than replacing the first', () => {
    expect(toggle(['Rewrite'], 'Patch', true)).toEqual(['Rewrite', 'Patch'])
  })

  it('removes a label already chosen, for either shape of question', () => {
    expect(toggle(['Rewrite'], 'Rewrite', false)).toEqual([])
    expect(toggle(['Rewrite', 'Patch'], 'Patch', true)).toEqual(['Rewrite'])
  })

  it('starts from nothing selected when given none', () => {
    expect(toggle(undefined, 'Rewrite', false)).toEqual(['Rewrite'])
    expect(toggle(undefined, 'Rewrite', true)).toEqual(['Rewrite'])
  })

  /* The shape that actually ships: `AskUserQuestion.vue` hands this an
     option's own index, not its label — see the function's own header for
     why. Pinning the numeric case is what stops somebody reading the label
     examples above as the real contract and keying selection by label again. */
  it('works on option indices exactly as it does on labels, since it is generic', () => {
    expect(toggle([0], 1, false)).toEqual([1])
    expect(toggle([0], 1, true)).toEqual([0, 1])
    expect(toggle([0, 1], 1, true)).toEqual([0])
  })
})

describe('selectedLabels', () => {
  const questions = parseQuestions({
    questions: [
      { question: 'Which approach?', options: [{ label: 'Rewrite' }, { label: 'Patch' }] },
      { question: 'Ship it today?', options: [{ label: 'Yes' }, { label: 'No' }] }
    ]
  })

  it('maps each chosen index back to that option’s own label', () => {
    expect(selectedLabels(questions, [[0], [1]])).toEqual([['Rewrite'], ['No']])
  })

  it('answers an unselected question with an empty list, not undefined', () => {
    expect(selectedLabels(questions, [[], [0]])).toEqual([[], ['Yes']])
  })

  it('reads an option whose own label defaulted to empty as an empty string', () => {
    const withBlankLabel = parseQuestions({ questions: [{ question: 'Q?', options: [{}, { label: 'ok' }] }] })
    expect(selectedLabels(withBlankLabel, [[0, 1]])).toEqual([['', 'ok']])
  })

  it('answers an index past the end of options with an empty string rather than throwing', () => {
    expect(selectedLabels(questions, [[5], []])).toEqual([[''], []])
  })
})

describe('formatAnswer', () => {
  it('joins several selected labels with a comma, for multiSelect', () => {
    expect(formatAnswer(['Rewrite', 'Patch'], '')).toBe('Rewrite, Patch')
  })

  it('is the one label itself when only one was chosen', () => {
    expect(formatAnswer(['Rewrite'], '')).toBe('Rewrite')
  })

  it('prefers a person’s own words over any selection', () => {
    expect(formatAnswer(['Rewrite'], 'Neither — do something else entirely')).toBe(
      'Neither — do something else entirely'
    )
  })

  it('trims the custom answer, so trailing whitespace does not count as content', () => {
    expect(formatAnswer([], '   ')).toBe('')
    expect(formatAnswer([], '  Rewrite it  ')).toBe('Rewrite it')
  })

  it('is empty when neither a selection nor a custom answer was given', () => {
    expect(formatAnswer([], '')).toBe('')
    expect(formatAnswer(undefined, undefined)).toBe('')
  })
})

describe('buildAnswers', () => {
  const questions = parseQuestions({
    questions: [
      { question: 'Which approach?', options: [{ label: 'Rewrite' }, { label: 'Patch' }] },
      { question: 'Ship it today?', options: [{ label: 'Yes' }, { label: 'No' }] }
    ]
  })

  it('keys the answers by each question’s own text', () => {
    expect(buildAnswers(questions, [['Rewrite'], ['Yes']], ['', ''])).toEqual({
      'Which approach?': 'Rewrite',
      'Ship it today?': 'Yes'
    })
  })

  it('leaves out a question with nothing answered rather than writing an empty string', () => {
    expect(buildAnswers(questions, [['Rewrite'], []], ['', ''])).toEqual({
      'Which approach?': 'Rewrite'
    })
  })

  it('takes a custom answer over a selection for the same question', () => {
    expect(buildAnswers(questions, [['Rewrite'], []], ['Do neither', ''])).toEqual({
      'Which approach?': 'Do neither'
    })
  })
})

describe('isComplete', () => {
  const questions = parseQuestions({
    questions: [
      { question: 'Which approach?', options: [{ label: 'Rewrite' }] },
      { question: 'Ship it today?', options: [{ label: 'Yes' }] }
    ]
  })

  it('is false while any question has nothing answered', () => {
    expect(isComplete(questions, [['Rewrite'], []], ['', ''])).toBe(false)
  })

  it('is true once every question has an answer, selected or typed', () => {
    expect(isComplete(questions, [['Rewrite'], []], ['', 'Not yet'])).toBe(true)
  })

  it('is false for an empty call — nothing to answer is not answered', () => {
    expect(isComplete([], [], [])).toBe(false)
  })
})

describe('canSubmitCustomAnswer', () => {
  const ready = {
    customAnswer: 'Which option would you recommend?',
    complete: true,
    state: 'pending',
    isComposing: false
  }

  it('allows Enter on a meaningful custom answer when the whole form is ready', () => {
    expect(canSubmitCustomAnswer(ready)).toBe(true)
  })

  it('refuses empty and whitespace-only custom answers', () => {
    expect(canSubmitCustomAnswer({ ...ready, customAnswer: '' })).toBe(false)
    expect(canSubmitCustomAnswer({ ...ready, customAnswer: '   \t' })).toBe(false)
  })

  it('refuses a partial multi-question form even when the focused field has text', () => {
    expect(canSubmitCustomAnswer({ ...ready, complete: false })).toBe(false)
  })

  it('refuses a composing IME value and a card that has already answered', () => {
    expect(canSubmitCustomAnswer({ ...ready, isComposing: true })).toBe(false)
    expect(canSubmitCustomAnswer({ ...ready, state: 'answered' })).toBe(false)
  })
})

/* This deliberately mounts the card without Vue Test Utils: the application
   already depends on Vue, while a second test-only DOM abstraction would add
   nothing to the keyboard path under test. The pure cases above pin the
   predicate; these checks prove the field actually routes Enter through the
   same emitted answer and settled state as Send answer. */
describe('AskUserQuestion custom-answer Enter', () => {
  const mounted = []

  afterEach(() => {
    mounted.splice(0).forEach(({ app, host }) => {
      app.unmount()
      host.remove()
    })
  })

  function mount(input) {
    const answers = []
    const host = document.createElement('div')
    document.body.append(host)
    const app = createApp(AskUserQuestion, {
      input,
      onAnswer: (...answer) => answers.push(answer)
    })
    app.mount(host)
    mounted.push({ app, host })
    return { answers, host }
  }

  async function enterCustomAnswer(host, value, { isComposing = false, presses = 1 } = {}) {
    const field = host.querySelector('[data-own] input')
    field.value = value
    field.dispatchEvent(new Event('input', { bubbles: true }))
    await nextTick()
    for (let press = 0; press < presses; press += 1) {
      const event = new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })
      Object.defineProperty(event, 'isComposing', { value: isComposing })
      field.dispatchEvent(event)
    }
    await nextTick()
  }

  it('sends a completed custom answer on Enter and settles once', async () => {
    const { answers, host } = mount({ questions: [{ question: 'Recommendation?' }] })

    await enterCustomAnswer(host, 'Use the safer option', { presses: 2 })

    expect(answers).toEqual([['allow', { 'Recommendation?': 'Use the safer option' }]])
    expect(host.querySelector('[data-ask]').dataset.state).toBe('answered')
  })

  it('keeps the card pending for an empty or whitespace-only custom answer', async () => {
    const { answers, host } = mount({ questions: [{ question: 'Recommendation?' }] })

    await enterCustomAnswer(host, '   ')

    expect(answers).toEqual([])
    expect(host.querySelector('[data-ask]').dataset.state).toBe('pending')
  })

  it('keeps a multi-question card pending until every question is complete', async () => {
    const { answers, host } = mount({
      questions: [{ question: 'First?' }, { question: 'Second?' }]
    })

    await enterCustomAnswer(host, 'First answer')

    expect(answers).toEqual([])
    expect(host.querySelector('[data-ask]').dataset.state).toBe('pending')
  })

  it('does not send an IME composition value on Enter', async () => {
    const { answers, host } = mount({ questions: [{ question: 'Recommendation?' }] })

    await enterCustomAnswer(host, 'in-progress input', { isComposing: true })

    expect(answers).toEqual([])
    expect(host.querySelector('[data-ask]').dataset.state).toBe('pending')
  })
})
