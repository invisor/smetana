import { describe, expect, it } from 'vitest'
import {
  DEFAULT_WAIT_MINUTES,
  isWaitMinutes,
  movePriority,
  priorityOrder,
  waitOptions
} from '../../../src/components/settings/runFailover.js'

describe('run failover settings', () => {
  it('offers every supported waiting threshold', () => {
    expect(waitOptions()).toEqual([
      { value: 0, label: 'Immediately' },
      { value: 1, label: '1 minute' },
      { value: 5, label: '5 minutes' },
      { value: 10, label: '10 minutes' },
      { value: 15, label: '15 minutes' },
      { value: 30, label: '30 minutes' }
    ])
    expect(isWaitMinutes(DEFAULT_WAIT_MINUTES)).toBe(true)
    expect(isWaitMinutes(4)).toBe(false)
  })

  it('removes unknown and duplicate agents before appending missing supported agents', () => {
    expect(priorityOrder(['codex', 'missing', 'codex'], ['claude', 'codex'])).toEqual(['codex', 'claude'])
  })

  it('moves one priority without losing the full supported order', () => {
    expect(movePriority(['claude', 'codex'], ['claude', 'codex'], 'codex', -1)).toEqual(['codex', 'claude'])
    expect(movePriority(['claude'], ['claude', 'codex'], 'claude', -1)).toEqual(['claude', 'codex'])
  })
})
