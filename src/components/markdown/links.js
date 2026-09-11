/* The rule that tells two breeds of link apart in the agent's prose, and the
   two things that follow once one is called local: the cut of its path into a
   head that may truncate and a tail that never does, and the `file://` href a
   local anchor carries for native focus, middle-click and "copy link" (section
   5 of `docs/design_handoff_conversation_panel/markup-contract.md`).

   Pure, like every module beside `markdown.js` in this family — no Vue, no
   DOM, no store — because a `.vue` file is the one thing no test in this
   repository can reach and `markdown.js`'s own header already says why a
   parser is the most test-shaped thing in the front end to keep that way.
   `markdown.js` is the one caller: `link()` there asks `classifyLink` what a
   target is before it decides what kind of node to build, so there is exactly
   one place in the tree that classifies a link target rather than two that
   could disagree. `absolutePath` from `src/paths.js` is the only import —
   pure itself, no Vue and no Tauri, so bringing it in does not cross the one
   import rule `MarkdownInline.vue`'s own header states.

   **The rule, stated once so it does not have to be reverse-engineered from
   the regexes below.** `https://` and `http://` are external — unchanged from
   what `markdown.js` did on its own before this module existed, scheme
   lowercased for `opener:allow-open-url`'s sake, everything else in the URL
   left exactly as typed. Any other named scheme — `mailto:`, `javascript:`,
   `data:`, and the literal `file:` a person might paste by hand — is refused
   and falls through to literal text, which is the invariant `markdown.js` is
   built on: an unrecognised construct costs nothing more than looking exactly
   as it did before this module existed. Everything else — a bare relative or
   absolute path, with no scheme in front of it at all — is local. That last
   arm is the whole of what changed: before this module, `[label](src/foo.js)`
   fell through the same door `javascript:` still falls through today, and
   nothing on screen said a path was a path.

   **A trailing slash is the only signal this module has for "this names a
   folder", and it is spent rather than kept.** `sm-prose.css` appends the
   folder glyph itself (`.sm-prose a[data-kind="dir"]::after`), so a slash left
   in `path` or in the displayed tail would draw two of them; `classifyLink`
   strips it before either is built. A trailing `:<line>` or `:<line>:<col>` on
   a *file* target is the opposite kind of signal — grep's and a compiler's own
   convention for naming a spot inside a file — and it is stripped from `path`
   for the same reason a folder's slash is: `path` is what a real file on disk
   is opened by, and no filesystem has a file whose name ends in a colon and a
   number.

   **The line does not reach the file it names, and that is a gap in
   `stores/tabs.js` rather than in this module.** `openFile` there takes a
   path and nothing else — no line, no column — so there is nowhere for one to
   go once a click reaches `views/DesktopApp.vue`. The line survives in
   `display`, which is what the tail is cut from below, precisely so it is not
   lost on the way there: a file opens at wherever it last was, and the line a
   person meant is still readable in the link itself rather than silently
   dropped. Should `openFile` ever grow the means to land on a line, this is
   the one place to widen — `path` would still be the clean target and
   `display` would still carry the number, so nothing here would have to
   change shape, only gain a reader. */
import { absolutePath } from '../../paths.js'

const HTTP_SCHEME = /^(https?):\/\//i

/* RFC 3986's own scheme grammar (`ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`
   followed by `:`), used only to recognise — and then decline — every scheme
   this app does not open. It is deliberately not narrowed to the handful of
   names in this file's own header: a scheme neither this app nor the header
   above has ever heard of is exactly as unopenable as `mailto:`, and refusing
   by shape rather than by a list is what keeps a local path with a genuine
   colon in it (a Windows drive letter, `C:\Users\x`) from slipping through
   unrefused — a single letter in front of a colon matches this grammar too,
   which is a known, accepted gap: this app's local links are written as
   repository-relative paths, where a colon never opens the string, and a
   Windows absolute path reaching this parser is not a case any fixture in
   this project has needed yet. */
const OTHER_SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:/

