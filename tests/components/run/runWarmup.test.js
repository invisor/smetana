import { describe, expect, it } from 'vitest'
import { warmingUp } from '../../../src/components/run/runWarmup.js'

const run = (kind) => ({ state: { kind } })

describe('whether a run is started but has no session yet', () => {
  it('is true while the project is being brought up and the board is being read', () => {
    expect(warmingUp(run('preflight'))).toBe(true)
    expect(warmingUp(run('deciding'))).toBe(true)
  })

  it('is false once a batch is running, waiting, resting, paused or over', () => {
    for (const kind of ['working', 'paused', 'resting', 'waiting_for_agent', 'waiting_for_any_agent', 'stopped']) {
      expect(warmingUp(run(kind))).toBe(false)
    }
  })

  it('is false for no run and for a run with no state', () => {
    expect(warmingUp(null)).toBe(false)
    expect(warmingUp(undefined)).toBe(false)
    expect(warmingUp({})).toBe(false)
  })
})
