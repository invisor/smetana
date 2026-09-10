/* How much of an html tab's width belongs to the eye/code toggle, and to
   nothing else.

   Another of the `reportTab.js` family — the whole of one rule, pure, with no
   Vue and no DOM in it, because a `.vue` file is the one thing no test in this
   repository can reach. This one is three CSS lengths, and it exists for a
   reason worth stating before the values: **two files have to agree about a
   corner, and when they disagreed the cost was a control nobody could press.**

   The toggle used to be laid straight over the tab's top-right corner, and the
   corner was already occupied. `FileEditor` draws a full-width notice band there
   — the stale-file band, whose `Reload` and `Keep mine` are pinned to its right
   end and are the *only* way out of a file that changed on disk under an unsaved
   buffer — and `@codemirror/search` opens its panel at the top too
   (`search({ top: true })` in `editor/extensions.js`), with a close button its
   own base theme pins at `right: 4px`. Measured against the tokens rather than
   guessed: the band's buttons end `--space-5` from the edge and stand
   `--control-h-sm` tall, while the toggle covered from `--space-3` to
   `--space-3 + --control-h-sm + 2 * --border-w` — an opaque 20×24 patch over the
   right end of `Keep mine` in comfortable density, 18×20 in compact, swallowing
   every press that landed in it. Neither density escaped, and no test in this
   repository can see it: both sides are `.vue`.

   **So the corner is reserved rather than shared.** The tab's content box is
   made narrower by `TOGGLE_LANE` and the button is placed in the strip that
   leaves, which puts its left edge exactly `TOGGLE_INSET` clear of the content —
   by construction, in both densities, whatever height a band happens to take and
   however many bands there are. That last part is what a lowered button would
   not have bought: the notice wraps when the path is long and the search panel
   wraps by its own theme, so an offset computed from a band's height would be
   right until somebody made the window narrow. The lane also clears what was
   never named in the defect — the editor's own scrollbar, and anything a future
   band pins to the same corner.

   What it costs is honest and visible: an html tab is that much narrower than
   its neighbours, and a document drawn in the frame gets a strip of app surface
   down its right side. The alternative shape — the toggle in a band of its own
   above the content — spends a whole row of chrome on one control and moves it
   out of the corner the design asked for.

   The three are here rather than in the component because the component owns
   only the strip it sits in: whoever draws the toggle has to reserve the lane,
   and a second copy of this arithmetic in `DesktopApp.vue` is exactly the drift
   that puts the button back over the buttons. `--control-h-sm` is `IconButton`'s
   `size="sm"` box, and the two border widths are the frame the toggle draws
   around it so that a glyph never sits directly on somebody's document. */

/* How far the control is held off the content and off the tab's own edge. */
export const TOGGLE_INSET = 'var(--space-3)'

/* The control's own box: the small icon button, plus the border around it. */
export const TOGGLE_BOX = 'calc(var(--control-h-sm) + var(--border-w) * 2)'

/* What the tab keeps clear on its right: the control, with its inset on both
   sides. Read by whatever draws the toggle — the centre column in the app, the
   demo boxes in the gallery — as padding on the content box, since an absolutely
   positioned child is placed against the padding box and therefore lands in the
   strip rather than over the content. */
export const TOGGLE_LANE = `calc(${TOGGLE_BOX} + ${TOGGLE_INSET} * 2)`
