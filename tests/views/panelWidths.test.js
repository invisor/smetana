import { describe, expect, it } from 'vitest'
import {
  CENTER_MIN,
  LEFT_DEFAULT,
  LEFT_MIN,
  RAIL,
  RAIL_CONTROL_MAX,
  RIGHT_MIN,
  clampWidth,
  maxWidth,
  resolveDrag
} from '../../src/views/panelWidths.js'

/* A window wide enough that neither cap is anywhere near binding, so a test
   about one rule is not quietly about another. */
const roomy = { other: 340, otherCollapsed: false, viewport: 1600 }

describe('maxWidth', () => {
  it('is a third of the window while the board still fits', () => {
    expect(maxWidth({ other: 340, otherCollapsed: false, viewport: 2400 })).toBe(800)
  })

  it('gives way to the board when a third would crowd it out', () => {
    // 1200/3 = 400, but 1200 - 340 - 400 = 460 is not the binding one; drop the
    // window until the board's floor bites.
    expect(maxWidth({ other: 340, otherCollapsed: false, viewport: 1000 })).toBe(
      1000 - 340 - CENTER_MIN
    )
  })

  it('counts a collapsed neighbour as a rail, not as its width', () => {
    const open = maxWidth({ other: 340, otherCollapsed: false, viewport: 1000 })
    const collapsed = maxWidth({ other: 340, otherCollapsed: true, viewport: 1000 })
    // Folding the neighbour hands its width back to the board's budget, and
    // this panel may take that room right up to the third-of-window cap —
    // which is what stops it short of the whole 340 - RAIL that was freed.
    expect(open).toBe(1000 - 340 - CENTER_MIN)
    expect(collapsed).toBeCloseTo(1000 / 3)
    expect(collapsed).toBeLessThan(open + (340 - RAIL))
  })
})

describe('clampWidth', () => {
  it('leaves a width that breaks no rule alone', () => {
    expect(clampWidth(300, { side: 'left', ...roomy })).toBe(300)
  })

  it('lifts anything below the panel minimum', () => {
    expect(clampWidth(40, { side: 'left', ...roomy })).toBe(LEFT_MIN)
    expect(clampWidth(40, { side: 'right', ...roomy })).toBe(RIGHT_MIN)
  })

  it('caps at a third of the window', () => {
    expect(clampWidth(9000, { side: 'left', other: 340, otherCollapsed: false, viewport: 2400 }))
      .toBe(800)
  })

  it('keeps the stored width intact — narrowing and widening restores it', () => {
    const stored = 500
    const narrow = clampWidth(stored, { side: 'left', other: 340, otherCollapsed: false, viewport: 1000 })
    expect(narrow).toBeLessThan(stored)
    expect(clampWidth(stored, { side: 'left', ...roomy })).toBe(stored)
  })

  it('keeps the panel usable when the window is too narrow for both rules', () => {
    // 600 - 340 - 400 is negative: something has to give, and it is the board.
    expect(clampWidth(252, { side: 'left', other: 340, otherCollapsed: false, viewport: 600 }))
      .toBe(LEFT_MIN)
  })
})

describe('resolveDrag', () => {
  const open = { base: 252, collapsed: false, ...roomy }

  it('grows the left panel rightwards and the right panel leftwards', () => {
    expect(resolveDrag('left', { ...open, delta: 60 }).width).toBe(312)
    expect(resolveDrag('right', { ...open, delta: -60 }).width).toBe(312)
  })

  it('collapses once the drag passes the slack below the minimum', () => {
    // 252 - 140 = 112, past the 120 the 180 minimum and its 60 slack leave.
    expect(resolveDrag('left', { ...open, delta: -140 })).toEqual({ width: 252, collapsed: true })
  })

  it('stops at the minimum inside the slack instead of collapsing', () => {
    // 252 - 135 = 117 would collapse; 122 is one pixel inside the slack.
    expect(resolveDrag('left', { ...open, delta: -130 })).toEqual({
      width: LEFT_MIN,
      collapsed: false
    })
  })

  it('remembers the width it collapsed at', () => {
    const folded = resolveDrag('left', { ...open, base: 420, delta: -400 })
    expect(folded).toEqual({ width: 420, collapsed: true })
    const opened = resolveDrag('left', { ...roomy, base: folded.width, collapsed: true, delta: 80 })
    expect(opened).toEqual({ width: 420, collapsed: false })
  })

  it('ignores a nudge too small to be a decision to reopen', () => {
    expect(resolveDrag('left', { ...roomy, base: 252, collapsed: true, delta: 20 })).toEqual({
      width: 252,
      collapsed: true
    })
  })

  it('falls back to the default when there is no width to come back to', () => {
    expect(resolveDrag('left', { ...roomy, base: 0, collapsed: true, delta: 80 })).toEqual({
      width: LEFT_DEFAULT,
      collapsed: false
    })
  })

  it('never reopens a panel by dragging it further into the rail', () => {
    expect(resolveDrag('left', { ...roomy, base: 252, collapsed: true, delta: -200 })).toEqual({
      width: 252,
      collapsed: true
    })
  })
})

describe('the rail and what it holds', () => {
  it('the expand button fits inside the rail', () => {
    /* The rail is the one width that does not grow with the app-wide font size
       — see RAIL_CONTROL_MAX for why it cannot — while the button in it is
       drawn at `--control-h-sm`, which does. `Panel` caps it at this number, so
       the number has to be one the rail can actually hold: at the top of the
       range an uncapped button is 44px inside a 32px strip, drawn over the
       column next door. The rail has no horizontal padding, so fitting is the
       whole test. */
    expect(RAIL_CONTROL_MAX).toBeLessThanOrEqual(RAIL)
  })

  it('the cap is not so small that it changes the shipped look', () => {
    /* `--control-h-sm` is 24px comfortable and 20px compact today, and `min()`
       against this cap has to leave both exactly where they are — a cap below
       24 would shrink the button for everybody at the shipped size to fix a
       problem that only exists at the top of the range. */
    expect(RAIL_CONTROL_MAX).toBeGreaterThanOrEqual(24)
  })
})
