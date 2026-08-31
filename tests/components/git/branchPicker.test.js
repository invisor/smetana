import { describe, expect, it } from 'vitest'
import {
  BRANCH_FILTER_LABEL,
  PICKER_KEY_HINT,
  branchCountLabel,
  branchMeta,
  fetchedLabel,
  matchingBranches,
  pickerRows,
  repoCountLabel,
  shortAge,
  stepCursor
} from '../../../src/components/git/branchPicker.js'

/* The whole of what the branch picker's list holds, which is the whole of why
   this module exists: `BranchPicker.vue` is a `.vue` file and no test in this
   repository can reach one. */

/* Epoch seconds, which is the unit everything in this module speaks. */
const NOW = 1_780_000_000
const MINUTE = 60
const HOUR = 60 * MINUTE
const DAY = 24 * HOUR

/* `at` is the field's name in the epic's contract — the branch's own last
   touch, in epoch seconds, null where git could not say. Deliberately not
   `updated_at`, which this front end already reads off a bd issue as an ISO
   string; a helper that invented its own name here would have defined the
   contract into existence and passed while the component drew nothing. */
const branch = (name, missing = [], at = null) => ({
  name,
  missing_in: missing,
  ...(at === null ? {} : { at })
})

describe('how old something is', () => {
  it('counts in the largest unit that still says something', () => {
    expect(shortAge(NOW - 2 * HOUR, NOW)).toBe('2h')
    expect(shortAge(NOW - 3 * DAY, NOW)).toBe('3d')
    expect(shortAge(NOW - 5 * MINUTE, NOW)).toBe('5m')
    expect(shortAge(NOW - 20 * DAY, NOW)).toBe('2w')
    expect(shortAge(NOW - 800 * DAY, NOW)).toBe('2y')
  })

  it('says now for anything under a minute', () => {
    expect(shortAge(NOW - 3, NOW)).toBe('now')
    expect(shortAge(NOW, NOW)).toBe('now')
  })

  /* Two clocks a minute apart is ordinary; `-1m` in a list of branches is not. */
  it('clamps a time in the future rather than counting backwards', () => {
    expect(shortAge(NOW + 5 * MINUTE, NOW)).toBe('now')
  })

  /* The whole point of the guard: a branch list older than the field that
     carries the time must not draw `NaN` on every row. */
  it('answers nothing at all for a time it was never given', () => {
    expect(shortAge(undefined, NOW)).toBe(null)
    expect(shortAge(null, NOW)).toBe(null)
    expect(shortAge('yesterday', NOW)).toBe(null)
    expect(shortAge(NaN, NOW)).toBe(null)
    expect(shortAge(NOW - HOUR, undefined)).toBe(null)
  })
})

describe('when origin was last asked', () => {
  it('keeps its ago, because it is a moment and not a duration', () => {
    expect(fetchedLabel(NOW - 2 * MINUTE, NOW)).toBe('fetched 2m ago')
    expect(fetchedLabel(NOW - 6 * HOUR, NOW)).toBe('fetched 6h ago')
  })

  it('says just now in words rather than composing fetched now ago', () => {
    expect(fetchedLabel(NOW - 10, NOW)).toBe('fetched just now')
  })

  it('a repository nobody has fetched into has no fetch to report', () => {
    expect(fetchedLabel(null, NOW)).toBe(null)
    expect(fetchedLabel(undefined, NOW)).toBe(null)
  })
})

describe('how much of the project has a branch', () => {
  it('is the project count less the repositories it is missing from', () => {
    expect(repoCountLabel(branch('main'), 6)).toBe('6 repos')
    expect(repoCountLabel(branch('release/7', ['admin', 'extension']), 6)).toBe('4 repos')
  })

  it('follows the count with the noun', () => {
    expect(repoCountLabel(branch('spike/auth', ['admin', 'extension']), 3)).toBe('1 repo')
  })

  /* A branch no repository has is an answer `reviewRows.js` already draws under
     its table; here it is a zero rather than a negative number. */
  it('never counts below nothing', () => {
    expect(repoCountLabel(branch('gone', ['a', 'b', 'c']), 2)).toBe('0 repos')
  })

  it('says nothing when the project count is not known', () => {
    expect(repoCountLabel(branch('main'), 0)).toBe(null)
    expect(repoCountLabel(branch('main'), undefined)).toBe(null)
  })

  it('a branch with no missing_in at all is in every repository', () => {
    expect(repoCountLabel({ name: 'main' }, 4)).toBe('4 repos')
  })
})

