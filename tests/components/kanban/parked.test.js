import { describe, expect, it } from 'vitest'
import {
  isParked,
  needsReadyWarning,
  openQuestions,
  PARKED,
  READY
} from '../../../src/components/kanban/parked.js'

describe('isParked', () => {
  it('is bd’s own word and nothing near it', () => {
    expect(isParked(PARKED)).toBe(true)
    for (const other of ['open', 'in_progress', 'closed', 'deferred', 'blocked', '', undefined]) {
      expect(isParked(other)).toBe(false)
    }
  })
})

describe('openQuestions', () => {
  it('reads the lines both writers actually produce', () => {
    // `runs::queue::parking_note` formats `parked: <question>`, and the
    // running-tasks skill tells a lead to write the same thing by hand.
    const notes = 'parked: needs a decision on where the strip sits\nparked: still waiting on the design call'
    expect(openQuestions(notes)).toEqual([
      'needs a decision on where the strip sits',
      'still waiting on the design call'
    ])
  })

  it('counts everything after the last resolved line and nothing above it', () => {
    // The pairing rule, and the reason it is positional by section rather than
    // line for line: a person answering three questions in one sentence writes
    // one `resolved:`, and pairing them off would call two of them still open.
    const notes = [
      'parked: which storage format',
      'resolved: sqlite, decided 2026-08-01',
      'parked: what happens on a schema bump'
    ].join('\n')
    expect(openQuestions(notes)).toEqual(['what happens on a schema bump'])
  })

  it('finds nothing once every question has been through a session', () => {
    const notes = 'parked: which storage format\nresolved: sqlite'
    expect(openQuestions(notes)).toEqual([])
  })

  it('ignores prose that is not a parked line', () => {
    // A note is free text: a run writes one shape, a person writes whatever.
    const notes = 'Talked to Ann about this.\nparked: who owns the migration\nStill unclear.'
    expect(openQuestions(notes)).toEqual(['who owns the migration'])
  })

  it('finds nothing in a task parked with no note', () => {
    // An ordinary case rather than a broken one — somebody parked it by hand.
    // The dialog says so in prose instead of drawing an empty list.
    for (const notes of ['', null, undefined, 'Nothing to say.']) {
      expect(openQuestions(notes)).toEqual([])
    }
  })

  it('tolerates the whitespace and the case a hand-written note carries', () => {
    expect(openQuestions('  Parked:   who owns the migration  ')).toEqual([
      'who owns the migration'
    ])
  })
})

describe('needsReadyWarning', () => {
  it('asks before putting a parked task back in the queue', () => {
    expect(needsReadyWarning(PARKED, READY)).toBe(true)
  })

  it('asks about nothing else a parked card can be moved to', () => {
    // Done closes the task, which decides the question no longer matters;
    // Pinned takes it off the queue. Ready is the one that hands it to an
    // agent with the question still open.
    for (const next of ['closed', 'pinned', PARKED]) {
      expect(needsReadyWarning(PARKED, next)).toBe(false)
    }
  })

  it('asks nothing about a task that was never parked', () => {
    for (const status of ['open', 'in_progress', 'closed', 'deferred', '', undefined]) {
      expect(needsReadyWarning(status, READY)).toBe(false)
    }
  })
})
