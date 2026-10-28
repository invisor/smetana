import { describe, expect, it } from 'vitest'
import { allowPlan, replyTarget, waitVerdict } from '../../../src/components/conversation/blockReply.js'

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

  it('waits while the turn runs, goes when it has closed and gives up on a question', () => {
    expect(waitVerdict('running')).toBe('wait')
    expect(waitVerdict('ready')).toBe('ready')
    expect(waitVerdict('failed')).toBe('ready')
    expect(waitVerdict('exited')).toBe('ready')
    expect(waitVerdict('needs-you')).toBe('abort')
  })

  it('sends to the pressed session while the panel still shows it and the block is open', () => {
    expect(replyTarget({ capturedId: 'a', currentId: 'a', blockOpen: true })).toBe('send')
  })

  it('aborts when the panel was re-pointed at another session meanwhile', () => {
    expect(replyTarget({ capturedId: 'a', currentId: 'b', blockOpen: true })).toBe('abort')
    expect(replyTarget({ capturedId: 'a', currentId: null, blockOpen: true })).toBe('abort')
  })

  it('aborts when the block is no longer open, or there was no session to capture', () => {
    expect(replyTarget({ capturedId: 'a', currentId: 'a', blockOpen: false })).toBe('abort')
    expect(replyTarget({ capturedId: null, currentId: null, blockOpen: true })).toBe('abort')
    expect(replyTarget({ capturedId: undefined, currentId: undefined, blockOpen: true })).toBe('abort')
  })
})
