import { describe, expect, it } from 'vitest'
import { lockIds, parentBlocked, unlockIds } from '../../../src/components/kanban/lockCascade.js'

/* A small issue, with just the fields this module reads. */
const issue = (over = {}) => ({
  id: 'bd-1',
  status: 'open',
  parent: null,
  labels: [],
  ...over
})

describe('lockIds', () => {
  it('locks a lone task with no children', () => {
    const issues = [issue({ id: 'bd-1' })]
    expect(lockIds(issues, 'bd-1')).toEqual(['bd-1'])
  })

  it('writes children before the epic, deepest first', () => {
    // A grandchild, a child and the epic: the write order has to reach the
    // deepest issue first, so the epic is never blocked over an open
    // descendant.
    const issues = [
      issue({ id: 'epic', status: 'open' }),
      issue({ id: 'child', parent: 'epic', status: 'open' }),
      issue({ id: 'grandchild', parent: 'child', status: 'open' })
    ]
    expect(lockIds(issues, 'epic')).toEqual(['grandchild', 'child', 'epic'])
  })

  it('skips a descendant in a status other than open', () => {
    const issues = [
      issue({ id: 'epic', status: 'open' }),
      issue({ id: 'in-progress', parent: 'epic', status: 'in_progress' }),
      issue({ id: 'ready-to-merge', parent: 'epic', status: 'ready_to_merge' }),
      issue({ id: 'parked', parent: 'epic', status: 'parked' }),
      issue({ id: 'deferred', parent: 'epic', status: 'deferred' }),
      issue({ id: 'closed', parent: 'epic', status: 'closed' }),
      issue({ id: 'pinned', parent: 'epic', status: 'pinned' }),
      issue({ id: 'open-one', parent: 'epic', status: 'open' })
    ]
    expect(lockIds(issues, 'epic')).toEqual(['open-one', 'epic'])
  })

  it('leaves the epic out when it is not itself open', () => {
    // A task already claimed by an agent: locking its still-open siblings must
    // not also write the epic, which is somebody else's business right now.
    const issues = [
      issue({ id: 'epic', status: 'in_progress' }),
      issue({ id: 'child', parent: 'epic', status: 'open' })
    ]
    expect(lockIds(issues, 'epic')).toEqual(['child'])
  })

  it('reaches a descendant several levels down through parent, not only direct children', () => {
    const issues = [
      issue({ id: 'epic', status: 'open' }),
      issue({ id: 'mid', parent: 'epic', status: 'closed' }),
      issue({ id: 'leaf', parent: 'mid', status: 'open' })
    ]
    expect(lockIds(issues, 'epic')).toEqual(['leaf', 'epic'])
  })

  it('never offers the merge lock as a descendant to write', () => {
    const issues = [
      issue({ id: 'epic', status: 'open' }),
      issue({ id: 'lock-issue', parent: 'epic', status: 'open', labels: ['smetana-lock'] }),
      issue({ id: 'child', parent: 'epic', status: 'open' })
    ]
    expect(lockIds(issues, 'epic')).toEqual(['child', 'epic'])
  })
})

