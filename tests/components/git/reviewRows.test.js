import { describe, expect, it } from 'vitest'
import {
  FOLLOWS_THE_RULE,
  NOT_IN_REVIEW,
  NO_SUCH_BRANCH,
  RULE_CAPTION,
  RULE_CAPTION_EMPTY,
  addableRepos,
  branchesIn,
  canReview,
  fetchFailures,
  fetchTargets,
  footerSummary,
  hasBranch,
  isManual,
  isOverride,
  missingRepos,
  oldestFetch,
  overrideIds,
  pairLabel,
  pairOf,
  pickerScope,
  refOf,
  reportPath,
  repoIdsWith,
  reviewForm,
  reviewNotes,
  reviewPairs,
  rowStatus,
  ruleCaption,
  sideLabel,
  tableSummary,
  withOverride,
  withPick,
  withRepo,
  withoutOverride,
  withoutRepo
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

const local = (ref) => ({ ref, remote: false })
const origin = (ref) => ({ ref, remote: true })

const opened = (branch = 'feat/x', options = {}) =>
  reviewForm(REPOS, branch, { branches: BRANCHES, ...options })

describe('reviewForm', () => {
  it('opens with a row for every repository that has the branch', () => {
    const form = opened()
    expect(form.repoIds).toEqual(['/p', '/p/admin'])
    expect(form.head).toEqual(local('feat/x'))
    expect(form.overrides).toEqual({})
  })

  /* Both sides start on the local branch. The choice between a local branch and
     what origin has is the person's, and the local one is what they were
     looking at. */
  it('starts both sides of the rule on the local branch', () => {
    const form = opened()
    expect(form.base.remote).toBe(false)
    expect(form.head.remote).toBe(false)
  })

  /* `branchChoice.js`'s existing order, because the run dialog answers this
     same question one screen over and a second order would be a second answer
     to it: what this project was left at, then `[defaults].target_branch`, then
     the top of the list. */
  it('fills the base with what was remembered, then what the project declares', () => {
    expect(opened('feat/x', { remembered: 'main', configured: 'feat/x' }).base.ref).toBe('main')
    expect(opened('feat/x', { configured: 'feat/x' }).base.ref).toBe('feat/x')
    expect(opened('feat/x').base.ref).toBe('main')
  })

  /* The `New review` door: no name to start from, so no branch to check and no
     rows at all. Picking a name is what builds the table, through the same rule
     the other door opened with. */
  it('opens with no branch and no rows at all when there is no name', () => {
    const form = reviewForm(REPOS, null, { branches: BRANCHES })
    expect(form.head).toBe(null)
    expect(form.repoIds).toEqual([])
    expect(canReview(form)).toBe(false)
  })

  /* A name `target_branches` has never heard of is missing from every
     repository it walked. The table is empty and the note under it says why,
     rather than the window looking as though it had failed to open. */
  it('gives no rows at all for a branch that is nowhere', () => {
    const form = opened('feat/nowhere')
    expect(form.repoIds).toEqual([])
    expect(missingRepos(REPOS, form, { branches: BRANCHES }).map((r) => r.name)).toEqual([
      '.',
      'admin',
      'shared'
    ])
  })

  it('answers an empty table for no repositories at all', () => {
    expect(reviewForm(null, 'feat/x', { branches: BRANCHES }).repoIds).toEqual([])
    expect(reviewForm([], 'feat/x', { branches: BRANCHES }).repoIds).toEqual([])
  })

  /* An absent branch list is the same answer as a branch nowhere, and that is
     why `DesktopApp.vue` waits for `target_branches` before it builds a form
     rather than after: a list that has not landed is not evidence that nothing
     has the branch, and this rule has no way to tell the two apart. */
  it('reads an absent branch list as a branch no repository has', () => {
    expect(reviewForm(REPOS, 'feat/x', { branches: null }).repoIds).toEqual([])
  })
})

describe('hasBranch', () => {
  it('reads the branch list a repository is not short of', () => {
    expect(hasBranch(REPOS[0], local('feat/x'), { branches: BRANCHES })).toBe(true)
    expect(hasBranch(REPOS[2], local('feat/x'), { branches: BRANCHES })).toBe(false)
  })

  /* A side meaning `origin` is asked of the remote list first: a branch that
     lives only on the server is exactly the case the local answer gets
     wrong. */
  it('asks the remote list for a side that means origin', () => {
    const context = { branches: BRANCHES, remote: { '/p/shared': ['feat/x'] } }
    expect(hasBranch(REPOS[2], origin('feat/x'), context)).toBe(true)
    expect(hasBranch(REPOS[1], origin('feat/x'), { ...context })).toBe(true)
  })

  /* A repository whose remote list has not landed yet has no entry at all, and
     a branch missing from a list nobody has read is not a fact about the
     repository — so it falls through to the local answer. */
  it('falls back to the local answer while a remote list is still on its way', () => {
    expect(hasBranch(REPOS[0], origin('feat/x'), { branches: BRANCHES, remote: {} })).toBe(true)
    expect(hasBranch(REPOS[2], origin('feat/x'), { branches: BRANCHES, remote: {} })).toBe(false)
  })

  it('has nothing to answer about a side nobody has filled in', () => {
    expect(hasBranch(REPOS[0], null, { branches: BRANCHES })).toBe(false)
    expect(hasBranch(REPOS[0], local('  '), { branches: BRANCHES })).toBe(false)
  })
})

describe('repoIdsWith', () => {
  it('names every repository that has the branch, by path and in order', () => {
    expect(repoIdsWith(REPOS, local('main'), { branches: BRANCHES })).toEqual([
      '/p',
      '/p/admin',
      '/p/shared'
    ])
    expect(repoIdsWith(REPOS, local('feat/x'), { branches: BRANCHES })).toEqual(['/p', '/p/admin'])
  })
})

describe('the effective pair', () => {
  /* The one line the whole model rests on: a row is reviewed with its own pair
     if it has one and with the project's rule otherwise. */
  it('is the rule for a row that has no override', () => {
    const form = opened()
    expect(pairOf(form, '/p')).toEqual({ base: local('main'), head: local('feat/x') })
    expect(isOverride(form, '/p')).toBe(false)
  })

  it('is the pair a row keeps of its own once it has one', () => {
    const form = withPick(withOverride(opened(), '/p/admin'), { side: 'head', repoId: '/p/admin' }, origin('main'))
    expect(pairOf(form, '/p/admin')).toEqual({ base: local('main'), head: origin('main') })
    expect(pairOf(form, '/p')).toEqual({ base: local('main'), head: local('feat/x') })
  })

  /* An override is made by copying the rule, never by starting an empty one:
     there is no arrangement of this form with a base and no branch to check. */
  it('starts an override as a whole copy of the rule', () => {
    const form = withOverride(opened(), '/p')
    expect(form.overrides['/p']).toEqual({ base: local('main'), head: local('feat/x') })
    expect(overrideIds(form)).toEqual(['/p'])
  })

  /* And going back to the rule touches this row and no neighbour. */
  it('drops one override and leaves the others where they were', () => {
    let form = withOverride(opened(), '/p')
    form = withOverride(form, '/p/admin')
    form = withoutOverride(form, '/p', { repos: REPOS, branches: BRANCHES })
    expect(overrideIds(form)).toEqual(['/p/admin'])
    expect(form.repoIds).toEqual(['/p', '/p/admin'])
    expect(pairOf(form, '/p')).toEqual({ base: local('main'), head: local('feat/x') })
  })

  /* Told nothing about the project, it drops the override and leaves the table
     alone: a caller that names no repositories has not said that no repository
     has the branch, it has said nothing, and reading the silence as "nowhere"
     would empty the table of every row that follows the rule. */
  it('leaves the table alone when it is told nothing about the project', () => {
    const form = withOverride(opened(), '/p')
    const back = withoutOverride(form, '/p')
    expect(back.overrides).toEqual({})
    expect(back.repoIds).toEqual(['/p', '/p/admin'])
  })

  it('has nothing to drop for a row that follows the rule', () => {
    const form = opened()
    expect(withoutOverride(form, '/p')).toBe(form)
  })

  /* Going back to the rule is going back to a rule that has to reach the row.
     `shared` has no `feat/x`; it was in the table because it kept a pair of its
     own, and the moment it gives that up it is a repository the review is not
     in — a row left behind would claim `follows the rule` while `reviewPairs`
     sent the agent a head that does not exist there, with the note under the
     table silent because a row in the table is not "left out". The same
     decision a change of branch makes, through the same rule. */
  it('takes a row out of the table when the rule it goes back to does not reach it', () => {
    let form = reviewForm(REPOS, 'main', { branches: BRANCHES })
    form = withOverride(form, '/p/shared')
    form = withPick(form, { side: 'head', repoId: null }, local('feat/x'), {
      repos: REPOS,
      branches: BRANCHES
    })
    expect(form.repoIds).toContain('/p/shared')
    const back = withoutOverride(form, '/p/shared', { repos: REPOS, branches: BRANCHES })
    expect(back.repoIds).toEqual(['/p', '/p/admin'])
    expect(back.overrides).toEqual({})
    expect(reviewPairs(back).map((pair) => pair.repo)).toEqual(['/p', '/p/admin'])
    expect(missingRepos(REPOS, back, { branches: BRANCHES }).map((r) => r.name)).toEqual(['shared'])
    expect(tableSummary(back)).toBe('2 · all follow the rule')
  })

  /* And a row the rule does reach keeps its place, with no neighbour touched:
     only a change to the rule's own checked side ever adds rows. */
  it('keeps a row the rule reaches, and adds nothing to the table', () => {
    const form = withOverride(opened(), '/p/admin')
    const back = withoutOverride(form, '/p/admin', { repos: REPOS, branches: BRANCHES })
    expect(back.repoIds).toEqual(['/p', '/p/admin'])
    expect(missingRepos(REPOS, back, { branches: BRANCHES }).map((r) => r.name)).toEqual(['shared'])
  })

  /* A row somebody added by hand has no rule to go back to — the reason it is
     in the table at all is that the rule's branch is not in its repository.
     Refused in the module rather than merely not offered in the template, so
     that `MAN` beside `follows the rule` is unrepresentable rather than one
     reordering of three icon buttons away. */
  it('refuses to put a hand-added row back on a rule it can never follow', () => {
    const form = withRepo(opened(), '/p/shared')
    expect(withoutOverride(form, '/p/shared', { repos: REPOS, branches: BRANCHES })).toBe(form)
    expect(isManual(form, '/p/shared')).toBe(true)
    expect(isOverride(form, '/p/shared')).toBe(true)
  })
})

describe('withPick', () => {
  it('writes one side of the rule and leaves the other alone', () => {
    const form = withPick(opened(), { side: 'base', repoId: null }, origin('main'))
    expect(form.base).toEqual(origin('main'))
    expect(form.head).toEqual(local('feat/x'))
  })

  /* The whole of the `New review` door: the first branch on the rule's checked
     side fills the table, through the same rule the other door opened with. */
  it('fills the table with the first branch picked on an empty rule', () => {
    const empty = reviewForm(REPOS, null, { branches: BRANCHES })
    const form = withPick(empty, { side: 'head', repoId: null }, local('feat/x'), {
      repos: REPOS,
      branches: BRANCHES
    })
    expect(form.repoIds).toEqual(['/p', '/p/admin'])
    expect(canReview(form)).toBe(true)
  })

  /* And every time after that, which is what stops the table claiming things
     about itself that stopped being true: a rule-following row in a repository
     that has no such branch is not a smaller review, it is a pair git would
     refuse. It leaves, and the note under the table names it. */
  it('rebuilds the rows the rule decides every time the branch changes', () => {
    const wide = reviewForm(REPOS, 'main', { branches: BRANCHES })
    expect(wide.repoIds).toEqual(['/p', '/p/admin', '/p/shared'])
    const narrow = withPick(wide, { side: 'head', repoId: null }, local('feat/x'), {
      repos: REPOS,
      branches: BRANCHES
    })
    expect(narrow.repoIds).toEqual(['/p', '/p/admin'])
    expect(missingRepos(REPOS, narrow, { branches: BRANCHES }).map((r) => r.name)).toEqual([
      'shared'
    ])
  })

  /* What the rebuild leaves alone is the whole of its manners: a row that
     differs carries its own pair and a row somebody added is a decision of
     theirs, so the rule's head reaches neither. */
  it('leaves the rows the rule does not decide where they are', () => {
    let form = withOverride(reviewForm(REPOS, 'main', { branches: BRANCHES }), '/p/shared')
    form = withPick(form, { side: 'head', repoId: null }, local('feat/x'), {
      repos: REPOS,
      branches: BRANCHES
    })
    /* `shared` has no `feat/x` and stays all the same: its pair is its own, and
       the rule's head does not reach it. */
    expect(form.repoIds).toEqual(['/p', '/p/admin', '/p/shared'])
    expect(pairOf(form, '/p/shared')).toEqual({ base: local('main'), head: local('main') })
    /* And it is not named as left out, because it is not left out. */
    expect(missingRepos(REPOS, form, { branches: BRANCHES })).toEqual([])
  })

  /* Rows the new branch brings in arrive at the end rather than shuffling the
     table under somebody's eye. */
  it('puts the rows a new branch brings in at the end', () => {
    const form = withPick(opened(), { side: 'head', repoId: null }, local('main'), {
      repos: REPOS,
      branches: BRANCHES
    })
    expect(form.repoIds).toEqual(['/p', '/p/admin', '/p/shared'])
  })

  /* The base is not what decides which repositories are in the review. */
  it('leaves the rows alone when the base changes', () => {
    const form = withPick(opened(), { side: 'base', repoId: null }, origin('develop'), {
      repos: REPOS,
      branches: BRANCHES
    })
    expect(form.repoIds).toEqual(['/p', '/p/admin'])
  })

  it('writes into the override of the row the list was opened for', () => {
    const form = withPick(opened(), { side: 'head', repoId: '/p/admin' }, origin('feat/x'))
    expect(form.overrides['/p/admin']).toEqual({ base: local('main'), head: origin('feat/x') })
    expect(form.head).toEqual(local('feat/x'))
  })
})

describe('adding and removing a repository', () => {
  /* A repository added by hand arrives as an override, and that is not a
     formality: the reason it was not in the table is that the rule's branch is
     not in it, so following the rule is the one thing this row cannot do. */
  it('adds a repository as an override at the end of the table', () => {
    const form = withRepo(opened(), '/p/shared')
    expect(form.repoIds).toEqual(['/p', '/p/admin', '/p/shared'])
    expect(form.overrides['/p/shared']).toEqual({ base: local('main'), head: local('feat/x') })
  })

  it('will not add a repository twice', () => {
    const form = withRepo(opened(), '/p')
    expect(form.repoIds).toEqual(['/p', '/p/admin'])
  })

  /* And there is nothing to add to a review with no branch to check. It answers
     with the form it was given, which is what the window reads to know that the
     branch list must not be opened over a row that does not exist — a pick
     there would write a pair into `overrides` under a repository outside
     `repoIds`, invisible until some later change of branch brought the row back
     wearing it. */
  it('adds nothing while there is no branch to check', () => {
    const empty = reviewForm(REPOS, null, { branches: BRANCHES })
    expect(withRepo(empty, '/p/shared')).toBe(empty)
    expect(withRepo(opened(), '/p')).not.toBe(opened())
  })

  /* A pair left behind for a row that is not in the table would come back the
     next time the same repository was added. */
  it('takes the override out with the row', () => {
    const form = withoutRepo(withRepo(opened(), '/p/shared'), '/p/shared')
    expect(form.repoIds).toEqual(['/p', '/p/admin'])
    expect(form.overrides).toEqual({})
  })

  /* Recorded rather than worked out from the branch. It was worked out at
     first — a row without the rule's branch could only have been named by a
     person — and the answer moved the moment the rule's head did: the row
     silently became an ordinary one and lost the `x`, which is the only way a
     row ever leaves this table. */
  it('records that a row was added by hand', () => {
    const form = withRepo(opened(), '/p/shared')
    expect(form.manual).toEqual(['/p/shared'])
    expect(isManual(form, '/p/shared')).toBe(true)
    expect(isManual(form, '/p')).toBe(false)
  })

  it('says nothing about a repository that is not in the table at all', () => {
    expect(isManual(opened(), '/p/shared')).toBe(false)
  })

  /* And it survives the rule's branch changing under it, which is what the
     derived answer could not do. */
  it('keeps a hand-added row its own after the rule changes branch', () => {
    let form = withRepo(opened(), '/p/shared')
    form = withPick(form, { side: 'head', repoId: null }, local('main'), {
      repos: REPOS,
      branches: BRANCHES
    })
    expect(isManual(form, '/p/shared')).toBe(true)
    expect(form.repoIds).toContain('/p/shared')
  })

  /* Both facts leave with the row: a pair somebody abandoned, or a badge on a
     row the rule put there, would come back the next time the same repository
     was added. */
  it('forgets the provenance with the row', () => {
    const form = withoutRepo(withRepo(opened(), '/p/shared'), '/p/shared')
    expect(form.manual).toEqual([])
    expect(form.overrides).toEqual({})
  })
})

describe('addableRepos', () => {
  /* Only what is not already in the table: offering a repository that has a row
     would be a second row for one repository, which is a pair this window has
     no way to mean. */
  it('offers what is not in the table, and says why each is out of it', () => {
    const form = opened()
    expect(addableRepos(REPOS, form, { branches: BRANCHES })).toEqual([
      { repo: REPOS[2], note: NO_SUCH_BRANCH }
    ])
  })

  it('says so when a repository is simply not in the review', () => {
    const form = { ...opened(), repoIds: ['/p'] }
    expect(addableRepos(REPOS, form, { branches: BRANCHES })).toEqual([
      { repo: REPOS[1], note: NOT_IN_REVIEW },
      { repo: REPOS[2], note: NO_SUCH_BRANCH }
    ])
  })
})

describe('canReview', () => {
  it('refuses a form with no branch to check and one with no rows', () => {
    expect(canReview(reviewForm(REPOS, null, { branches: BRANCHES }))).toBe(false)
    expect(canReview({ ...opened(), repoIds: [] })).toBe(false)
    expect(canReview(null)).toBe(false)
  })

  it('allows a form with a branch and a row to check it in', () => {
    expect(canReview(opened())).toBe(true)
  })
})

describe('refOf', () => {
  it('spells a local branch as itself and a remote one with its remote', () => {
    expect(refOf(local('main'))).toBe('main')
    expect(refOf(origin('main'))).toBe('origin/main')
  })

  it('has nothing to spell for a side nobody has answered', () => {
    expect(refOf(null)).toBe('')
    expect(refOf({ ref: '', remote: true })).toBe('')
  })
})

describe('the labels a side is drawn with', () => {
  /* The prefix apart from the name, because the window draws the prefix muted
     and the name in the ordinary colour — which is the whole of how `local` and
     `origin` are said now that there is no second control for them. */
  it('splits the remote prefix off the name', () => {
    expect(sideLabel(origin('main'))).toEqual({ prefix: 'origin/', ref: 'main' })
    expect(sideLabel(local('main'))).toEqual({ prefix: '', ref: 'main' })
    expect(sideLabel(null)).toEqual({ prefix: '', ref: '' })
  })

  it('does the same for the pair one row draws', () => {
    expect(pairLabel({ base: local('main'), head: origin('feat/x') })).toEqual({
      base: { prefix: '', ref: 'main' },
      head: { prefix: 'origin/', ref: 'feat/x' }
    })
  })
})

describe('reviewPairs', () => {
  /* The exit, and it has not moved: `{ repo, base, head }` apiece with the refs
     resolved, which is `src-tauri/src/agents/mod.rs`' `ReviewPair`. */
  it('resolves every row into the pair of refs git takes', () => {
    let form = opened()
    form = withPick(form, { side: 'base', repoId: null }, origin('main'))
    form = withPick(form, { side: 'head', repoId: '/p/admin' }, local('feat/y'))
    expect(reviewPairs(form)).toEqual([
      { repo: '/p', base: 'origin/main', head: 'feat/x' },
      { repo: '/p/admin', base: 'origin/main', head: 'feat/y' }
    ])
  })

  it('has nothing to send for a form nobody has filled in', () => {
    expect(reviewPairs(reviewForm(REPOS, null, { branches: BRANCHES }))).toEqual([])
  })
})

describe('fetchTargets', () => {
  it('names nothing when every side is local', () => {
    expect(fetchTargets(opened())).toEqual([])
  })

  /* `origin/main` is only as current as the last fetch, so a review that reads
     it without one is a review of a commit nobody asked about — and nothing on
     screen would have said so. */
  it('names a repository with origin on either side of its own pair', () => {
    const rule = withPick(opened(), { side: 'base', repoId: null }, origin('main'))
    expect(fetchTargets(rule)).toEqual(['/p', '/p/admin'])
    const one = withPick(opened(), { side: 'head', repoId: '/p/admin' }, origin('feat/x'))
    expect(fetchTargets(one)).toEqual(['/p/admin'])
  })

  /* A row that differs is fetched on its own terms: an override back on the
     local branch needs no fetch even under a rule that reads origin. */
  it('leaves out a row whose override is local under a remote rule', () => {
    let form = withPick(opened(), { side: 'base', repoId: null }, origin('main'))
    form = withOverride(form, '/p/admin')
    form = withPick(form, { side: 'base', repoId: '/p/admin' }, local('main'))
    expect(fetchTargets(form)).toEqual(['/p'])
  })
})

describe('fetchFailures', () => {
  /* The verdicts arrive as `Promise.all` left them — one per target, in the
     order the targets were fetched in — and the answer is the paths, because
     that is what a row is keyed by, what a pair names a repository by and what
     the prompt lists them in. */
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

describe('branchesIn', () => {
  /* One project-wide answer filtered per row, rather than a second git read for
     every repository in the table. The records travel whole, because the list
     this fills draws an age and a repository count off them. */
  it('leaves out the branches this repository is short of', () => {
    expect(branchesIn(BRANCHES, '.').map((b) => b.name)).toEqual(['main', 'feat/x'])
    expect(branchesIn(BRANCHES, 'shared').map((b) => b.name)).toEqual(['main'])
  })

  /* `missing_in` is where a branch is absent across the *project*, and the list
     this fills subtracts it from however many repositories it is drawn
     against — one, here. Left in place, a branch absent from two other
     repositories came out as `local · 0 repos`: the list denying that anybody
     has the branch, directly under the field saying it is the one being
     checked. */
  it('scopes each record to the one repository the list is drawn for', () => {
    expect(branchesIn(BRANCHES, '.').map((b) => b.missing_in)).toEqual([[], []])
  })

  it('does not touch the records it was given', () => {
    branchesIn(BRANCHES, '.')
    expect(BRANCHES[1].missing_in).toEqual(['shared'])
  })

  it('has nothing to offer before the list has landed', () => {
    expect(branchesIn(null, '.')).toEqual([])
  })
})

describe('the words under and around the table', () => {
  it('names the two fields under them and says what a choice there reaches', () => {
    expect(ruleCaption(opened())).toBe(RULE_CAPTION)
    expect(ruleCaption(reviewForm(REPOS, null, { branches: BRANCHES }))).toBe(RULE_CAPTION_EMPTY)
  })

  /* The one thing that tells a list opened for the whole project from one
     opened for a single row, and those two do very different things. */
  it('says what picking a branch applies to', () => {
    expect(pickerScope()).toBe('sets every repository below')
    expect(pickerScope('backend')).toBe('this repository only · backend')
  })

  /* The verb follows the count, always: `1 follow the rule` over a row saying
     `follows the rule` is the sort of sentence that makes a person doubt
     everything else on the screen. */
  it('counts the rows and how many of them follow the rule', () => {
    expect(tableSummary(opened())).toBe('2 · all follow the rule')
    expect(tableSummary(withOverride(opened(), '/p'))).toBe('2 · 1 follows the rule · 1 differs')
    const wide = reviewForm(REPOS, 'main', { branches: BRANCHES })
    expect(tableSummary(withOverride(wide, '/p'))).toBe('3 · 2 follow the rule · 1 differs')
  })

  it('summarises what a press of Review would start', () => {
    expect(footerSummary(opened())).toBe('2 pairs')
    expect(footerSummary(withOverride(opened(), '/p'))).toBe('2 pairs · 1 override')
    expect(footerSummary(opened(), { notes: 2 })).toBe('2 pairs · 2 notes')
    expect(footerSummary(reviewForm(REPOS, null, { branches: BRANCHES }))).toBe('0 pairs')
  })

  /* In busy the button that said what would happen has just gone quiet, and
     this sentence is the only thing left saying why. */
  it('says what is happening instead once it is running', () => {
    expect(footerSummary(opened(), { busy: true })).toBe('starting the review session · 2 pairs')
  })

  it('keeps the verb and the noun in step for one of anything', () => {
    const one = { ...opened(), repoIds: ['/p'] }
    expect(footerSummary(one)).toBe('1 pair')
  })
})

describe('rowStatus', () => {
  const NOW = 1_700_000_000

  it('says a row follows the rule, and says nothing about one that differs', () => {
    expect(rowStatus({}).text).toBe(FOLLOWS_THE_RULE)
    expect(rowStatus({ override: true }).text).toBe('')
  })

  it('turns while origin is being fetched', () => {
    expect(rowStatus({ fetching: true })).toEqual({
      text: 'fetching origin',
      icon: 'loader-circle',
      spin: true
    })
  })

  /* A fetch that did not reach the remote leaves the review reading the copy of
     origin already on this disk. It is how old the answer is and not an error,
     so it is a triangle in the muted colour and never red. */
  it('says how old the copy of origin is when the fetch did not reach it', () => {
    expect(rowStatus({ stale: true, at: NOW - 2 * 3600, now: NOW })).toEqual({
      text: 'using origin from 2h ago',
      icon: 'triangle-alert',
      spin: false
    })
  })

  /* A repository nobody has ever fetched into has no age to give, and
     `from  ago` with a hole in it would be worse than the shorter sentence. */
  it('leaves the age out when there is no fetch to date it by', () => {
    expect(rowStatus({ stale: true, at: null, now: NOW }).text).toBe('using origin from before')
  })
})

describe('reviewNotes', () => {
  /* One block of lines, each a glyph and a sentence with the identifiers in it
     drawn in mono — the sentence is prose and a repository's name is not. */
  it('says nothing at all when there is nothing to say', () => {
    expect(reviewNotes({})).toEqual([])
    expect(reviewNotes({ fetching: [], failed: [], missing: [] })).toEqual([])
  })

  it('counts the repositories origin is being fetched for', () => {
    const [note] = reviewNotes({ fetching: ['/p'] })
    expect(note.icon).toBe('loader-circle')
    expect(note.spin).toBe(true)
    expect(note.parts.map((part) => part.text).join('')).toBe(
      'Fetching origin for 1 repository.'
    )
    const [many] = reviewNotes({ fetching: ['/p', '/p/admin'] })
    expect(many.parts.map((part) => part.text).join('')).toBe(
      'Fetching origin for 2 repositories.'
    )
  })

  /* A failed fetch is a sentence and not a refusal: what origin holds on this
     machine is still readable, merely older. */
  it('says which repositories were not reached, without calling it off', () => {
    const [note] = reviewNotes({ failed: ['driver'] })
    expect(note.icon).toBe('triangle-alert')
    expect(note.parts.map((part) => part.text).join('')).toBe(
      'Fetch failed for driver. The review still runs and reads the copy of origin from before.'
    )
    expect(note.parts.filter((part) => part.mono).map((part) => part.text)).toEqual(['driver'])
  })

  it('names the repositories with no such branch in one line', () => {
    const [note] = reviewNotes({ missing: ['extension', 'docs'] })
    expect(note.icon).toBe('circle-dashed')
    expect(note.parts.map((part) => part.text).join('')).toBe(
      'No such branch in extension, docs. They are left out of the review.'
    )
    const [one] = reviewNotes({ missing: ['extension'] })
    expect(one.parts.map((part) => part.text).join('')).toBe(
      'No such branch in extension. It is left out of the review.'
    )
  })

  /* All three at once is the case the block was drawn for: three sentences
     loose under a table read as one paragraph, and each of these is about a
     different thing. */
  it('keeps the three in one order however many of them there are', () => {
    const notes = reviewNotes({ fetching: ['/p'], failed: ['driver'], missing: ['docs'] })
    expect(notes.map((note) => note.key)).toEqual(['fetching', 'failed', 'missing'])
  })
})

describe('oldestFetch', () => {
  /* One number stands for several repositories in the list opened for the
     project's rule, and the oldest is the only one that cannot mislead: a pair
     set for every repository is as stale as the least recently fetched of
     them. */
  it('answers the least recent of the fetch times', () => {
    expect(oldestFetch(['/p', '/p/admin'], { '/p': 200, '/p/admin': 100 })).toBe(100)
  })

  /* A repository nobody has ever fetched into takes the answer away entirely:
     there is no honest number for a project holding one, and the list then says
     `origin` with nothing after it. */
  it('has no answer at all when one repository has never been fetched', () => {
    expect(oldestFetch(['/p', '/p/admin'], { '/p': 200 })).toBe(null)
    expect(oldestFetch([], {})).toBe(null)
    expect(oldestFetch(null, null)).toBe(null)
  })
})

describe('missingRepos', () => {
  /* Anything already in the table is not among them, which is what keeps a note
     from being a sentence about a row directly above it. */
  it('names what is left out and never what is in the table', () => {
    const form = opened()
    expect(missingRepos(REPOS, form, { branches: BRANCHES }).map((r) => r.name)).toEqual(['shared'])
    const added = withRepo(form, '/p/shared')
    expect(missingRepos(REPOS, added, { branches: BRANCHES })).toEqual([])
  })

  /* Without a branch there is nothing to be missing. The `New review` door
     would otherwise open by naming every repository of the project under a
     sentence about a branch nobody has chosen yet. */
  it('names nobody while there is no branch to check', () => {
    const empty = reviewForm(REPOS, null, { branches: BRANCHES })
    expect(missingRepos(REPOS, empty, { branches: BRANCHES })).toEqual([])
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
