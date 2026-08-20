import { describe, expect, it } from 'vitest'
import {
  HEADER_COLLAPSE,
  HEADER_HIDE_RAIL,
  RAIL_EXPAND,
  headerLabel,
  nextFromHeader,
  nextFromRail
} from '../../src/views/leftChrome.js'

/* The three stops, by the names the module's own comment gives them. */
const A = { railOpen: true, leftCollapsed: false }
const B = { railOpen: false, leftCollapsed: false }
const C = { railOpen: false, leftCollapsed: true }

describe('the header button', () => {
  it('hides the project rail first and leaves the panel where it is', () => {
    expect(nextFromHeader(A)).toEqual(B)
  })

  it('folds the whole column once the rail is already hidden', () => {
    expect(nextFromHeader(B)).toEqual(C)
  })

  it('has nothing to answer while the column is folded, since it is not drawn there', () => {
    expect(nextFromHeader(C)).toEqual(C)
    expect(nextFromHeader({ railOpen: true, leftCollapsed: true })).toEqual({
      railOpen: true,
      leftCollapsed: true
    })
  })
})

describe('the folded strip button', () => {
  it('closes the cycle by bringing back both the panel and the rail', () => {
    expect(nextFromRail()).toEqual(A)
  })

  it('reads no state, so what folded the column cannot change the way out', () => {
    /* A drag past COLLAPSE_SLACK while the rail was already hidden leaves the
       same C the second step of the cycle does, and takes the same way out.
       The function taking no argument at all is what makes that true by
       construction rather than by a branch somebody could add. */
    expect(nextFromRail).toHaveLength(0)
  })
})

describe('the cycle', () => {
  it('returns to where it started in three presses', () => {
    const b = nextFromHeader(A)
    const c = nextFromHeader(b)
    expect(nextFromRail()).toEqual(A)
    expect([b, c]).toEqual([B, C])
  })

  it('adds no state of its own beyond the two stored flags', () => {
    expect(Object.keys(nextFromHeader(A)).sort()).toEqual(['leftCollapsed', 'railOpen'])
    expect(Object.keys(nextFromRail()).sort()).toEqual(['leftCollapsed', 'railOpen'])
  })
})

describe('headerLabel', () => {
  it('says what the next press hides', () => {
    expect(headerLabel(A)).toBe(HEADER_HIDE_RAIL)
    expect(headerLabel(B)).toBe(HEADER_COLLAPSE)
  })

  it('is sentence case, like every other label in the app', () => {
    for (const label of [HEADER_HIDE_RAIL, HEADER_COLLAPSE, RAIL_EXPAND]) {
      expect(label).toBe(label[0].toUpperCase() + label.slice(1).toLowerCase())
    }
  })
})
