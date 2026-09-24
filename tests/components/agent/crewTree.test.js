import { describe, expect, it } from 'vitest'
import { canMessage, crewRows } from '../../../src/components/agent/crewTree.js'

describe('Crew agent tree', () => {
  it('keeps a stable hierarchy and does not offer a composer for a finished child', () => {
    const rows = crewRows([
      { id: 1, parent: null, state: 'running', canMessage: true, label: 'Lead' },
      { id: 2, parent: 1, state: 'running', canMessage: true, label: 'Worker A' },
      { id: 3, parent: 1, state: 'done', canMessage: true, label: 'Worker B' },
      { id: 4, parent: 2, state: 'waiting', canMessage: true, label: 'Nested worker' }
    ])
    expect(rows.map(({ id, depth }) => [id, depth])).toEqual([[1, 0], [2, 1], [4, 2], [3, 1]])
    expect(rows.find((row) => row.id === 2).canMessage).toBe(true)
    expect(rows.find((row) => row.id === 3).canMessage).toBe(false)
  })

  it('never turns a missing or terminal node into an addressed message target', () => {
    expect(canMessage(null)).toBe(false)
    expect(canMessage({ state: 'failed', canMessage: true })).toBe(false)
    expect(canMessage({ state: 'running', canMessage: false })).toBe(false)
  })
})
