import { describe, expect, it } from 'vitest'
import {
  counterLabel,
  filterIssues,
  relationOf,
  sectionLabel,
  shortId,
  stepIndex,
  waitingLabel
} from '../../../src/components/shell/commandPalette.js'

const issue = (over = {}) => ({
  id: 'smetana-a1a',
  title: 'A title',
  status: 'open',
  parent: null,
  dependent_count: 0,
  updated_at: '2026-08-01T00:00:00Z',
  ...over
})

const noEdges = { blockedBy: new Map(), blocking: new Map() }

describe('filterIssues', () => {
  it('answers nothing at all for an empty query', () => {
    expect(filterIssues([issue()], '')).toEqual([])
    expect(filterIssues([issue()], '   ')).toEqual([])
  })

  it('matches an id and a title alike, case-insensitively', () => {
    const issues = [
      issue({ id: 'x-1', title: 'Collapse the card header' }),
      issue({ id: 'x-2', title: 'Nothing' })
    ]
    expect(filterIssues(issues, 'COLLAPSE').map((hit) => hit.id)).toEqual(['x-1'])
    expect(filterIssues(issues, 'X-2').map((hit) => hit.id)).toEqual(['x-2'])
  })

  it('matches across the join between the id and the title', () => {
    expect(filterIssues([issue({ id: 'x-1', title: 'Bell' })], 'x-1 bell').map((h) => h.id)).toEqual([
      'x-1'
    ])
  })

  it('does not look at the prose fields', () => {
    const issues = [issue({ id: 'x-1', title: 'Nothing', description: 'the bell rings', notes: 'bell' })]
    expect(filterIssues(issues, 'bell')).toEqual([])
  })

  it('puts an earlier occurrence above a later one', () => {
    const issues = [
      issue({ id: 'x-1', title: 'Long preamble before the bell' }),
      issue({ id: 'x-2', title: 'Bell at the front' })
    ]
    expect(filterIssues(issues, 'bell').map((hit) => hit.id)).toEqual(['x-2', 'x-1'])
  })

  it('breaks a tie by the newest, and then by the id, so no two rows can trade places', () => {
    const issues = [
      issue({ id: 'x-2', title: 'Bell', updated_at: '2026-08-01T00:00:00Z' }),
      issue({ id: 'x-1', title: 'Bell', updated_at: '2026-08-01T00:00:00Z' }),
      issue({ id: 'x-3', title: 'Bell', updated_at: '2026-08-02T00:00:00Z' })
    ]
    expect(filterIssues(issues, 'bell').map((hit) => hit.id)).toEqual(['x-3', 'x-1', 'x-2'])
  })

  it('carries the status through, since the row draws it', () => {
    expect(filterIssues([issue({ status: 'running' })], 'a1a')[0].status).toBe('running')
  })

  it('stops at the limit rather than filling the panel twice over', () => {
    const issues = Array.from({ length: 30 }, (_, at) => issue({ id: `x-${at}`, title: 'Bell' }))
    expect(filterIssues(issues, 'bell')).toHaveLength(20)
  })
})

describe('shortId', () => {
  it('keeps the last segment of an id', () => {
    expect(shortId('holiday-curb-bhyv')).toBe('bhyv')
  })

  it('answers the whole id when there is no segment to take', () => {
    expect(shortId('bhyv')).toBe('bhyv')
  })
})

describe('relationOf', () => {
  it('answers nothing when the issue stands alone', () => {
    expect(relationOf(issue(), noEdges)).toBeNull()
  })

  it('names the blocker before anything else', () => {
    const edges = {
      blockedBy: new Map([['smetana-a1a', ['holiday-curb-77e1']]]),
      blocking: new Map([['smetana-a1a', ['x-9']]])
    }
    expect(relationOf(issue({ parent: 'smetana-epic' }), edges)).toEqual({
      icon: 'lock',
      label: '77e1'
    })
  })

  it('names the parent when nothing blocks', () => {
    const edges = { blockedBy: new Map(), blocking: new Map([['smetana-a1a', ['x-9']]]) }
    expect(relationOf(issue({ parent: 'holiday-curb-epic' }), edges)).toEqual({
      icon: 'corner-down-right',
      label: 'epic'
    })
  })

  it('counts what waits on it when it has neither a blocker nor a parent', () => {
    const edges = { blockedBy: new Map(), blocking: new Map([['smetana-a1a', ['x-9', 'x-8']]]) }
    expect(relationOf(issue(), edges)).toEqual({ icon: 'git-fork', label: '2' })
  })

  it('reads the store maps rather than the issue counters, so a closed blocker is gone', () => {
    expect(relationOf(issue({ dependent_count: 4 }), noEdges)).toBeNull()
  })
})

describe('sectionLabel', () => {
  it('offers the recent tasks while nothing is typed', () => {
    expect(sectionLabel({ query: '', answered: false })).toBe('Recent')
    expect(sectionLabel({ query: '  ', answered: true })).toBe('Recent')
  })

  it('says by meaning only once an answer has actually landed', () => {
    expect(sectionLabel({ query: 'bell', answered: false })).toBe('Matching text')
    expect(sectionLabel({ query: 'bell', answered: true })).toBe('By meaning')
  })
})

describe('counterLabel', () => {
  /* Both silences are one rule: a counter is drawn only when it says something.
     Nothing in scope has nothing to count, and nothing shown is already said by
     the empty state — the second block competing with the first is the fault
     this design was redrawn to remove. */
  it('says nothing when either end of the fraction is zero', () => {
    expect(counterLabel(0, 128)).toBe('')
    expect(counterLabel(3, 0)).toBe('')
  })

  it('counts the shown against everything in scope', () => {
    expect(counterLabel(3, 128)).toBe('3 of 128')
  })
})

describe('waitingLabel', () => {
  /* The seconds are the whole reason the row exists: against a ninety-second
     deadline they are what tells a long answer from a hung one. */
  it('names the wait and how long it has lasted', () => {
    expect(waitingLabel(0)).toBe('Asking the agent… 0s')
    expect(waitingLabel(12)).toBe('Asking the agent… 12s')
    expect(waitingLabel(89)).toBe('Asking the agent… 89s')
  })

  it('says whole seconds only, since it is read at a glance', () => {
    expect(waitingLabel(3.7)).toBe('Asking the agent… 3s')
  })

  it('reads an impossible elapsed time as nought rather than drawing it', () => {
    expect(waitingLabel(-4)).toBe('Asking the agent… 0s')
    expect(waitingLabel(Number.NaN)).toBe('Asking the agent… 0s')
    expect(waitingLabel(undefined)).toBe('Asking the agent… 0s')
  })
})

describe('stepIndex', () => {
  it('wraps at both ends', () => {
    expect(stepIndex(2, 1, 3)).toBe(0)
    expect(stepIndex(0, -1, 3)).toBe(2)
  })

  it('stays at nothing when there is nothing to step through', () => {
    expect(stepIndex(0, 1, 0)).toBe(0)
  })
})
