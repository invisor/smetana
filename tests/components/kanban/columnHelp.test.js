import { describe, expect, it } from 'vitest'
import { columnHelp, COLUMN_HELP_DELAY } from '../../../src/components/kanban/columnHelp.js'

/* The sentences, written out here a second time on purpose.

   These are UI copy authored in the issue's own Design section, and this module
   is the only part of the column tooltip a test can reach at all — no test in
   this repository renders a component, so nothing else would notice a sentence
   quietly reworded, a dropped clause or a straightened dash. Pinning them by
   length or by being distinct from each other, which is what this file did
   first, passes every one of those. */
const SAID = {
  ready: 'Ready to start — nothing unfinished is holding it up. A run takes its batch from here.',
  running: 'Being worked on right now: an agent has claimed it and is on it.',
  blocked:
    'Waiting on another task: something it depends on is not finished. When that one is done, the card moves to Ready on its own.',
  done: 'Finished and closed. Nothing left to do here.',
  deferred:
    'Put off on purpose, with nothing holding it up. Findings that turned up outside their own task land here. A run never picks one up — only a person moves it back to Ready.',
  pinned:
    'A standing item that is never closed. It stays out of the queue and holds nothing else up.',
  hooked:
    'An agent has taken a whole group of related tasks at once. It says who owns the work, not how far it has got, and a run leaves these alone.',
  parked:
    "A run stopped here on a question it could not answer itself. The question is in the task's notes; Answer questions starts an agent that puts it to you and returns the task to the queue.",
  'ready-to-merge':
    'Done and reviewed, waiting to be merged into the target branch. It closes once it lands there.',
  'human-check':
    'Done and merged, waiting for someone to look at it by hand. A run leaves one of these behind when it could not check the work itself; you go through it, then close it or send it back to Ready.'
}

const UNKNOWN =
  'A status this app knows nothing about. Tasks are here because that is the status they carry.'

describe('columnHelp', () => {
  for (const [status, sentence] of Object.entries(SAID)) {
    it(`says what ${status} is, word for word`, () => {
      expect(columnHelp(status)).toBe(sentence)
    })
  }

  it('reads bd’s spelling and the board’s as one status', () => {
    // `normalizeStatus` collapses every run of non-alphanumerics to a dash, so
    // the custom status as bd stores it and as the design system writes it are
    // the same entry rather than two that have to be kept in step.
    expect(columnHelp('ready_to_merge')).toBe(SAID['ready-to-merge'])
    expect(columnHelp('Ready To Merge')).toBe(SAID['ready-to-merge'])
    expect(columnHelp('human_check')).toBe(SAID['human-check'])
  })

  it('does not teach that a parent task is what blocks', () => {
    // Only bd's `blocks` dependency holds a card in the Blocked column;
    // `parent-child` does not, so the sentence names neither parents nor bd.
    expect(SAID.blocked.toLowerCase()).not.toContain('parent')
    expect(SAID.blocked.toLowerCase()).not.toContain('bd')
  })

  it('gives a status it has never heard of a sentence rather than silence', () => {
    expect(columnHelp('needs-triage')).toBe(UNKNOWN)
    expect(columnHelp('')).toBe(UNKNOWN)
    expect(columnHelp(undefined)).toBe(UNKNOWN)
    // Attention levels are not statuses a task is filed under, and no column
    // carries either name — so they get the same sentence as anything else
    // this table does not hold.
    expect(columnHelp('needs-you')).toBe(UNKNOWN)
    expect(columnHelp('failed')).toBe(UNKNOWN)
  })

  it('gives every column its own sentence', () => {
    // A copied line left unedited would read plausibly on screen and say the
    // wrong thing about one of the two columns.
    const said = Object.keys(SAID).map((status) => columnHelp(status))
    expect(new Set(said).size).toBe(said.length)
    expect(said).not.toContain(UNKNOWN)
  })

  it('waits two seconds', () => {
    expect(COLUMN_HELP_DELAY).toBe(2000)
  })
})
