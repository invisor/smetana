import { describe, expect, it } from 'vitest'
import {
  LOCAL,
  ORIGIN,
  canReview,
  fetchFailedCaption,
  fetchFailures,
  fetchTargets,
  fetchingCaption,
  localNames,
  refOf,
  reportPath,
  reviewPairs,
  reviewRows,
  withoutCaption
} from '../../../src/components/git/reviewRows.js'

/* `vcsState.repos`' shape, and the names are the ones `[project].repos` gives —
   which is the vocabulary `missing_in` speaks in too. */
const REPOS = [
  { name: '.', path: '/p', branch: 'main', detached: null },
  { name: 'admin', path: '/p/admin', branch: 'main', detached: null },
  { name: 'shared', path: '/p/shared', branch: 'main', detached: null }
]

/* `target_branches`' answer: every local branch of every repository, with the
   repositories each one is short of. */
const BRANCHES = [
  { name: 'main', missing_in: [] },
  { name: 'feat/x', missing_in: ['shared'] }
]

describe('reviewRows', () => {
  it('gives a row to every repository that has the branch', () => {
    const { rows } = reviewRows(REPOS, 'feat/x', { branches: BRANCHES })
    expect(rows.map((r) => r.repo)).toEqual(['/p', '/p/admin'])
    expect(rows.every((r) => r.head === 'feat/x')).toBe(true)
  })

  /* A repository without a branch of that name is not a broken row and not an
     error: it is a fact, said once under the table, and somebody adds it by
     hand with the name the branch goes by there. */
  it('leaves a repository the branch is missing from out of the table and names it', () => {
    const { rows, without } = reviewRows(REPOS, 'feat/x', { branches: BRANCHES })
    expect(rows.map((r) => r.name)).not.toContain('shared')
    expect(without).toEqual(['shared'])
  })

  /* Both sides start local. The choice between a local branch and what origin
     has is the person's, and the local one is what they were looking at. */
  it('starts both sides of every row on the local branch', () => {
    const { rows } = reviewRows(REPOS, 'feat/x', { branches: BRANCHES })
    for (const row of rows) {
      expect(row.baseSide).toBe(LOCAL)
      expect(row.headSide).toBe(LOCAL)
    }
  })

  /* `branchChoice.js`'s existing order, because the run dialog answers this
     same question one screen over and a second order would be a second answer
     to it: what this project was left at, then `[defaults].target_branch`, then
     the top of the list. */
  it('fills the base with what was remembered, then what the project declares', () => {
    const remembered = reviewRows(REPOS, 'feat/x', {
      branches: BRANCHES,
      remembered: 'main',
      configured: 'feat/x'
    })
    expect(remembered.rows[0].base).toBe('main')
    const configured = reviewRows(REPOS, 'feat/x', {
      branches: BRANCHES,
      configured: 'feat/x'
    })
    expect(configured.rows[0].base).toBe('feat/x')
    const top = reviewRows(REPOS, 'feat/x', { branches: BRANCHES })
    expect(top.rows[0].base).toBe('main')
  })

  /* The base somebody had already chosen on the lone row, carried into the
     rebuild that the first branch pick sets off.

     It is the whole of a defect this shipped with for one review pass. The
     `New review` door opens one row with the base filled and the head empty,
     and the table's columns run Repository, Base, To check — so the ordinary
     way through the window is to set the base and then the branch. With
     nothing carried, that first pick rebuilt every row at the default and put
     it back over the base in the instant somebody's eye had moved to the next
     column, and the review then ran against a comparison nobody asked for. It
     rides in as `remembered` because that is `pickBranch`'s first term. */
  it('keeps a base chosen on the lone row when the table is built around it', () => {
    const { rows } = reviewRows(REPOS, 'feat/x', {
      branches: BRANCHES,
      remembered: 'feat/x',
      configured: 'main'
    })
    expect(rows.every((row) => row.base === 'feat/x')).toBe(true)
  })

  /* And a base that has gone since it was chosen falls back rather than
     standing: `pickBranch` skips a remembered name the list no longer holds,
     which is what makes carrying one safe with no check of its own here. */
  it('falls back when the carried base is no longer in the list', () => {
    const { rows } = reviewRows(REPOS, 'feat/x', {
      branches: BRANCHES,
      remembered: 'feat/deleted',
      configured: 'main'
    })
    expect(rows[0].base).toBe('main')
  })

  /* The `New review` door: no name to start from, so one row on the repository
     the Git panel is showing, with the base filled and the checked side empty.
     Picking a name there calls this function again and builds the rest. */
  it('opens on one row with an empty checked side when there is no name', () => {
    const { rows, without } = reviewRows(REPOS, null, {
      branches: BRANCHES,
      selected: '/p/admin'
    })
    expect(rows).toHaveLength(1)
    expect(rows[0].repo).toBe('/p/admin')
    expect(rows[0].base).toBe('main')
    expect(rows[0].head).toBe('')
    expect(without).toEqual([])
  })

  /* A selection that names no repository of this project is not a reason to
     draw nothing: the window is about the project, and the first repository is
     the one the panel would have been showing. */
  it('falls back to the first repository when nothing is selected', () => {
    const { rows } = reviewRows(REPOS, '', { branches: BRANCHES })
    expect(rows.map((r) => r.repo)).toEqual(['/p'])
  })

  /* A name `target_branches` has never heard of is missing from every
     repository it walked. The table is empty and the caption says why, rather
     than the window looking as though it had failed to open. */
  it('gives no rows at all for a branch that is nowhere', () => {
    const { rows, without } = reviewRows(REPOS, 'feat/nowhere', { branches: BRANCHES })
    expect(rows).toEqual([])
    expect(without).toEqual(['.', 'admin', 'shared'])
  })

  it('answers an empty table for no repositories at all', () => {
    expect(reviewRows(null, 'feat/x', { branches: BRANCHES })).toEqual({ rows: [], without: [] })
    expect(reviewRows([], 'feat/x', { branches: BRANCHES })).toEqual({ rows: [], without: [] })
  })

  /* An absent branch list is the same answer as a branch nowhere, and that is
     why `DesktopApp.vue` waits for `target_branches` before it builds a table
     rather than after: a list that has not landed is not evidence that nothing
     has the branch, and this function has no way to tell the two apart. */
  it('reads an absent branch list as a branch no repository has', () => {
    const { rows, without } = reviewRows(REPOS, 'feat/x', { branches: null })
    expect(rows).toEqual([])
    expect(without).toEqual(['.', 'admin', 'shared'])
  })
})

