import { describe, expect, it } from 'vitest'
import { promoteTitle, taskCount } from '../../../src/components/kanban/promoteTitle.js'

describe('taskCount', () => {
  it('says the singular for one and the plural for anything else', () => {
    expect(taskCount(1)).toBe('1 task')
    expect(taskCount(2)).toBe('2 tasks')
    expect(taskCount(0)).toBe('0 tasks')
  })
})

describe('promoteTitle', () => {
  it('asks the question while nothing has been attempted', () => {
    expect(promoteTitle({ count: 12 })).toBe('Move 12 tasks to ready?')
  })

  it('asks about one task in the singular', () => {
    expect(promoteTitle({ count: 1 })).toBe('Move 1 task to ready?')
  })

  it('reports rather than asks once anything has been attempted', () => {
    expect(promoteTitle({ count: 12, moved: 9, failed: 3 })).toBe('Moved 9 of 12')
  })

  it('reports a run that lost none, since a zero is still an answer', () => {
    expect(promoteTitle({ count: 12, moved: 12, failed: 0 })).toBe('Moved 12 of 12')
  })

  it('treats progress with nothing attempted yet as the question it still is', () => {
    expect(promoteTitle({ count: 12, moved: 4, failed: null })).toBe('Move 12 tasks to ready?')
  })
})
