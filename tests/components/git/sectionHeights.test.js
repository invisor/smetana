import { describe, expect, it } from 'vitest'
import {
  FILLER_MIN_ROWS,
  MIN_ROWS,
  UNDRAGGED_ROWS,
  clampRows,
  filler,
  maxRows,
  resolveDrag
} from '../../../src/components/git/sectionHeights.js'

/* The sections as `GitPanel` hands them over: the ones actually drawn, in the
   order they are drawn, each with whether it is unfolded. Changes and Branches
   are absent from the list entirely when the panel has no repository to have
   changed anything — a section that is not on screen is not a section this rule
   has an opinion about. */
const all = (open = {}) => [
  { id: 'repos', open: open.repos ?? true },
  { id: 'changes', open: open.changes ?? true },
  { id: 'branches', open: open.branches ?? true }
]

describe('which section absorbs the leftover height', () => {
  /* The changes are what somebody opened this panel for, so they take the
     slack while they are on screen and unfolded. The whole of the layout
     follows from this one answer. */
  it('is the changes whenever they are drawn and unfolded', () => {
    expect(filler(all())).toBe('changes')
    expect(filler(all({ repos: false, branches: false }))).toBe('changes')
  })

  /* Folding the filler does not leave the panel without one. The order is the
     panel's own, top down, so the slack lands as close to where the eye already
     is as the folds allow. */
  it('falls to the repositories, then to the branches, as sections fold', () => {
    expect(filler(all({ changes: false }))).toBe('repos')
    expect(filler(all({ changes: false, repos: false }))).toBe('branches')
  })

  /* Three folded headers stacked at the top with honest empty space under them
     is the right drawing of "everything is folded". Inventing a filler out of a
     folded section would unfold one nobody unfolded. */
  it('is nobody when every section is folded', () => {
    expect(filler(all({ repos: false, changes: false, branches: false }))).toBe(null)
    expect(filler([])).toBe(null)
  })

  /* A panel with no repository draws neither the changes nor the branches, and
     the rule must not reach for a section that is not there. */
  it('ignores sections that are not drawn', () => {
    expect(filler([{ id: 'repos', open: true }])).toBe('repos')
  })
})

describe('how tall a dragged section may be drawn', () => {
  /* Every header on screen costs a row whether its section is folded or not,
     the other dragged section costs what it was dragged to, and the filler
     keeps its floor. What is left is this section's ceiling. */
  it('leaves room for the headers, the other dragged section and the filler', () => {
    expect(maxRows({ available: 30, headers: 3, fixed: 6 })).toBe(30 - 3 - 6 - FILLER_MIN_ROWS)
  })

  /* An untouched section follows its content and gives way on its own, so what
     it costs this one's ceiling is not its content but a single row — enough
     that dragging past it leaves a row rather than a sliver of one. */
  it('leaves a whole row for a section that was never dragged', () => {
    expect(maxRows({ available: 30, headers: 3, fixed: UNDRAGGED_ROWS })).toBe(
      30 - 3 - 1 - FILLER_MIN_ROWS
    )
    expect(UNDRAGGED_ROWS).toBeLessThan(MIN_ROWS)
  })
})

describe('the stored row count against the panel it is drawn in', () => {
  /* The stored number is what somebody dragged to and the drawn number is that
     number clamped against the panel it is in now — the same two numbers
     `panelWidths.js` keeps apart, one axis over. */
  it('draws what was asked for when it fits', () => {
    expect(clampRows(8, { available: 40, headers: 3, fixed: 0 })).toBe(8)
  })

  it('never draws a section shorter than its minimum', () => {
    expect(clampRows(0, { available: 40, headers: 3, fixed: 0 })).toBe(MIN_ROWS)
    expect(clampRows(-5, { available: 40, headers: 3, fixed: 0 })).toBe(MIN_ROWS)
  })

  it('never draws it past what the panel has left', () => {
    const geometry = { available: 20, headers: 3, fixed: 0 }
    expect(clampRows(999, geometry)).toBe(maxRows(geometry))
  })

  /* A panel too short to honour both this section's minimum and the filler's
     floor keeps the minimum: a two-row list is still a list, and the filler
     scrolls. The same trade `clampWidth` makes for the board. */
  it('keeps the minimum when there is no room for it', () => {
    expect(clampRows(6, { available: 5, headers: 3, fixed: 0 })).toBe(MIN_ROWS)
  })

  /* Rows and not pixels is the whole point — a half row peeking out from under
     the fold is exactly what a pixel height produces. */
  it('lands on a whole row', () => {
    expect(clampRows(7.4, { available: 40, headers: 3, fixed: 0 })).toBe(7)
    expect(clampRows(7.6, { available: 40, headers: 3, fixed: 0 })).toBe(8)
  })

  /* A panel is rarely a whole number of rows tall, and the leftover fraction is
     not a row anybody can have. Rounding the ceiling up hands out one that is
     not there, and the panel takes it back from whichever section had no claim
     on its own height — which draws that section as a sliver, the very thing
     the arithmetic reserves a row to prevent. */
  it('floors a fractional ceiling rather than rounding past it', () => {
    const geometry = { available: 13.8, headers: 3, fixed: 1 }
    expect(maxRows(geometry)).toBeCloseTo(6.8)
    expect(clampRows(99, geometry)).toBe(6)
  })
})

describe('where a drag leaves a section', () => {
  const geometry = { available: 40, headers: 3, fixed: 0 }

  /* The repositories sit above the filler and the branches below it, in every
     configuration of folds — the filler's own separator is never drawn, so
     neither section ever changes which side of it it is on. That is what lets
     the direction be a property of the section rather than of the moment. */
  it('grows the repositories downwards and the branches upwards', () => {
    expect(resolveDrag('repos', { base: 5, delta: 3, ...geometry })).toBe(8)
    expect(resolveDrag('branches', { base: 5, delta: 3, ...geometry })).toBe(2)
    expect(resolveDrag('branches', { base: 5, delta: -3, ...geometry })).toBe(8)
  })

  /* `delta` is the separator's displacement since `dragstart` and `base` the
     rows snapshotted there, which is what keeps a clamped move from becoming
     the next move's origin — the drift `Resizer`'s own contract warns about. */
  it('measures from the row count the drag started at', () => {
    expect(resolveDrag('repos', { base: 5, delta: 2, ...geometry })).toBe(7)
    expect(resolveDrag('repos', { base: 5, delta: 4, ...geometry })).toBe(9)
  })

  /* Dragging a section to nothing stops at its minimum rather than folding it.
     A side panel's rail had to invent a fold out of the drag because a rail has
     no other affordance; a section here carries a chevron that is always on
     screen and always reversible, so a one-way door out of the drag would be a
     fold nobody could undo from where they made it. */
  it('stops at the minimum instead of folding the section', () => {
    expect(resolveDrag('repos', { base: 4, delta: -99, ...geometry })).toBe(MIN_ROWS)
    expect(resolveDrag('branches', { base: 4, delta: 99, ...geometry })).toBe(MIN_ROWS)
  })

  it('stops at the ceiling the panel leaves', () => {
    const short = { available: 20, headers: 3, fixed: 0 }
    expect(resolveDrag('repos', { base: 4, delta: 99, ...short })).toBe(maxRows(short))
  })
})
