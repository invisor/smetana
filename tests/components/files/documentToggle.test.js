import { describe, expect, it } from 'vitest'
import {
  TOGGLE_BOX,
  TOGGLE_INSET,
  TOGGLE_LANE
} from '../../../src/components/files/documentToggle.js'

/* The lane an html tab keeps clear for its eye/code toggle.

   There is little here to compute, and that is the point: what these pin is that
   the reserved width and the control that sits in it are built from the same
   pieces. They were not, once — the control was laid over the tab's corner while
   the corner already held `FileEditor`'s stale-file band and the search panel's
   close button — and no runner in this repository could see it, both sides being
   `.vue`. This is the half of that guarantee a test can hold: the day somebody
   gives the button a different size, the lane has to move with it or these
   fail. */

describe('the toggle lane', () => {
  it('reserves the control and its inset on both sides', () => {
    expect(TOGGLE_LANE).toBe(`calc(${TOGGLE_BOX} + ${TOGGLE_INSET} * 2)`)
  })

  it('is built from the control the toggle actually draws', () => {
    // `IconButton size="sm"` is a square of `--control-h-sm`, and the toggle
    // puts a border of its own around it so a glyph never sits directly on
    // somebody's document.
    expect(TOGGLE_BOX).toContain('var(--control-h-sm)')
    expect(TOGGLE_BOX).toContain('var(--border-w)')
  })

  it('names tokens and never a length of its own', () => {
    // Both densities and both themes come from the tokens; a literal here would
    // be right in exactly one of the four combinations.
    for (const value of [TOGGLE_INSET, TOGGLE_BOX, TOGGLE_LANE]) {
      expect(value).not.toMatch(/\d+(px|rem|em)/)
    }
  })
})
