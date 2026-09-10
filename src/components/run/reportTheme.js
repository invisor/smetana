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

/* Where a mark of ours may go: the end of the document's prologue, walked token
   by token from the first byte rather than searched for anywhere in the string.

   **The insertion point may not be steerable by the document, and searching for
   a tag made it exactly that.** `html.match(/<html\b[^>]*>/i)` finds the first
   root tag *text*, and in a document that opens with a conditional comment —
   the HTML5 Boilerplate header, still under saved pages and old templates
   everywhere — that text is inside the comment:

       <!--[if lt IE 7]> <html class="no-js lt-ie7"> <![endif]-->
       <!--[if gt IE 8]><!--> <html class="no-js"> <!--<![endif]-->
       <head><link rel="stylesheet" href="https://cdn.example.com/a.css">

   Both marks landed in there, measured by running this module rather than by
   reading it. A theme lost in a comment is a document that does not follow the
   app; a **policy** lost in a comment is the whole of what this file is for,
   gone with nothing on screen or in the console to say so — and the only visible
   symptom, a document ignoring the app's theme, is what an honest foreign
   document looks like anyway. It is the same defect `guarded` already carried
   once in `html.includes(CSP_META)`, one step earlier: a rule that reads the
   document's *text* is a rule the document's author gets to decide.

   So the scan consumes tokens instead, and consumes **only what can neither
   fetch anything nor open a raw-text context**: whitespace, comments, the bogus
   comments a browser makes of `<?xml …?>` and `<!…>`, one doctype, the root tag
   and the head tag. Anything else stops it where it stands.

   **Two things have to be true of the anchor, and only the first is a property
   of that list.** Nothing consumable can fetch, so no `<link>`, `<script>`,
   `<style>` or `<img>` is ever *before* the meta, and the worst a crafted
   document can force is an anchor earlier than it needed to be — the meta at the
   very front, which is live and first. The second is that the anchor is a
   position the parser is between tokens at, and that one is a property of every
   token *matching the tokenizer*, which is where this was wrong twice: once in
   the comment endings, and once in a tag's `>` inside a quoted attribute value,
   where the meta stopped being a meta and became attributes on somebody's
   `<head>`. Each token below carries which of the spec's states it is written
   against.

   So the honest statement is measured rather than absolute: **against every form
   below it has been driven through a real parser, both halves hold** — the forms
   are the boilerplate header, a quoted `>` in a tag and in a doctype, the four
   comment endings, an unterminated comment, a raw-text trap, an XML declaration,
   a CDATA section, a BOM and a bare fragment. This is a scanner and not a
   parser, so a construct nobody has thought of could still put the anchor
   somewhere the parser is not; the failure would be what it was here, a meta
   that is not one, with nothing on screen to say so. That is the residual, and
   it is named rather than promised away. A new form found is a test below and a
   token fixed here — never a note that it usually works.

   The comment token is one of the two that have to follow the tokenizer rather
   than approximate it, since ending a comment late would swallow real content
   into it. All of the spec's endings are here: `-->`, the incorrectly closed
   `--!>`, the abrupt `<!-->` and `<!--->`, and end of input, which closes a
   comment the document never did. The tag scanners below are the other, and
   their note carries the rest of it.

   Sticky rather than global: each is asked "do you match *here*", which is what
   makes this a walk over the prologue and not a search through the document. */
const WHITESPACE = /\s+/y
const COMMENT = /<!--(?:>|->|[\s\S]*?--!?>|[\s\S]*$)/y

/* **A tag ends at the first `>` that is not inside a quoted attribute value**,
   and `[^>]*>` did not know that. `<head data-x="a>b">` was read as ending at
   the `>` in the middle of the value, so the anchor was *inside an unclosed
   tag* — and a meta put there is not a late meta, it is not a meta at all. What
   the browser built out of it, measured in a `srcdoc` frame with this app's
   sandbox: no `<meta http-equiv>` anywhere in the document, `head` wearing
   `content="default-src 'none'…"` as an attribute, and the stylesheet on the
   next line going out to the network. Nothing about the rest of the walk was
   wrong; the token was.

   The three alternatives are mutually exclusive — a `"` can only open the
   double-quoted branch, a `'` only the single-quoted, and the unquoted branch
   takes neither — which is a correctness rule twice over. It is faithful, since
   the tokenizer refuses a quote inside an unquoted value as a parse error. And
   it is what keeps the match **linear**: with `[^>]` as the third branch, as
   this was first written for review, a `"` is matched by two branches at once,
   so a tag whose quote is never closed can be split exponentially many ways
   while the engine looks for a `>` that is not there. Measured on
   `<html ` + `a="b" `×n + `"`: 1.7 ms at n=10, 130 ms at n=16, 6 s at n=20, 41 s
   at n=22, and this walks a file of up to 2 MiB on the render thread. The
   exclusive form answers the same inputs in 0.1 ms.

   What the exclusive form gives up is matching a tag with an **unterminated**
   quote at all, and that is the safe direction: no match means the walk stops
   there and the anchor stays wherever it already was — at the doctype, or in
   front of the whole document — which is live and first. The other form would
   match up to the next `>`, which the parser is still reading as attribute
   value, and put the meta inside it.

   **The doctype deliberately does not take this treatment.** A `>` ends a
   doctype even inside quotes — the tokenizer's abrupt-doctype-public-identifier
   and abrupt-doctype-system-identifier states emit the token there — so
   `[^>]*>` *is* the faithful reading, and a quote-aware version would put the
   anchor past a point the parser has already left. Checked rather than
   inherited from the spec text: parsing `<!doctype html SYSTEM "a>b"><p>x</p>`
   leaves `b">x` as document text, so the doctype did end inside the quotes.
   `BOGUS` is left alone for the same reason — a bogus comment ends at the first
   `>` whatever is around it. */
