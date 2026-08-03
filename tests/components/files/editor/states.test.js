import { beforeEach, describe, expect, it, vi } from 'vitest'

/* The module keeps a plain Map at module scope — it has to be recreated between
   tests, otherwise the previous test's entries survive into the next one. */
let states

beforeEach(async () => {
  vi.resetModules()
  states = await import('../../../../src/components/files/editor/states.js')
})

describe('the editor state cache', () => {
  it('an unknown path gives null, not undefined', () => {
    expect(states.peekState('no-such-file.txt')).toBe(null)
  })

  it('a stored state reads back together with its scroll position', () => {
    const state = { dummy: 'state' }
    states.putState('a.txt', state, 120)

    expect(states.peekState('a.txt')).toEqual({ state, scrollTop: 120 })
  })

  it('reading does not remove the entry: it is read twice per switch', () => {
    states.putState('a.txt', { one: 1 }, 0)

    states.peekState('a.txt')
    expect(states.peekState('a.txt')).not.toBe(null)
  })

  it('a repeat write replaces the previous one', () => {
    states.putState('a.txt', { one: 1 }, 10)
    states.putState('a.txt', { two: 2 }, 20)

    expect(states.peekState('a.txt')).toEqual({ state: { two: 2 }, scrollTop: 20 })
  })

  it('keepOnly drops paths outside the list and keeps the live ones', () => {
    states.putState('a.txt', { a: 1 }, 0)
    states.putState('b.txt', { b: 2 }, 0)
    states.putState('c.txt', { c: 3 }, 0)

    states.keepOnly(['a.txt', 'c.txt'])

    expect(states.peekState('a.txt')).not.toBe(null)
    expect(states.peekState('b.txt')).toBe(null)
    expect(states.peekState('c.txt')).not.toBe(null)
  })

  it('an empty list clears everything', () => {
    states.putState('a.txt', { a: 1 }, 0)
    states.keepOnly([])

    expect(states.peekState('a.txt')).toBe(null)
  })
})
