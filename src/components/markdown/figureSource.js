/* What an illustration node in the conversation panel may point at — two
   separate questions, both decided here rather than guessed by
   `MarkdownFigure.vue`, because a `.vue` file is the one thing no test in
   this repository can reach.

   `markdown.js`'s own `image()` branch deliberately carries no scheme gate on
   `src` (see its header): the acceptance criteria for that parser require a
   bare relative path — `./a.png` — to survive untouched, and the gate a
   *link* carries belongs to `link()` alone. That leaves the question of which
   sources this renderer may actually draw entirely open, and it is answered
   here rather than by reusing `link()`'s own `OPENABLE`.

   The two are not the same question. A link is only ever opened by an
   explicit click, through the OS's own browser
   (`opener:allow-open-url`) — nothing this app fetches on its own account.
   An illustration is the opposite: the moment the markup exists the webview
   paints an `<img>`, unattended, so a scheme this app cannot make sense of as
   a picture — `javascript:` chief among them — is refused outright rather
   than handed to the DOM at all. A remote `http(s)://` address is refused for
   a different reason: this is an offline-first desktop app that fetches
   nothing of its own accord, and an inline illustration that silently asked a
   stranger's server for bytes on every render of a task nobody opened a
   connection for is a tracking pixel with no click behind it — the one thing
   a link is trusted with because a click is a decision and a paint is not.
   `//host/path` is refused for the same reason and not a separate one: it has
   no scheme by RFC 3986's own grammar, but every renderer resolves it against
   the current origin's scheme (`new URL('//evil.example.com/x', location.href)`
   is `http://evil.example.com/x` in the dev server and in a Windows release
   alike), so it is exactly as live a request as a written-out `http://` and
   is refused before the scheme test ever runs.

   What is accepted: a source with no scheme at all and no leading `//` — a
   relative or an absolute filesystem path, `./a.png` among them — and a
   self-contained `data:` URI whose media type is `image/…`, since it asks the
   network for nothing. Loading a path's bytes into the front end is not this
   module's question and not yet built at all (the wire still carries only a
   path); what this module decides is narrower — which sources the renderer
   may even attempt to draw. */

/* A scheme, by RFC 3986's own grammar (`ALPHA *( ALPHA / DIGIT / "+" / "-" /
   "." )`), with one restriction this module adds on top: at least two
   characters ahead of the colon. RFC 3986 puts no floor on a scheme's own
   length, and no scheme this short is registered, so the floor exists for one
   reason only — `C:\Users\x\fig.png`, an ordinary Windows absolute path, is
   not refused as though its drive letter were a one-letter scheme. It does
   not make such a path resolve to a real picture (nothing does — see this
   module's own tests and the task's tracker note), only keeps it from being
   confused with `javascript:` at this gate. */
const SCHEME = /^([a-z][a-z0-9+.-]+):/i

/* The media type of a `data:` URI, lower-cased, stopping at the first `;` or
   `,` the way the URI's own grammar does — `data:image/svg+xml;base64,AAAA`
   answers `image/svg+xml`, `data:text/html,<script>` answers `text/html`. */
function dataUriMediaType(src) {
  const rest = src.slice('data:'.length)
  const end = rest.search(/[;,]/)
  return (end === -1 ? rest : rest.slice(0, end)).trim().toLowerCase()
}

export function isAllowedFigureSrc(src) {
  if (typeof src !== 'string') return false
  const trimmed = src.trim()
  if (trimmed === '') return false
  /* Protocol-relative first, ahead of the scheme test: `//host/path` has no
     scheme by the grammar below, and would otherwise fall through the "no
     scheme" branch as though it were a filesystem path. */
  if (trimmed.startsWith('//')) return false
  const scheme = SCHEME.exec(trimmed)
  if (!scheme) return true
  if (scheme[1].toLowerCase() !== 'data') return false
  /* A `data:` URI is only ever accepted as a picture — `data:text/html,…` is
     inert inside the `<img>`/`readInlineSvg` doors this module opens, but
     admitting it here would be an unforced "any media type will do" answer
     nobody asked for. */
  return dataUriMediaType(trimmed).startsWith('image/')
}

