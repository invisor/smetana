/* The two appearance questions that have an answer before any DOM exists: what
   a stored theme actually means right now, and what the type scale looks like at
   a chosen size. Pure — no Vue, no DOM — which is what makes them the part of the
   settings window a test can reach at all, the same bargain `panelWidths.js` and
   the `branchChoice.js` family make.

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

/* The authored type scale of `styles/tokens/typography.css`, in pixels, in the
   order it is written there. It is repeated here because there is no way to read
   it back: the moment the scale is overridden on the root, `getComputedStyle`
   answers with the override rather than with what the stylesheet said, so the
   second change would scale the first change's output. The stylesheet stays the
   source of truth for the shipped look and this list must follow it. */
const SCALE = {
  '--text-2xs': 10,
  '--text-xs': 11,
  '--text-sm': 12,
  '--text-md': 13,
  '--text-lg': 15,
  '--text-xl': 18,
  '--text-2xl': 22,
  '--text-3xl': 28
}

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

/* The whole type scale at a chosen app size, plus the editor's own pinned size.

   The scale is multiplied rather than shifted, and every step moves together:
   that is what keeps a label smaller than a row and a heading bigger than both.
   Changing only the semantic aliases (`--text-ui-size` and its neighbours) would
   have been less code and would have flattened the hierarchy at every size but
   the default.

   `--text-code-size` is set to the editor's own number and not scaled with the
   rest: the app size answers "how big is this interface" and the editor size
   answers "how big is code", and a person who wants small chrome around large
   code has said so twice. Its own alias in the stylesheet (`var(--text-sm)`) is
   what it falls back to, so a window that never applies these keeps today's
   look. */
export function fontVars(uiFontSize, editorFontSize) {
  const factor = clampFont(uiFontSize, UI_FONT_DEFAULT) / UI_FONT_DEFAULT
  const vars = {}
  for (const [name, px] of Object.entries(SCALE)) {
    vars[name] = `${Math.max(1, Math.round(px * factor))}px`
  }
  vars['--text-code-size'] = `${clampFont(editorFontSize, EDITOR_FONT_DEFAULT)}px`
  return vars
}
