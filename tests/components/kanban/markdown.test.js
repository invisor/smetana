import { describe, it, expect } from 'vitest'
import { parseInline, parseMarkdown } from '../../../src/components/kanban/markdown.js'

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

  it('reads the checkbox of a task item, and its absence', () => {
    const [list] = parseMarkdown('- [ ] open\n- [x] done\n- plain')
    expect(list.items.map((i) => i.checked)).toEqual([false, true, null])
    expect(list.items[1].blocks[0].children[0].value).toBe('done')
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

  it('draws an image as its alt text and a link, and never as a picture', () => {
    expect(parseInline('![a shot](https://example.com/s.png)')).toEqual([
      { type: 'text', value: 'a shot ' },
      {
        type: 'link',
        href: 'https://example.com/s.png',
        children: [{ type: 'text', value: 'https://example.com/s.png' }]
      }
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
        if (b.type === 'quote') return visible(b.blocks)
        if (b.type === 'list') return b.items.map((i) => visible(i.blocks)).join(' ')
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

  const SOURCE = [
    '# Title',
    '',
    'A **bold** claim about `src/paths.js`, see [docs](https://example.com).',
    '',
    '| a | b |',
    '| - | - |',
    '| 1 | 2 |',
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

  /* The five constructs the task names, in one string: an unclosed fence, a
     lone asterisk, a GFM table, a reference link and an HTML tag. Not one of
     them is in the supported subset, and every word of all five is still on
     screen. The fence goes last because an unclosed one swallows the rest of
     the input by design — which is itself the invariant working. */
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