/* Whether a source is the one shape that can ever become the preferred,
   inline `<svg>` form — a self-contained `data:image/svg+xml` URI, with
   arbitrary parameters ahead of its payload (`;base64`, `;charset=utf-8`,
   the common non-standard `;utf8`, any order, any of them absent). The
   parameter list is deliberately not a closed set: the one thing that must
   never happen is `isAllowedFigureSrc` accepting an `image/svg+xml` source
   this regexp does not also recognise, since that source would then reach
   the raster `<img>` branch un-walked — an SVG with an arbitrary payload,
   including a hard-coded colour, painted matted and unchecked. Nothing else
   carries literal markup for this renderer to draw: an ordinary path or a
   `data:image/png` source is always the raster branch. */
const DATA_SVG = /^data:image\/svg\+xml((?:;[a-z0-9_-]+(?:=[^;,]*)?)*),([\s\S]*)$/i

export function isInlineSvgSrc(src) {
  return typeof src === 'string' && DATA_SVG.test(src.trim())
}

function decodeSvgPayload(src) {
  const match = DATA_SVG.exec(src.trim())
  if (!match) return null
  const [, params, payload] = match
  const base64 = /(^|;)base64(;|$)/i.test(params)
  try {
    return base64 ? atob(payload) : decodeURIComponent(payload)
  } catch {
    /* Neither valid base64 nor a valid percent-encoding — not a payload this
       data URI's own rules allow, so nothing here reads it as one. */
    return null
  }
}

/* The short, readable name for a source — what the error placeholder prints
   instead of a percent-encoded `data:` payload that can run to hundreds of
   characters and clips silently inside the figure's own frame. A `data:`
   source is named by its media type alone (`data:image/svg+xml`, never its
   body); anything else is short enough already to print whole. */
export function describeFigureSrc(src) {
  if (typeof src !== 'string') return ''
  const trimmed = src.trim()
  if (SCHEME.exec(trimmed)?.[1]?.toLowerCase() === 'data') {
    return `data:${dataUriMediaType(trimmed)}`
  }
  return trimmed
}

/* The closed vocabulary of an agent-drawn diagram: shapes and text, nothing
   that loads a second resource (`use`, `image`, an `href` of any kind),
   nothing that carries a script or a foreign stylesheet (`script`, `style`,
   `foreignObject`). `svg` names only the document's own root — `readElement`
   below refuses it at any deeper depth — so there is no nested viewport to
   reason about either.

   This set is attribute *names*; it says nothing about their values. Every
   name in `ELEMENT_ATTRS` is drawn whatever it says — `d`, `points`,
   `transform` and the rest are geometry, not colour, and are not checked
   past being present at all. Only the two in `COLOR_ATTRS` below have their
   *value* walked, against the token rule. Adding an attribute that can also
   carry a colour — `stop-color`, `flood-color`, `lighting-color` — to any
   element's set without adding it to `COLOR_ATTRS` reopens exactly the hole
   this module exists to close. */
const SVG_ELEMENTS = new Set([
  'svg',
  'g',
  'path',
  'rect',
  'circle',
  'ellipse',
  'line',
  'polyline',
  'polygon',
  'text',
  'tspan',
  'title'
])

/* Presentation attributes every shape may carry. `font-family` and
   `font-size` are deliberately absent — the contract's own rule is that text
   inside an agent-drawn diagram is styled by us, mono at `--text-2xs`, and
   `sm-prose.css` already forces both on `svg text`; there is nothing for the
   source to usefully spend those two attributes on. */
const COMMON_ATTRS = new Set([
  'transform',
  'opacity',
  'fill',
  'stroke',
  'stroke-width',
  'stroke-linecap',
  'stroke-linejoin',
  'stroke-dasharray',
  'fill-opacity',
  'stroke-opacity'
])

const ELEMENT_ATTRS = {
  svg: new Set(['xmlns', 'viewBox', 'role', 'aria-label', 'aria-hidden', 'preserveAspectRatio']),
  path: new Set(['d']),
  rect: new Set(['x', 'y', 'width', 'height', 'rx', 'ry']),
  circle: new Set(['cx', 'cy', 'r']),
  ellipse: new Set(['cx', 'cy', 'rx', 'ry']),
  line: new Set(['x1', 'y1', 'x2', 'y2']),
  polyline: new Set(['points']),
  polygon: new Set(['points']),
  text: new Set(['x', 'y', 'text-anchor', 'dominant-baseline']),
  tspan: new Set(['x', 'y', 'dx', 'dy']),
  title: new Set()
}

/* The one rule the contract states in so many words: a colour attribute paints
   only `currentColor`, `none`, `transparent` or a `var(--token)` reference —
   never a hex, an `rgb()` and never a named colour. `none`/`transparent` are
   the same two structural exceptions the rest of this design system allows.
   `fill`/`stroke` are the only two attributes in the whole vocabulary above
   that can paint a colour at all — see `SVG_ELEMENTS`'s own comment for what
   adding a third one obliges. */
