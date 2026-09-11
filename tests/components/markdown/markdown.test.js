import { describe, it, expect } from 'vitest'
import { parseInline, parseMarkdown } from '../../../src/components/markdown/markdown.js'

describe('parseMarkdown blocks', () => {
  it('reads a heading and its level', () => {
    expect(parseMarkdown('## Acceptance Criteria')).toEqual([
      { type: 'heading', level: 2, children: [{ type: 'text', value: 'Acceptance Criteria' }] }
    ])
  })

  /* A closing run of hashes needs whitespace in front of it. Without that rule
     `C#` and `F#` lose the character that names them, which is a word going
     missing from a description with nothing on screen to say so — and the word
     check in `the invariant` below cannot see it, because it strips `#` from
     the source before comparing. */
  it('keeps a hash that belongs to the words, and drops one that closes the heading', () => {
    expect(parseMarkdown('## Migrate to C#')[0].children).toEqual([
      { type: 'text', value: 'Migrate to C#' }
    ])
    expect(parseMarkdown('# F#')[0].children).toEqual([{ type: 'text', value: 'F#' }])
    expect(parseMarkdown('### a #')[0].children).toEqual([{ type: 'text', value: 'a' }])
    expect(parseMarkdown('## Title ##')[0].children).toEqual([{ type: 'text', value: 'Title' }])
  })

  it('joins the lines of one paragraph and separates two', () => {
    const blocks = parseMarkdown('one\ntwo\n\nthree')
    expect(blocks.map((b) => b.type)).toEqual(['paragraph', 'paragraph'])
    expect(blocks[0].children[0].value).toBe('one\ntwo')
    expect(blocks[1].children[0].value).toBe('three')
  })

  it('reads a fenced block with its language, keeping the lines as typed', () => {
    const blocks = parseMarkdown('```sh\nnpm test\n  npm run dev\n```')
    expect(blocks).toEqual([{ type: 'code', lang: 'sh', text: 'npm test\n  npm run dev' }])
  })

  it('runs an unclosed fence to the end rather than dropping it', () => {
    expect(parseMarkdown('```\nstill here')).toEqual([
      { type: 'code', lang: null, text: 'still here' }
    ])
  })

  it('reads a thematic break, and does not mistake it for a bullet', () => {
    expect(parseMarkdown('---')).toEqual([{ type: 'rule' }])
  })

  it('reads a bulleted list', () => {
    const [list] = parseMarkdown('- one\n- two')
    expect(list.type).toBe('list')
    expect(list.ordered).toBe(false)
    expect(list.items.map((i) => i.blocks[0].children[0].value)).toEqual(['one', 'two'])
  })

  it('reads a numbered list and where it starts', () => {
    const [list] = parseMarkdown('3. three\n4. four')
    expect(list.ordered).toBe(true)
    expect(list.start).toBe(3)
    expect(list.items).toHaveLength(2)
  })

  /* `sm-prose.css` draws the task box on every direct child of `ul[data-task]`
     unconditionally, so `takeList` only claims a list is one — and only then
     consumes the marker — when every item of it opens with `[x]`/`[ ]`. This
     is the case where that is true of the whole list. */
  it('reads the checkbox of every item in a bulleted list where all of them are marked', () => {
    const [list] = parseMarkdown('- [ ] open\n- [x] done')
    expect(list.items.map((i) => i.checked)).toEqual([false, true])
    expect(list.items[0].blocks[0].children[0].value).toBe('open')
    expect(list.items[1].blocks[0].children[0].value).toBe('done')
  })

  /* An ordered list has no task box in the contract — `ul[data-task]` only —
     so a `[x]` typed on a numbered item is not a marker this parser owns: it
     is left as the words it is, and the item's `checked` stays `null`. */
  it('leaves a checkbox marker on a numbered item as text, since an ol has no task box', () => {
    const [list] = parseMarkdown('1. [x] done')
    expect(list.ordered).toBe(true)
    expect(list.items[0].checked).toBeNull()
    expect(list.items[0].blocks[0].children[0].value).toBe('[x] done')
  })

  /* One marked item beside one plain bullet is not a list of tasks — the box
     is drawn on the whole `ul`, so claiming it here would put an empty,
     unchecked box in front of a bullet the person never marked. Every marker
     in a list like this is left as text instead, `checked` staying `null`
     throughout, so nothing is invented and nothing typed is lost either. */
  it('leaves every marker in a mixed bulleted list as text, since the box is drawn on the whole list', () => {
    const [list] = parseMarkdown('- [x] done\n- plain')
    expect(list.items.map((i) => i.checked)).toEqual([null, null])
    expect(list.items[0].blocks[0].children[0].value).toBe('[x] done')
    expect(list.items[1].blocks[0].children[0].value).toBe('plain')
  })

  it('nests a list inside its item', () => {
    const [list] = parseMarkdown('- outer\n  - inner')
    expect(list.items[0].blocks[1].type).toBe('list')
    expect(list.items[0].blocks[1].items[0].blocks[0].children[0].value).toBe('inner')
  })

  it('reads a quote as blocks of its own', () => {
    const [quote] = parseMarkdown('> quoted\n> ## inside')
    expect(quote.type).toBe('quote')
    expect(quote.blocks.map((b) => b.type)).toEqual(['paragraph', 'heading'])
  })

  describe('tables', () => {
    it('reads a GFM table and each column’s alignment', () => {
      const source = ['| Step | Local | ms |', '| :--- | :---: | ---: |', '| identity | ok | 12 |'].join(
        '\n'
      )
      const [table] = parseMarkdown(source)
      expect(table.type).toBe('table')
      expect(table.align).toEqual(['left', 'center', 'right'])
      expect(table.head.map((cell) => cell[0].value)).toEqual(['Step', 'Local', 'ms'])
      expect(table.rows).toEqual([
        [
          [{ type: 'text', value: 'identity' }],
          [{ type: 'text', value: 'ok' }],
          [{ type: 'text', value: '12' }]
        ]
      ])
    })

    it('parses a column with no colon as unaligned rather than guessing', () => {
      const [table] = parseMarkdown(['| a | b |', '| --- | ---: |', '| 1 | 2 |'].join('\n'))
      expect(table.align).toEqual([null, 'right'])
    })

    it('reads the same table with no leading or trailing pipes on any row', () => {
      const [table] = parseMarkdown(['a | b | c', ':--- | :---: | ---:', '1 | 2 | 3'].join('\n'))
      expect(table.align).toEqual(['left', 'center', 'right'])
      expect(table.rows[0].map((cell) => cell[0].value)).toEqual(['1', '2', '3'])
    })

    it('parses a table cell as inline, keeping a marker inside a cell', () => {
      const [table] = parseMarkdown(['| a |', '| --- |', '| **bold** |'].join('\n'))
      expect(table.rows[0][0]).toEqual([{ type: 'strong', children: [{ type: 'text', value: 'bold' }] }])
    })

    /* The invariant, for a construct that is entirely new in this file: a
       row whose second line only looks like an alignment row — three words
       joined by hyphens, not `-`/`:-`/`-:` cells — leaves the whole thing
       exactly where it would have landed before tables existed. */
    it('leaves the whole chunk as paragraphs when the second row is not a GFM alignment row', () => {
      const source = ['| a | b |', '| not | aligned |', '| 1 | 2 |'].join('\n')
      const blocks = parseMarkdown(source)
      expect(blocks.every((b) => b.type === 'paragraph')).toBe(true)
      const shown = blocks.map((b) => b.children.map((c) => c.value).join('')).join('\n')
      for (const word of source.replace(/[|:-]/g, ' ').split(/\s+/).filter(Boolean)) {
        expect(shown).toContain(word)
      }
    })

    /* A row wider than the header is content, not a marker — unlike a
       heading's closing `#` or a table's own `|`, a cell past the header is
       somebody's words, and the parser already read them (`splitTableRow`
       returns them) before this row was built. Clipping to the header count
       would drop them with nothing on screen to say so, which is exactly
       the failure the module's invariant exists to rule out. */
    it('keeps every cell of a row wider than its header, rather than clipping to it', () => {
      const [table] = parseMarkdown(['| a | b |', '| --- | --- |', '| 1 | 2 | SECRET |'].join('\n'))
      expect(table.align).toEqual([null, null, null])
      expect(table.head.map((cell) => cell[0]?.value)).toEqual(['a', 'b', undefined])
      expect(table.rows[0].map((cell) => cell[0]?.value)).toEqual(['1', '2', 'SECRET'])
    })

    it('pads a row shorter than the header with empty cells', () => {
      const [table] = parseMarkdown(['| a | b | c |', '| --- | --- | --- |', '| 1 |'].join('\n'))
      expect(table.rows[0].map((cell) => cell.length)).toEqual([1, 0, 0])
    })

    it('nests a table inside a quote without losing the quote', () => {
      const source = ['> | a | b |', '> | --- | --- |', '> | 1 | 2 |'].join('\n')
      const [quote] = parseMarkdown(source)
      expect(quote.type).toBe('quote')
      expect(quote.blocks[0].type).toBe('table')
      expect(quote.blocks[0].rows[0].map((cell) => cell[0].value)).toEqual(['1', '2'])
    })

    it('keeps a quote marker typed inside a table cell as the plain text it is', () => {
      const [table] = parseMarkdown(['| note |', '| --- |', '| > not a nested quote |'].join('\n'))
      expect(table.rows[0][0]).toEqual([{ type: 'text', value: '> not a nested quote' }])
    })

    /* Deep quote nesting already clamps at `MAX_BLOCK_DEPTH`; this pins that a
       table sitting at the bottom of one does not change that number, since
       neither a table nor its cells add a level of their own. */
    it('keeps the same block-depth clamp when a table sits inside deep quote nesting', () => {
      const quoteDepth = (blocks) =>
        blocks.reduce((d, b) => (b.type === 'quote' ? Math.max(d, 1 + quoteDepth(b.blocks)) : d), 0)
      const quotes = '>'.repeat(20)
      const source = [`${quotes} | a |`, `${quotes} | --- |`, `${quotes} | x |`].join('\n')
      let tree
      expect(() => {
        tree = parseMarkdown(source)
      }).not.toThrow()
      expect(quoteDepth(tree)).toBeLessThanOrEqual(16)
    })
  })

  describe('definition lists', () => {
    it('reads a term and its definition as a dl', () => {
      const [dl] = parseMarkdown('bd\n: the tracker CLI')
      expect(dl.type).toBe('dl')
      expect(dl.items).toEqual([
        {
          term: [{ type: 'text', value: 'bd' }],
          definitions: [[{ type: 'text', value: 'the tracker CLI' }]]
        }
      ])
    })

    it('reads more than one definition under one term', () => {
      const [dl] = parseMarkdown('bd\n: the tracker CLI\n: also a sidecar binary')
      expect(dl.items[0].definitions).toHaveLength(2)
    })

    it('leaves a definition line with no term above it as an ordinary paragraph', () => {
      const blocks = parseMarkdown(': stray definition')
      expect(blocks).toEqual([
        { type: 'paragraph', children: [{ type: 'text', value: ': stray definition' }] }
      ])
    })
  })

  describe('block images', () => {
    it('reads an image alone on its own line as a block, not a link', () => {
      expect(parseMarkdown('![a caption](./a.png)')).toEqual([
        { type: 'image', src: './a.png', alt: 'a caption' }
      ])
    })

    it('reads a block image between two paragraphs', () => {
      const blocks = parseMarkdown('before\n\n![a caption](./a.png)\n\nafter')
      expect(blocks.map((b) => b.type)).toEqual(['paragraph', 'image', 'paragraph'])
    })

    it('keeps an image beside other text on its line as inline, inside the paragraph', () => {
      const [paragraph] = parseMarkdown('see ![a caption](./a.png) here')
      expect(paragraph.type).toBe('paragraph')
      expect(paragraph.children.some((c) => c.type === 'image')).toBe(true)
    })
  })

  it('is empty for empty input, and for whitespace', () => {
    expect(parseMarkdown('')).toEqual([])
    expect(parseMarkdown('   \n\n')).toEqual([])
    expect(parseMarkdown(null)).toEqual([])
  })
})

