import { describe, expect, it } from 'vitest'
import {
  BRANCH_TABS,
  DEFAULT_BRANCH_TAB,
  FILTER_DELAY_MS,
  MAX_FAVORITES,
  branchRows,
  branchTabLabels,
  currentChain,
  expandedFolders,
  filterBranches,
  filterDelay,
  liftedOut,
  originBranches,
  originBranchRows,
  toggleFavorite,
  toggleFolder,
  toggleRemoteFolder
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

  /* The block a lifted row belongs to, which is what the component draws a
     surface from. With nothing marked the top block is the current branch
     alone, and every row under it is a row of the tree with no block at all. */
  it('marks the current branch as its own block when nothing is marked', () => {
    const rows = branchRows(branches('*main', 'develop'), [])
    expect(rows[0].block).toBe('current')
    expect(rows[1].block).toBeUndefined()
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

  /* The marked rows are a block of their own under the current one, so the two
     groups can be told apart by their surfaces rather than by one hairline
     under the pair of them. */
  it('marks the pinned favourites as the favourite block, under the current one', () => {
    const rows = branchRows(branches('*main', 'develop', 'spike'), [], ['develop', 'spike'])
    expect(rows.map((row) => row.block)).toEqual(['current', 'favourite', 'favourite'])
  })

  /* A branch that is both current and marked is one row in the current block,
     with the star on it: the first group wins, and the mark rides along. */
  it('draws a branch that is both current and marked in the current block', () => {
    const rows = branchRows(branches('*main', 'develop'), [], ['main'])
    expect(rows[0].block).toBe('current')
    expect(rows[0].favorite).toBe(true)
  })

  /* A project can hold several repositories and the list is one list, so a name
     that is nowhere in this repository is the ordinary case. It draws nothing
     and changes nothing. */
  it('draws no row for a name this repository does not have', () => {
    const rows = branchRows(branches('*main', 'develop'), [], ['nothing-called-this'])
    expect(labels(rows)).toEqual(['main', 'develop'])
    expect(rows[0].block).toBe('current')
    expect(rows[1].block).toBeUndefined()
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

  /* The ceiling is `MAX_FAVORITE_BRANCHES` in `settings/model.rs`, written out
     on this side too. It has to be refused **here**, because Rust trims from
     the tail: a front end that accepted the 51st would draw the star, keep it
     for the session and lose it on the next load, with nothing on screen
     saying a mark somebody made had gone. */
  it('refuses a mark past the ceiling rather than letting Rust drop it later', () => {
    const full = Array.from({ length: MAX_FAVORITES }, (_, at) => `branch/${at}`)
    const after = toggleFavorite(full, 'one-too-many')
    expect(after).toHaveLength(MAX_FAVORITES)
    expect(after).not.toContain('one-too-many')
  })

  /* Unmarking is never refused, and a hand-edited file already over the ceiling
     is exactly the case: a list somebody cannot shorten is a list they cannot
     fix. */
  it('always lets a mark come off, even from a list already over the ceiling', () => {
    const over = Array.from({ length: MAX_FAVORITES + 5 }, (_, at) => `branch/${at}`)
    expect(toggleFavorite(over, 'branch/0')).toHaveLength(MAX_FAVORITES + 4)
  })

  /* Marking a name already in the list is an unmark, so a full list is not
     frozen against its own members. */
  it('unmarks a member of a full list rather than reading it as an add', () => {
    const full = Array.from({ length: MAX_FAVORITES }, (_, at) => `branch/${at}`)
    expect(toggleFavorite(full, 'branch/3')).not.toContain('branch/3')
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

/* The Origin tab: the whole of what `origin` has, drawn instead of the local
   list rather than under it. `local` is `vcs_branches`' own shape, `remote` the
   plain names `vcs_remote_branches` answers with, and `expanded` the fold paths
   as `remoteBranchFolders` keeps them — with no group prefix on them any more,
   since there is no heading above them to prefix with. */
describe('the origin tab', () => {
  const local = branches('*main', 'feature/one')

  it('draws nothing at all when origin has nothing', () => {
    expect(originBranchRows([], local, [])).toEqual([])
  })

  /* The change of mind this tab is: the group it replaces held only what had no
     local twin, because a group under the local list would otherwise have been a
     duplicate of it. A tab called Origin shows origin. */
  it('draws every branch origin has, local twin or not', () => {
    const rows = originBranchRows(['develop', 'main'], local, [])
    expect(labels(rows)).toEqual(['develop', 'main'])
  })

  /* And the case that made the group unfindable in the other direction: a
     repository whose origin holds nothing this one lacks drew no group at all,
     which on screen was indistinguishable from the feature being broken. */
  it('draws a full list where the old group drew nothing', () => {
    expect(labels(originBranchRows(['main', 'feature/one'], local, []))).toEqual([
      'main',
      '/feature'
    ])
  })

  it('marks the rows this repository already has, and only those', () => {
    const rows = originBranchRows(['develop', 'main'], local, [])
    expect(rows.map((row) => [row.name, row.hasLocal])).toEqual([
      ['develop', false],
      ['main', true]
    ])
  })

  /* The tick, and the one thing that can carry it. A repository can only ever
     be standing on a branch it has, so `current` is never true of a row that
     `hasLocal` is false of. */
  it('marks the branch the repository is on, which always has a local twin', () => {
    const rows = originBranchRows(['develop', 'main'], local, [])
    expect(rows.map((row) => [row.name, row.current])).toEqual([
      ['develop', false],
      ['main', true]
    ])
    expect(rows.every((row) => !row.current || row.hasLocal)).toBe(true)
  })

  it('folds names with a slash in them by the same rule the local tab uses', () => {
    const rows = originBranchRows(['feature/two', 'main'], local, [])
    expect(rows.map((row) => [row.kind, row.label, row.depth])).toEqual([
      ['folder', 'feature', 0],
      ['branch', 'main', 0]
    ])
    expect(rows[0].count).toBe(1)
  })

  /* The paths carry no group prefix, which is the whole of what changed about
     the stored list. `origin/feature`, which is what the old shape wrote, now
     matches nothing — and an entry matching nothing means a folded folder,
     which is why the spec asks for no migration. */
  it('unfolds a folder by its bare path and ignores the old prefixed one', () => {
    expect(originBranchRows(['feature/two'], local, ['feature']).map((row) => row.label)).toEqual([
      'feature',
      'two'
    ])
    expect(originBranchRows(['feature/two'], local, ['origin/feature'])).toEqual([
      { kind: 'folder', path: 'feature', label: 'feature', depth: 0, count: 1, expanded: false }
    ])
  })

  it('nests as deeply as the name does', () => {
    const rows = originBranchRows(['fix/legacy/depot'], local, ['fix', 'fix/legacy'])
    expect(rows.map((row) => [row.kind, row.label, row.depth])).toEqual([
      ['folder', 'fix', 0],
      ['folder', 'legacy', 1],
      ['branch', 'depot', 2]
    ])
    expect(rows[2]).toMatchObject({ name: 'fix/legacy/depot' })
  })

  /* The collision the `remoteBranchFolders` field exists for, from the side a
     test can reach: a local branch called `origin/spike` puts a folder named
     `origin` in the *local* tree, and this tab has a folder of its own for a
     name like `feature`. Two tabs, two lists, and neither unfolds the other. */
  it('folds by its own list and never by the local one', () => {
    const withLocalFolder = branches('*main', 'feature/one')
    expect(labels(originBranchRows(['feature/two'], withLocalFolder, []))).toEqual(['/feature'])
    expect(labels(branchRows(withLocalFolder, ['feature'], []))).toEqual([
      'main',
      '/feature',
      'one'
    ])
  })

  it('keeps the alphabetical order the command answered in', () => {
    expect(labels(originBranchRows(['zeta', 'alpha'], local, []))).toEqual(['zeta', 'alpha'])
  })

  /* Nothing is lifted to the top here, unlike the local tab: the alphabet holds
     all the way down, so the branch the repository is on stays where its name
     puts it. */
  it('lifts nothing to the top, not even the current branch', () => {
    const rows = originBranchRows(['zeta', 'main'], local, [])
    expect(labels(rows)).toEqual(['zeta', 'main'])
    expect(rows.some((row) => row.pinned || row.block)).toBe(false)
  })

  it('carries no favourite mark and no children on a folder row', () => {
    const rows = originBranchRows(['feature/two', 'main'], local, ['feature'])
    expect(rows.every((row) => !('children' in row) && !('favorite' in row))).toBe(true)
  })

  it('reads a missing list of either side as nothing rather than throwing', () => {
    expect(originBranchRows(undefined, undefined, undefined)).toEqual([])
    expect(originBranchRows([null, ''], local, [])).toEqual([])
  })
})

/* The closed list itself, which is the half of this that a person can get wrong
   silently: a word the front end offers and `settings/model.rs` has never heard
   of is rewritten to the default on the way in, and the only symptom is the app
   forgetting a choice after a restart. */
describe('the two tabs', () => {
  it('names both sides and nothing else', () => {
    expect(BRANCH_TABS).toEqual(['local', 'origin'])
  })

  it('starts on the side the list has always been', () => {
    expect(DEFAULT_BRANCH_TAB).toBe('local')
    expect(BRANCH_TABS).toContain(DEFAULT_BRANCH_TAB)
  })
})

describe('toggleRemoteFolder', () => {
  it('opens a folder that is closed and closes one that is open', () => {
    expect(toggleRemoteFolder([], 'feature')).toEqual(['feature'])
    expect(toggleRemoteFolder(['feature', 'fix'], 'fix')).toEqual(['feature'])
  })

  it('reads a missing list as everything folded', () => {
    expect(toggleRemoteFolder(undefined, 'feature')).toEqual(['feature'])
  })

  it('always returns a new array, so the settings watcher notices', () => {
    const stored = ['feature']
    expect(toggleRemoteFolder(stored, 'fix')).not.toBe(stored)
    expect(stored).toEqual(['feature'])
  })
})

/* The filter, which is the whole of what a person types into the section's
   caption. The list is the one the panel is looked at with: names that share a
   prefix and differ by a few characters, which is what rules fuzzy matching out
   and what makes the substring worth pinning in both directions. */
describe('filtering the list by name', () => {
  const list = branches(
    '*develop-emerald',
    'feat/nxc-204-kickbox-email-validation',
    'fix/kickbox-timeout-retry',
    'main'
  )

  it('matches a case-insensitive substring of the whole name', () => {
    expect(filterBranches(list, 'KICK').map((hit) => hit.name)).toEqual([
      'feat/nxc-204-kickbox-email-validation',
      'fix/kickbox-timeout-retry'
    ])
  })

  /* The prefix is part of the name and not a field beside it, which is what
     lets somebody narrow a result by typing the folder they mean — and what
     makes a query spanning the slash of a *different* name find nothing. */
  it('matches across the slash, so a prefix narrows it', () => {
    expect(filterBranches(list, 'feat/kick').map((hit) => hit.name)).toEqual([])
    expect(filterBranches(list, 'feat/nxc').map((hit) => hit.name)).toEqual([
      'feat/nxc-204-kickbox-email-validation'
    ])
  })

  /* The order is the tab's own and the result is flat: nothing is ranked, and
     the current branch is not lifted to the top of its own hits. */
  it('keeps the order the list arrived in and lifts nothing', () => {
    const hits = filterBranches(list, 'e')
    expect(hits.map((hit) => hit.name)).toEqual([
      'develop-emerald',
      'feat/nxc-204-kickbox-email-validation',
      'fix/kickbox-timeout-retry'
    ])
    expect(hits.every((hit) => hit.depth === 0 && !hit.block)).toBe(true)
  })

  it('splits the name at the last slash and says where the match is', () => {
    const [hit] = filterBranches(list, 'kickbox-t')
    expect(hit.prefix).toBe('fix/')
    expect(hit.tail).toBe('kickbox-timeout-retry')
    expect(hit.match).toEqual({ start: 4, end: 13 })
  })

  /* A name with no slash in it is all tail and no prefix, which is what stops
     the drawing putting an empty muted span in front of half the rows. */
  it('leaves a name with no slash entirely in the tail', () => {
    const [hit] = filterBranches(list, 'main')
    expect(hit.prefix).toBe('')
    expect(hit.tail).toBe('main')
    expect(hit.label).toBe('main')
  })

  it('carries the star and the tick through', () => {
    const hits = filterBranches(list, 'e', {
      favorites: ['main', 'fix/kickbox-timeout-retry']
    })
    expect(hits.map((hit) => Boolean(hit.favorite))).toEqual([false, false, true])
    expect(hits[0].current).toBe(true)
  })

  /* The Origin tab has no `current` field on its entries when they are built by
     hand, so the option is the way that tab's tick arrives. */
  it('takes the current branch by name where the list does not carry one', () => {
    const hits = filterBranches([{ name: 'release' }, { name: 'develop' }], 'e', {
      current: 'develop'
    })
    expect(hits.map((hit) => [hit.name, Boolean(hit.current)])).toEqual([
      ['release', false],
      ['develop', true]
    ])
  })

  /* Anything else on the entry travels, which is how an Origin hit knows
     whether it is a `cloud` row or a `git-branch` one. */
  it('carries the rest of the entry through untouched', () => {
    const [hit] = filterBranches([{ name: 'spike', hasLocal: false }], 'spi')
    expect(hit.hasLocal).toBe(false)
    expect(hit.kind).toBe('branch')
    expect(hit.pinned).toBe(true)
  })

  it('answers nothing for an empty or blank query', () => {
    expect(filterBranches(list, '')).toEqual([])
    expect(filterBranches(list, '   ')).toEqual([])
    expect(filterBranches(list, null)).toEqual([])
  })

  it('reads a missing list as nothing rather than throwing', () => {
    expect(filterBranches(undefined, 'main')).toEqual([])
    expect(filterBranches([null, {}], 'main')).toEqual([])
  })
})

/* The entries both readers of the Origin tab share. What the filter needs from
   this is the pair of facts a row is drawn by, which is why it is one rule and
   not a mapping written out at each of the two call sites. */
describe('the entries behind the origin tab', () => {
  const local = branches('*main', 'feature/one')

  it('says which names this repository already has and which it is on', () => {
    expect(originBranches(['develop', 'main'], local)).toEqual([
      { name: 'develop', hasLocal: false, current: false },
      { name: 'main', hasLocal: true, current: true }
    ])
  })

  it('drops the names there is no row to draw for', () => {
    expect(originBranches([null, '', 'main'], local)).toEqual([
      { name: 'main', hasLocal: true, current: true }
    ])
  })

  it('reads a missing list of either side as nothing', () => {
    expect(originBranches(undefined, undefined)).toEqual([])
  })
})

/* The tab labels, which is where the counts go while the caption is a field.
   The second number is the load-bearing one: without it a tab with no matches
   is an empty state telling the truth about the wrong half of the repository. */
describe('what the two tabs are called', () => {
  it('names them plainly with no filter', () => {
    expect(branchTabLabels({ tab: 'local', query: '', localTotal: 346, originTotal: 593 })).toEqual([
      { id: 'local', label: 'Local' },
      { id: 'origin', label: 'Origin' }
    ])
  })

  it('puts the hit count in the active tab and the other tab reports its own', () => {
    expect(
      branchTabLabels({
        tab: 'local',
        query: 'kick',
        localTotal: 346,
        originTotal: 593,
        localHits: 3,
        originHits: 9
      })
    ).toEqual([
      { id: 'local', label: '3 of 346' },
      { id: 'origin', label: 'Origin 9' }
    ])
    expect(
      branchTabLabels({
        tab: 'origin',
        query: 'kick',
        localTotal: 346,
        originTotal: 593,
        localHits: 3,
        originHits: 9
      })
    ).toEqual([
      { id: 'local', label: 'Local 3' },
      { id: 'origin', label: '9 of 593' }
    ])
  })

  /* Zero is drawn like any other count here, unlike the caption's own, which
     refuses one: `0 of 346` beside `Origin 9` is the sentence the empty state
     under it is about. */
  it('draws a zero rather than falling silent', () => {
    expect(
      branchTabLabels({ tab: 'local', query: 'zzz', localTotal: 346, originTotal: 593 })
    ).toEqual([
      { id: 'local', label: '0 of 346' },
      { id: 'origin', label: 'Origin 0' }
    ])
  })

  it('answers the two ids in the order the tab row draws them', () => {
    expect(branchTabLabels({}).map((tab) => tab.id)).toEqual(BRANCH_TABS)
  })

  it('reads a blank query as no filter at all', () => {
    expect(branchTabLabels({ tab: 'local', query: '   ', localTotal: 2 })[0].label).toBe('Local')
  })
})

/* How long the field waits, which is `--dur-fast` in whatever unit the
   stylesheet happens to be written in. */
describe('how long the field waits', () => {
  it('reads the token as the stylesheet writes it today', () => {
    expect(filterDelay('90ms')).toBe(90)
  })

  /* Tolerated rather than expected: nothing normalises a custom property's
     value, and this rule is not the place to find out what the browser did
     with the whitespace after the colon. */
  it('tolerates whitespace around the value', () => {
    expect(filterDelay(' 90ms ')).toBe(90)
  })

  it('reads seconds as seconds', () => {
    expect(filterDelay('.09s')).toBe(90)
    expect(filterDelay(' 1s ')).toBe(1000)
  })

  /* `motion.css` zeroes the token under `prefers-reduced-motion`, and a filter
     that answers on the keystroke is the right reading of that. */
  it('honours a zero rather than falling back to the default', () => {
    expect(filterDelay('0ms')).toBe(0)
  })

  it('falls back where there is nothing readable, no unit, or a negative one', () => {
    expect(filterDelay('')).toBe(FILTER_DELAY_MS)
    expect(filterDelay(undefined)).toBe(FILTER_DELAY_MS)
    expect(filterDelay('fast')).toBe(FILTER_DELAY_MS)
    expect(filterDelay('90')).toBe(FILTER_DELAY_MS)
    expect(filterDelay('-90ms')).toBe(FILTER_DELAY_MS)
  })
})
