import { describe, expect, it } from 'vitest'
import {
  SUBSCRIPTION_STEPS,
  isThreshold,
  reconcile,
  thresholdOptions
} from '../../../src/components/settings/subscription.js'

describe('the subscription thresholds offered on the Agents tab', () => {
  it('offers off and then the ladder, in order', () => {
    expect(SUBSCRIPTION_STEPS).toEqual([0, 50, 60, 70, 75, 80, 85, 90, 95])
    expect(thresholdOptions()[0]).toEqual({ value: 0, label: 'Off' })
    expect(thresholdOptions().at(-1)).toEqual({ value: 95, label: '95%' })
  })

  it('knows a rung from anything else', () => {
    expect(isThreshold(90)).toBe(true)
    /* Off is a value somebody chose, not the absence of one. */
    expect(isThreshold(0)).toBe(true)
    expect(isThreshold(63)).toBe(false)
    expect(isThreshold('90')).toBe(false)
    expect(isThreshold(null)).toBe(false)
  })
})

describe('reconciling the two thresholds', () => {
  it('turns the reduced band off once it reaches the pause', () => {
    expect(reconcile(75, 75)).toBe(0)
    expect(reconcile(70, 75)).toBe(0)
  })

  it('leaves a reduced band with room under the pause alone', () => {
    expect(reconcile(90, 75)).toBe(75)
    expect(reconcile(80, 75)).toBe(75)
  })

  it('reconciles nothing against a pause that is off', () => {
    /* `0` is off rather than a percentage, so there is no pause to be under and
       a chosen reduced band stands — Rust's `pause_at != OFF` guard exactly. */
    expect(reconcile(0, 95)).toBe(95)
    expect(reconcile(0, 0)).toBe(0)
  })

  it('leaves a reduced band already off where it is', () => {
    expect(reconcile(90, 0)).toBe(0)
  })
})
