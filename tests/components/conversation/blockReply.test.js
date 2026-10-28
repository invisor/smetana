import { describe, expect, it } from 'vitest'
import { allowPlan, turnClosed } from '../../../src/components/conversation/blockReply.js'

describe('when an auto-mode block may be allowed', () => {
  it('interrupts first while the agent is mid-turn', () => {
    expect(allowPlan('running')).toBe('interrupt-then-send')
    expect(allowPlan('needs-you')).toBe('interrupt-then-send')
  })

  it('sends at once when the agent is idle', () => {
    for (const state of ['ready', 'starting', 'failed', 'exited']) expect(allowPlan(state)).toBe('send')
  })

  it('treats a state it has never heard of as idle, the way the journal does', () => {
    expect(allowPlan('something-new')).toBe('send')
    expect(allowPlan(undefined)).toBe('send')
  })

  it('waits for the turn to close on the session state alone', () => {
    expect(turnClosed('running')).toBe(false)
    expect(turnClosed('needs-you')).toBe(false)
    expect(turnClosed('ready')).toBe(true)
    expect(turnClosed('failed')).toBe(true)
    expect(turnClosed('exited')).toBe(true)
  })
})
