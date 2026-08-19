/* Which palette a run's document is drawn in when this app is the one showing it.

   Another of the `reportTab.js` family — the whole of one rule, pure, with no
   Vue, no DOM and no Tauri in it, because a `.vue` file is the one thing no test
   in this repository can reach. `ReportView.vue` keeps the acting half and none
   of the deciding.

   The document carries both palettes itself: `report.rs` writes the light one on
   a bare `:root`, the dark one under `prefers-color-scheme`, and both again under
   `[data-theme]`. So the same file opened in a browser follows the machine, and
   opened here follows whatever attribute it is handed. This file is the only
   thing that ever hands it one.

   **The attribute goes into the string, not onto the element**, and that is
   forced rather than chosen. `ReportView` draws the document in an
   `<iframe sandbox="" srcdoc>`, and an empty sandbox is every restriction at once
   — no scripts, no same-origin — so nothing on this side can reach the frame's
   own DOM. What it can do is compose the string the frame is built from, which is
   what happens here. Nothing on disk is touched: the file stays exactly as the run
   wrote it, and a report written before any of this existed is not rewritten by
   being looked at.

   The theme arriving is `dark` or `light` and never `system`: `App.vue` has
   already resolved that through `effectiveTheme`, since `system` is the absence of
   a choice and the app has to know which of the two it is painting anyway. A value
   that is neither is a value this rule cannot honour, so it declines rather than
   guessing — the document then reads `prefers-color-scheme` and lands where it
   lands. */

/* The opening tag of the document's root element, wherever it sits after the
   doctype. Deliberately without `g`: `replace` then touches the first one only,
   and a second `<html` can exist solely in a document somebody has edited by
   hand — `report.rs` escapes every `<` it writes — where the first is still the
   real root. */
const ROOT = /<html\b[^>]*>/i

/* A `data-theme` the tag already carries, in any of the three quotings HTML
   allows. Stripped rather than left beside the new one: two of the same attribute
   is a parse error resolved by taking the first, so appending would be a rule
   that silently does nothing. */
const OWN_THEME = /\s+data-theme\s*=\s*(?:"[^"]*"|'[^']*'|[^\s>]*)/gi

/* The document as this app should draw it: the same bytes with the root tag
   naming a theme.

   An attribute already there is replaced, not respected, and that is the choice
   worth naming. Inside this app's tab the app is the one showing the document,
   and its theme is the answer — a `data-theme` found in a file could only come
   from a hand edit or a future writer, and honouring it would leave one tab
   light in a dark window, which is the whole of the fault being fixed. It also
   makes the rule idempotent, so it can be applied to its own output without
   accumulating anything. */
export function themed(html, theme) {
  /* The prop defaults to `''`, but the buffer behind it belongs to a store and
     may be absent while a tab is still loading or after a failed read. A rule
     that threw here would blank the tab over a value the caller is entitled to
     have. */
  if (typeof html !== 'string') return ''
  if (theme !== 'dark' && theme !== 'light') return html

  const found = html.match(ROOT)
  /* Nothing to mark: an empty buffer, a failed read, or a file mangled since it
     was written. Inventing a root would be rewriting somebody's document, and a
     document with no root is still one a browser will draw. */
  if (!found) return html

  const inner = found[0].slice(1, -1).replace(OWN_THEME, '').trimEnd()
  /* A function replacer rather than a string one: `$&` and its relatives are
     substitution patterns in the string form, and the tag being spliced back in
     came out of a document rather than out of this file. */
  return html.replace(ROOT, () => `<${inner} data-theme="${theme}">`)
}