describe('parseInline', () => {
  it('reads strong, emphasis and code, and the text around them', () => {
    expect(parseInline('a **b** c *d* e `f`')).toEqual([
      { type: 'text', value: 'a ' },
      { type: 'strong', children: [{ type: 'text', value: 'b' }] },
      { type: 'text', value: ' c ' },
      { type: 'em', children: [{ type: 'text', value: 'd' }] },
      { type: 'text', value: ' e ' },
      { type: 'code', value: 'f' }
    ])
  })

  it('keeps a path in code exactly as typed', () => {
    expect(parseInline('see `src/stores/app.js`')[1]).toEqual({
      type: 'code',
      value: 'src/stores/app.js'
    })
  })

  it('nests emphasis inside strong', () => {
    const [strong] = parseInline('**a *b***')
    expect(strong.type).toBe('strong')
    expect(strong.children.map((n) => n.type)).toEqual(['text', 'em'])
  })

  it('reads both link forms', () => {
    expect(parseInline('[docs](https://example.com/x)')).toEqual([
      { type: 'link', href: 'https://example.com/x', children: [{ type: 'text', value: 'docs' }] }
    ])
    expect(parseInline('<http://localhost:5173>')).toEqual([
      {
        type: 'link',
        href: 'http://localhost:5173',
        children: [{ type: 'text', value: 'http://localhost:5173' }]
      }
    ])
  })

  /* A scheme is case-insensitive by RFC 3986. The href keeps only its scheme
     lowercased — that is the half the opener's `https://*` scope has to match —
     and the rest of the URL, and the label, stay exactly as they were typed. */
  it('opens a link whose scheme is in capitals, and stores that scheme lowercased', () => {
    expect(parseInline('[docs](HTTPS://Example.com/X)')).toEqual([
      { type: 'link', href: 'https://Example.com/X', children: [{ type: 'text', value: 'docs' }] }
    ])
    expect(parseInline('<HtTp://LocalHost:5173/x>')).toEqual([
      {
        type: 'link',
        href: 'http://LocalHost:5173/x',
        children: [{ type: 'text', value: 'HtTp://LocalHost:5173/x' }]
      }
    ])
  })

  it('still refuses a scheme it cannot open, however it is spelled', () => {
    expect(parseInline('[x](JAVASCRIPT:alert(1))')).toEqual([
      { type: 'text', value: '[x](JAVASCRIPT:alert(1))' }
    ])
  })

  it('leaves a link this app cannot open as text, brackets included', () => {
    expect(parseInline('[x](file:///Users/x)')).toEqual([
      { type: 'text', value: '[x](file:///Users/x)' }
    ])
  })

  /* The widened half: a bare relative path with no scheme in front of it is a
     local link now, not literal text — `./links.js` decides this, and the
     rule itself is pinned by its own tests (`links.test.js`). What is pinned
     here is that `markdown.js` reaches that module at all and builds the
     node shape `MarkdownInline.vue` draws: `local: true`, the clean path,
     which breed it is, and the head/tail split — of the target's own text
     when the label carries nothing of its own, and of the label otherwise.
     Nothing here is discarded either way; see this file's own header. */
  it('splits the target into head and tail when the label is the same path written twice', () => {
    expect(parseInline('[src-tauri/tauri.conf.json:41](src-tauri/tauri.conf.json:41)')).toEqual([
      {
        type: 'link',
        local: true,
        path: 'src-tauri/tauri.conf.json',
        targetKind: 'file',
        head: 'src-tauri/',
        tail: 'tauri.conf.json:41'
      }
    ])
  })

  it('splits the target when the label is empty, the same as a matching one', () => {
    expect(parseInline('[](src-tauri/tauri.conf.json:41)')).toEqual([
      {
        type: 'link',
        local: true,
        path: 'src-tauri/tauri.conf.json',
        targetKind: 'file',
        head: 'src-tauri/',
        tail: 'tauri.conf.json:41'
      }
    ])
  })

  /* The invariant this whole family rests on, stated as a test: a label that
     says something the target does not is not thrown away in favour of the
     path. `data-path`, `data-kind` and the click still carry the real
     target — see `MarkdownInline.vue` — but what a person reads is what they
     wrote. */
  it('keeps a distinct label verbatim, as an unsplit tail, rather than showing the target', () => {
    expect(parseInline('[the manifest](src-tauri/tauri.conf.json)')).toEqual([
      {
        type: 'link',
        local: true,
        path: 'src-tauri/tauri.conf.json',
        targetKind: 'file',
        head: '',
        tail: 'the manifest'
      }
    ])
  })

  it('reads a trailing slash as a directory target, splitting a matching label the same way', () => {
    expect(parseInline('[docs/design/](docs/design/)')).toEqual([
      {
        type: 'link',
        local: true,
        path: 'docs/design',
        targetKind: 'dir',
        head: 'docs/',
        tail: 'design'
      }
    ])
  })

  it('reads a bare file name with no folder above it as an all-tail local link', () => {
    expect(parseInline('[README.md](README.md)')).toEqual([
      { type: 'link', local: true, path: 'README.md', targetKind: 'file', head: '', tail: 'README.md' }
    ])
  })

  /* A scheme-less target with no slash and no extension is ordinary prose,
     not a path — the positive shape `links.js`'s own tests pin in full; this
     one line is what proves `markdown.js` actually reaches that rule rather
     than reading "no scheme" as license enough on its own. */
  it('leaves a score-shaped target as text — no slash, no extension, no path', () => {
    expect(parseInline('a score of [two nil](2:1) settled it')).toEqual([
      { type: 'text', value: 'a score of [two nil](2:1) settled it' }
    ])
  })

  it('reads an inline image beside other text as its own node, not a link', () => {
    expect(parseInline('a shot: ![a shot](https://example.com/s.png) above')).toEqual([
      { type: 'text', value: 'a shot: ' },
      { type: 'image', src: 'https://example.com/s.png', alt: 'a shot' },
      { type: 'text', value: ' above' }
    ])
  })

  it('carries a relative image source unchanged, with no scheme gate', () => {
    expect(parseInline('see ![a](./a.png) there')).toEqual([
      { type: 'text', value: 'see ' },
      { type: 'image', src: './a.png', alt: 'a' },
      { type: 'text', value: ' there' }
    ])
  })

  it('reads strikethrough, and leaves a lone tilde as itself', () => {
    expect(parseInline('~~gone~~ but not ~forgotten~')).toEqual([
      { type: 'del', children: [{ type: 'text', value: 'gone' }] },
      { type: 'text', value: ' but not ~forgotten~' }
    ])
  })

  /* The contract lists `kbd` and `small` among the panel's prose elements
     (`docs/design_handoff_conversation_panel/markup-contract.md`, section 2),
     but nothing in this file's markdown vocabulary spells either of them —
     see the module header. An HTML tag, `<kbd>` included, is out of scope and
     stays literal text like any other one; this pins that down so a future
     change does not quietly invent a syntax for it. */
  it('leaves a `kbd`-shaped and a `small`-shaped HTML tag as plain text', () => {
    expect(parseInline('press <kbd>Enter</kbd> or read the <small>fine print</small>')).toEqual([
      { type: 'text', value: 'press <kbd>Enter</kbd> or read the <small>fine print</small>' }
    ])
  })

  it('leaves an unpaired marker as itself', () => {
    expect(parseInline('2 * 3 and a stray `tick')).toEqual([
      { type: 'text', value: '2 * 3 and a stray `tick' }
    ])
  })
})

