import { describe, expect, it } from 'vitest'
import {
  appendQuote, appendText, composeMessage, emptyDraft, isEmptyDraft,
  messageText, normalizeDraft, quoteFromSelection, removeQuote
} from '../../../src/components/conversation/draftSegments.js'

const T = (text) => ({ kind: 'text', text })
const Q = (text) => ({ kind: 'quote', text })

describe('appendQuote', () => {
  it('puts the first quote in front of the empty trailing text', () => {
    expect(appendQuote(emptyDraft(), 'said')).toEqual([Q('said'), T('')])
  })
  it('puts a quote after non-empty text and opens a new line under it', () => {
    expect(appendQuote([T('hi')], 'said')).toEqual([T('hi'), Q('said'), T('')])
  })
  it('keeps an empty text between two quotes', () => {
    expect(appendQuote([Q('a'), T('')], 'b')).toEqual([Q('a'), T(''), Q('b'), T('')])
  })
  it('does not mutate its input', () => {
    const draft = emptyDraft()
    appendQuote(draft, 'x')
    expect(draft).toEqual(emptyDraft())
  })
})

describe('removeQuote', () => {
  it('merges the texts around the quote with one newline and puts the caret at the join', () => {
    expect(removeQuote([T('above'), Q('q'), T('below')], 1)).toEqual({
      segments: [T('above\nbelow')],
      focus: { index: 0, offset: 5 }
    })
  })
  it('joins with nothing when the text above is empty', () => {
    expect(removeQuote([Q('q'), T('below')], 0)).toEqual({
      segments: [T('below')],
      focus: { index: 0, offset: 0 }
    })
  })
  it('joins with nothing when the text below is empty', () => {
    expect(removeQuote([T('above'), Q('q'), T('')], 1)).toEqual({
      segments: [T('above')],
      focus: { index: 0, offset: 5 }
    })
  })
  it('leaves an empty draft behind the last quote', () => {
    expect(removeQuote([Q('q'), T('')], 0)).toEqual({ segments: emptyDraft(), focus: { index: 0, offset: 0 } })
  })
  it('ignores an index that is not a quote', () => {
    const draft = [T('a'), Q('q'), T('b')]
    expect(removeQuote(draft, 0)).toEqual({ segments: draft, focus: null })
  })
})

describe('composeMessage', () => {
  it('writes a quote as a block quote and the comment under a blank line', () => {
    expect(composeMessage([Q('the agent said'), T('I disagree')])).toBe('> the agent said\n\nI disagree')
  })
  it('prefixes every line and writes a blank line inside a quote as a bare marker', () => {
    expect(composeMessage([Q('one\n\ntwo'), T('')])).toBe('> one\n>\n> two')
  })
  it('drops blank text segments and keeps the order', () => {
    expect(composeMessage([T('a'), Q('q1'), T('  '), Q('q2'), T('b')])).toBe('a\n\n> q1\n\n> q2\n\nb')
  })
  it('is empty for an empty draft', () => {
    expect(composeMessage(emptyDraft())).toBe('')
  })
})

describe('isEmptyDraft', () => {
  it('is true for the empty draft and for whitespace', () => {
    expect(isEmptyDraft(emptyDraft())).toBe(true)
    expect(isEmptyDraft([T('  \n')])).toBe(true)
  })
  it('is false with a quote and no words', () => {
    expect(isEmptyDraft([Q('q'), T('')])).toBe(false)
  })
})

describe('normalizeDraft', () => {
  it('wraps a string as one text segment', () => {
    expect(normalizeDraft('hello')).toEqual([T('hello')])
  })
  it('repairs a list to the invariants', () => {
    expect(normalizeDraft([T('a'), T('b'), Q('q'), Q('r')])).toEqual([T('a\nb'), Q('q'), T(''), Q('r'), T('')])
  })
  it('drops malformed entries and answers the empty draft for anything else', () => {
    expect(normalizeDraft([{ kind: 'bogus' }, null, T('ok')])).toEqual([T('ok')])
    expect(normalizeDraft(undefined)).toEqual(emptyDraft())
    expect(normalizeDraft(42)).toEqual(emptyDraft())
  })
})

describe('appendText', () => {
  it('appends with one space to a non-empty trailing text', () => {
    expect(appendText([T('see')], '/a/b.png ')).toEqual([T('see /a/b.png ')])
  })
  it('appends with no space to an empty trailing text', () => {
    expect(appendText([Q('q'), T('')], '/a/b.png ')).toEqual([Q('q'), T('/a/b.png ')])
  })
})

describe('quoteFromSelection', () => {
  it('normalises line endings and trims', () => {
    expect(quoteFromSelection('  one\r\ntwo \n')).toBe('one\ntwo')
  })
  it('is null for nothing', () => {
    expect(quoteFromSelection('  \n')).toBe(null)
    expect(quoteFromSelection(undefined)).toBe(null)
  })
})

describe('messageText', () => {
  it('passes a string through and composes a list', () => {
    expect(messageText('plain')).toBe('plain')
    expect(messageText([Q('q'), T('c')])).toBe('> q\n\nc')
  })
})
