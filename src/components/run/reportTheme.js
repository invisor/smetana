/* Which palette a document is drawn in when this app is the one showing it.

   **It is no longer only a run's own document.** Every `.html` in the project
   opens in `ReportView` now — `reportTab.js`'s `isDocumentPath` — so what
   arrives here is arbitrary html off somebody's disk, written by a tool that
   never heard of this app and declaring no palette under `[data-theme]` at all.
   The decision taken rather than dodged: **the attribute is stamped on that
   document too, unconditionally.**

   For the overwhelming case it is inert, and inert in the strong sense rather
   than the hopeful one: an attribute on the root element changes nothing a
   browser draws unless a selector asks about it, and a document with no
   `[data-theme]` rule in it has no such selector. Nothing on disk is touched
   either — the file the frame is built from is a string this function returned,
   and the bytes on the disk are the bytes the tool wrote.

   For the small case where the document *does* carry `[data-theme]` rules of its
   own, the stamp overrides the author. That is the same trade this file already
   makes for a report somebody hand-edited, taken for the same reason and with
   one added: inside this app's tab, this app is the one showing the document and
   its theme is the honest answer, and the alternative — asking whether a
   document is "one of ours" before theming it — is exactly the folder test that
   was just taken out of the drawing decision, put back in a second place where
   it would be wrong about the same folders. A document that would rather answer
   the machine still can: it is a file on disk and a browser is one double-click
   away.

   Another of the `reportTab.js` family — the whole of one rule, pure, with no
   Vue, no DOM and no Tauri in it, because a `.vue` file is the one thing no test
   in this repository can reach. `ReportView.vue` keeps the acting half and none
   of the deciding.

   A document this app wrote carries both palettes itself: `report.rs` writes the
   light one on a bare `:root`, the dark one under `prefers-color-scheme`, and
   both again under `[data-theme]`. So the same file opened in a browser follows
   the machine, and opened here follows whatever attribute it is handed. This
   file is the only thing that ever hands it one.

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

/* What the frame is allowed to reach, which is nothing.

   **`sandbox=""` does not stop a document loading subresources, and the comment
   that said it did was measured false.** An empty sandbox is no scripts, no
   forms, no same-origin and no navigation — it has never had anything to say
   about a `<link rel=stylesheet>`, an `<img>` or an `@font-face`. Driven against
   a local server logging every hit, a `srcdoc` frame with an empty sandbox
   fetched all three. Nothing was inherited to save it either: `csp` in
   `src-tauri/tauri.conf.json` is `null`, so the app's own document declares no
   policy for a child frame to be bounded by.

   That was tolerable while the only documents reaching this frame were written
   by this app or by its agent against this app's prompt. It stopped being
   tolerable the moment any `.html` in the project opens here on a single click:
   the file belongs to a repository somebody is supervising rather than one they
   wrote, and every URL in it is a beacon fired from their machine at an address
   its author chose. Scripts still do not run and the origin is still opaque, so
   this is a beacon rather than execution — and a beacon is exactly what the
   third point of this feature's design forbids in as many words.

   So the policy is carried in the document, the same way the theme is and for
   the same reason: an empty sandbox puts the frame's own DOM out of reach, and
   the string is the only thing this side can still write to. Three directives
   and each earns its place. `default-src 'none'` is the whole of it — scripts,
   frames, fonts, media, connections and anything added to the platform later,
   all refused by the fallback rather than by a list somebody has to keep.
   `style-src 'unsafe-inline'` is the one thing given back, because the documents
   this frame exists for carry their entire appearance in one inline `<style>`
   (`src-tauri/src/runs/report.rs`, a documented exception to the token rule for
   exactly that reason) — it permits an inline block and a `style` attribute and
   still refuses a stylesheet off a URL. `img-src data:` allows a picture that is
   already in the document and no picture that has to be fetched.

   A document carrying a policy of its own keeps it: two policies are enforced as
   an intersection, so a second one can only make the frame stricter. That is
   also why the meta below goes in unconditionally rather than being skipped for
   a document that appears to have one already — see `guarded`. */
const CSP = "default-src 'none'; style-src 'unsafe-inline'; img-src data:"
const CSP_META = `<meta http-equiv="Content-Security-Policy" content="${CSP}">`

/* Where the meta may be put, in the order they are tried, and the order is the
   whole of the correctness: a policy only binds what the parser meets after it,
   and a `<meta>` whose parent is not the head is ignored outright.

   **The root tag first, and the `<head>` only as a fallback** — which is the
   opposite of the order this was written with, and the swap was measured. The
   head branch matches the first `<head` in the *string*, and a document that
   opens no head of its own but carries an unescaped `<head>` somewhere in its
   body puts that match in the middle of the content: the meta lands with the
   body as its parent, the policy is dropped without a single console message,
   and the frame is back to a bare sandbox. Driven at a probe server, exactly
   that document fetched its stylesheet.

   After the root tag the parser opens an implicit head, puts the meta in it and
   ignores the explicit `<head>` that follows — the spec's own behaviour rather
   than a trick, and pinned by a test below. So root-first is right for every
   well-formed document and cannot be fooled by content, and it is the same
   exposure `themed` already takes on `ROOT` and already accounts for, rather
   than a second, different one.

   The head branch still earns its place: `<html>` is an optional tag, so
   `<!doctype html><head>…` is a valid document with no root tag to sit behind.
   Failing both, the doctype, and never before one: a doctype with anything in
   front of it is not a doctype, and the document would fall into quirks mode
   over a security header. */
const HEAD = /<head\b[^>]*>/i
const DOCTYPE = /<!doctype[^>]*>/i

/* The same document with this app's policy in front of everything in it.

   **Unconditional, and the shortcut that used to stand here was the whole of the
   protection handed to the author of the file.** It read `if
   (html.includes(CSP_META)) return html` — a substring search over the entire
   document, not a check that a live policy sits in the head — so a file carrying
   that exact literal inside an HTML comment was passed through untouched. The
   meta in the comment is inert, the frame falls back to a bare sandbox, and the
   beacon this whole rule exists to stop goes out. Measured, not deduced: that
   document fetched a stylesheet, an image and a font.

   Nothing is lost by dropping it. Applying this to its own output puts a second
   identical meta in, and two policies are enforced as an intersection, so the
   frame is bounded by exactly the same policy — the shortcut bought a byte count
   and cost the guarantee. A *different* policy the document brought itself is
   left alone for the same arithmetic: an intersection can only be stricter, so
   there is no ordering, and now no content, by which a file can talk its way out
   of this one. */
export function guarded(html) {
  if (typeof html !== 'string') return ''
  for (const at of [ROOT, HEAD, DOCTYPE]) {
    const found = html.match(at)
    if (found) return html.replace(at, () => `${found[0]}${CSP_META}`)
  }
  /* A fragment with no root, no head and no doctype — a buffer still loading, a
     read that refused, a snippet somebody saved. There is nothing to sit behind,
     so the policy goes first and the whole of it is covered. */
  return `${CSP_META}${html}`
}

/* The string the frame is actually built from, and the one thing `ReportView`
   calls.

   The two rules are composed here rather than in the component, because a `.vue`
   file is the one thing no test in this repository can reach — and "the document
   is themed **and** cannot reach the network" is exactly the kind of guarantee
   that quietly becomes half true when the composition lives somewhere nothing
   checks it.

   The theme is stamped first and the policy inserted second, so the root tag
   `themed` rewrites is the document's own rather than one this file has already
   touched. The other order works too; this one is simply the one that keeps each
   rule reading the document it was written about. */
export function documentFor(html, theme) {
  return guarded(themed(html, theme))
}