describe('names with underscores in them', () => {
  it('leaves an identifier alone, however many of them share a sentence', () => {
    expect(parseInline('bd emits close_reason beside acceptance_criteria')).toEqual([
      { type: 'text', value: 'bd emits close_reason beside acceptance_criteria' }
    ])
  })

  /* The guard this test is for is the one on the character *before* the
     opening underscore. The trailing lookahead alone passes both of these, so
     without this assertion the guard can be deleted with every other test in
     the file staying green. */
  it('refuses an underscore with a word pressed against its left side', () => {
    expect(parseInline('x_y_ z')).toEqual([{ type: 'text', value: 'x_y_ z' }])
    expect(parseInline('see close_reason, then design_ok_ here')).toEqual([
      { type: 'text', value: 'see close_reason, then design_ok_ here' }
    ])
  })

  it('still reads an underscore emphasis standing on its own', () => {
    expect(parseInline('_quietly_')).toEqual([
      { type: 'em', children: [{ type: 'text', value: 'quietly' }] }
    ])
  })
})

/* The one assertion that stands for the whole module: whatever it does not
   understand, it still shows. */
describe('the invariant', () => {
  /* A link's label and its href both count as shown. The URL is not drawn as
     text — it is what the link opens, the one thing the parse moves rather than
     drops, and the reason a scheme this app cannot open is refused by the
     parser instead of being hidden behind a label. */
  const flatten = (nodes) =>
    nodes
      .map((n) => {
        if (n.type === 'text' || n.type === 'code') return n.value
        if (n.type === 'link') return `${flatten(n.children)} ${n.href}`
        if (n.children) return flatten(n.children)
        return ''
      })
      .join('')

  const visible = (blocks) =>
    blocks
      .map((b) => {
        if (b.type === 'code') return b.text
        if (b.type === 'rule') return ''
        if (b.type === 'image') return b.alt
        if (b.type === 'quote') return visible(b.blocks)
        if (b.type === 'list') return b.items.map((i) => visible(i.blocks)).join(' ')
        if (b.type === 'table') return [...b.head, ...b.rows.flat()].map(flatten).join(' ')
        if (b.type === 'dl') {
          return b.items
            .map((item) => `${flatten(item.term)} ${item.definitions.map(flatten).join(' ')}`)
            .join(' ')
        }
        return flatten(b.children)
      })
      .join(' ')

  /* Every word the source puts on the page. A fence line is dropped first: it
     is marker and nothing else, and its info string says what the fence is the
     way `#` says what a heading is — neither is prose, and neither is drawn. */
  const words = (source) =>
    source
      .split('\n')
      .filter((line) => !/^ {0,3}(`{3,}|~{3,})/.test(line))
      .join('\n')
      .replace(/[#*`>|[\]()-]/g, ' ')
      .split(/\s+/)
      .filter(Boolean)

  /* The table's body row is deliberately one cell wider than its header
     (`ragged`, past `a`/`b`) — this is a genuinely recognised table, blank
     line and all, so it is `takeTable` doing the widening rather than a
     paragraph carrying the word along as ordinary text. This is the fixture
     that would have caught a clip-to-the-header bug: `visible()` walks every
     column of every row, and a dropped cell is a missing word this test
     notices without knowing to look for one. */
  const SOURCE = [
    '# Title',
    '',
    'A **bold** claim about `src/paths.js`, see [docs](https://example.com).',
    '',
    '| a | b |',
    '| - | - |',
    '| 1 | 2 | ragged |',
    '',
    '- [ ] one',
    '- [x] two',
    '  - nested',
    '',
    '> quoted',
    '',
    '```js',
    'const x = 1',
    '```',
    '',
    '---',
    '',
    'Trailing <b>tag</b> and [a][ref].'
  ].join('\n')

  /* Five constructs in one string, and every word of all five is still on
     screen. Four are simply outside the supported subset: a lone asterisk, a
     reference link, an HTML tag and — glued straight onto the sentence ahead
     of it, with no blank line to open it as a block — a run of table-shaped
     lines, which stays part of that sentence rather than becoming a `table`
     (see the table branch in `parseBlocks`). The fifth, the unclosed fence,
     goes last because it swallows the rest of the input by design — which is
     itself the invariant working. */
  const UNSUPPORTED = [
    'A ratio of 2 * 3, a <b>tag</b> and a link like [text][ref].',
    '| column | other |',
    '| --- | --- |',
    '| one | two |',
    '```',
    'bd update smetana-9n5 --claim'
  ].join('\n')

  /* `words()` above strips the ten marker characters before comparing, so it is
     blind to the loss of one of those characters *as a character* — which is
     exactly how a heading eating the `#` of `C#` stayed invisible to it. This
     one compares the whole line instead: nothing in it is supported syntax, so
     the parse owes it back character for character, asterisk, pipe, brackets
     and angle brackets included. */
  it('gives an unsupported line back character for character', () => {
    const line = 'A ratio of 2 * 3, a <b>tag</b> and a link like [text][ref].'
    expect(parseMarkdown(line)).toEqual([
      { type: 'paragraph', children: [{ type: 'text', value: line }] }
    ])
    const row = '| column | other |'
    expect(parseMarkdown(row)).toEqual([
      { type: 'paragraph', children: [{ type: 'text', value: row }] }
    ])
  })

  it('shows every word of the source', () => {
    const shown = visible(parseMarkdown(SOURCE))
    for (const word of words(SOURCE)) expect(shown).toContain(word)
  })

  it('shows every word of what it does not understand', () => {
    const shown = visible(parseMarkdown(UNSUPPORTED))
    for (const word of words(UNSUPPORTED)) expect(shown).toContain(word)
  })
})

