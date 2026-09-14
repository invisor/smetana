import { describe, expect, it } from 'vitest'
import { drivenRowId } from '../../../src/components/agent/drivenRows.js'
import { hasLiveAgent, selectedAttachTarget } from '../../../src/components/files/attachTarget.js'

/* The two row shapes `orderedAgentRows` merges: a PTY row keyed by the
   terminal worker's own number, and a driven row keyed by the prefix
   `drivenRowId` puts in front of a conversation's. Both carry `state` already
   in the design system's words — `toUiState`/`statusOf` have already run by
   the time this module ever sees a row. */
const pty = (id, state = 'running', extra = {}) => ({ id, state, ...extra })
const driven = (conversation, state = 'running') => ({ id: drivenRowId(conversation), state })

describe('hasLiveAgent', () => {
  it('is false with no rows at all', () => {
    expect(hasLiveAgent([])).toBe(false)
  })

  it('is true with a live PTY row', () => {
    expect(hasLiveAgent([pty(1, 'running')])).toBe(true)
  })

  it('is true with a live driven row', () => {
    expect(hasLiveAgent([driven(1, 'needs-you')])).toBe(true)
  })

  it('is false when every row has ended', () => {
    expect(hasLiveAgent([pty(1, 'done'), driven(2, 'failed')])).toBe(false)
  })

  it('is false for a start ticket, which has not become a session yet', () => {
    expect(hasLiveAgent([pty('start-1', 'running', { starting: true })])).toBe(false)
  })

  it('is false for a restored row, which has no process behind it', () => {
    expect(hasLiveAgent([pty(1, 'done', { restored: true })])).toBe(false)
  })
})

describe('selectedAttachTarget', () => {
  it('answers null with nothing selected', () => {
    expect(selectedAttachTarget({ selectedId: null, rows: [pty(1, 'running')] })).toBeNull()
  })

  it('answers null when the selection names no row the panel draws', () => {
    expect(selectedAttachTarget({ selectedId: 9, rows: [pty(1, 'running')] })).toBeNull()
  })

  it('targets the PTY road when the selected row is a live PTY session', () => {
    const rows = [pty(1, 'running'), driven(4, 'ready')]
    expect(selectedAttachTarget({ selectedId: 1, rows })).toEqual({ road: 'pty', id: 1 })
  })

  it('targets the driven road when the selected row is a live conversation', () => {
    const rows = [pty(1, 'running'), driven(4, 'ready')]
    expect(selectedAttachTarget({ selectedId: drivenRowId(4), rows })).toEqual({
      road: 'driven',
      id: 4
    })
  })

  it('answers null for a selected PTY row that has finished', () => {
    const rows = [pty(1, 'done')]
    expect(selectedAttachTarget({ selectedId: 1, rows })).toBeNull()
  })

  it('answers null for a selected PTY row that failed', () => {
    const rows = [pty(1, 'failed')]
    expect(selectedAttachTarget({ selectedId: 1, rows })).toBeNull()
  })

  it('answers null for a selected driven row whose conversation has ended', () => {
    const rows = [driven(4, 'done')]
    expect(selectedAttachTarget({ selectedId: drivenRowId(4), rows })).toBeNull()
  })

  it('answers null for a selected row that is still only a start ticket', () => {
    const rows = [pty('start-1', 'running', { starting: true })]
    expect(selectedAttachTarget({ selectedId: 'start-1', rows })).toBeNull()
  })

  it('answers null for a selected restored row, which has nothing live behind it', () => {
    const rows = [pty(1, 'done', { restored: true })]
    expect(selectedAttachTarget({ selectedId: 1, rows })).toBeNull()
  })

  it('does not fall back to some other live agent when the selection itself is dead', () => {
    // The whole safety of the gesture is that it reaches the agent the person
    // is looking at and never a substitute — see the header of `attachToAgent`
    // in `DesktopApp.vue`.
    const rows = [pty(1, 'done'), pty(2, 'running')]
    expect(selectedAttachTarget({ selectedId: 1, rows })).toBeNull()
  })
})
