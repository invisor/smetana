import { describe, expect, it } from 'vitest'
import { columnHelp, COLUMN_HELP_DELAY } from '../../../src/components/kanban/columnHelp.js'

describe('columnHelp', () => {
  it('answers for every column the board actually draws', () => {
    // The built-in statuses the tracker maps onto this board, plus the two the
    // projects using it add themselves. Each one has to say something about the
    // tasks in it, since a header explains nothing on its own.
    const drawn = [
      'ready',
      'running',
      'blocked',
      'done',
      'deferred',
      'pinned',
      'hooked',
      'parked',
      'ready-to-merge',
      'human-check'
    ]
    const said = drawn.map((status) => columnHelp(status))
    for (const sentence of said) {
      expect(sentence.length).toBeGreaterThan(0)
    }
    // No two columns share a sentence, and none of them fell through to the
    // phrase for a status nothing knows about.
    expect(new Set(said).size).toBe(drawn.length)
    expect(said).not.toContain(columnHelp('a status invented this morning'))
  })

  it('says what Ready is for, word for word', () => {
    expect(columnHelp('ready')).toBe(
      'Ready to start — nothing unfinished is holding it up. A run takes its batch from here.'
    )
  })

  it('reads bd’s spelling and the board’s as one status', () => {
    // `normalizeStatus` collapses every run of non-alphanumerics to a dash, so
    // the custom status as bd stores it and as the design system writes it are
    // the same entry rather than two that have to be kept in step.
    expect(columnHelp('ready_to_merge')).toBe(columnHelp('ready-to-merge'))
    expect(columnHelp('Ready To Merge')).toBe(columnHelp('ready-to-merge'))
    expect(columnHelp('human_check')).toBe(columnHelp('human-check'))
  })

  it('does not teach that a parent task is what blocks', () => {
    // Only bd's `blocks` dependency holds a card in the Blocked column;
    // `parent-child` does not, so the sentence names neither parents nor bd.
    const blocked = columnHelp('blocked')
    expect(blocked).toContain('something it depends on is not finished')
    expect(blocked.toLowerCase()).not.toContain('parent')
  })

  it('gives a status it has never heard of a sentence rather than silence', () => {
    const unknown = 'A status this app knows nothing about. Tasks are here because that is the status they carry.'
    expect(columnHelp('needs-triage')).toBe(unknown)
    expect(columnHelp('')).toBe(unknown)
    expect(columnHelp(undefined)).toBe(unknown)
    // Attention levels are not statuses a task is filed under, and no column
    // carries either name — so they get the same sentence as anything else
    // this table does not hold.
    expect(columnHelp('needs-you')).toBe(unknown)
    expect(columnHelp('failed')).toBe(unknown)
  })

  it('waits two seconds', () => {
    expect(COLUMN_HELP_DELAY).toBe(2000)
  })
})