/* A line, or a line and a column, glued onto the end of a file target with a
   colon — never applied to a directory target, since a folder has no line to
   name and a name that merely ends in digits after a colon (rare, but legal
   on every filesystem this app runs on) must not lose them. */
const LINE_SUFFIX = /:(\d+)(?::\d+)?$/

/* Classifies one link target. Answers `{ kind: 'external', href }`,
   `{ kind: 'local', targetKind, path, display }`, or `null` for anything this
   module does not recognise — the fall-through `markdown.js`'s own invariant
   depends on: a target this function declines must reach the caller exactly
   as `null` always has, so the surrounding markdown stays literal text.

   `path` is what a local target is opened by — cleaned of its trailing slash
   and its trailing line reference, so it is always a name a filesystem could
   hold. `display` is what the visible text is built from (`splitPath` below):
   the same string with the slash cut but the line reference still on it,
   because the line is worth reading even though it plays no part in opening
   the file. */
export function classifyLink(href) {
  if (typeof href !== 'string' || href === '') return null

  const http = HTTP_SCHEME.exec(href)
  if (http) {
    return { kind: 'external', href: http[1].toLowerCase() + href.slice(http[1].length) }
  }

  /* Every other named scheme is out of scope and stays literal text, exactly
     as it did before this module existed. */
  if (OTHER_SCHEME.test(href)) return null

  /* A bare fragment names a spot in a document this parser does not produce
     ids for, never a place on disk — refusing it here is what keeps it
     falling through to text rather than being read as a very short relative
     path. */
  if (href.startsWith('#')) return null

  const isDir = href.endsWith('/')
  const trimmed = isDir ? href.slice(0, -1) : href
  if (!trimmed) return null

  const line = isDir ? null : LINE_SUFFIX.exec(trimmed)
  const path = line ? trimmed.slice(0, line.index) : trimmed
  if (!path) return null

  return { kind: 'local', targetKind: isDir ? 'dir' : 'file', path, display: trimmed }
}

/* The middle-truncation split, done once here rather than in CSS, which
   cannot do it (the contract's own reasoning, section 5): `head` is
   everything up to and including the last `/` — the part that may lose
   letters to an ellipsis — and `tail` is the file name and any line number,
   which must survive at any column width. A target with no `/` in it at all
   is all tail and an empty head, which is also what lets `sm-prose.css`'s
   `p > a[data-path]:only-child` rule give the whole column to a lone link
   with nothing to truncate. */
export function splitPath(display) {
  const at = display.lastIndexOf('/')
  if (at === -1) return { head: '', tail: display }
  return { head: display.slice(0, at + 1), tail: display.slice(at + 1) }
}

/* The `href` a local anchor carries, for the three things `data-path` and
   `data-kind` deliberately do not decide on their own: the browser's native
   focus ring, a middle-click, and "copy link". `root` is the active project's
   absolute path (`filesState.root`); `path` is the relative one `classifyLink`
   produced. `''` for no root at all — a link drawn before a project's files
   are known, or drawn where nothing supplies one at all (see the note on the
   task inspector in `MarkdownInline.vue`) — since a `file://` URI with nothing
   after it would be worse than none.

   Three slashes and not two: `file://` is the scheme and its authority (empty,
   meaning "this machine"), and the path after it must start with its own `/`
   — `file:///Users/…` — or the string reads as a hostname. `absolutePath`
   answers a Windows path with a leading drive letter and no leading
   separator, so that case is given one; a Unix root already has it and is
   left alone. Backslashes are turned to `/` first, since a URI's path is
   always `/`-separated whatever the filesystem underneath spells it with, and
   `encodeURI` — not `encodeURIComponent` — is what escapes the characters a
   URI cannot carry raw (spaces, a `#`) while leaving `/` and `:` alone, since
   those are the URI's own structure and not part of any one path segment. */
export function localHref(root, path) {
  if (!root) return ''
  const forward = absolutePath(root, path).replace(/\\/g, '/')
  const rooted = forward.startsWith('/') ? forward : `/${forward}`
  return `file://${encodeURI(rooted)}`
}