describe('canReview', () => {
  const pair = (over = {}) => ({
    repo: '/p',
    name: '.',
    base: 'main',
    baseSide: LOCAL,
    head: 'feat/x',
    headSide: LOCAL,
    ...over
  })

  it('refuses an empty table', () => {
    expect(canReview([])).toBe(false)
    expect(canReview(null)).toBe(false)
  })

  it('refuses a row with a side nobody has answered', () => {
    expect(canReview([pair(), pair({ head: '' })])).toBe(false)
    expect(canReview([pair({ base: '' })])).toBe(false)
  })

  it('allows a table whose every row has both sides', () => {
    expect(canReview([pair(), pair({ repo: '/p/admin', name: 'admin' })])).toBe(true)
  })
})

describe('refOf', () => {
  it('spells a local branch as itself and a remote one with its remote', () => {
    expect(refOf('main', LOCAL)).toBe('main')
    expect(refOf('main', ORIGIN)).toBe('origin/main')
  })

  it('has nothing to spell for an unanswered side', () => {
    expect(refOf('', ORIGIN)).toBe('')
  })
})

describe('reviewPairs', () => {
  it('resolves every side into the ref git takes', () => {
    const rows = [
      { repo: '/p', base: 'main', baseSide: LOCAL, head: 'feat/x', headSide: ORIGIN },
      { repo: '/p/admin', base: 'develop', baseSide: ORIGIN, head: 'feat/x', headSide: LOCAL }
    ]
    expect(reviewPairs(rows)).toEqual([
      { repo: '/p', base: 'main', head: 'origin/feat/x' },
      { repo: '/p/admin', base: 'origin/develop', head: 'feat/x' }
    ])
  })
})

describe('fetchTargets', () => {
  it('names nothing when every side is local', () => {
    expect(
      fetchTargets([{ repo: '/p', baseSide: LOCAL, headSide: LOCAL }])
    ).toEqual([])
  })

  /* `origin/main` is only as current as the last fetch, so a review that reads
     it without one is a review of a commit nobody asked about — and nothing on
     screen would have said so. */
  it('names a repository with origin on either side, once', () => {
    expect(
      fetchTargets([
        { repo: '/p', baseSide: ORIGIN, headSide: LOCAL },
        { repo: '/p', baseSide: LOCAL, headSide: ORIGIN },
        { repo: '/p/admin', baseSide: LOCAL, headSide: LOCAL }
      ])
    ).toEqual(['/p'])
  })
})

