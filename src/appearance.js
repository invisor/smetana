/* The two appearance questions that have an answer before any DOM exists: what
   a stored theme actually means right now, and what factor a chosen font size
   comes to. Pure — no Vue, no DOM — which is what makes them the part of the
   settings window a test can reach at all, the same bargain `panelWidths.js` and
   the `branchChoice.js` family make.

   Deliberately small on the font side. The sizes themselves — the eight type
   steps and the row and control heights — live in `styles/tokens/`, written in
   terms of `--ui-scale`, and this file only says what that scale is. It held a
   copy of the eight steps first, and the copy was worse than a duplicate: the
   values were written onto the root as inline custom properties, which beat the
   stylesheet, so `typography.css` went dead on startup and could be edited with
   no effect and no test noticing.

   It sits at the top of `src/` rather than under the part of the interface it is
   a rule about, for the reason `paths.js` gives about itself: two views want it
   at once — the app window and the settings window — so there is no "under" to
   put it in. */

/* What the theme control offers, and what `settings.json` may hold. Written out
   here as well as in `THEMES` in `src-tauri/src/settings/model.rs`: what crosses
   the IPC boundary is validated there, and this copy is what the dropdown draws.
   A value added to one and not the other is the familiar silent failure — it
   would be offered here and quietly become `dark` on the way to disk. */
export const THEME_CHOICES = [
  { value: 'system', label: 'System' },
  { value: 'dark', label: 'Dark' },
  { value: 'light', label: 'Light' }
]

/* The sizes both dropdowns offer, and the two shipped ones. The defaults mirror
   `UI_FONT_DEFAULT` and `EDITOR_FONT_DEFAULT` in Rust — and, more to the point,
   today's `--text-md` and `--text-code-size`: picking the default in the
   dropdown has to leave the app looking exactly as it did before this screen
   existed. */
export const FONT_MIN = 10
export const FONT_MAX = 24
export const UI_FONT_DEFAULT = 13
export const EDITOR_FONT_DEFAULT = 12
export const FONT_SIZES = Array.from({ length: FONT_MAX - FONT_MIN + 1 }, (_, i) => FONT_MIN + i)

/* Which of the two painted themes is on screen right now.

   `system` is not a third palette: it is the absence of a choice, and the answer
   then comes from the machine — `prefers-color-scheme`, watched live, because a
   laptop that switches at sunset must not leave the app wrong for the evening.
   Anything this front end has never heard of is read the same way rather than
   forced to `dark`: a value it cannot name is a value it cannot honour, and the
   machine's own answer is the closest thing to a right one. Rust has already
   turned genuine junk into `dark` before it ever gets here. */
export function effectiveTheme(theme, prefersDark) {
  if (theme === 'dark' || theme === 'light') return theme
  return prefersDark ? 'dark' : 'light'
}

/* A size that is not one of the offered ones takes the shipped one, exactly as
   Rust's `font_in_range` does — the two have to agree, or a hand-edited file
   would draw at one size and save at another. Clamping is refused for the same
   reason it is refused there: a 2 in that field is a mis-edit, not somebody
   asking for the smallest size. */
export function clampFont(value, fallback) {
  return Number.isInteger(value) && value >= FONT_MIN && value <= FONT_MAX ? value : fallback
}

/* The two custom properties a window's root needs to draw at a chosen size.

   **Two, not ten**, and that is the whole point. The type scale and the row and
   control heights are `calc(<n> * var(--ui-scale) * 1px)` in
   `tokens/typography.css` and `tokens/space.css`, so all this has to say is what
   the factor is — the numbers stay in the stylesheet, where a reader would look
   for them and where editing one still changes the screen. Computing the eight
   sizes here instead was the first version and it silently killed the
   stylesheet: an inline custom property on the root beats every rule in it.

   A factor is also why the hierarchy survives every size. Moving only the
   semantic aliases (`--text-ui-size` and its neighbours) would have been less
   work and would have flattened a label, a row and a heading into one size at
   every setting but the default.

   The factor is not rounded. Rounding each step to whole pixels was the earlier
   behaviour and it pushed steps together — 10 and 11 both land on 12 at some
   factors — and the browser lays out fractional pixels perfectly well.

   `--text-code-size` is the one step the factor deliberately does not reach: the
   app size answers "how big is this interface" and the editor size answers "how
   big is code". It is set to the editor's own number, and a window that applies
   none of this keeps the stylesheet's `var(--text-sm)`. */
export function fontVars(uiFontSize, editorFontSize) {
  return {
    '--ui-scale': String(clampFont(uiFontSize, UI_FONT_DEFAULT) / UI_FONT_DEFAULT),
    '--text-code-size': `${clampFont(editorFontSize, EDITOR_FONT_DEFAULT)}px`
  }
}
