import { describe, expect, it } from 'vitest'
import { branchOptions, needsCutting, pickBranch } from '../../../src/components/run/branchChoice.js'

/* The list the field is handed: what the worker sends, which is a branch and
   the repositories that do not have it. */
const everywhere = (...names) => names.map((name) => ({ name, missing_in: [] }))

describe('which branch the run dialog opens on', () => {
  it('takes what this project was left at last time', () => {
    expect(pickBranch(everywhere('main', 'staging'), 'staging', 'main')).toBe('staging')
  })

  it('falls to the project default when nothing was remembered', () => {
    expect(pickBranch(everywhere('main', 'staging'), null, 'staging')).toBe('staging')
    expect(pickBranch(everywhere('main', 'staging'), '', 'staging')).toBe('staging')
  })

  it('falls to the project default when the remembered branch is gone', () => {
    expect(pickBranch(everywhere('main', 'staging'), 'feature/merged-and-deleted', 'main')).toBe('main')
  })

  it('falls to the most recent branch when neither name is in the list', () => {
    expect(pickBranch(everywhere('staging', 'main'), 'gone', 'also-gone')).toBe('staging')
  })

  it('never offers a branch that does not exist', () => {
    expect(pickBranch([], 'main', 'staging')).toBe('')
    expect(pickBranch(everywhere('staging'), 'main', 'main')).toBe('staging')
  })

  it('an absent list is an empty field, not a failure', () => {
    expect(pickBranch(undefined, 'main', 'staging')).toBe('')
    expect(pickBranch(null, null, null)).toBe('')
  })

  /* A branch short of a repository is still a branch this project uses, and
     `[defaults].target_branch` may well name one. Skipping it would fall
     through to whatever the list happened to put first. */
  it('will open on a branch some repositories are missing', () => {
    const list = [{ name: 'main', missing_in: [] }, { name: 'release/7', missing_in: ['admin'] }]
    expect(pickBranch(list, null, 'release/7')).toBe('release/7')
  })
})

describe('whether choosing a branch means cutting one', () => {
  /* The defect this exists for: `develop` present in all four repositories was
     read as new because the list came from a fifth, and the run went out
     telling the agent to cut a branch that was already there. */
  it('says no about a branch every repository has', () => {
    expect(needsCutting(everywhere('develop', 'main'), 'develop')).toBe(false)
  })

  it('says yes about a name nothing in the list carries', () => {
    expect(needsCutting(everywhere('develop'), 'release/9')).toBe(true)
  })

  /* The half that has no equivalent in a single-repository project: the branch
     exists, and cutting it is still work that has to happen somewhere. */
  it('says yes about a branch some repositories are missing', () => {
    expect(needsCutting([{ name: 'release/7', missing_in: ['admin'] }], 'release/7')).toBe(true)
  })

  it('an absent list means anything named has to be cut', () => {
    expect(needsCutting(undefined, 'develop')).toBe(true)
    expect(needsCutting([], 'develop')).toBe(true)
  })
})

describe('the list the branch field draws', () => {
  /* The common case, and this project's own: one repository, so nothing can be
     short of anything and a caption would be a heading over the whole list. */
  it('draws no captions at all when every branch is everywhere', () => {
    expect(branchOptions(everywhere('develop', 'main'))).toEqual([
      { value: 'develop', label: 'develop' },
      { value: 'main', label: 'main' }
    ])
  })

  /* The other half of the same rule, and the reachable one: repositories on
     `main` and `master` with nothing else carry no branch in common, because the
     list always holds each repository's own HEAD. An `Everywhere` caption over
     nothing is pruned again by `Dropdown`, and the field's rows and its cursor
     then count different lists. */
  it('draws only the partial caption when no branch is everywhere', () => {
    const list = [
      { name: 'main', missing_in: ['extension'] },
      { name: 'master', missing_in: ['admin'] }
    ]
    expect(branchOptions(list)).toEqual([
      { header: true, label: 'Not everywhere' },
      { value: 'main', label: 'main', note: 'not in extension' },
      { value: 'master', label: 'master', note: 'not in admin' }
    ])
  })

  it('splits the two groups and says where a partial branch is missing', () => {
    const list = [
      { name: 'develop', missing_in: [] },
      { name: 'release/7', missing_in: ['admin', 'extension'] }
    ]
    expect(branchOptions(list)).toEqual([
      { header: true, label: 'Everywhere' },
      { value: 'develop', label: 'develop' },
      { header: true, label: 'Not everywhere' },
      { value: 'release/7', label: 'release/7', note: 'not in admin, extension' }
    ])
  })

  /* The worker sorts; this only names the groups. Reordering here would be a
     second ordering rule quietly disagreeing with `by_recency`. */
  it('keeps the order it was given inside each group', () => {
    const list = [
      { name: 'b', missing_in: [] },
      { name: 'a', missing_in: [] },
      { name: 'd', missing_in: ['x'] },
      { name: 'c', missing_in: ['x'] }
    ]
    expect(branchOptions(list).filter((o) => !o.header).map((o) => o.value)).toEqual(['b', 'a', 'd', 'c'])
  })

  it('an absent list is an empty list', () => {
    expect(branchOptions(undefined)).toEqual([])
    expect(branchOptions(null)).toEqual([])
  })
})
