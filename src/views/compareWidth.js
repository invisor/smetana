/* How wide the compare window's file list may be, kept apart from the window
   that draws it. Nothing here touches Vue or the DOM, which is what makes it
   the one part of this drag a test can reach — the drag itself is a pointer
   against a real element and is checked by eye.

   `panelWidths.js` is the sibling and deliberately not the same file. That one
   is about the app's three columns: two panels sharing a window, a board with a
   floor under it, a project rail outside the panel's own width, and a rail a
   panel folds into. This window has two panes and no rail at all, so every one
   of those operands would arrive here as a constant nobody varies — a geometry
   object of five fields to express one subtraction. What the two do share is the
   split that matters, and it is kept: the width somebody dragged to is one
   number, and the width drawn in the window there is now is another.

   Where they differ is what remembers. The app's widths live in
   `settings.json`; this one lives for as long as the window does. The compare
   window keeps no geometry of its own either — `compare_window_open` builds it
   at 1040x680 every time, and only the main window's position is persisted — so
   a list width that outlived the window would be the single thing about this
   window's shape that came back, which is a stranger promise than keeping
   nothing. */

/** The list, at its narrowest.
 *
 * A measurement rather than a round number, and it is the whole of what keeps
 * the mode switch legible. Those are two buttons sharing this width, and what
 * has to survive is the wider label at the largest font the settings window
 * offers: "Diverged" measures 80.9px at `FONT_MAX`, a button is that plus
 * `2 x --space-4` of padding and its two borders, and the pair with its gap and
 * the switch's own padding comes to 217.8 in the comfortable density — 204.8 in
 * compact, which is the slacker of the two. Below this a label is cut through
 * the middle inside its own button, which is what this window shipped with and
 * what this number exists to prevent.
 *
 * The number is the measurement rounded **up**, and the slack is the point: it
 * was taken in Chrome, through the dev server, and the app runs in WKWebView,
 * WebKitGTK and WebView2, which measure the same string in the same font to
 * within a pixel or two of each other and not to the same pixel. At 218 the
 * margin was one pixel, which is a margin only on the machine it was measured
 * on.
 *
 * The measurement is per label, so shortening or lengthening either word means
 * taking it again — in `?view=compare`, at `FONT_MAX`, in the comfortable
 * density.
 */
export const LIST_MIN = 228

/** What the diff keeps, whatever the list wants.
 *
 * Two columns of code side by side, which is the whole reason this window is
 * wide. Together with `LIST_MIN` it comes to 588, inside the 640 that
 * `compare_window_open` sets as the window's own minimum — so both floors are
 * honourable at the narrowest window this app can open, and the clamp below
 * never has to choose between them in practice.
 *
 * The separator between the two panes is not in this arithmetic, so the diff
 * actually keeps `--resizer-w` less than this. `panelWidths.js` leaves its two
 * out of the same sum for the same reason: these are floors under how much room
 * a pane needs to be worth drawing, chosen to the nearest ten, and five pixels
 * is inside the rounding rather than an error the number could be made to
 * absorb. Reading the token from here would mean writing it out a second time,
 * in a file whose whole property is that it touches no stylesheet.
 */
export const DIFF_MIN = 360

/** The width the window opens at.
 *
 * The same 320 that `--panel-right-w` names, as a number: these functions do
 * arithmetic with it, and a token is not readable as one from here. That is
 * `RAIL`'s reasoning in `panelWidths.js`, and it costs the same thing — the
 * number is written in two places and they are not checked against each other.
 * It does **not** grow with the app-wide font size, and now genuinely need not:
 * the labels on the switch fit at every size the settings window offers.
 */
export const LIST_DEFAULT = 320

/** One arrow key on the separator. */
export const STEP = 16

/** The most the list may take right now: everything but the diff's floor. */
export function maxListWidth(viewport) {
  return viewport - DIFF_MIN
}

/**
 * Stored width -> the width to draw. A window too narrow to honour both floors
 * leaves the list at its minimum and lets the diff take the squeeze: a diff
 * scrolls, and a list whose only control is cut in half does not.
 *
 * There is no third-of-the-window cap here, and its absence is deliberate. In
 * the app that cap protects a board which is the point of the screen; in this
 * window the list is half of what somebody came for, and a person dragging it
 * wide is reading long paths. The diff's floor is the only honest limit.
 */
export function clampListWidth(want, viewport) {
  const max = maxListWidth(viewport)
  if (max < LIST_MIN) return LIST_MIN
  return Math.round(Math.min(Math.max(want, LIST_MIN), max))
}

/**
 * Where a drag leaves the list. `delta` is the separator's own displacement,
 * positive rightwards, measured from the width snapshotted at `dragstart` and
 * never from the previous frame — clamping against the last frame would make
 * every clamped move the new origin and the list would drift away from the
 * pointer.
 */
export function resolveDrag({ base, delta, viewport }) {
  return clampListWidth(base + delta, viewport)
}
