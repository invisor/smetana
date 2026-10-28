import { describe, expect, it } from 'vitest'
import { RUN_FROM, queueRun } from '../src/boardRun.js'

describe('starting the queue from the board', () => {
  it('stands in the Ready column', () => {
    expect(RUN_FROM).toBe('ready')
  })

  it.each(['ok', 'broken'])('is offered when the configuration is %s', (state) => {
    expect(queueRun(state, []).runnable).toBe(true)
  })

  it.each(['missing', '', undefined])('is not offered when the configuration is %j', (state) => {
    expect(queueRun(state, [])).toEqual({ runnable: false, blockedReason: '' })
  })

  it('can be pressed when no run is going', () => {
    expect(queueRun('ok', [])).toEqual({ runnable: true, blockedReason: '' })
  })

  it('is grey with a reason while a run over the queue is going', () => {
    const going = [{ token: 1, state: { kind: 'working', iteration: 1 }, settings: { scope: { kind: 'queue' } } }]
    const result = queueRun('ok', going)
    expect(result.runnable).toBe(true)
    expect(result.blockedReason).not.toBe('')
  })
})