const TAG_TAIL = '(?:"[^"]*"|\'[^\']*\'|[^"\'>])*>'
const DOCTYPE = /<!doctype[^>]*>/iy
const BOGUS = /<[!?/][^>]*>/y
const ROOT = new RegExp(`<html\\b${TAG_TAIL}`, 'iy')
const HEAD = new RegExp(`<head\\b${TAG_TAIL}`, 'iy')

const at = (re, html, index) => {
  re.lastIndex = index
  return re.exec(html)
}

/* The walk: `{ root, anchor }`, where `root` is the real root tag with the index
   it sits at — `null` when the prologue holds none — and `anchor` is the index
   the policy goes in front of.

   The anchor is the end of the root tag, or of the head tag, or of the doctype,
   or the very start, in that order, and the order falls out of the document
   rather than out of a list here: whichever of the three the walk reaches, it
   reaches in the order a document writes them.

   `COMMENT` is tried before `BOGUS` because `<!--` also fits `<!…>` whenever the
   comment holds a `>`, and `DOCTYPE` before `BOGUS` for the same reason. A
   second doctype is left to `BOGUS` and skipped, which is what a parser does
   with one too. */
function prologue(html) {
  let index = 0
  let anchor = 0
  let doctype = false
  for (;;) {
    const skip = at(WHITESPACE, html, index) ?? at(COMMENT, html, index)
    if (skip) {
      index += skip[0].length
      continue
    }
    if (!doctype) {
      const found = at(DOCTYPE, html, index)
      if (found) {
        doctype = true
        index += found[0].length
        anchor = index
        continue
      }
    }
    const bogus = at(BOGUS, html, index)
    if (bogus) {
      index += bogus[0].length
      continue
    }
    const root = at(ROOT, html, index)
    if (root) return { root: { index, tag: root[0] }, anchor: index + root[0].length }
    const head = at(HEAD, html, index)
    if (head) return { root: null, anchor: index + head[0].length }
    return { root: null, anchor }
  }
}

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
   accumulating anything.

   The tag it marks is the prologue's, never the first `<html` in the text: a
   root tag hidden inside a leading conditional comment is a comment, and
   stamping a theme in there both loses the theme and edits somebody's comment.
   The rule that finds it is one walk above, shared with `guarded` so the two
   marks cannot land in two different places. */
export function themed(html, theme) {
  /* The prop defaults to `''`, but the buffer behind it belongs to a store and
     may be absent while a tab is still loading or after a failed read. A rule
     that threw here would blank the tab over a value the caller is entitled to
     have. */
  if (typeof html !== 'string') return ''
  if (theme !== 'dark' && theme !== 'light') return html

  const { root } = prologue(html)
  /* Nothing to mark: an empty buffer, a failed read, a file mangled since it was
     written, or a document whose root tag exists only inside a comment.
     Inventing a root would be rewriting somebody's document, and a document with
     no root of its own is still one a browser will draw. */
  if (!root) return html

  const inner = root.tag.slice(1, -1).replace(OWN_THEME, '').trimEnd()
  /* Spliced by index rather than through `replace`: the tag being put back came
     out of a document rather than out of this file, and in the string form of
     `replace` a `$&` inside it would be a substitution pattern. */
  return `${html.slice(0, root.index)}<${inner} data-theme="${theme}">${html.slice(
    root.index + root.tag.length
  )}`
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

/* The same document with this app's policy in front of everything in it.

   Where it goes is the whole of the correctness, and it is the walk above that
   decides: a policy binds only what the parser meets after it, and a `<meta>`
   whose parent is not the head is ignored outright. Behind the root tag the
   parser opens an implicit head, puts the meta in it and ignores the explicit
   `<head>` that follows — the spec's own behaviour rather than a trick, and
   pinned by a test. Behind the head tag for a document that omits the root,
   `<html>` being an optional tag. Behind the doctype for a fragment with
   neither, and never in front of one: a doctype with anything before it is not a
   doctype, and the document would fall into quirks mode over a security header.
   In front of everything for a fragment that has no doctype either — a buffer
   still loading, a read that refused, a snippet somebody saved.

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
   there is no **ordering** by which a file talks its way out of this one.

   That is deliberately narrower than what stood here, which added "and no
   content". Content is exactly how a file talked its way out twice — a `<html`
   inside a leading conditional comment, and then a `>` inside a quoted attribute
   value, each of which put this meta somewhere it was not a meta. Both are
   closed and both are pinned by tests, and what holds them closed is the walk
   above agreeing with the tokenizer, which is a thing measured against a list of
   forms rather than a thing proved. The header above carries that list and the
   residual it leaves. */
export function guarded(html) {
  if (typeof html !== 'string') return ''
  const { anchor } = prologue(html)
  return `${html.slice(0, anchor)}${CSP_META}${html.slice(anchor)}`
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
