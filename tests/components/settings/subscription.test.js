import { describe, expect, it } from 'vitest'
import {
  SUBSCRIPTION_STEPS,
  isThreshold,
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
