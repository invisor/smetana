/* Markdown to a tree, for the task inspector.

   Pure — no Vue and no DOM — because a `.vue` file is the one thing no test in
   this repository can reach, and a parser is the most test-shaped thing in the
   front end. `Markdown.vue` draws what this returns and holds no rules of its
   own.

   The invariant every branch below is written to keep: **no character of the
   source disappears.** Anything unrecognised — an unclosed fence, a stray
   asterisk, a table, a reference link, an HTML tag — comes back as ordinary
   text, so the worst outcome for an unsupported construct is the panel as it
   looked before this module existed. That is what makes it safe to put between
   a person and the only copy of a task's description. */

/* The closing run of hashes is optional and must have whitespace before it.
   Without that whitespace `## Migrate to C#` loses the character that makes the
   language a language, silently and with nothing on screen to say a word went
   missing — the one thing this module exists not to do. `### a #` still drops
   its hash, which is what a closing sequence is. */
const HEADING = /^ {0,3}(#{1,6})\s+(.*?)(?:\s+#+)?\s*$/
const FENCE = /^ {0,3}(`{3,}|~{3,})\s*(\S*)\s*$/
const RULE = /^ {0,3}([-*_])\s*(?:\1\s*){2,}$/
const QUOTE = /^ {0,3}> ?(.*)$/
const BULLET = /^(\s*)([-*+])(\s+)(.*)$/
const ORDERED = /^(\s*)(\d{1,9})[.)](\s+)(.*)$/
const TASK = /^\[([ xX])\]\s+(.*)$/

/* How deep a quote may nest inside a quote, and a list inside a list. Both are
   recursive block constructs, and both take their depth from the input rather
   than from anything this app decides: one line of ten thousand `>` is ten
   thousand levels, and it overflowed the stack — which in a computed on the UI
   thread is a blank panel where an issue used to be. Sixteen is far past any
   prose a person writes and far short of the stack; past it the markers are
   drawn as the characters they are, so nothing is lost, only unnested. */
const MAX_BLOCK_DEPTH = 16

export function parseMarkdown(text) {
  if (typeof text !== 'string') return []
  return parseBlocks(text.replace(/\r\n?/g, '\n').split('\n'))
}

function parseBlocks(lines, depth = 0) {
  const nested = depth < MAX_BLOCK_DEPTH
  const blocks = []
  let i = 0
  while (i < lines.length) {
    const line = lines[i]
    if (!line.trim()) {
      i++
      continue
    }

    const fence = FENCE.exec(line)
    if (fence) {
      const close = new RegExp(`^ {0,3}${fence[1][0] === '`' ? '`' : '~'}{${fence[1].length},}\\s*$`)
      const body = []
      i++
      while (i < lines.length && !close.test(lines[i])) body.push(lines[i++])
      /* An unclosed fence runs to the end of the input rather than being
         reconsidered: the lines are still on screen either way, which is the
         invariant, and re-reading them as prose would swallow their leading
         spaces. */
      if (i < lines.length) i++
      blocks.push({ type: 'code', lang: fence[2] || null, text: body.join('\n') })
      continue
    }

    /* Before the list, so that `- - -` and `***` are breaks rather than a
       bullet holding an emphasis. */
    if (RULE.test(line)) {
      blocks.push({ type: 'rule' })
      i++
      continue
    }

    const heading = HEADING.exec(line)
    if (heading) {
      blocks.push({
        type: 'heading',
        level: heading[1].length,
        children: parseInline(heading[2])
      })
      i++
      continue
    }

    if (QUOTE.test(line) && nested) {
      const body = []
      while (i < lines.length && QUOTE.test(lines[i])) body.push(QUOTE.exec(lines[i++])[1])
      blocks.push({ type: 'quote', blocks: parseBlocks(body, depth + 1) })
      continue
    }

    if ((BULLET.test(line) || ORDERED.test(line)) && nested) {
      const [list, next] = takeList(lines, i, depth)
      blocks.push(list)
      i = next
      continue
    }

    /* The first line is taken whatever it is, and only then does `startsBlock`
       decide where the paragraph ends. Past the clamp above the line reaching
       here *is* a block starter — a quote marker drawn as text — and a loop that
       consulted `startsBlock` first would take nothing and never advance. */
    const body = [lines[i++]]
    while (i < lines.length && lines[i].trim() && !startsBlock(lines[i])) body.push(lines[i++])
    blocks.push({ type: 'paragraph', children: parseInline(body.join('\n')) })
  }
  return blocks
}

/* What ends a paragraph without a blank line before it. A list is deliberately
   in here: `bd` descriptions are written with criteria straight under their
   sentence, and a paragraph that swallowed them would be the bug this whole
   change exists to remove. */
function startsBlock(line) {
  return (
    FENCE.test(line) ||
    RULE.test(line) ||
    HEADING.test(line) ||
    QUOTE.test(line) ||
    BULLET.test(line) ||
    ORDERED.test(line)
  )
}

function takeList(lines, start, depth = 0) {
  const first = BULLET.exec(lines[start]) || ORDERED.exec(lines[start])
  const ordered = !BULLET.test(lines[start])
  const list = {
    type: 'list',
    ordered,
    start: ordered ? Number(first[2]) : 1,
    items: []
  }
  let i = start
  while (i < lines.length) {
    const match = BULLET.exec(lines[i]) || ORDERED.exec(lines[i])
    /* A less-indented marker belongs to an outer list, and any other line at
       this point is the end of this one — its continuations were taken by the
       item below. */
    if (!match || match[1].length !== first[1].length || isOrdered(lines[i]) !== ordered) break
    const width = match[1].length + match[2].length + match[3].length
    const body = [match[4]]
    i++
    while (i < lines.length && (!lines[i].trim() || leading(lines[i]) >= width)) {
      /* A blank line ends the item unless the list carries on under it. */
      if (!lines[i].trim() && !(i + 1 < lines.length && leading(lines[i + 1]) >= width)) break
      body.push(lines[i].slice(Math.min(leading(lines[i]), width)))
      i++
    }
    const task = TASK.exec(body[0])
    if (task) body[0] = task[2]
    list.items.push({
      checked: task ? task[1].toLowerCase() === 'x' : null,
      blocks: parseBlocks(body, depth + 1)
    })
  }
  return [list, i]
}

function isOrdered(line) {
  return ORDERED.test(line) && !BULLET.test(line)
}

function leading(line) {
  return line.length - line.trimStart().length
}

/* Inline markers, tried in this order at every position. Code first, so a
   backtick span wins over anything inside it; the image before the link, so its
   `!` is not left behind; `**` before `*` for the obvious reason.

   A link node is produced only for http and https. Every other scheme — file,
   mailto, and the ones that would be a security question elsewhere — stays
   literal text, which is both the honest thing to draw (this app cannot open
   it) and what keeps the URL itself on screen.

   Two guards that are not decoration. The closers of `**` and `__` refuse a
   third marker, so `**a *b***` closes on the outer pair and the emphasis inside
   it survives; without that the non-greedy match closes early and the last
   asterisks are left as litter. And `_` emphasis is refused when a letter or a
   digit sits against it on either side, because a tracker description is full
   of `close_reason` and `settings_load`: two such names in one sentence would
   otherwise open an emphasis at the first underscore and close it at the last,
   swallowing both markers and italicising the words between them. `*` keeps no
   such guard — nothing in this vocabulary is spelled with one. */
const INLINE = [
  /* The opener is capped rather than open-ended. A backreferenced greedy run
     around a lazy body is cubic in the length of a run of backticks — 3200 of
     them took 3.6 seconds, on the UI thread, inside a computed — and no code
     span is fenced by more than a few. Past ten the run is not an opener at
     all and falls through to plain text, markers included. */
  [/^(`{1,10})([\s\S]*?[^`])\1(?!`)/, (m) => ({ type: 'code', value: m[2] })],
  [/^!\[([^\]]*)\]\(\s*(\S+?)\s*\)/, (m) => image(m[1], m[2])],
  [/^\[([^\]]*)\]\(\s*(\S+?)\s*\)/, (m) => link(m[2], parseInline(m[1]))],
  [/^<(https?:\/\/[^>\s]+)>/i, (m) => link(m[1], [{ type: 'text', value: m[1] }])],
  [/^\*\*([\s\S]+?)\*\*(?!\*)/, (m) => ({ type: 'strong', children: parseInline(m[1]) })],
  [
    /^__([\s\S]+?)__(?![\p{L}\p{N}_])/u,
    (m, before) => (WORD.test(before) ? null : { type: 'strong', children: parseInline(m[1]) })
  ],
  [/^\*([^\s*][\s\S]*?)\*(?!\*)/, (m) => ({ type: 'em', children: parseInline(m[1]) })],
  [
    /^_([^\s_][\s\S]*?)_(?![\p{L}\p{N}_])/u,
    (m, before) => (WORD.test(before) ? null : { type: 'em', children: parseInline(m[1]) })
  ]
]

