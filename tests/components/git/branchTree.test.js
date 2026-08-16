import { describe, expect, it } from 'vitest'
import {
  branchRows,
  currentChain,
  expandedFolders,
  toggleFolder
} from '../../../src/components/git/branchTree.js'

/* `vcs_branches`' own shape, in `git::by_recency`'s order — what was worked on
   most recently first, which is the order the panel draws and the order this
   rule has to leave alone. */
const branches = (...names) =>
  names.map((name) => ({ name: name.replace(/^\*/, ''), current: name.startsWith('*') }))

const labels = (rows) => rows.map((row) => `${row.kind === 'folder' ? '/' : ''}${row.label}`)

describe('what a list of branch names becomes', () => {
  /* A name with no slash in it is not in a folder and never becomes one. Half
     the rows in any repository are these, and burying `main` under a heading
     would be the whole feature backfiring. */
  it('leaves a name with no slash where it is', () => {
    const rows = branchRows(branches('*main', 'develop'), [])
    expect(labels(rows)).toEqual(['main', 'develop'])
    expect(rows.every((row) => row.kind === 'branch')).toBe(true)
    expect(rows.every((row) => row.depth === 0)).toBe(true)
  })

  /* The order is the load-bearing part. `BranchList` opens by saying that the
     branch somebody merges into every day is nowhere in particular
     alphabetically, so re-sorting would bury the one row that matters — and
     grouping is a re-sort unless a folder inherits the position of the most
     recent branch under it. */
  it('puts a folder where its most recent branch was', () => {
    const rows = branchRows(
      branches('feature/one', '*main', 'fix/two', 'develop', 'feature/three'),
      ['feature', 'fix']
    )
    expect(labels(rows)).toEqual([
      '/feature',
      'one',
      'three',
      'main',
      '/fix',
      'two',
      'develop'
    ])
  })

  /* Recency inside the folder as well as outside it: the branches under a
     heading arrive in the order git gave them and are not touched either. */
  it('keeps the branches inside a folder in the order they arrived', () => {
    const rows = branchRows(branches('feature/b', 'feature/a', 'feature/c'), ['feature'])
    expect(labels(rows).slice(1)).toEqual(['b', 'a', 'c'])
  })

  /* Every slash is a folder, not only the first. `fix/legacy/…` is a name this
     tree already carries, and a rule that split once would draw a folder called
     `fix` holding a branch still called `legacy/warehouse-geocode`. */
  it('nests as deeply as the name does', () => {
    const rows = branchRows(branches('fix/legacy/geocode'), ['fix', 'fix/legacy'])
    expect(rows.map((row) => [row.kind, row.label, row.depth])).toEqual([
      ['folder', 'fix', 0],
      ['folder', 'legacy', 1],
      ['branch', 'geocode', 2]
    ])
    expect(rows[1].path).toBe('fix/legacy')
  })

  /* What the count on a folder means: every branch beneath it, however deep,
     because that is what a person is deciding whether to unfold. Counting only
     the immediate children would say `1` over a heading hiding four. */
  it('counts every branch beneath a folder, at any depth', () => {
    const rows = branchRows(branches('fix/legacy/a', 'fix/legacy/b', 'fix/c'), ['fix'])
    const [outer, inner] = rows
    expect([outer.path, outer.count]).toEqual(['fix', 3])
    expect([inner.path, inner.count]).toEqual(['fix/legacy', 2])
  })

  /* Folded is absent rather than hidden: the rows are what the panel draws, and
     what says the branches are still there is the count on the folder. This is
     also the whole of the height the feature buys back. */
  it('leaves the branches out of a folded folder altogether', () => {
    const rows = branchRows(branches('feature/one', 'feature/two', '*main'), [])
    expect(labels(rows)).toEqual(['/feature', 'main'])
    expect(rows[0]).toMatchObject({ count: 2, expanded: false })
  })

  /* The leaf is what is drawn and the whole name is what git is given. Losing
     the second would offer a checkout of a branch that does not exist, so both
     travel on the row. Everything else the branch arrived with travels too —
     `current` today, whatever `vcs_branches` grows next. */
  it('carries the whole name beside the leaf it draws', () => {
    const [row] = branchRows(
      [{ name: 'feature/holiday-curb-y5bt.8-drop-depot-columns', current: true }],
      ['feature']
    ).slice(1)
    expect(row.name).toBe('feature/holiday-curb-y5bt.8-drop-depot-columns')
    expect(row.label).toBe('holiday-curb-y5bt.8-drop-depot-columns')
    expect(row.current).toBe(true)
  })

  /* A folder that is open somewhere else in the tree does not open this one:
     the whole path is the key, or `fix/legacy` and `feature/legacy` would fold
     and unfold together. */
  it('keys a folder by its whole path', () => {
    const rows = branchRows(branches('fix/legacy/a', 'feature/legacy/b'), ['fix', 'feature'])
    expect(rows.filter((row) => row.kind === 'folder').map((row) => row.expanded)).toEqual([
      true,
      false,
      true,
      false
    ])
  })
})

