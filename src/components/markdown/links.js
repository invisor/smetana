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
   as it did before this module existed. **Local is not "everything else"** —
   a scheme-less target still has to look like a path before it is read as
   one, which is the whole of `LOOKS_LIKE_PATH` below. `[label](src/foo.js)`
   used to fall through the same door `javascript:` still falls through
   today, and nothing on screen said a path was a path; a bare
   `[two nil](2:1)` used to fall through it too, and has to go on doing so —
   a scheme-less string with no slash and no extension is ordinary prose this
   module cannot tell from a path, and its job when it cannot tell is to
   leave the characters alone.

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

/* RFC 3986's own scheme grammar, `ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )`
   followed by `:`, in full — the dot included, which an earlier version of
   this line dropped and should not have. Narrowing the grammar itself was
   the wrong fix: it stopped `tauri.conf.json:41` reading as a scheme, but it
   also stopped every *dotted* scheme being recognised as one at all, and a
   reverse-DNS scheme is an ordinary construct — `com.example.app://…` is the
   everyday shape of a desktop deep link, and a narrowed grammar read it as a
   local path and drew it as a pressable anchor over a click that could only
   fail. The collision is separated instead, with a negative lookahead on
   what follows the colon: `(?!\d)` refuses the match when a digit comes
   right after it. Every scheme this app declines is still declined by
   it — `mailto:`, `javascript:`, `data:`, `file:`, and `com.example.app://…`
   — because none of them is followed by a digit; `tauri.conf.json:41` is
   followed by `4`, so it is no longer read as a scheme and reaches
   `LOOKS_LIKE_PATH` below, which is what it takes to be a local target.

   It is deliberately not narrowed to a handful of named schemes: one neither
   this app nor the paragraph above has ever heard of is exactly as
   unopenable as `mailto:`, and refusing by shape rather than by a list is
   what keeps a local path with a genuine colon in it (a Windows drive
   letter, `C:\Users\x`) from slipping through unrefused — a single letter in
   front of a colon matches this grammar too, which is a known, accepted gap:
   this app's local links are written as repository-relative paths, where a
   colon never opens the string, and a Windows absolute path reaching this
   parser is not a case any fixture in this project has needed yet.

   **What this closes and what it does not, stated plainly so the second
   half is not rediscovered as a bug.** Closed: `com.example.app://…`, the
   realistic shape of a reverse-DNS deep link, `//` after the colon and
   declined like any other scheme. **Not closed**: `com.example.app:41` — a
   dotted scheme followed by a bare number, no `/` anywhere. The lookahead
   separates a dotted scheme from a dotted *file name* only by what follows
   the colon, a `/` against a digit, and `com.example.app:41` has the digit,
   so it passes the lookahead and reaches `LOOKS_LIKE_PATH` below with the
   identical shape a real file and a line number has — a dot, a short
   alphanumeric run, a line-shaped suffix — and reads as local, the same as
   `tauri.conf.json:41` does and must: nothing in either regex can tell the
   two apart, and nothing should try to. An allow-list of real extensions was
   considered and refused for the reason `LOOKS_LIKE_PATH`'s own header
   already gives for reading "any short alphanumeric run" rather than a
   list — this repository alone holds `.js`, `.vue`, `.rs`, `.toml`, `.json`,
   `.md`, `.css`, `.mjs` and `.py`, a list a person keeps by hand goes stale
   the first time somebody adds a file type nobody thought of, and at that
   point a real path stops being a link with nothing on screen to say why.
   `com.example.app:41` is not a construct anybody writes by accident either
   — it only becomes a link at all inside an explicit `[label](target)`,
   which somebody has to type on purpose — so the realistic case is closed
   and this one is left as a documented, accepted gap rather than chased.
   This is not the residual gap `LOOKS_LIKE_PATH`'s own header names
   (`foo:1/bar`, which needs a `/` as well) — it is a second, narrower one,
   worth naming here rather than discovering by surprise. */
const OTHER_SCHEME = /^[a-zA-Z][a-zA-Z0-9+.-]*:(?!\d)/

/* The positive shape a scheme-less target has to have before it is read as
   local at all — without this, "everything that is not a scheme" swallowed
   ordinary prose that happens to carry a colon: a football score written
   `[two nil](2:1)`, an aspect ratio `16:9`, a time `12:30`. None of those has
   a scheme (`OTHER_SCHEME` only refuses a string that *starts* with a
   letter), so all of them used to reach `classifyLink`'s local branch, and
   `LINE_SUFFIX` then chewed `2:1` down to the single character `'2'` — a
   construct this parser used to leave as text quietly becoming a link with
   most of what was typed gone.

   Two shapes count as a path, tested against the **whole target exactly as
   it arrived**, before anything below strips a trailing slash or a line
   suffix off it: a `/` anywhere (a relative or absolute path, or a
   directory's own trailing one), or a dot followed by a short run of letters
   and digits — an extension — optionally followed by a line reference,
   `.rs`, `.json:41`, `.rs:12:3`. Testing the stripped-down string instead
   would have let `2:1` back in by a different door: strip the line suffix
   first and `'2'` is what is left to judge, which looks exactly like a
   one-character extension-less name — the very shape this rule exists to
   keep out.

   A bare word with no slash and no extension — `docs`, `notes` — fails both
   and falls through to text, and that is the honest answer rather than a
   gap: this module cannot tell it from an ordinary word, and it is not this
   module's business to guess. The Windows drive-letter gap `OTHER_SCHEME`'s
   own comment names is untouched by this — a path like `C:\Users\x\a.rs` is
   declined earlier, as an unrecognised scheme, and never reaches this
   check.

   **One more gap, named rather than chased.** A scheme whose colon is
   followed by a digit *and* which also contains a `/` — `foo:1/bar` — passes
   `OTHER_SCHEME`'s lookahead for the reason `tauri.conf.json:41` needs it to,
   and then reads as local off the `/` alone. It is contrived, it never
   reaches `openExternal` (nothing gets there but the external branch above),
   and Rust's own `resolve_within` holds the filesystem boundary regardless
   of what this module classifies — so the cost of chasing it is higher than
   the cost of leaving it. `OTHER_SCHEME`'s own header names the narrower
   sibling of this same gap, a dotted scheme with a digit after its colon and
   no `/` at all. */
const LOOKS_LIKE_PATH = /\/|\.[A-Za-z0-9]{1,10}(?::\d+(?::\d+)?)?$/

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

  /* The positive shape check, against the whole target as written — before
     the slash or the line suffix below is touched, for the reason
     `LOOKS_LIKE_PATH`'s own comment gives. Anything that does not look like a
     path is an ordinary word this module cannot tell from prose. */
  if (!LOOKS_LIKE_PATH.test(href)) return null

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
