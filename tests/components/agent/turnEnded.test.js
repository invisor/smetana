import { describe, expect, it } from 'vitest'
import { turnEnded } from '../../../src/components/agent/turnEnded.js'

const map = (entries) => new Map(Object.entries(entries))

describe('turnEnded', () => {
  it.each(['ready', 'needs-you', 'done', 'failed'])('answers yes for running -> %s on one key', (next) => {
    expect(turnEnded(map({ 'conversation:1': 'running' }), map({ 'conversation:1': next }))).toBe(true)
  })

  it('answers no for running -> running', () => {
    expect(turnEnded(map({ a: 'running' }), map({ a: 'running' }))).toBe(false)
  })

  it('answers no for ready -> running, a start of a turn', () => {
    expect(turnEnded(map({ a: 'ready' }), map({ a: 'running' }))).toBe(false)
  })

  it('answers no for ready -> done, which was never running', () => {
    expect(turnEnded(map({ a: 'ready' }), map({ a: 'done' }))).toBe(false)
  })

  it('ignores a key present on one side only', () => {
    expect(turnEnded(map({ a: 'running' }), map({}))).toBe(false)
    expect(turnEnded(map({}), map({ a: 'ready' }))).toBe(false)
    expect(turnEnded(map({ a: 'running' }), map({ b: 'ready' }))).toBe(false)
  })

  it('answers yes when any one of several rows ended its turn', () => {
    expect(
      turnEnded(map({ a: 'running', b: 'running' }), map({ a: 'running', b: 'ready' }))
    ).toBe(true)
  })

  it('answers no for empty maps', () => {
    expect(turnEnded(new Map(), new Map())).toBe(false)
  })
})
