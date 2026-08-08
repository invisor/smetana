import { describe, expect, it } from 'vitest'
import { pickBranch } from '../../../src/components/run/branchChoice.js'

describe('the branch the run dialog opens on', () => {
  it('takes what this project was left at last time', () => {
    expect(pickBranch(['main', 'staging'], 'staging', 'main')).toBe('staging')
  })

  it('falls to the project default when nothing was remembered', () => {
    expect(pickBranch(['main', 'staging'], null, 'staging')).toBe('staging')
    expect(pickBranch(['main', 'staging'], '', 'staging')).toBe('staging')
  })

  it('falls to the project default when the remembered branch is gone', () => {
    expect(pickBranch(['main', 'staging'], 'feature/merged-and-deleted', 'main')).toBe('main')
  })

  /* git_branches orders by the reflog, so the first entry is the branch most
     recently worked on — the best guess left once neither name survives. */
  it('falls to the most recent branch when neither name is in the list', () => {
    expect(pickBranch(['staging', 'main'], 'gone', 'also-gone')).toBe('staging')
  })

  it('never offers a branch that does not exist', () => {
    expect(pickBranch([], 'main', 'staging')).toBe('')
    expect(pickBranch(['staging'], 'main', 'main')).toBe('staging')
  })

  /* The dialog reads this while its branches are still on their way, and an
     absent list must leave the field empty rather than throw under it. */
  it('an absent list is an empty field, not a failure', () => {
    expect(pickBranch(undefined, 'main', 'staging')).toBe('')
    expect(pickBranch(null, null, null)).toBe('')
  })
})
