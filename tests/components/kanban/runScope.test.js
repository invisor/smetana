import { describe, expect, it } from 'vitest'
import { cardScope, openChildren, runOffered, runnable } from '../../../src/components/kanban/runScope.js'

const issue = (id, extra = {}) => ({ id, status: 'open', ...extra })
const card = (extra = {}) => ({ id: 'x-1', status: 'ready', bdStatus: 'open', blockedBy: 0, ...extra })

describe('cardScope', () => {
  it('is an epic when a child is not finished, a task otherwise', () => {
    const issues = new Map([
      ['x-1', issue('x-1')],
      ['x-2', issue('x-2', { parent: 'x-1' })]
    ])
    expect(cardScope(issues, 'x-1')).toEqual({ kind: 'epic', id: 'x-1' })
    expect(cardScope(issues, 'x-2')).toEqual({ kind: 'task', id: 'x-2' })
  })

  it('ignores finished children, in a Map or an array', () => {
    const issues = [issue('x-1'), issue('x-2', { status: 'closed', parent: 'x-1' }), issue('x-3', { status: 'done', parent: 'x-1' })]
    expect(cardScope(issues, 'x-1').kind).toBe('task')
    expect(openChildren(issues, 'x-1')).toEqual([])
  })
})

describe('runOffered', () => {
  it('is a configured or a broken project and nothing else', () => {
    expect(runOffered('ok')).toBe(true)
    expect(runOffered('broken')).toBe(true)
    expect(runOffered('missing')).toBe(false)
    expect(runOffered(undefined)).toBe(false)
  })
})

describe('runnable', () => {
  it('refuses a run on done, blocked, parked and when not offered', () => {
    expect(runnable(card(), true)).toBe(true)
    expect(runnable(card({ status: 'done', bdStatus: 'closed' }), true)).toBe(false)
    expect(runnable(card({ status: 'blocked', bdStatus: 'blocked' }), true)).toBe(false)
    expect(runnable(card({ status: 'parked', bdStatus: 'parked' }), true)).toBe(false)
    expect(runnable(card(), false)).toBe(false)
  })

  it('refuses a card that waits on a blocker and would have to be moved', () => {
    expect(runnable(card({ status: 'blocked', bdStatus: 'open', blockedBy: 1 }), true)).toBe(false)
    expect(runnable(card({ status: 'running', bdStatus: 'in_progress', blockedBy: 1 }), true)).toBe(true)
  })
})
