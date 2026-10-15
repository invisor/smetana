import { describe, expect, it } from 'vitest'
import { createTracker } from '../../src/bridge/sessionState.js'

const ev = (seq, kind, extra = {}) => ({ seq, kind, ...extra })

describe('a driven session\'s state, from its events', () => {
  it('is starting before it has said anything', () => {
    expect(createTracker().seed(1, [], 'starting')).toBe('starting')
  })

  it('is running while a turn is open, and ready when it closes', () => {
    const tracker = createTracker()
    tracker.seed(1, [], 'starting')
    expect(tracker.absorb(1, [ev(1, 'turn-start', { by: 'person' })])).toBe('running')
    expect(tracker.absorb(1, [ev(2, 'result')])).toBe('ready')
  })

  it('waits on a person while a permission is open, and runs again when it is answered', () => {
    const tracker = createTracker()
    tracker.seed(1, [ev(1, 'turn-start')], 'running')
    expect(tracker.absorb(1, [ev(2, 'permission', { id: 'p1' })])).toBe('needs-you')
    expect(tracker.absorb(1, [ev(3, 'permission-answered', { id: 'p1' })])).toBe('running')
  })

  it('settles a question by its id, so an answer out of order leaves the other standing', () => {
    const tracker = createTracker()
    tracker.seed(1, [ev(1, 'turn-start'), ev(2, 'permission', { id: 'a' }), ev(3, 'permission', { id: 'b' })], 'needs-you')
    expect(tracker.absorb(1, [ev(4, 'permission-answered', { id: 'a' })])).toBeNull()
    expect(tracker.absorb(1, [ev(5, 'permission-answered', { id: 'b' })])).toBe('running')
  })

  it('waits on a person after the agent asks in words, until the next turn starts', () => {
    const tracker = createTracker()
    tracker.seed(1, [ev(1, 'turn-start'), ev(2, 'result')], 'ready')
    expect(tracker.absorb(1, [ev(3, 'text-question')])).toBe('needs-you')
    expect(tracker.absorb(1, [ev(4, 'turn-start')])).toBe('running')
  })

  it('a turn that failed is closed', () => {
    const tracker = createTracker()
    tracker.seed(1, [ev(1, 'turn-start')], 'running')
    expect(tracker.absorb(1, [ev(2, 'turn-failed')])).toBe('ready')
  })

  it('says nothing when a batch does not move the state', () => {
    const tracker = createTracker()
    tracker.seed(1, [ev(1, 'turn-start')], 'running')
    expect(tracker.absorb(1, [ev(2, 'text-delta'), ev(3, 'text')])).toBeNull()
  })

  it('works out the state of a snapshot from its whole journal', () => {
    const journal = [ev(1, 'turn-start'), ev(2, 'permission', { id: 'p' })]
    expect(createTracker().seed(1, journal, 'needs-you')).toBe('needs-you')
  })

  it('keeps a session that was over when it was read over', () => {
    const tracker = createTracker()
    tracker.seed(1, [ev(1, 'turn-start')], 'failed')
    expect(tracker.absorb(1, [ev(2, 'result')])).toBeNull()
    tracker.seed(2, [ev(1, 'turn-start'), ev(2, 'result')], 'exited')
    expect(tracker.absorb(2, [ev(3, 'turn-start')])).toBeNull()
  })

  it('has no state for a session nobody has read', () => {
    expect(createTracker().absorb(9, [ev(1, 'turn-start')])).toBeNull()
  })

  it('keeps sessions apart', () => {
    const tracker = createTracker()
    tracker.seed(1, [], 'starting')
    tracker.seed(2, [], 'starting')
    tracker.absorb(1, [ev(1, 'turn-start')])
    expect(tracker.absorb(2, [ev(1, 'turn-start')])).toBe('running')
  })

  it('reads the journal again from a new snapshot', () => {
    const tracker = createTracker()
    tracker.seed(1, [ev(1, 'turn-start')], 'running')
    tracker.seed(1, [ev(1, 'turn-start'), ev(2, 'result')], 'ready')
    expect(tracker.absorb(1, [ev(3, 'turn-start')])).toBe('running')
  })
})
