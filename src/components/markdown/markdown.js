/* Markdown to a tree, for the task inspector and the conversation panel.

   Pure — no Vue and no DOM — because a `.vue` file is the one thing no test in
   this repository can reach, and a parser is the most test-shaped thing in the
   front end. `Markdown.vue` draws what this returns and holds no rules of its
   own.

   The invariant every branch below is written to keep: **no character of the
   source disappears.** Anything unrecognised — an unclosed fence, a stray
   asterisk, a malformed table, a reference link, an HTML tag — comes back as
   ordinary text, so the worst outcome for an unsupported construct is the
   panel as it looked before this module existed. That is what makes it safe
   to put between a person and the only copy of a task's description, and it
   does not soften for a *recognised* construct's own content — only for the
   marker characters whose entire job is to say what the construct is, and
   which carry no information once it is known: a heading's closing `#`, a
   quote's leading `>`, a table's `|`. A table row with more cells than its
   header is not a marker going missing, it is a person's words the parser
   read and must not drop — `takeTable` widens the whole table to its widest
   row rather than clipping to the header, so an extra cell still lands
   somewhere rather than nowhere.

   `docs/design_handoff_conversation_panel/markup-contract.md` (sections 2, 3
   and 7) is the closed list of what the conversation panel is allowed to
   render; this module only has to be able to *reach* each element in it from
   markdown, not to render it. Two elements on that list, `kbd` and `small`,
   have no markdown syntax anywhere in this file's vocabulary — no library, no
   convention in a `bd` description, nothing this codebase already leans on —
   and the honest answer is to leave them unreached rather than invent a
   syntax nobody typed. A stray `<kbd>` or `<small>` in a source string is an
   HTML tag, already out of scope, and stays literal text like any other one.
   `del` has a real, common markdown spelling (`~~text~~`, GFM's own) and gets
   a node; `kbd` and `small` do not, and no branch below produces either. */

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

/* A line that is nothing but an image is a block of its own — the illustration
   this document is attaching — while the same syntax beside other words on a
   line is part of a sentence and stays inline (see `INLINE` below). Anchored
   at both ends so `![a](b) and more text` does not match. */
const IMAGE_LINE = /^ {0,3}!\[([^\]]*)\]\(\s*(\S+?)\s*\)\s*$/

/* A definition list's `dd` line. Never matched on its own — only a term line
   immediately above one turns a run of these into a `dl`; met without a term
   above it, `: like this` is nothing this file knows and stays a paragraph,
   which is the point of checking the pair rather than the line alone. */
const DEFINITION = /^: (.*)$/

/* The GFM alignment row's own cell shape: a run of dashes with an optional
   colon on either side. Checked against every cell of the row directly under
   a candidate header before either is trusted, so a row that merely looks
   like one — three words separated by hyphens, say — leaves both lines to
   fall through to ordinary paragraphs untouched. */
const ALIGN_CELL = /^:?-+:?$/

