/* The rules that decide how wide a side panel may be, kept apart from the view
   that draws it. Nothing here touches Vue or the DOM, which is what makes this
   the one part of the resize work that a test can reach at all — the drag
   itself is a pointer against a real element and is checked by eye.

   Two numbers per panel, and they are not the same number. The *stored* width
   is what a person dragged to and what settings.json keeps; the *effective*
   width is that number clamped against the window it is being drawn in. Only a
   drag writes the stored one back. Narrowing the window squeezes the panel and
   widening it again restores what was asked for — a resized window must not
   silently rewrite a preference. */

/** Collapsed, a panel is this rail — the width `Panel` reserves for one. */
export const RAIL = 32

export const LEFT_MIN = 180
export const RIGHT_MIN = 240
export const LEFT_DEFAULT = 252
export const RIGHT_DEFAULT = 340

/** The board keeps this much no matter what the panels want. */
export const CENTER_MIN = 400
/** Neither panel takes more than a third of the window. */
export const MAX_FRACTION = 1 / 3

/** Drag a panel this far below its minimum and it folds into the rail. */
export const COLLAPSE_SLACK = 60
/** Pull this far out of the rail and it opens again. */
export const EXPAND_PULL = 60
/** One arrow key on the separator. */
export const STEP = 16

const bounds = (side) =>
  side === 'left'
    ? { min: LEFT_MIN, fallback: LEFT_DEFAULT }
    : { min: RIGHT_MIN, fallback: RIGHT_DEFAULT }

/**
 * The most this panel may occupy right now: a third of the window, and never
 * so much that the board drops below its floor. The other panel is part of the
 * sum — collapsed it costs a rail, open it costs its own width.
 */
export function maxWidth({ other, otherCollapsed, viewport }) {
  const taken = otherCollapsed ? RAIL : other
  return Math.min(viewport * MAX_FRACTION, viewport - taken - CENTER_MIN)
}

/**
 * Stored width → the width to draw. When the window is too narrow to honour
 * both the panel's minimum and the board's floor, the panel keeps its minimum
 * and the board takes the squeeze: the board's content scrolls, a file tree at
 * 90px does not. Someone who needs that room collapses the panel outright.
 */
export function clampWidth(want, { side, other, otherCollapsed, viewport }) {
  const { min } = bounds(side)
  const max = maxWidth({ other, otherCollapsed, viewport })
  if (max < min) return min
  return Math.round(Math.min(Math.max(want, min), max))
}

/**
 * Where a drag leaves the panel. `delta` is the separator's own displacement —
 * positive is rightwards for both sides, so the left panel grows with it and
 * the right panel shrinks.
 *
 * Collapsing keeps the stored width untouched: the panel folds to a rail and
 * comes back the width it left at, whether it is reopened by this gesture or
 * by the button in the panel header.
 */
export function resolveDrag(side, { base, delta, collapsed, other, otherCollapsed, viewport }) {
  const { min, fallback } = bounds(side)
  const grow = side === 'left' ? delta : -delta
  const geometry = { side, other, otherCollapsed, viewport }

  /* From the rail exactly two things can happen, and neither of them is a
     width: it opens, or it stays a rail. Anything in between would make the
     panel flicker at the edge of the threshold. */
  if (collapsed) {
    return grow > EXPAND_PULL
      ? { width: clampWidth(base || fallback, geometry), collapsed: false }
      : { width: base, collapsed: true }
  }

  const want = base + grow
  if (want < min - COLLAPSE_SLACK) return { width: base, collapsed: true }
  return { width: clampWidth(want, geometry), collapsed: false }
}
