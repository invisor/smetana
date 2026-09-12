import { describe, expect, it } from 'vitest'
import {
  ASK_USER_QUESTION_TOOL,
  buildAnswers,
  formatAnswer,
  isAskUserQuestion,
  isComplete,
  parseQuestions,
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
