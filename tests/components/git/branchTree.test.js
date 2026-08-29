import { describe, expect, it } from 'vitest'
import {
  branchRows,
  currentChain,
  expandedFolders,
  liftedOut,
  toggleFavorite,
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
      branches('feature/one', 'main', 'fix/two', '*develop', 'feature/three'),
      ['feature', 'fix']
    )
    expect(labels(rows)).toEqual([
      'develop',
      '/feature',
      'one',
      'three',
      'main',
      '/fix',
      'two'
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
    expect(labels(rows)).toEqual(['main', '/feature'])
    expect(rows[1]).toMatchObject({ count: 2, expanded: false })
  })

  /* The leaf is what is drawn and the whole name is what git is given. Losing
     the second would offer a checkout of a branch that does not exist, so both
     travel on the row. Everything else the branch arrived with travels too —
     whatever `vcs_branches` grows next, stood in for here by a field it does
     not have. */
  it('carries the whole name beside the leaf it draws', () => {
    const [row] = branchRows(
      [
        { name: 'main', current: true },
        { name: 'feature/holiday-curb-y5bt.8-drop-depot-columns', tracked: true }
      ],
      ['feature']
    ).slice(2)
    expect(row.name).toBe('feature/holiday-curb-y5bt.8-drop-depot-columns')
    expect(row.label).toBe('holiday-curb-y5bt.8-drop-depot-columns')
    expect(row.tracked).toBe(true)
  })

  /* The whole point of the exercise: the branch the repository is on is the
     first row whatever the reflog said and whatever fold its name would put it
     behind. Its label is the whole name, because there is no heading above it
     carrying the prefix. */
  it('draws the current branch first, whole name and all', () => {
    const rows = branchRows(branches('fix/two', 'develop', '*feature/one', 'main'), [])
    expect(labels(rows)).toEqual(['feature/one', '/fix', 'develop', 'main'])
    expect(rows[0]).toMatchObject({
      name: 'feature/one',
      label: 'feature/one',
      depth: 0,
      pinned: true,
      current: true
    })
  })

  /* Lifted out rather than copied out: an unfolded heading holding the row
     that is already at the top would be the same branch on screen twice, and a
     checkout pressed on either of them the same act. */
  it('leaves the current branch out of the tree it was lifted from', () => {
    const rows = branchRows(branches('*feature/one', 'feature/two'), ['feature'])
    expect(labels(rows)).toEqual(['feature/one', '/feature', 'two'])
    expect(rows[1].count).toBe(1)
  })

  /* And a heading with nothing left under it is not drawn at all, rather than
     drawn empty: the count is what a heading is for. */
  it('drops a folder the current branch was the whole of', () => {
    expect(labels(branchRows(branches('*feature/one', 'main'), ['feature']))).toEqual([
      'feature/one',
      'main'
    ])
  })

  /* The hairline goes under the last row of the block above the list, and with
     nothing marked that is still the current branch — which is what the rule
     was before favourites existed. */
  it('rules off under the current branch when nothing is marked', () => {
    const rows = branchRows(branches('*main', 'develop'), [])
    expect(rows[0].divider).toBe(true)
    expect(rows[1].divider).toBeUndefined()
  })

  /* A repository nobody is standing in — a detached HEAD, or a list that
     arrived before HEAD did — pins nothing and draws the tree it always drew. */
  it('pins nothing when no branch is current', () => {
    expect(labels(branchRows(branches('feature/one', 'main'), ['feature']))).toEqual([
      '/feature',
      'one',
      'main'
    ])
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

describe('the branches somebody pinned', () => {
  /* Three groups: the current branch, then the marked ones, then the tree. Both
     of the first two draw their whole name at depth 0, because there is no
     heading above either of them to carry a prefix. */
  it('draws the marked branches under the current one and above the tree', () => {
    const rows = branchRows(
      branches('feature/one', '*main', 'fix/two', 'develop'),
      ['feature', 'fix'],
      ['fix/two', 'develop']
    )
    expect(labels(rows)).toEqual(['main', 'fix/two', 'develop', '/feature', 'one'])
    expect(rows[1]).toMatchObject({ name: 'fix/two', depth: 0, pinned: true, favorite: true })
    expect(rows[2]).toMatchObject({ name: 'develop', depth: 0, pinned: true, favorite: true })
  })

  /* The order inside the group is the order the list arrived in — `by_recency`'s
     — and never the order they were marked in. A second ordering inside one
     list would be invisible, since nothing on a row says when it was pinned. */
  it('keeps the marked branches in the order the list arrived in', () => {
    const rows = branchRows(
      branches('*main', 'one', 'two', 'three'),
      [],
      /* Marked in the opposite order to the one they arrive in. */
      ['three', 'two', 'one']
    )
    expect(labels(rows)).toEqual(['main', 'one', 'two', 'three'])
  })

  /* Lifted rather than copied, exactly as the current branch is: it is gone
     from its folder, the count on the heading comes down, and a heading it was
     the whole of is not drawn at all. */
  it('takes a marked branch out of the folder it was in', () => {
    const rows = branchRows(
      branches('*main', 'fix/one', 'fix/two', 'spike/only'),
      ['fix', 'spike'],
      ['fix/one', 'spike/only']
    )
    expect(labels(rows)).toEqual(['main', 'fix/one', 'spike/only', '/fix', 'two'])
    expect(rows.find((row) => row.kind === 'folder').count).toBe(1)
  })

  /* One row and not two: the branch the repository is on wins the first
     position, and the mark rides on that row. */
  it('draws a branch that is both current and marked once, at the top', () => {
    const rows = branchRows(branches('*main', 'develop'), [], ['main'])
    expect(labels(rows)).toEqual(['main', 'develop'])
    expect(rows[0]).toMatchObject({ name: 'main', current: true, favorite: true, pinned: true })
  })

  /* The hairline is about the bottom of the block rather than about the current
     branch: it says the real list starts below, and it says it once. */
  it('rules off under the last marked row rather than under the current branch', () => {
    const rows = branchRows(branches('*main', 'develop', 'spike'), [], ['develop', 'spike'])
    expect(rows.map((row) => Boolean(row.divider))).toEqual([false, false, true])
  })

  /* A project can hold several repositories and the list is one list, so a name
     that is nowhere in this repository is the ordinary case. It draws nothing
     and changes nothing. */
  it('draws no row for a name this repository does not have', () => {
    const rows = branchRows(branches('*main', 'develop'), [], ['nothing-called-this'])
    expect(labels(rows)).toEqual(['main', 'develop'])
    expect(rows[0].divider).toBe(true)
  })

  /* Nothing marked is the state every project starts in, and it has to draw
     exactly what it drew before this existed. */
  it('is the list it always was when nothing is marked', () => {
    const list = branches('*feature/one', 'main')
    expect(labels(branchRows(list, ['feature']))).toEqual(
      labels(branchRows(list, ['feature'], []))
    )
  })

  /* Which rows are above the tree, which is the same question `tracking.js` has
     to ask about a fold: a heading standing in for a row that is on screen
     anyway would be saying it twice. */
  it('counts the current branch and every marked one as lifted out', () => {
    expect(liftedOut({ name: 'main', current: true }, [])).toBe(true)
    expect(liftedOut({ name: 'spike', current: false }, ['spike'])).toBe(true)
    expect(liftedOut({ name: 'spike', current: false }, [])).toBe(false)
    expect(liftedOut(undefined, undefined)).toBe(false)
  })
})

describe('what a press on the favourite item leaves behind', () => {
  it('marks an unmarked branch and unmarks a marked one', () => {
    expect(toggleFavorite([], 'main')).toEqual(['main'])
    expect(toggleFavorite(['main', 'spike'], 'main')).toEqual(['spike'])
  })

  /* Nothing has ever been marked in a project that has never had a list, and
     the first press has to write one out. */
  it('writes a list out over nothing at all', () => {
    expect(toggleFavorite(null, 'main')).toEqual(['main'])
    expect(toggleFavorite(undefined, 'main')).toEqual(['main'])
  })

  /* A fresh array every time, for `toggleFolder`'s reason: the caller assigns
     it into `settings.json`, and a list mutated in place gives the store's own
     watcher nothing to notice. */
  it('answers with a new list rather than the one it was given', () => {
    const stored = ['main']
    expect(toggleFavorite(stored, 'spike')).not.toBe(stored)
    expect(stored).toEqual(['main'])
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