/* How deep a quote may nest inside a quote, and a list inside a list. Both are
   recursive block constructs, and both take their depth from the input rather
   than from anything this app decides: one line of ten thousand `>` is ten
   thousand levels, and it overflowed the stack — which in a computed on the UI
   thread is a blank panel where an issue used to be. Sixteen is far past any
   prose a person writes and far short of the stack; past it the markers are
   drawn as the characters they are, so nothing is lost, only unnested.

   Tables and definition lists join headings and rules rather than quotes and
   lists here: neither recurses into `parseBlocks` — a table cell and a `dd`
   are read as inline text, never as nested blocks — so neither this counter
   nor `nested` below has anything to say about them. Putting one inside a
   quote nested near the clamp is still safe, but for the ordinary reason that
   the quote's own recursion is bounded, not because the table does anything
   to keep itself so. */
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

    /* Quote and list before the image, the table and the definition list
       below: both of those strip their own marker and recurse into this same
       function on what is left, which is what lets a table live inside a
       quote (and a quote's `>` live inside a table cell as the plain text it
       is, never as a nested block — cells are read as inline, never as
       blocks). Checking image/table/dl first here would instead try to read
       `> | a | b |` as a table row with the quote marker baked into its first
       cell. */
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

    const imageLine = IMAGE_LINE.exec(line)
    if (imageLine) {
      blocks.push({ type: 'image', src: imageLine[2], alt: imageLine[1] })
      i++
      continue
    }

    /* A table is only ever recognised here, at the start of a fresh block —
       never partway through an already-running paragraph, since a table row
       is not in `startsBlock` below. In practice that means a table right
       after a heading, a rule or another table's last row is read as one
       immediately, but a table directly under a line of prose — with no blank
       line between them — is swallowed by that paragraph's own continuation
       rule and stays prose, exactly as it did before this file could read
       tables at all. That case is not in the acceptance criteria, and GFM
       tables are conventionally written with a blank line ahead of them
       anyway. */
    if (line.includes('|') && i + 1 < lines.length) {
      const headerCells = splitTableRow(line)
      const delimiterCells = splitTableRow(lines[i + 1])
      if (
        headerCells.length > 0 &&
        delimiterCells.length === headerCells.length &&
        delimiterCells.every((cell) => ALIGN_CELL.test(cell))
      ) {
        const [table, next] = takeTable(lines, i, headerCells, delimiterCells)
        blocks.push(table)
        i = next
        continue
      }
    }

    if (DEFINITION.test(lines[i + 1] || '') && !DEFINITION.test(line)) {
      const [dl, next] = takeDefinitionList(lines, i)
      blocks.push(dl)
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
   change exists to remove. An image line joins it for the same reason as the
   block dispatch above: a picture on its own line is a block wherever it
   turns up, not only when a blank line happens to precede it. Tables and
   definition lists are deliberately left out — see the table branch above for
   why a table needs a blank line ahead of it to be read as one. */
function startsBlock(line) {
  return (
    FENCE.test(line) ||
    RULE.test(line) ||
    HEADING.test(line) ||
    IMAGE_LINE.test(line) ||
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
  /* Every item's raw body is gathered first, unparsed, so the marker decision
     below can look at the whole list before committing to it — see the
     comment on `isTask`. */
  const bodies = []
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
    bodies.push(body)
  }

  /* `sm-prose.css` draws the task box on `ul[data-task] > li` unconditionally
     — every direct child, not just the ones that opened with a marker — so a
     list may only claim to be one when every one of its items genuinely is:
     a bullet list (an `ol` has no task box in the contract, and its numbers
     are worth more than a checkbox markdown never gave them a place to keep)
     where **every** item opens with `[x]`/`[ ]`. Anything short of that —
     one plain bullet among marked ones, or a marker on a numbered item —
     leaves every marker in the list as the literal text it is, rather than
     consuming it and then having nowhere to draw what it meant: the box is
     drawn on the container, so a partial claim would either invent a box
     under a plain bullet or eat a person's `[x]` and show nothing for it. */
  const isTask = !ordered && bodies.length > 0 && bodies.every((body) => TASK.test(body[0]))

  for (const body of bodies) {
    const task = isTask ? TASK.exec(body[0]) : null
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

/* A GFM row, split on its cell boundary. Outer pipes are optional and
   stripped when present, so `| a | b |` and `a | b` split the same way — the
   acceptance criteria's own case for a table missing its edge pipes. A
   backtick run inside a cell is tracked the same way the inline `code` opener
   is (a run, not a single character), so a cdhash or a path typed as `` `a|b`
   `` keeps its pipe: without that guard the cell would split in the middle of
   a code span instead of on the column boundary. */
function splitTableRow(line) {
  let body = line.trim()
  if (body.startsWith('|')) body = body.slice(1)
  if (body.endsWith('|')) body = body.slice(0, -1)
  const cells = []
  let cell = ''
  let fence = ''
  let i = 0
  while (i < body.length) {
    if (body[i] === '`') {
      let j = i
      while (body[j] === '`') j++
      const run = body.slice(i, j)
      if (!fence) fence = run
      else if (run === fence) fence = ''
      cell += run
      i = j
      continue
    }
    if (body[i] === '|' && !fence) {
      cells.push(cell.trim())
      cell = ''
      i++
      continue
    }
    cell += body[i]
    i++
  }
  cells.push(cell.trim())
  return cells
}

/* `left`/`center`/`right` from the marker on each side of the alignment row's
   cell, `null` when neither colon is there — the column carries no opinion,
   and the render step is expected to skip `data-align` rather than write it
   with an empty or a made-up value. */
function columnAlign(cell) {
  const left = cell.startsWith(':')
  const right = cell.endsWith(':')
  if (left && right) return 'center'
  if (left) return 'left'
  if (right) return 'right'
  return null
}

/* The header and the alignment row are already read by the time this runs —
   `parseBlocks` only calls it once both have been checked against
   `ALIGN_CELL` — so this just gathers the body first, one raw cell array per
   row, before deciding the table's width. The width is the *widest* row of
   the three (header, alignment, body), not the header's own count: GFM says
   to drop a body row's extra cells and this file does not, on the same
   ground the heading branch above already stands on — a cell past the
   header is somebody's words, not a marker, and the invariant does not let
   content go quiet just because the construct around it is a recognised
   one. A short row still pads with empty cells, which loses nothing because
   there is nothing there to lose; a column with nothing in the alignment
   row gets `null`, which is the same "no opinion" `columnAlign` already
   returns for a plain `---`. */
function takeTable(lines, start, headerCells, delimiterCells) {
  const rows = []
  let i = start + 2
  while (i < lines.length && lines[i].trim() && !startsBlock(lines[i])) {
    rows.push(splitTableRow(lines[i]))
    i++
  }
  const width = Math.max(headerCells.length, ...rows.map((cells) => cells.length))
  const columns = Array.from({ length: width }, (_, column) => column)
  const table = {
    type: 'table',
    align: columns.map((column) => columnAlign(delimiterCells[column] ?? '')),
    head: columns.map((column) => parseInline(headerCells[column] ?? '')),
    rows: rows.map((cells) => columns.map((column) => parseInline(cells[column] ?? '')))
  }
  return [table, i]
}

/* A `dl` is a run of one or more term-then-definitions pairs, each found the
   same way: a line that is not already some other block, immediately above a
   line starting `: `. The pairing is what keeps a bare `: stray` from turning
   into a definition list of its own — checked here, and not in `startsBlock`,
   because unlike a table or a list a lone `: ` line carries nothing that
   marks it as the *start* of anything; only the term above it does. */
function takeDefinitionList(lines, start) {
  const dl = { type: 'dl', items: [] }
  let i = start
  while (
    i + 1 < lines.length &&
    lines[i].trim() &&
    !startsBlock(lines[i]) &&
    !DEFINITION.test(lines[i]) &&
    DEFINITION.test(lines[i + 1])
  ) {
    const term = parseInline(lines[i])
    i++
    const definitions = []
    while (i < lines.length && DEFINITION.test(lines[i])) {
      definitions.push(parseInline(DEFINITION.exec(lines[i])[1]))
      i++
    }
    dl.items.push({ term, definitions })
  }
  return [dl, i]
}

/* Inline markers, tried in this order at every position. Code first, so a
   backtick span wins over anything inside it; the image before the link, so its
   `!` is not left behind; `**` before `*` for the obvious reason.

   A link node is produced only for http and https. Every other scheme — file,
   mailto, and the ones that would be a security question elsewhere — stays
   literal text, which is both the honest thing to draw (this app cannot open
   it) and what keeps the URL itself on screen. An image node carries no such
   restriction on its `src`: it is not opened through `opener:allow-open-url`
   or any other opener, only handed to whatever the illustrations task uses to
   load it, and the acceptance criteria's own example (`./a.png`) is a relative
   path with no scheme at all — the http/https gate belongs to `link` alone.

   Two guards that are not decoration. The closers of `**` and `__` refuse a
   third marker, so `**a *b***` closes on the outer pair and the emphasis inside
   it survives; without that the non-greedy match closes early and the last
   asterisks are left as litter. And `_` emphasis is refused when a letter or a
   digit sits against it on either side, because a tracker description is full
   of `close_reason` and `settings_load`: two such names in one sentence would
   otherwise open an emphasis at the first underscore and close it at the last,
   swallowing both markers and italicising the words between them. `*` keeps no
   such guard — nothing in this vocabulary is spelled with one. `~~` keeps none
   either, for the same reason: nothing here is spelled with a single `~`. */
const INLINE = [
  /* The opener is capped rather than open-ended. A backreferenced greedy run
     around a lazy body is cubic in the length of a run of backticks — 3200 of
     them took 3.6 seconds, on the UI thread, inside a computed — and no code
     span is fenced by more than a few. Past ten the run is not an opener at
     all and falls through to plain text, markers included. */
  [/^(`{1,10})([\s\S]*?[^`])\1(?!`)/, (m) => ({ type: 'code', value: m[2] })],
  [/^!\[([^\]]*)\]\(\s*(\S+?)\s*\)/, (m) => ({ type: 'image', src: m[2], alt: m[1] })],
  [/^\[([^\]]*)\]\(\s*(\S+?)\s*\)/, (m) => link(m[2], parseInline(m[1]))],
  [/^<(https?:\/\/[^>\s]+)>/i, (m) => link(m[1], [{ type: 'text', value: m[1] }])],
  [/^\*\*([\s\S]+?)\*\*(?!\*)/, (m) => ({ type: 'strong', children: parseInline(m[1]) })],
  [
    /^__([\s\S]+?)__(?![\p{L}\p{N}_])/u,
    (m, before) => (WORD.test(before) ? null : { type: 'strong', children: parseInline(m[1]) })
  ],
  [/^~~([\s\S]+?)~~(?!~)/, (m) => ({ type: 'del', children: parseInline(m[1]) })],
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