const OPENABLE = /^(https?):\/\//i
const WORD = /[\p{L}\p{N}_]/u

/* A scheme is case-insensitive by RFC 3986, so `HTTPS://x` is a link — but the
   href stored here is what reaches `opener:allow-open-url`, whose scope is
   spelled `https://*` and `http://*`, and nothing promises that glob is matched
   case-blind. Lowercasing the scheme and nothing else makes the two agree by
   construction: the rest of the URL is left exactly as written, because case
   is meaningful in a path, and the link's own label is what the person typed. */
function link(href, children) {
  const scheme = OPENABLE.exec(href)
  if (!scheme) return null
  return { type: 'link', href: scheme[1].toLowerCase() + href.slice(scheme[1].length), children }
}

function image(alt, href) {
  /* Never a picture: images are forbidden in this system apart from the app
     icon and the file-type icons, and a description is not a third exception. */
  const target = link(href, [{ type: 'text', value: href }])
  if (!target) return null
  return [alt ? { type: 'text', value: `${alt} ` } : null, target].filter(Boolean)
}

export function parseInline(text) {
  if (!text) return []
  const nodes = []
  let plain = ''
  let rest = text
  const flush = () => {
    if (plain) nodes.push({ type: 'text', value: plain })
    plain = ''
  }
  while (rest) {
    /* The character this position follows, in the original string — what tells
       an underscore between two words from one starting an emphasis. Empty at
       the very beginning, which no guard treats as a word. */
    const before = text.slice(0, text.length - rest.length).slice(-1)
    let hit = null
    for (const [pattern, build] of INLINE) {
      const match = pattern.exec(rest)
      if (!match) continue
      const built = build(match, before)
      /* A marker this module recognises but cannot honour — a link it may not
         open, an underscore inside a name — falls through to the plain branch
         below, so its characters stay on screen exactly as written. */
      if (!built) continue
      hit = { built, length: match[0].length }
      break
    }
    if (hit) {
      flush()
      nodes.push(...(Array.isArray(hit.built) ? hit.built : [hit.built]))
      rest = rest.slice(hit.length)
    } else {
      plain += rest[0]
      rest = rest.slice(1)
    }
  }
  flush()
  return nodes
}
