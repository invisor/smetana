/* How tall each section of the Git panel is drawn, and which of them absorbs
   whatever height is left over.

   Pure, with no Vue and no DOM in it — the family `gitActions.js`,
   `changeStatus.js` and `branchChoice.js` belong to, and for the reason that
   family exists: a `.vue` file is the one thing no test in this repository can
   reach, so the whole of a rule lives outside the component that draws it. The
   drag itself is a pointer against a real element and is checked by eye, the
   same division `views/panelWidths.js` records for the side panels.

   **The arithmetic is in rows, not in pixels, and that is the load-bearing
   decision here.** A row is `--row-h`, which the space tokens redefine under
   `[data-density="compact"]` and scale again by the app-wide font size, so a
   count follows a person's settings where a pixel height would have to be
   rewritten by every one of them. It is the reason the branch cap this file
   inherited was written as a count in the first place. It also spares the panel
   the defect a pixel drag always produces: a section stops on a row boundary,
   so no half row is left peeking out from under the fold. The one pixel
   measurement lives at the edge, in the component, where a drag's displacement
   is divided by the height of a header — which *is* a row — to reach this file
   in the units it speaks.

   The stored count and the drawn count are different numbers, and conflating
   them would be the defect here, exactly as `panelWidths.js` says it would be
   one axis over. What `settings.json` keeps is what a person dragged to; what
   the panel draws is that number clamped against the panel it is in now. Only
   a drag writes the stored one back, so a shortened window squeezes a section
   and a lengthened one gives back what was asked for. */

/** The fewest rows a section may be dragged to. */
export const MIN_ROWS = 2

/** How many rows the branch section claims before it scrolls, until dragged.
 *
 * This is the cap that used to live in `GitPanel.vue`, and it survives the move
 * unchanged because the reason for it is unchanged: without one the section's
 * basis is its content, so a repository with forty branches claims forty rows
 * of the column and the changes somebody opened this panel for are squeezed to
 * nothing. Six is enough to reach for a branch and short enough that the
 * changes stay the content.
 *
 * It is a *default* now rather than a ceiling — a drag replaces it, and until
 * there is one the section is drawn exactly as it was before any of this. */
export const BRANCH_ROWS = 6

/** What the filler keeps however far its neighbours are dragged.
 *
 * Three rows rather than `MIN_ROWS`: the filler is the section the panel is
 * about, and a two-row window onto it reads as a scrollbar rather than as a
 * list. It is a floor and not a size — the filler is normally most of the
 * panel. */
export const FILLER_MIN_ROWS = 3

/** What a section nobody has dragged keeps when a neighbour is dragged past it.
 *
 * An undragged section has no claim on the panel and gives way on its own,
 * which is right — but giving way to nothing at all draws a row clipped to a
 * sliver of itself, which is the same defect a panel too short for its own
 * contents produces. One row is the least that is still a row. */
export const UNDRAGGED_ROWS = 1

/** The largest count `settings.json` will keep, mirrored by `validate` in
 *  `settings/model.rs`. Nothing draws this tall; it is the guard that stops a
 *  hand-edited file from carrying an absurd number back into the panel. */
export const MAX_ROWS = 40

/* Top down, and that order is the answer itself when a fold moves the filler:
   the slack lands as close to where the eye already is as the folds allow. The
   changes lead it because they are what this panel is for. */
const FILL_ORDER = ['changes', 'repos', 'branches']

/**
 * Which section takes the height the others do not claim.
 *
 * `sections` is the ones actually **drawn**, each `{ id, open }` — Changes and
 * Branches are absent altogether when there is no repository to have changed
 * anything, and a section that is not on screen is not one this rule has an
 * opinion about.
 *
 * Every section folded is `null` and not a fallback: three folded headers
 * stacked at the top with honest empty space under them is the right drawing of
 * that state, where reaching for one anyway would unfold a section nobody
 * unfolded.
 */
export function filler(sections) {
  const open = new Set((sections ?? []).filter((s) => s.open).map((s) => s.id))
  return FILL_ORDER.find((id) => open.has(id)) ?? null
}

/**
 * The tallest this section may be drawn right now.
 *
 * `available` is the panel's own height in rows; `headers` is how many section
 * headers are on screen, each costing a row whether its section is folded or
 * not; `fixed` is what the *other* dragged section was dragged to. A section
 * nobody has dragged is not in that sum — it follows its content and gives way
 * on its own — so it costs nothing here beyond its header.
 */
export function maxRows({ available, headers, fixed }) {
  return available - headers - fixed - FILLER_MIN_ROWS
}

/**
 * Stored rows → the rows to draw.
 *
 * A panel too short to honour both this section's minimum and the filler's
 * floor keeps the minimum and lets the filler take the squeeze: a two-row list
 * is still a list, and the filler scrolls. That is the same trade `clampWidth`
 * makes in favour of a side panel over the board.
 */
export function clampRows(want, geometry) {
  /* Floored and not rounded, and the difference is not cosmetic: a panel
     leaving room for 6.8 rows leaves room for 6, and rounding the ceiling up
     hands out a seventh row that is not there — which the panel then takes back
     out of whichever section had no claim on its own height, drawing it as a
     sliver. The wish below is rounded, because that is a person aiming at a row;
     the ceiling is a fact about the panel and cannot be aimed past. */
  const max = Math.floor(maxRows(geometry))
  if (max < MIN_ROWS) return MIN_ROWS
  return Math.min(Math.max(Math.round(want), MIN_ROWS), max)
}

/* Which way a separator's displacement moves the section it belongs to.
   Positive is downwards, so a section above its separator grows with it and one
   below shrinks.

   The pair is fixed rather than worked out per drag, and it is allowed to be:
   the filler's own separator is never drawn — there is nothing to take height
   from on the side that is already taking the leftovers — so the repositories
   are always above the filler and the branches always below it, whichever
   section the folds have made the filler. */
const DIRECTION = { repos: 1, branches: -1 }

/**
 * Where a drag leaves a section, in rows.
 *
 * `base` is the count snapshotted at `dragstart` and `delta` the separator's
 * displacement since that same moment, never since the last frame: clamping
 * against the previous frame would make each clamped move the new origin and
 * the section would drift away from the pointer, which is the drift `Resizer`'s
 * own contract warns about.
 *
 * Dragging a section to nothing stops at `MIN_ROWS` rather than folding it. A
 * side panel had to invent a fold out of the drag because a rail carries no
 * other affordance; a section here has a chevron that is always on screen and
 * always reversible, so folding by drag would be a one-way door — the separator
 * of a folded section is not drawn, and there would be no way back from where
 * the gesture was made.
 */
export function resolveDrag(section, { base, delta, ...geometry }) {
  const grow = (DIRECTION[section] ?? 1) * delta
  return clampRows(base + grow, geometry)
}
