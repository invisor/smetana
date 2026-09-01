import { describe, expect, it } from 'vitest'
import {
  DROP_SPACE_LOGICAL,
  DROP_SPACE_PHYSICAL,
  DROP_SPACES,
  dropSpaceFromPlatform,
  viewportPoint
} from '../../../src/components/terminal/dropPoint.js'

describe('dropSpaceFromPlatform', () => {
  it('keeps either of the two words the back end may answer with', () => {
    expect(dropSpaceFromPlatform('logical')).toBe(DROP_SPACE_LOGICAL)
    expect(dropSpaceFromPlatform('physical')).toBe(DROP_SPACE_PHYSICAL)
  })

  /* A browser answers nothing at all, because the command is not there — and a
     browser reports no drops either, so this is a fallback for a rename rather
     than for a mode anybody runs in. Physical is what every platform was read
     as before the units were measured. */
  it('reads anything else as physical', () => {
    expect(dropSpaceFromPlatform(undefined)).toBe(DROP_SPACE_PHYSICAL)
    expect(dropSpaceFromPlatform(null)).toBe(DROP_SPACE_PHYSICAL)
    expect(dropSpaceFromPlatform('points')).toBe(DROP_SPACE_PHYSICAL)
    expect(dropSpaceFromPlatform(2)).toBe(DROP_SPACE_PHYSICAL)
  })

  it('the closed list is those two and nothing else', () => {
    expect(DROP_SPACES).toEqual(['logical', 'physical'])
  })
})

describe('viewportPoint', () => {
  /* Windows: the point is in device pixels off the client area, so the ratio is
     what turns it into the CSS pixels the hit test reads. */
  it('divides a physical point by the device pixel ratio', () => {
    expect(viewportPoint({ x: 800, y: 600 }, DROP_SPACE_PHYSICAL, 2)).toEqual({ x: 400, y: 300 })
    expect(viewportPoint({ x: 800, y: 600 }, DROP_SPACE_PHYSICAL, 1)).toEqual({ x: 800, y: 600 })
  })

  /* macOS and Linux: AppKit points and GTK widget coordinates are already the
     webview's own CSS pixels, and this is the whole of the bug — the division
     used to happen here too, so on a Retina Mac a drag in the middle of the
     agent panel was hit-tested a quarter of the way into the window. */
  it('leaves a logical point exactly where it arrived, at any ratio', () => {
    expect(viewportPoint({ x: 800, y: 600 }, DROP_SPACE_LOGICAL, 2)).toEqual({ x: 800, y: 600 })
    expect(viewportPoint({ x: 800, y: 600 }, DROP_SPACE_LOGICAL, 1)).toEqual({ x: 800, y: 600 })
    expect(viewportPoint({ x: 800, y: 600 }, DROP_SPACE_LOGICAL, 1.5)).toEqual({ x: 800, y: 600 })
  })

  it('keeps the fractional part rather than rounding to a whole pixel', () => {
    expect(viewportPoint({ x: 5, y: 7 }, DROP_SPACE_PHYSICAL, 2)).toEqual({ x: 2.5, y: 3.5 })
  })

  /* The corner is the point the acceptance of this gesture is measured at, and
     zero has to survive both readings unchanged. */
  it('leaves the top left corner at the top left corner', () => {
    expect(viewportPoint({ x: 0, y: 0 }, DROP_SPACE_PHYSICAL, 2)).toEqual({ x: 0, y: 0 })
    expect(viewportPoint({ x: 0, y: 0 }, DROP_SPACE_LOGICAL, 2)).toEqual({ x: 0, y: 0 })
  })

  /* A ratio of zero would put every point at infinity, and a hit test cannot
     report that something went wrong — it would simply refuse every drop. */
  it('treats a ratio that is not a positive number as one', () => {
    expect(viewportPoint({ x: 40, y: 60 }, DROP_SPACE_PHYSICAL, 0)).toEqual({ x: 40, y: 60 })
    expect(viewportPoint({ x: 40, y: 60 }, DROP_SPACE_PHYSICAL, undefined)).toEqual({ x: 40, y: 60 })
    expect(viewportPoint({ x: 40, y: 60 }, DROP_SPACE_PHYSICAL, -2)).toEqual({ x: 40, y: 60 })
  })
})
