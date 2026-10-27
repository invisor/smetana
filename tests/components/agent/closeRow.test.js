import { describe, expect, it } from 'vitest'
import { closeActsFor } from '../../../src/components/agent/closeRow.js'

describe('what closing an agent row does', () => {
  it('closes the whole crew from its lead', () => {
    expect(closeActsFor({ id: 'crew:1:1' })).toEqual({ kind: 'crew-root', root: 'crew:1:1' })
  })

  it('does nothing for a member of a crew', () => {
    expect(closeActsFor({ id: 'crew:1:2' })).toEqual({ kind: 'crew-child' })
  })

  it('closes, forgets and un-records a driven conversation', () => {
    expect(closeActsFor({ id: 'conversation:7', conversation: 'abc' })).toEqual({
      kind: 'driven',
      session: 7,
      recorded: 'abc'
    })
    expect(closeActsFor({ id: 'conversation:8' })).toEqual({ kind: 'driven', session: 8, recorded: null })
  })

  it('forgets the record behind an offline row', () => {
    expect(closeActsFor({ id: 'abc', conversation: 'abc', restored: true })).toEqual({
      kind: 'offered',
      conversation: 'abc'
    })
  })

  it('removes an ordinary terminal session', () => {
    expect(closeActsFor({ id: 4 })).toEqual({ kind: 'pty', id: 4 })
  })
})