describe('what one row says about itself', () => {
  it('names the side, how much of the project has it, and its age', () => {
    const meta = branchMeta(branch('main', [], NOW - 2 * HOUR), { repos: 6, now: NOW })

    expect(meta).toBe('local · 6 repos · 2h')
  })

  it('names origin and the last fetch on the other side of the pair', () => {
    const meta = branchMeta(branch('main'), {
      origin: true,
      repos: 6,
      now: NOW,
      fetchedAt: NOW - 2 * MINUTE
    })

    expect(meta).toBe('origin · fetched 2m ago')
  })

  /* The acceptance criterion this module was written for: a missing label
     shortens the line and never leaves a hole in it. */
  it('a branch with no timestamp is short by a piece and no more', () => {
    expect(branchMeta(branch('main'), { repos: 6, now: NOW })).toBe('local · 6 repos')
    expect(branchMeta(branch('main'), { origin: true, now: NOW })).toBe('origin')
  })

  it('never draws NaN, Invalid Date or an empty piece', () => {
    const meta = branchMeta(
      { name: 'main', missing_in: null, at: 'lately' },
      { origin: false, repos: null, now: null, fetchedAt: 'never' }
    )

    expect(meta).toBe('local')
    expect(meta).not.toMatch(/NaN|Invalid|undefined|null/)
  })

  /* Written against a literal object rather than the helper above, which is the
     whole value of it: the helper spells the field itself, so every other test
     in this file would go on passing with the field renamed and the component
     drawing no age at all. */
  it('reads the branch time off at, which is the name the contract gives it', () => {
    expect(branchMeta({ name: 'main', at: NOW - 2 * HOUR }, { now: NOW })).toBe('local · 2h')
  })

  /* The trap the name avoids: `updated_at` is a bd issue's field in this front
     end and an ISO string there. A branch carrying one is a branch this module
     knows nothing about the age of, and it says so by leaving the piece out. */
  it('does not read a bd issue field that happens to be nearby', () => {
    expect(branchMeta({ name: 'main', updated_at: NOW - 2 * HOUR }, { now: NOW })).toBe('local')
  })

  it('says the side even when it knows nothing else at all', () => {
    expect(branchMeta(undefined, {})).toBe('local')
    expect(branchMeta(undefined, { origin: true })).toBe('origin')
  })
})

describe('the filter', () => {
  const all = [branch('main'), branch('Develop'), branch('feature/smetana-4nsa-remote-branches-repo')]

  it('matches a substring anywhere in the name, and not only a prefix', () => {
    expect(matchingBranches(all, '4nsa').map((b) => b.name)).toEqual([
      'feature/smetana-4nsa-remote-branches-repo'
    ])
  })

  it('ignores case on both sides', () => {
    expect(matchingBranches(all, 'DEVELOP').map((b) => b.name)).toEqual(['Develop'])
    expect(matchingBranches(all, 'main').map((b) => b.name)).toEqual(['main'])
  })

  it('an empty needle is no filter at all, spaces included', () => {
    expect(matchingBranches(all, '').length).toBe(3)
    expect(matchingBranches(all, '   ').length).toBe(3)
    expect(matchingBranches(all, undefined).length).toBe(3)
  })

  it('trims what was typed, so a pasted trailing space still matches', () => {
    expect(matchingBranches(all, ' main ').map((b) => b.name)).toEqual(['main'])
  })

  /* `origin/` is a fact about the row rather than part of a branch's name, so
     typing it must not empty the list — that would be the second control this
     component exists to remove, wearing a disguise. */
  it('does not read the origin prefix as part of a name', () => {
    expect(matchingBranches(all, 'origin')).toEqual([])
  })

  it('keeps nothing that has no name to match', () => {
    expect(matchingBranches([{ missing_in: [] }, branch('main'), null], '').map((b) => b.name)).toEqual([
      'main'
    ])
  })

  it('anything that is not a list is an empty one', () => {
    expect(matchingBranches(null, '')).toEqual([])
    expect(matchingBranches(undefined, 'main')).toEqual([])
  })
})