const COLOR_ATTRS = new Set(['fill', 'stroke'])
const SAFE_COLOR = /^(none|transparent|currentcolor)$/i
const TOKEN_COLOR = /^var\(--[a-z0-9-]+\)$/i

function isSafeColorValue(value) {
  const v = value.trim()
  return SAFE_COLOR.test(v) || TOKEN_COLOR.test(v)
}

/* How deep a diagram's own groups may nest, the same guard `markdown.js`
   keeps against a pathological quote or list: depth taken from the input
   rather than decided by this app, and a computed on the UI thread is not the
   place to find out how deep is too deep. Far past anything an agent draws by
   hand and far short of the stack. */
const MAX_SVG_DEPTH = 32

/* How many elements a diagram may carry in total, beside the depth cap above.
   Depth alone does not bound a flat diagram of a great many siblings — every
   accepted element becomes a Vue component instance through
   `InlineFigureSvg`'s own recursion, which is exactly the class of cost
   `markdown.js`'s ten-backtick cap on a code span exists to refuse elsewhere
   in this same file family. Far past anything an agent draws by hand. */
const MAX_SVG_NODES = 500

function readElement(el, depth, budget) {
  const tag = el.tagName
  if (depth > MAX_SVG_DEPTH) {
    throw new Error('the diagram nests too deep to draw safely')
  }
  budget.count += 1
  if (budget.count > MAX_SVG_NODES) {
    throw new Error('the diagram has too many elements to draw safely')
  }
  if (!SVG_ELEMENTS.has(tag) || (tag === 'svg' && depth > 0)) {
    throw new Error(`"<${tag}>" is not a diagram element this renderer draws`)
  }
  const allowed = ELEMENT_ATTRS[tag]
    ? new Set([...COMMON_ATTRS, ...ELEMENT_ATTRS[tag]])
    : COMMON_ATTRS
  const attrs = {}
  for (const attr of el.attributes) {
    const name = attr.name
    /* A colon names a foreign namespace — `xlink:href` chief among them,
       which is exactly the "load a second resource" door this vocabulary
       refuses to open at all. */
    if (name.includes(':')) {
      throw new Error(`"${name}" names a namespace this renderer does not draw`)
    }
    if (!allowed.has(name)) {
      throw new Error(`"${name}" is not an attribute this renderer draws`)
    }
    if (COLOR_ATTRS.has(name) && !isSafeColorValue(attr.value)) {
      throw new Error(`"${name}" paints a colour outside the token set`)
    }
    attrs[name] = attr.value
  }
  const children = []
  for (const child of el.children) {
    children.push(readElement(child, depth + 1, budget))
  }
  const text =
    tag === 'text' || tag === 'tspan' || tag === 'title'
      ? Array.from(el.childNodes)
          .filter((node) => node.nodeType === 3)
          .map((node) => node.textContent)
          .join('')
      : ''
  return { tag, attrs, children, text }
}

/* The whole of "the renderer checks the render": decode a `data:image/svg+xml`
   payload, parse it and walk it against the vocabulary above. Anything the
   walk refuses — an unknown element, an attribute outside the set, a foreign
   namespace, a colour outside the tokens — fails the *whole* figure rather
   than being stripped and drawn anyway, because a sanitiser that still shows
   something is a sanitiser somebody has to trust the parts of; this one is a
   gate. `MarkdownFigure.vue` draws the failure exactly the way it draws a
   raster that failed to load — the same placeholder, with the reason below as
   its caption — which is what keeps this a render rule and not a second kind
   of error the panel has to explain twice. */
export function readInlineSvg(src) {
  const payload = decodeSvgPayload(src)
  if (payload == null) return { ok: false, reason: 'the data URI could not be decoded' }
  let doc
  try {
    doc = new DOMParser().parseFromString(payload, 'image/svg+xml')
  } catch {
    return { ok: false, reason: 'the markup could not be parsed' }
  }
  if (doc.getElementsByTagName('parsererror').length > 0) {
    return { ok: false, reason: 'the markup is not well-formed XML' }
  }
  const root = doc.documentElement
  if (!root || root.tagName !== 'svg') {
    return { ok: false, reason: 'the markup has no <svg> root' }
  }
  try {
    return { ok: true, root: readElement(root, 0, { count: 0 }) }
  } catch (err) {
    return { ok: false, reason: err.message }
  }
}