describe('fetchFailures', () => {
  /* The verdicts arrive as `Promise.all` left them — one per target, in the
     order the targets were fetched in — and the answer is the paths, because
     that is what a row carries, what a pair names a repository by and what the
     prompt lists them in. The window's names are this list rendered, never a
     second walk of the same array. */
  it('keeps the targets whose fetch did not answer, in order', () => {
    expect(fetchFailures(['/p', '/p/admin', '/p/shared'], [true, false, false])).toEqual([
      '/p/admin',
      '/p/shared'
    ])
  })

  it('answers nothing when every fetch worked', () => {
    expect(fetchFailures(['/p', '/p/admin'], [true, true])).toEqual([])
    expect(fetchFailures([], [])).toEqual([])
  })

  /* A verdict that is missing is not a fetch that worked: a shorter list than
     the targets it is about can only mean something went wrong on the way, and
     reading the hole as a success is the one way this feature goes quiet in
     exactly the case it exists for. */
  it('reads a missing verdict as a repository that was not reached', () => {
    expect(fetchFailures(['/p', '/p/admin'], [true])).toEqual(['/p/admin'])
    expect(fetchFailures(['/p'], null)).toEqual(['/p'])
  })
})

describe('reportPath', () => {
  const AT = new Date(2026, 7, 31, 13, 45)

  it('puts the date, the minute and the branch under .smetana/reviews', () => {
    expect(reportPath('feature/x', AT)).toBe('.smetana/reviews/2026-08-31-1345-feature-x')
  })

  /* It lands in a path on somebody's disk: a slash would be a directory nobody
     asked for, and a space or a hash is a filename an OS argues about.

     The two non-Latin strings below are the **input under test** and not prose:
     this repository is written in English throughout, and the one thing that
     rule cannot reach is a fixture whose whole subject is what happens to an
     alphabet the slug has no letters for. A branch named in Cyrillic is an
     ordinary branch on somebody's machine, and what it must not become is a
     path. */
  it('reduces a name to lower-case letters, digits and hyphens', () => {
    expect(reportPath('Feature/Проверка #1 — x', AT)).toBe('.smetana/reviews/2026-08-31-1345-feature-1-x')
    expect(reportPath('  feat//x  ', AT)).toBe('.smetana/reviews/2026-08-31-1345-feat-x')
  })

  /* A name that reduces to nothing at all still has a tail: a path ending in
     the minute would be a file named after a clock. */
  it('calls it a review when the name reduces to nothing', () => {
    expect(reportPath('', AT)).toBe('.smetana/reviews/2026-08-31-1345-review')
    expect(reportPath('Проверка', AT)).toBe('.smetana/reviews/2026-08-31-1345-review')
    expect(reportPath(null, AT)).toBe('.smetana/reviews/2026-08-31-1345-review')
  })

  it('pads a single-digit month, day, hour and minute', () => {
    expect(reportPath('x', new Date(2026, 0, 2, 3, 4))).toBe('.smetana/reviews/2026-01-02-0304-x')
  })
})

describe('the captions under the table', () => {
  /* One sentence for all of them and never a row each: a repository without a
     branch of that name is a fact, not a broken row. */
  it('names the repositories the branch is missing from in one line', () => {
    expect(withoutCaption(['shared'], 'feat/x')).toBe(
      'shared does not have a branch called feat/x'
    )
    expect(withoutCaption(['shared', 'infra'], 'feat/x')).toBe(
      'shared, infra do not have a branch called feat/x'
    )
  })

  it('says nothing when nothing is missing and nothing when there is no name', () => {
    expect(withoutCaption([], 'feat/x')).toBe('')
    expect(withoutCaption(null, 'feat/x')).toBe('')
    expect(withoutCaption(['shared'], '')).toBe('')
  })

  it('counts the repositories it is fetching for', () => {
    expect(fetchingCaption([])).toBe('')
    expect(fetchingCaption(['/p'])).toBe('Fetching origin for 1 repository…')
    expect(fetchingCaption(['/p', '/p/admin'])).toBe('Fetching origin for 2 repositories…')
  })

  /* A failed fetch is a sentence and not a refusal: what origin holds on this
     machine is still readable, merely older. */
  it('says which repositories were not reached, without calling it off', () => {
    expect(fetchFailedCaption([])).toBe('')
    expect(fetchFailedCaption(['admin'])).toBe(
      'admin was not reached — the review reads what origin was last known to have'
    )
    expect(fetchFailedCaption(['admin', 'shared'])).toBe(
      'admin, shared were not reached — the review reads what origin was last known to have'
    )
  })
})

describe('localNames', () => {
  /* One project-wide answer filtered per row, rather than a second git read for
     every repository in the table. */
  it('leaves out the branches this repository is short of', () => {
    expect(localNames(BRANCHES, '.')).toEqual(['main', 'feat/x'])
    expect(localNames(BRANCHES, 'shared')).toEqual(['main'])
  })

  it('has nothing to offer before the list has landed', () => {
    expect(localNames(null, '.')).toEqual([])
  })
})
