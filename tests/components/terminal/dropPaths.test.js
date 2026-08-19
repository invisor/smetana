import { describe, expect, it } from 'vitest'
import { dropText, quotePath } from '../../../src/components/terminal/dropPaths.js'

describe('quotePath', () => {
  /* The ordinary case, and the reason quoting is conditional: a bare path reads
     better than a quoted one in the middle of a half-typed sentence. */
  it('leaves an ordinary path alone', () => {
    expect(quotePath('/Users/ada/notes/plan.md')).toBe('/Users/ada/notes/plan.md')
    expect(quotePath('/tmp/a-b_c.2026+final@v1,2%3=4:5.png')).toBe(
      '/tmp/a-b_c.2026+final@v1,2%3=4:5.png'
    )
  })

  it('quotes a path with a space in it', () => {
    expect(quotePath('/Users/ada/Desktop/Screenshot 2026-08-19 at 10.11.12.png')).toBe(
      "'/Users/ada/Desktop/Screenshot 2026-08-19 at 10.11.12.png'"
    )
  })

  it('quotes every character a shell would read as something else', () => {
    expect(quotePath('/tmp/a$b')).toBe("'/tmp/a$b'")
    expect(quotePath('/tmp/a`b`')).toBe("'/tmp/a`b`'")
    expect(quotePath('/tmp/a"b"')).toBe('\'/tmp/a"b"\'')
    expect(quotePath('/tmp/a\\b')).toBe("'/tmp/a\\b'")
    expect(quotePath('/tmp/a*b')).toBe("'/tmp/a*b'")
    expect(quotePath('/tmp/a?b')).toBe("'/tmp/a?b'")
    expect(quotePath('/tmp/[draft]')).toBe("'/tmp/[draft]'")
    expect(quotePath('/tmp/a;rm -rf b')).toBe("'/tmp/a;rm -rf b'")
    expect(quotePath('/tmp/~ada')).toBe("'/tmp/~ada'")
  })

  /* A single quote cannot appear inside single quotes, so the run is closed, an
     escaped quote written, and a new run opened. */
  it('writes a single quote as its own escape', () => {
    expect(quotePath("/tmp/ada's file.png")).toBe("'/tmp/ada'\\''s file.png'")
  })
})

describe('dropText', () => {
  it('ends in one space, so a person keeps typing around what landed', () => {
    expect(dropText(['/tmp/a.png'])).toBe('/tmp/a.png ')
  })

  it('joins several paths with one space, in the order the event gave them', () => {
    expect(dropText(['/tmp/b.png', '/tmp/a.png'])).toBe('/tmp/b.png /tmp/a.png ')
  })

  it('quotes only the paths that need it', () => {
    expect(dropText(['/tmp/one two.png', '/tmp/three.png'])).toBe(
      "'/tmp/one two.png' /tmp/three.png "
    )
  })

  /* Quoting answers a shell parse, and this text is typed into a PTY instead:
     the line discipline and the TUI read a control character before any shell
     would, and single quotes do not reach them. A line feed or a carriage
     return in a filename is a Return — the one keystroke this gesture exists
     not to press — so such a path is refused rather than repaired. */
  it('refuses a path carrying a control character rather than typing it', () => {
    expect(dropText(['/tmp/a\nb.png'])).toBe('')
    expect(dropText(['/tmp/a\rb.png'])).toBe('')
    expect(dropText(['/tmp/a\tb.png'])).toBe('')
    expect(dropText(['/tmp/a\u001b[2Jb.png'])).toBe('')
    expect(dropText(['/tmp/a\u0000b.png'])).toBe('')
    expect(dropText(['/tmp/a\u007fb.png'])).toBe('')
  })

  /* Per path and not per drop: one pathological name in a selection of three
     must not cost the person the other two. */
  it('lets the good paths of a drop through beside a refused one', () => {
    expect(dropText(['/tmp/a.png', '/tmp/b\nc.png', '/tmp/d.png'])).toBe('/tmp/a.png /tmp/d.png ')
  })

  /* Nothing to insert must not move the cursor of a session somebody is typing
     in, so it is the empty string rather than a lone space. */
  it('says nothing at all about a drop that carried no path', () => {
    expect(dropText([])).toBe('')
    expect(dropText(null)).toBe('')
    expect(dropText(undefined)).toBe('')
    expect(dropText([''])).toBe('')
  })
})
