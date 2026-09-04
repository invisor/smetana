import { describe, expect, it } from 'vitest'
import {
  agentKey,
  conversationsOf,
  isPinned,
  moveAgent,
  orderAgents,
  togglePin
} from '../../../src/components/agent/agentOrder.js'

/* A live row: the worker's own counter for an id, and the conversation id the
   app chose at the spawn beside it. The two are deliberately unalike, since
   telling them apart is the whole subject of this file. */
const live = (id, conversation) => ({ id, conversation, state: 'running' })
/* A row with no conversation id — a run's batch, a fork, a harness that cannot
   be told one. It sits in the order and reaches no file. */
const keyless = (id) => ({ id, conversation: null, state: 'running' })
/* A start ticket: this window's own id, no conversation, gone in a second. */
const starting = (id) => ({ id, conversation: null, starting: true, state: 'running' })
/* And a row off `.smetana/agents.json`, whose id *is* its conversation id. */
const offline = (conversation) => ({ id: conversation, conversation, restored: true, state: 'done' })

describe('what a row is known by', () => {
  it('is the conversation id when there is one', () => {
    expect(agentKey(live(7, 'conv-a'))).toBe('conv-a')
    expect(agentKey(offline('conv-a'))).toBe('conv-a')
  })

  /* The fallback is this window's own and reaches no settings file. It exists
     so that a row without a conversation can still be dragged and still hold
     its place for as long as the window is open. */
  it('falls back to the row\'s own id when there is none', () => {
    expect(agentKey(keyless(7))).toBe(7)
    expect(agentKey(starting('start-1'))).toBe('start-1')
  })
})

describe('the panel\'s order', () => {
  /* The state every project is in until somebody drags something, and the
     reference is what says so: the caller tells "never arranged" from
     "arranged, and this is what it came to" without comparing contents. */
  it('gives back the very rows it was handed when nothing is stored', () => {
    const rows = [live(1, 'a'), live(2, 'b')]
    expect(orderAgents(rows, [], [])).toBe(rows)
    expect(orderAgents(rows, null, null)).toBe(rows)
  })

  it('draws the rows in the stored sequence', () => {
    const rows = [live(1, 'a'), live(2, 'b'), live(3, 'c')]
    expect(orderAgents(rows, ['c', 'a', 'b'], []).map((row) => row.id)).toEqual([3, 1, 2])
  })

  /* An agent started since the last drag has to appear, and the end is the only
     honest place for it: the neighbours it would have been slotted between have
     been moved by hand. */
  it('puts a row the stored order has never heard of at the end', () => {
    const rows = [live(1, 'a'), live(2, 'b'), live(3, 'c')]
    expect(orderAgents(rows, ['c', 'a'], []).map((row) => row.id)).toEqual([3, 1, 2])
  })

  /* Passed over rather than pruned: the sessions of yesterday are offered back
     as offline rows, and they find the place they were left in. */
  it('passes over a stored id that matches nothing', () => {
    const rows = [live(1, 'a'), live(2, 'b')]
    expect(orderAgents(rows, ['gone', 'b', 'also-gone', 'a'], []).map((row) => row.id)).toEqual([
      2, 1
    ])
  })

  /* One flat zone: a start and an offline record are dragged past a live
     session like anything else, and a row with no conversation holds its place
     by its own id for as long as this window lives. */
  it('orders live rows, starts and offline records in one list', () => {
    const rows = [live(1, 'a'), starting('start-1'), offline('b')]
    expect(orderAgents(rows, ['b', 'start-1', 'a'], []).map((row) => row.id)).toEqual([
      'b',
      'start-1',
      1
    ])
  })
})

describe('the pinned block', () => {
  it('lifts the pinned rows to the top, in the order they were pinned', () => {
    const rows = [live(1, 'a'), live(2, 'b'), live(3, 'c')]
    expect(orderAgents(rows, [], ['c', 'a']).map((row) => row.id)).toEqual([3, 1, 2])
  })

  /* A row with no conversation id cannot be pinned at all: there would be
     nothing to remember it by, which is the one thing a pin has to do. Here the
     stored pin happens to be the row's own id, and it is still refused. */
  it('never takes a row with no conversation id into the block', () => {
    const rows = [live(1, 'a'), keyless(7)]
    expect(orderAgents(rows, [], [7, '7', 'a']).map((row) => row.id)).toEqual([1, 7])
    expect(isPinned(keyless(7), [7])).toBe(false)
    expect(isPinned(live(1, 'a'), ['a'])).toBe(true)
  })

  /* Inside the block the dragged order rules, which is what makes dragging one
     pinned row past another work at all — and what keeps the two lists separate
     fields. */
  it('orders the block by the dragged order where it knows the rows', () => {
    const rows = [live(1, 'a'), live(2, 'b'), live(3, 'c')]
    expect(orderAgents(rows, ['b', 'a', 'c'], ['a', 'b']).map((row) => row.id)).toEqual([2, 1, 3])
  })

  /* And a pin put on a row nobody has ever dragged falls in behind the ones the
     order does know, rather than in front of them. */
  it('puts a pinned row the order has never heard of behind those it has', () => {
    const rows = [live(1, 'a'), live(2, 'b'), live(3, 'c')]
    expect(orderAgents(rows, ['c'], ['a', 'c']).map((row) => row.id)).toEqual([3, 1, 2])
  })

  /* Unpinning is what returns a row to its place in the general list, and this
     is that: the same rows, the same stored order, one name out of the pins. */
  it('returns an unpinned row to the place the order still remembers', () => {
    const rows = [live(1, 'a'), live(2, 'b'), live(3, 'c')]
    expect(orderAgents(rows, ['a', 'b', 'c'], ['c']).map((row) => row.id)).toEqual([3, 1, 2])
    expect(orderAgents(rows, ['a', 'b', 'c'], []).map((row) => row.id)).toEqual([1, 2, 3])
  })

  /* A pinned agent whose session ended comes back as an offline row under the
     very same conversation id — which is the whole reason pinning exists. */
  it('keeps an offline row at the top under the same conversation id', () => {
    const rows = [live(1, 'a'), offline('b')]
    expect(orderAgents(rows, [], ['b']).map((row) => row.id)).toEqual(['b', 1])
  })
})

describe('one row moved', () => {
  it('moves it, and answers by reference when it did not', () => {
    const order = ['a', 'b', 'c']
    expect(moveAgent(order, 0, 2)).toEqual(['b', 'c', 'a'])
    expect(moveAgent(order, 1, 1)).toBe(order)
    expect(moveAgent(order, -1, 1)).toBe(order)
    expect(moveAgent(order, 0, 9)).toBe(order)
  })
})

describe('what reaches settings.json', () => {
  /* Conversation ids alone. A session number written here would be the very
     thing this file exists to refuse: the counter starts at 1 again tomorrow. */
  it('is the conversation ids, in the drawn order', () => {
    expect(conversationsOf([offline('b'), keyless(7), live(1, 'a'), starting('start-1')])).toEqual([
      'b',
      'a'
    ])
  })
})

describe('pinning and unpinning', () => {
  it('appends a new pin and takes an old one out', () => {
    expect(togglePin([], 'a')).toEqual(['a'])
    expect(togglePin(['a'], 'b')).toEqual(['a', 'b'])
    expect(togglePin(['a', 'b'], 'a')).toEqual(['b'])
  })

  it('refuses a row with no conversation id, and copies rather than mutates', () => {
    const pins = ['a']
    expect(togglePin(pins, null)).toEqual(['a'])
    expect(togglePin(pins, null)).not.toBe(pins)
    expect(togglePin(null, 'a')).toEqual(['a'])
  })
})
