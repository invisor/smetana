import { describe, expect, it } from 'vitest'
import { formatDuration, formatElapsedClock } from '../../../src/components/conversation/elapsed.js'

describe('the receipt duration', () => {
  it('spells anything under a second in milliseconds', () => {
    expect(formatDuration(840)).toBe('840 ms')
    expect(formatDuration(0)).toBe('0 ms')
  })

  it('spells a second up to a minute to a tenth of a second', () => {
    expect(formatDuration(4200)).toBe('4.2 s')
    expect(formatDuration(13000)).toBe('13.0 s')
  })

  it('spells a minute and over as minutes and whole seconds', () => {
    expect(formatDuration(124300)).toBe('2 m 04 s')
    expect(formatDuration(134000)).toBe('2 m 14 s')
  })

  it('reads a missing or non-numeric value as zero', () => {
    expect(formatDuration(undefined)).toBe('0 ms')
    expect(formatDuration(null)).toBe('0 ms')
  })
})

describe('the ticking clock', () => {
  it('counts whole seconds under a minute, with no space before the letter', () => {
    expect(formatElapsedClock(4000)).toBe('4s')
    expect(formatElapsedClock(12000)).toBe('12s')
  })

  it('rounds down to the whole second the strip is ticking on', () => {
    expect(formatElapsedClock(4999)).toBe('4s')
  })

  it('folds into minutes and seconds past sixty, one space between the two', () => {
    expect(formatElapsedClock(134000)).toBe('2m 14s')
  })

  it('never reads negative — a clock does not run backwards', () => {
    expect(formatElapsedClock(-500)).toBe('0s')
  })

  it('reads a missing or non-numeric value as zero', () => {
    expect(formatElapsedClock(undefined)).toBe('0s')
    expect(formatElapsedClock(null)).toBe('0s')
  })
})
