import { describe, expect, it } from 'vitest'
import {
  BATCH_PAUSE_CEILING,
  BATCH_PAUSE_DEFAULTS,
  isValidPause,
  pauseFrom,
  pausePayload,
  restDetail,
  restLabel,
  toMinutes,
  validatePause
} from '../../../src/components/run/batchPause.js'

describe('pauseFrom', () => {
  it('opens on 10 and 30 when nothing is remembered', () => {
    expect(pauseFrom(null)).toEqual({ min: 10, max: 30 })
    expect(pauseFrom({})).toEqual({ min: 10, max: 30 })
    expect(BATCH_PAUSE_DEFAULTS).toEqual({ min: 10, max: 30 })
  })

  it('takes what the project remembers, each half on its own', () => {
    expect(pauseFrom({ batchPauseMin: 2, batchPauseMax: 5 })).toEqual({ min: 2, max: 5 })
    expect(pauseFrom({ batchPauseMin: 0 })).toEqual({ min: 0, max: 30 })
  })
})

describe('toMinutes', () => {
  it('reads whole numbers and digit strings', () => {
    expect(toMinutes(7)).toBe(7)
    expect(toMinutes(' 12 ')).toBe(12)
    expect(toMinutes('0')).toBe(0)
  })

  it('refuses everything else, and keeps zero apart from not-a-number', () => {
    for (const bad of ['', '  ', 'x', '-1', '2.5', 2.5, null, undefined, NaN]) {
      expect(toMinutes(bad), String(bad)).toBeNull()
    }
  })
})

describe('validatePause', () => {
  it('accepts the defaults, a fixed pause and no pause at all', () => {
    expect(validatePause(10, 30)).toEqual({})
    expect(validatePause(15, 15)).toEqual({})
    expect(validatePause(0, 0)).toEqual({})
  })

  it('accepts both ends of the range', () => {
    expect(isValidPause(0, BATCH_PAUSE_CEILING)).toBe(true)
    expect(isValidPause(BATCH_PAUSE_CEILING, BATCH_PAUSE_CEILING)).toBe(true)
  })

  it('refuses a pair that runs backwards', () => {
    expect(validatePause(30, 10).max).toBeTruthy()
    expect(isValidPause(1, 0)).toBe(false)
  })

  it('refuses a number outside 0 to 720 and names the field', () => {
    expect(validatePause(10, 721)).toHaveProperty('max')
    expect(validatePause(721, 721)).toHaveProperty('min')
    expect(validatePause(-1, 5)).toHaveProperty('min')
  })

  it('refuses empty and fractional text', () => {
    expect(isValidPause('', 30)).toBe(false)
    expect(isValidPause(10, '')).toBe(false)
    expect(isValidPause('1.5', 30)).toBe(false)
  })

  it('accepts what a number field hands back as text', () => {
    expect(isValidPause('10', '30')).toBe(true)
  })
})

describe('pausePayload', () => {
  it('sends numbers for a valid pair and null for an invalid one', () => {
    expect(pausePayload('10', '30')).toEqual({ min: 10, max: 30 })
    expect(pausePayload(30, 10)).toBeNull()
  })
})

describe('the resting words', () => {
  const state = { kind: 'resting', until: '2026-10-02T12:30:00Z', minutes: 17 }

  it('says how many minutes the pause is', () => {
    expect(restLabel(state)).toBe('Pausing 17 min before the next batch')
  })

  it('says nothing for any other state', () => {
    expect(restLabel({ kind: 'paused' })).toBeNull()
    expect(restDetail({ kind: 'working' })).toBeNull()
  })

  it('gives an end time, and none for an unreadable one', () => {
    expect(restDetail(state, 'en-GB')).toMatch(/^until \d\d:\d\d$/)
    expect(restDetail({ ...state, until: 'soon' })).toBeNull()
  })
})