describe('names git would not produce and a person might', () => {
  /* An empty segment is not a folder. `feature//one` is one slash worth of
     typing away from a name in this list, and a folder with no name at all
     would draw a heading nobody could point at. */
  it('passes over empty segments', () => {
    const rows = branchRows(branches('feature//one', '/main', 'release/'), ['feature'])
    expect(labels(rows)).toEqual(['/feature', 'one', 'main', 'release'])
  })

  /* git refuses to hold both `feature` and `feature/one` — the first is a file
     where the second needs a directory — so this is a list nothing here can
     produce. It still must not throw: what arrives is whatever git said, and a
     rule that falls over takes the whole panel with it. */
  it('draws a branch named like a folder as a branch', () => {
    const rows = branchRows(branches('feature', 'feature/one'), ['feature'])
    expect(labels(rows)).toEqual(['feature', '/feature', 'one'])
  })

  it('answers with nothing for nothing', () => {
    expect(branchRows([], [])).toEqual([])
    expect(branchRows(null, null)).toEqual([])
    expect(branchRows(branches(''), [])).toEqual([])
  })
})

describe('the folders of the branch a repository is on', () => {
  /* Outermost first, and every step of the way: unfolding `fix/legacy` while
     `fix` stays folded would put the current branch inside a heading that is
     not on screen. */
  it('is the whole chain, outermost first', () => {
    expect(currentChain(branches('main', '*fix/legacy/geocode'))).toEqual(['fix', 'fix/legacy'])
  })

  it('is empty for a branch in no folder, and for no current branch', () => {
    expect(currentChain(branches('*main', 'feature/one'))).toEqual([])
    expect(currentChain(branches('main', 'feature/one'))).toEqual([])
    expect(currentChain(null)).toEqual([])
  })
})

describe('never chosen against chosen to be empty', () => {
  /* `null` is a state and not a missing value, the same distinction the section
     heights keep one file over. Without it there is no way to fold the last
     folder away: the empty list would read as "nobody has chosen" and the
     current branch's folder would be back open on the next start. */
  it('opens the current branch folder until somebody chooses', () => {
    expect(expandedFolders(null, branches('*feature/one', 'main'))).toEqual(['feature'])
    expect(expandedFolders([], branches('*feature/one', 'main'))).toEqual([])
    expect(expandedFolders(['fix'], branches('*feature/one'))).toEqual(['fix'])
  })
})

describe('what a press on a folder leaves behind', () => {
  const list = branches('*feature/one', 'fix/two', 'main')

  it('opens a folded folder and folds an open one', () => {
    expect(toggleFolder(['feature'], list, 'fix')).toEqual(['feature', 'fix'])
    expect(toggleFolder(['feature', 'fix'], list, 'feature')).toEqual(['fix'])
  })

  /* The first press lands on a list nobody has written yet, so it resolves the
     seed first and writes the answer whole. Folding the seeded folder is how a
     person reaches the empty list, which is the one state that says "all of
     them, folded, on purpose". */
  it('writes the seed out on the first press', () => {
    expect(toggleFolder(null, list, 'fix')).toEqual(['feature', 'fix'])
    expect(toggleFolder(null, list, 'feature')).toEqual([])
  })

  /* A fresh array every time: the caller assigns it into `settings.json`, and
     mutating the list in place would leave the store's own watcher with nothing
     to notice. */
  it('answers with a new list rather than the one it was given', () => {
    const stored = ['feature']
    expect(toggleFolder(stored, list, 'fix')).not.toBe(stored)
    expect(stored).toEqual(['feature'])
  })
})
