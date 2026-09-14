import { describe, it, expect } from 'vitest'
import { needsSetup, needsStart } from '../../../src/components/run/setupGate.js'

describe('the setup gate', () => {
  it('offers the setup over a populated folder with no file', () => {
    const config = { state: 'missing', empty: false }
    expect(needsSetup(config)).toBe(true)
    expect(needsStart(config)).toBe(false)
  })

  it('offers a founding session over an empty folder with no file', () => {
    const config = { state: 'missing', empty: true }
    expect(needsSetup(config)).toBe(false)
    expect(needsStart(config)).toBe(true)
  })

  it('offers neither once a file is there, whatever the folder holds', () => {
    for (const state of ['ok', 'broken']) {
      for (const empty of [true, false]) {
        expect(needsSetup({ state, empty })).toBe(false)
        expect(needsStart({ state, empty })).toBe(false)
      }
    }
  })

  it('reads a reply without the field as a populated folder', () => {
    // An older back end, or the mock: the setup is the safer of the two
    // offers, since its agent knows how to ask.
    expect(needsSetup({ state: 'missing' })).toBe(true)
    expect(needsStart({ state: 'missing' })).toBe(false)
  })
})