describe('the list itself', () => {
  const all = [branch('main', [], NOW - 2 * HOUR), branch('develop', ['admin'], NOW - 3 * DAY)]

  it('draws the local row and then the origin row of every branch', () => {
    const rows = pickerRows(all, { repos: 6, now: NOW, fetchedAt: NOW - 2 * MINUTE })

    expect(rows.map((row) => `${row.origin ? 'origin/' : ''}${row.name}`)).toEqual([
      'main',
      'origin/main',
      'develop',
      'origin/develop'
    ])
  })

  it('keeps the order the branches arrived in, which is by recency', () => {
    const rows = pickerRows([branch('zeta'), branch('alpha')], {})

    expect(rows.map((row) => row.name)).toEqual(['zeta', 'zeta', 'alpha', 'alpha'])
  })

  /* The two rows of a pair carry the same name, so the side has to be part of
     the key or Vue reuses one row for the other side of the same branch. */
  it('gives the two rows of a pair different keys', () => {
    const [local, origin] = pickerRows([branch('main')], {})

    expect(local.key).not.toBe(origin.key)
    expect(local.origin).toBe(false)
    expect(origin.origin).toBe(true)
  })

  it('carries the meta line each side of the pair earns', () => {
    const rows = pickerRows(all, { repos: 6, now: NOW, fetchedAt: NOW - 2 * MINUTE })

    expect(rows[0].meta).toBe('local · 6 repos · 2h')
    expect(rows[1].meta).toBe('origin · fetched 2m ago')
    expect(rows[2].meta).toBe('local · 5 repos · 3d')
  })

  it('filters on the way in, both rows of a branch at a time', () => {
    const rows = pickerRows(all, { query: 'dev', repos: 6, now: NOW })

    expect(rows.map((row) => row.key)).toEqual(['local:develop', 'origin:develop'])
  })

  it('a filter that matches nothing is an empty list and not a broken one', () => {
    expect(pickerRows(all, { query: 'nothing-is-called-this' })).toEqual([])
  })
})

describe('the counter', () => {
  it('says how many of how many', () => {
    expect(branchCountLabel(4, 41)).toBe('4 of 41')
    expect(branchCountLabel(41, 41)).toBe('41 of 41')
  })

  it('a project with no branches says so rather than dividing by nothing', () => {
    expect(branchCountLabel(0, 0)).toBe('0 of 0')
  })

  it('never claims more matched than there are', () => {
    expect(branchCountLabel(9, 3)).toBe('3 of 3')
  })

  it('a count it was never given is nothing rather than NaN', () => {
    expect(branchCountLabel(undefined, undefined)).toBe('0 of 0')
    expect(branchCountLabel(null, 5)).toBe('0 of 5')
  })
})

describe('walking the list with the arrows', () => {
  it('moves one row at a time', () => {
    expect(stepCursor(0, 1, 4)).toBe(1)
    expect(stepCursor(2, -1, 4)).toBe(1)
  })

  it('wraps at both ends, so the arrows always move', () => {
    expect(stepCursor(3, 1, 4)).toBe(0)
    expect(stepCursor(0, -1, 4)).toBe(3)
  })

  it('an empty list stops rather than spinning', () => {
    expect(stepCursor(0, 1, 0)).toBe(0)
    expect(stepCursor(2, -1, 0)).toBe(0)
  })

  it('a cursor left past the end of a shorter list comes back inside it', () => {
    expect(stepCursor(9, 1, 4)).toBe(2)
    expect(stepCursor(9, -1, 4)).toBe(0)
  })
})

describe('the words the component draws', () => {
  it('are sentence case, as everything in this system is', () => {
    expect(BRANCH_FILTER_LABEL).toBe('Filter branches')
  })

  it('name the three keys the list answers', () => {
    expect(PICKER_KEY_HINT).toContain('enter select')
    expect(PICKER_KEY_HINT).toContain('esc close')
  })
})
