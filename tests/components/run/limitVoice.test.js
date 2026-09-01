import { describe, expect, it } from 'vitest'
import { limitVoice } from '../../../src/components/run/limitVoice.js'

const run = (token, kind) => ({ token, state: { kind } })

describe('which paused run says why they are all waiting', () => {
  it('gives the sentence to the first paused run and to no other', () => {
    const runs = [run(1, 'working'), run(2, 'paused'), run(3, 'paused')]

    /* The oldest of the paused ones — the list is oldest first and the footer
       draws it in that order, so the sentence sits leftmost and stays put. */
    expect(limitVoice(runs)).toBe(2)
  })

  it('says nobody speaks when nothing is paused', () => {
    expect(limitVoice([run(1, 'working'), run(2, 'stopped')])).toBe(null)
    expect(limitVoice([])).toBe(null)
  })

  it('gives it to the only paused run when there is one', () => {
    expect(limitVoice([run(7, 'paused')])).toBe(7)
  })

  it('refuses to let every segment claim the sentence when the list is not one', () => {
    expect(limitVoice(null)).toBe(null)
    expect(limitVoice(undefined)).toBe(null)
  })

  it('passes over a run with no state at all rather than falling over on it', () => {
    expect(limitVoice([{ token: 1 }, run(2, 'paused')])).toBe(2)
  })
})