/* Input nobody types, which is the point: these fields are written by
   autonomous agents and pasted into by people, and the parse runs synchronously
   in a computed on the UI thread. A hang and a stack overflow are both a blank
   panel where an issue used to be. */
describe('input written by a machine', () => {
  const shown = (blocks) =>
    blocks
      .map((b) => {
        if (b.type === 'code') return b.text
        if (b.type === 'quote') return shown(b.blocks)
        if (b.type === 'list') return b.items.map((i) => shown(i.blocks)).join('')
        return (b.children ?? []).map((c) => c.value ?? '').join('')
      })
      .join('')

  const depth = (blocks) =>
    blocks.reduce((d, b) => (b.type === 'quote' ? Math.max(d, 1 + depth(b.blocks)) : d), 0)

  const listDepth = (blocks) =>
    blocks.reduce(
      (d, b) =>
        b.type === 'list'
          ? Math.max(d, 1 + Math.max(0, ...b.items.map((i) => listDepth(i.blocks))))
          : d,
      0
    )

  /* The bound is about the class of failure rather than about this machine. The
     unbounded opener was cubic and the bounded one is quadratic, so over these
     two thousand backticks the same input measures about 20ms with the cap and
     about 1640ms without it — an eighty-fold gap, with `1000` sitting between
     the two and near neither. That is why the number is not arbitrary, and why
     the fixture is two thousand rather than four: at four the cap-present cost
     was 379ms under the full suite's workers, and 2.6× of headroom is how the
     list fixture below came to pass on one machine and fail on another. */
  it('does not take seconds over a run of backticks, and keeps every one', () => {
    const source = '`'.repeat(2000) + ' x'
    const started = Date.now()
    const nodes = parseInline(source)
    expect(Date.now() - started).toBeLessThan(1000)
    expect(nodes.map((n) => n.value).join('')).toBe(source)
  })

  it('clamps how deep a quote nests, and draws the markers past the clamp', () => {
    const source = '>'.repeat(10000) + ' x'
    const tree = parseMarkdown(source)
    const drawn = shown(tree)
    /* The invariant, stated over this input: every one of the ten thousand
       markers is either a level of nesting — drawn as a quote's own rule — or a
       character on screen. None of them is neither. The clamp is not named
       here, so moving it does not move this test. */
    expect(drawn.split('>').length - 1 + depth(tree)).toBe(10000)
    expect(drawn.endsWith(' x')).toBe(true)
  })

  /* The crash this was written for needed four thousand levels — a 16 MB
     fixture that parsed in about five seconds, which is inside vitest's default
     limit alone and outside it under the full suite's workers. The size is not
     what the test is about: two hundred levels reach the clamp exactly as four
     thousand do, in forty kilobytes and milliseconds, and it is the depth
     assertion rather than the length of the input that says the clamp is
     working — without one, this tree nests once per item. */
  it('clamps how deep a list nests rather than overflowing the stack', () => {
    const items = 200
    const source = Array.from({ length: items }, (_, i) => ' '.repeat(i * 2) + '- x').join('\n')
    let tree
    expect(() => {
      tree = parseMarkdown(source)
    }).not.toThrow()
    expect(listDepth(tree)).toBeLessThan(items)
    /* And the invariant under the clamp: every item is still on screen, the
       ones past it as the characters they are. */
    expect(shown(tree).split('x').length - 1).toBe(items)
  })
})
