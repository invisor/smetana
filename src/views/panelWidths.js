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

/** The biggest the rail's expand button may be drawn.
 *
 * The rail is the one width in the app that does **not** grow with the app-wide
 * font size, and it cannot: `RAIL` is a layout constant these pure functions do
 * arithmetic with — the neighbour's cost, the collapse and expand thresholds,
 * the clamp against the stored width — so a scale-dependent rail would have to
 * be threaded through every one of them and into the geometry each caller
 * builds, to make a 32px strip 59px wide. A rail is a strip; that is the whole
 * of what it is for.
 *
 * What does grow is what sits in it: the expand button is an `IconButton
 * size="sm"`, drawn at `--control-h-sm`, which is 44px at the top of the range
 * and spilled over the neighbouring column. So the button is capped instead —
 * `min(var(--control-h-sm), RAIL_CONTROL_MAX)`, which leaves both densities
 * exactly as they are at the shipped size and stops the growth at the rail's
 * edge. Pinning a control inside fixed chrome is the same answer `Tab`'s 16px
 * close and `CodeBlock`'s 18px copy already give.
 */
export const RAIL_CONTROL_MAX = 24

/** The project rail: fixed, and outside the panel's own width.
 *
 * Fixed for the reason `RAIL` and `RAIL_CONTROL_MAX` are: these functions do
 * arithmetic with the number — what the board has left, what a third of the
 * window comes to — and a width that grew with `--ui-scale` would have to be
 * threaded through every one of them to make a 44px strip wider. What sits in
 * it is a 28px tile with two mono characters, and a strip is a strip.
 */
export const PROJECT_RAIL = 44

export const LEFT_MIN = 180
export const RIGHT_MIN = 240
/* 236 is the handoff's panel. A width already in settings.json is honoured as
   it stands: this is what somebody who has never dragged the separator gets,
   and rewriting a stored preference to match a new default would be an edit
   nobody asked for. The same number is `LEFT_WIDTH_DEFAULT` in
   `src-tauri/src/settings/model.rs`, so that a file with no width in it opens
   the same width the front end would have chosen on its own. */
export const LEFT_DEFAULT = 236
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
 * sum — collapsed it costs a rail, open it costs its own width — and so is the
 * project rail, which is drawn out of the same window and would otherwise leave
 * this arithmetic measuring a window 44px wider than the one there is. Off by
 * default, so a caller that has not heard of it keeps the meaning it had.
 *
 * It joins the sum that protects the board and not the third-of-the-window cap,
 * and the split is deliberate: the floor is a guarantee about how much room the
 * board actually has, which 44px of rail genuinely takes away, while the
 * fraction is a rule about how much of a window one panel may dominate, and a
 * strip of chrome does not change the answer to that.
 */
export function maxWidth({ other, otherCollapsed, viewport, railOpen = false }) {
  const taken = (otherCollapsed ? RAIL : other) + (railOpen ? PROJECT_RAIL : 0)
  return Math.min(viewport * MAX_FRACTION, viewport - taken - CENTER_MIN)
}

/**
 * Stored width → the width to draw. When the window is too narrow to honour
 * both the panel's minimum and the board's floor, the panel keeps its minimum
 * and the board takes the squeeze: the board's content scrolls, a file tree at
 * 90px does not. Someone who needs that room collapses the panel outright.
 */
export function clampWidth(want, { side, other, otherCollapsed, viewport, railOpen = false }) {
  const { min } = bounds(side)
  const max = maxWidth({ other, otherCollapsed, viewport, railOpen })
  if (max < min) return min
  return Math.round(Math.min(Math.max(want, min), max))
}

/**
 * Where a drag leaves the panel. `delta` is the separator's own displacement —
 * positive is rightwards for both sides, so the left panel grows with it and
 * the right panel shrinks.
 *
 * Collapsing keeps the stored width untouched: the panel folds to a rail and
 * comes back the width it left at, whether it is reopened by this gesture or by
 * the button inside the folded rail. The left panel's *header* button is a third
 * way in and out, and the steps it walks are `leftChrome.js`'s rather than this
 * file's — it hides the project rail on the first press and folds the column on
 * the second, and it writes no width either, so a column folded from the header
 * comes back the same width as one folded by a drag.
 */
export function resolveDrag(
  side,
  { base, delta, collapsed, other, otherCollapsed, viewport, railOpen = false }
) {
  const { min, fallback } = bounds(side)
  const grow = side === 'left' ? delta : -delta
  const geometry = { side, other, otherCollapsed, viewport, railOpen }

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