describe('unlockIds', () => {
  it('unlocks a lone task with no children', () => {
    const issues = [issue({ id: 'bd-1', status: 'blocked' })]
    expect(unlockIds(issues, 'bd-1')).toEqual(['bd-1'])
  })

  it('writes the epic before its locked descendants', () => {
    const issues = [
      issue({ id: 'epic', status: 'blocked' }),
      issue({ id: 'child', parent: 'epic', status: 'blocked' }),
      issue({ id: 'grandchild', parent: 'child', status: 'blocked' })
    ]
    expect(unlockIds(issues, 'epic')).toEqual(['epic', 'child', 'grandchild'])
  })

  it('the 3-ready-plus-3-dependent scenario: only the locked descendants come back with the epic', () => {
    // Three siblings ready on their own, three each blocked on one of the
    // first three by an ordinary `blocks` dependency: locking the epic put all
    // six into `blocked`. Unblocking releases the epic and the three that were
    // only along for the epic's own lock; the other three are this module's
    // business no further — `boardColumns` puts them back in Blocked on its
    // own once they are `open` again, because their dependency is still there.
    const issues = [
      issue({ id: 'epic', status: 'blocked' }),
      issue({ id: 'a1', parent: 'epic', status: 'blocked' }),
      issue({ id: 'a2', parent: 'epic', status: 'blocked' }),
      issue({ id: 'a3', parent: 'epic', status: 'blocked' }),
      issue({ id: 'b1', parent: 'epic', status: 'blocked' }),
      issue({ id: 'b2', parent: 'epic', status: 'blocked' }),
      issue({ id: 'b3', parent: 'epic', status: 'blocked' })
    ]
    expect(unlockIds(issues, 'epic')).toEqual(['epic', 'a1', 'a2', 'a3', 'b1', 'b2', 'b3'])
  })

  it('skips a descendant that was never locked', () => {
    const issues = [
      issue({ id: 'epic', status: 'blocked' }),
      issue({ id: 'still-running', parent: 'epic', status: 'in_progress' }),
      issue({ id: 'was-locked', parent: 'epic', status: 'blocked' })
    ]
    expect(unlockIds(issues, 'epic')).toEqual(['epic', 'was-locked'])
  })

  it('leaves the epic out when it is not itself locked', () => {
    const issues = [
      issue({ id: 'epic', status: 'open' }),
      issue({ id: 'child', parent: 'epic', status: 'blocked' })
    ]
    expect(unlockIds(issues, 'epic')).toEqual(['child'])
  })

  it('never offers the merge lock as a descendant to write', () => {
    const issues = [
      issue({ id: 'epic', status: 'blocked' }),
      issue({ id: 'lock-issue', parent: 'epic', status: 'blocked', labels: ['smetana-lock'] }),
      issue({ id: 'child', parent: 'epic', status: 'blocked' })
    ]
    expect(unlockIds(issues, 'epic')).toEqual(['epic', 'child'])
  })
})

describe('parentBlocked', () => {
  it('is false for a task with no parent at all', () => {
    const issues = [issue({ id: 'bd-1' })]
    expect(parentBlocked(issues, 'bd-1')).toBe(false)
  })

  it('is false when the direct parent is not locked', () => {
    const issues = [
      issue({ id: 'epic', status: 'open' }),
      issue({ id: 'child', parent: 'epic', status: 'open' })
    ]
    expect(parentBlocked(issues, 'child')).toBe(false)
  })

  it('is true when the direct parent is locked', () => {
    const issues = [
      issue({ id: 'epic', status: 'blocked' }),
      issue({ id: 'child', parent: 'epic', status: 'blocked' })
    ]
    expect(parentBlocked(issues, 'child')).toBe(true)
  })

  it('reaches a locked grandparent even when the immediate parent is open', () => {
    // The child was locked by hand before the epic was — its own parent is
    // still `open`, and only the grandparent carries the lock.
    const issues = [
      issue({ id: 'epic', status: 'blocked' }),
      issue({ id: 'mid', parent: 'epic', status: 'open' }),
      issue({ id: 'child', parent: 'mid', status: 'blocked' })
    ]
    expect(parentBlocked(issues, 'child')).toBe(true)
  })

  it('is false about itself: a locked epic is not its own locked parent', () => {
    const issues = [issue({ id: 'epic', status: 'blocked' })]
    expect(parentBlocked(issues, 'epic')).toBe(false)
  })

  it('does not let the merge lock stand in as a blocking ancestor', () => {
    const issues = [
      issue({ id: 'lock-issue', status: 'blocked', labels: ['smetana-lock'] }),
      issue({ id: 'child', parent: 'lock-issue', status: 'blocked' })
    ]
    expect(parentBlocked(issues, 'child')).toBe(false)
  })
})
