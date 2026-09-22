import { describe, expect, it } from 'vitest'
import { failoverDetail, failoverLabel } from '../../../src/components/run/failoverVoice.js'

describe('run failover footer voice', () => {
  it('names a specific limited agent without making it an error', () => {
    expect(failoverLabel({ kind: 'waiting_for_agent', agent: 'codex', pct: 100 }))
      .toBe('Waiting for codex (100%)')
    expect(failoverDetail({ kind: 'waiting_for_agent', resets: 'at 11:00' }))
      .toBe('resets at 11:00')
  })

  it('distinguishes an all-limited wait from an unknown reset', () => {
    expect(failoverLabel({ kind: 'waiting_for_any_agent' })).toBe('Waiting for any available agent')
    expect(failoverDetail({ kind: 'waiting_for_any_agent' })).toBe('re-checking every minute')
  })

  it('names both sides of a safe-boundary switch', () => {
    expect(failoverLabel({ kind: 'switching_agent', from: 'claude', to: 'codex' }))
      .toBe('Switching from claude to codex')
    expect(failoverDetail({ kind: 'switching_agent' })).toBe(null)
    expect(failoverLabel({ kind: 'working' })).toBe(null)
  })
})
