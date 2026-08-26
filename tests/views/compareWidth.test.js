import { describe, expect, it } from 'vitest'
import {
  DIFF_MIN,
  LIST_DEFAULT,
  LIST_MIN,
  clampListWidth,
  maxListWidth,
  resolveDrag
} from '../../src/views/compareWidth.js'

/* The window as `compare_window_open` builds it, and as narrow as it lets
   anybody drag it. Both floors fit inside the second one, which is the property
   the constants were chosen for. */
const SHIPPED = 1040
const NARROWEST = 640

describe('maxListWidth', () => {
  it('leaves the diff its floor and hands the rest to the list', () => {
    expect(maxListWidth(SHIPPED)).toBe(SHIPPED - DIFF_MIN)
  })

  it('still leaves room for the list at the narrowest window', () => {
    expect(maxListWidth(NARROWEST)).toBeGreaterThanOrEqual(LIST_MIN)
  })
})

describe('clampListWidth', () => {
  it('leaves a width that breaks no rule alone', () => {
    expect(clampListWidth(LIST_DEFAULT, SHIPPED)).toBe(LIST_DEFAULT)
  })

  it('will not go below the width the mode switch needs', () => {
    expect(clampListWidth(40, SHIPPED)).toBe(LIST_MIN)
  })

  it('will not eat into the diff', () => {
    expect(clampListWidth(4000, SHIPPED)).toBe(SHIPPED - DIFF_MIN)
  })

  it('keeps the list whole and lets the diff take the squeeze', () => {
    // Narrower than both floors together: the list holds its minimum rather
    // than shrinking to a switch cut in half.
    expect(clampListWidth(LIST_DEFAULT, LIST_MIN + DIFF_MIN - 100)).toBe(LIST_MIN)
  })

  it('answers a whole number of pixels', () => {
    expect(clampListWidth(300.4, SHIPPED)).toBe(300)
  })
})

describe('resolveDrag', () => {
  it('follows the pointer between the two floors', () => {
    expect(resolveDrag({ base: LIST_DEFAULT, delta: 60, viewport: SHIPPED })).toBe(
      LIST_DEFAULT + 60
    )
    expect(resolveDrag({ base: LIST_DEFAULT, delta: -60, viewport: SHIPPED })).toBe(
      LIST_DEFAULT - 60
    )
  })

  it('measures every delta from the width the drag began at', () => {
    // Two moves of the same gesture, the second further left than the first.
    // Both are answered against `base`, so pulling back up hands the width back
    // rather than compounding what the clamp already swallowed.
    const past = resolveDrag({ base: LIST_DEFAULT, delta: -400, viewport: SHIPPED })
    expect(past).toBe(LIST_MIN)
    expect(resolveDrag({ base: LIST_DEFAULT, delta: -40, viewport: SHIPPED })).toBe(
      LIST_DEFAULT - 40
    )
  })
})
