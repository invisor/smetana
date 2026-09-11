import { describe, expect, it } from 'vitest'
import {
  describeFigureSrc,
  isAllowedFigureSrc,
  isInlineSvgSrc,
  readInlineSvg
} from '../../../src/components/markdown/figureSource.js'

describe('isAllowedFigureSrc', () => {
  it('accepts a bare relative path, the parser acceptance criteria own case', () => {
    expect(isAllowedFigureSrc('./a.png')).toBe(true)
    expect(isAllowedFigureSrc('assets/fig-latency.png')).toBe(true)
  })

  it('accepts an absolute filesystem path', () => {
    expect(isAllowedFigureSrc('/Users/ada/fig.png')).toBe(true)
  })

  it('does not mistake a Windows drive letter for a one-letter scheme', () => {
    expect(isAllowedFigureSrc('C:\\Users\\ada\\fig.png')).toBe(true)
  })

  it('accepts a self-contained data URI', () => {
    expect(isAllowedFigureSrc('data:image/png;base64,AAAA')).toBe(true)
    expect(isAllowedFigureSrc('data:image/svg+xml,<svg></svg>')).toBe(true)
  })

  it('refuses a remote address, unlike the parser that produced the node', () => {
    expect(isAllowedFigureSrc('https://example.com/s.png')).toBe(false)
    expect(isAllowedFigureSrc('http://example.com/s.png')).toBe(false)
  })

  it('refuses a scheme this app cannot make sense of as a picture', () => {
    expect(isAllowedFigureSrc('javascript:alert(1)')).toBe(false)
    expect(isAllowedFigureSrc('JAVASCRIPT:alert(1)')).toBe(false)
    expect(isAllowedFigureSrc('vbscript:msgbox(1)')).toBe(false)
    expect(isAllowedFigureSrc('file:///etc/passwd')).toBe(false)
  })

  it('refuses a scheme with leading whitespace ahead of it the same way', () => {
    expect(isAllowedFigureSrc(' javascript:alert(1)')).toBe(false)
    expect(isAllowedFigureSrc('\tjavascript:alert(1)')).toBe(false)
  })

  /* A protocol-relative source has no scheme by RFC 3986's own grammar, but
     every renderer resolves it against the current origin's own scheme —
     `new URL('//evil.example.com/x', location.href)` is `http://…` in the dev
     server and in a Windows release alike — so it is refused ahead of the
     scheme test rather than falling through the "no scheme" branch as though
     it were a filesystem path. */
  it('refuses a protocol-relative source', () => {
    expect(isAllowedFigureSrc('//evil.example.com/x.png')).toBe(false)
    expect(isAllowedFigureSrc('//attacker.tld/p.png?t=abc')).toBe(false)
  })

  it('refuses a data URI whose media type is not a picture at all', () => {
    expect(isAllowedFigureSrc('data:text/html,<script>alert(1)</script>')).toBe(false)
    expect(isAllowedFigureSrc('data:,plain text')).toBe(false)
  })

  it('refuses an empty or non-string source', () => {
    expect(isAllowedFigureSrc('')).toBe(false)
    expect(isAllowedFigureSrc('   ')).toBe(false)
    expect(isAllowedFigureSrc(null)).toBe(false)
    expect(isAllowedFigureSrc(undefined)).toBe(false)
  })
})

describe('isInlineSvgSrc', () => {
  it('recognises a plain data:image/svg+xml URI', () => {
    expect(isInlineSvgSrc('data:image/svg+xml,<svg></svg>')).toBe(true)
  })

  it('recognises a base64 data:image/svg+xml URI', () => {
    expect(isInlineSvgSrc('data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=')).toBe(true)
  })

  it('does not mistake a raster data URI for one', () => {
    expect(isInlineSvgSrc('data:image/png;base64,AAAA')).toBe(false)
  })

  it('does not mistake an ordinary path for one', () => {
    expect(isInlineSvgSrc('./a.png')).toBe(false)
  })

  /* `;utf8` is a common, non-standard shorthand browsers accept in an
     ordinary `<img src>`, so a source spelled that way still has to reach
     `readInlineSvg` rather than the raster branch un-walked — the whole
     point of this function existing beside `isAllowedFigureSrc` at all. */
  it('recognises the common ;utf8 parameter', () => {
    expect(isInlineSvgSrc('data:image/svg+xml;utf8,<svg></svg>')).toBe(true)
  })

  it('recognises charset and base64 together, in either order', () => {
    expect(isInlineSvgSrc('data:image/svg+xml;charset=utf-8;base64,PHN2Zz48L3N2Zz4=')).toBe(true)
    expect(isInlineSvgSrc('data:image/svg+xml;base64;charset=utf-8,PHN2Zz48L3N2Zz4=')).toBe(true)
  })

  /* The property that closes the gate the review found open: every
     `data:image/svg+xml` source `isAllowedFigureSrc` admits must also be one
     this function recognises, or it reaches the raster `<img>` branch
     un-walked — an SVG painting any colour it likes, matted, unchecked. */
  it('agrees with isAllowedFigureSrc on every accepted svg+xml source', () => {
    const sources = [
      'data:image/svg+xml,<svg></svg>',
      'data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=',
      'data:image/svg+xml;utf8,<svg></svg>',
      'data:image/svg+xml;charset=utf-8,<svg></svg>',
      'data:image/svg+xml;charset=utf-8;base64,PHN2Zz48L3N2Zz4=',
      'data:image/svg+xml;base64;charset=utf-8,PHN2Zz48L3N2Zz4='
    ]
    for (const src of sources) {
      expect(isAllowedFigureSrc(src)).toBe(true)
      expect(isInlineSvgSrc(src)).toBe(true)
    }
  })
})

describe('describeFigureSrc', () => {
  it('names a data URI by its media type alone, never its payload', () => {
    expect(describeFigureSrc('data:image/svg+xml,<svg viewBox="0 0 1 1">…</svg>')).toBe(
      'data:image/svg+xml'
    )
    expect(describeFigureSrc('data:image/png;base64,AAAAAAAAAAAAAAAAAAAA')).toBe('data:image/png')
  })

  it('prints an ordinary path whole', () => {
    expect(describeFigureSrc('assets/queue-depth.svg')).toBe('assets/queue-depth.svg')
  })
})

describe('readInlineSvg', () => {
  it('accepts a diagram drawn entirely with currentColor and tokens', () => {
    const src =
      'data:image/svg+xml,' +
      encodeURIComponent(
        '<svg viewBox="0 0 320 60" role="img" aria-label="Pipeline">' +
          '<rect x="4" y="4" width="60" height="24" fill="none" stroke="currentColor"/>' +
          '<text x="8" y="20" fill="var(--text-secondary)">build</text>' +
          '</svg>'
      )
    const result = readInlineSvg(src)
    expect(result.ok).toBe(true)
    expect(result.root.tag).toBe('svg')
    expect(result.root.attrs.viewBox).toBe('0 0 320 60')
    expect(result.root.children.map((child) => child.tag)).toEqual(['rect', 'text'])
  })

  it('accepts a base64-encoded diagram the same way', () => {
    const markup = '<svg viewBox="0 0 10 10"><circle cx="5" cy="5" r="4" fill="currentColor"/></svg>'
    const src = `data:image/svg+xml;base64,${btoa(markup)}`
    const result = readInlineSvg(src)
    expect(result.ok).toBe(true)
    expect(result.root.children[0].tag).toBe('circle')
  })

  it('refuses a hard-coded hex colour rather than drawing it anyway', () => {
    const src =
      'data:image/svg+xml,' +
      encodeURIComponent('<svg viewBox="0 0 10 10"><rect width="4" height="4" fill="#ff0000"/></svg>')
    const result = readInlineSvg(src)
    expect(result.ok).toBe(false)
    expect(result.reason).toMatch(/colour outside the token set/)
  })

  it('refuses a named colour the same way it refuses a hex one', () => {
    const src =
      'data:image/svg+xml,' +
      encodeURIComponent('<svg viewBox="0 0 10 10"><rect width="4" height="4" stroke="red"/></svg>')
    expect(readInlineSvg(src).ok).toBe(false)
  })

  it('refuses an element outside the diagram vocabulary', () => {
    const src =
      'data:image/svg+xml,' +
      encodeURIComponent('<svg viewBox="0 0 10 10"><script>alert(1)</script></svg>')
    const result = readInlineSvg(src)
    expect(result.ok).toBe(false)
    expect(result.reason).toMatch(/not a diagram element/)
  })

  it('refuses an attribute outside the allowed set, an event handler included', () => {
    const src =
      'data:image/svg+xml,' +
      encodeURIComponent('<svg viewBox="0 0 10 10"><rect width="4" height="4" onclick="alert(1)"/></svg>')
    const result = readInlineSvg(src)
    expect(result.ok).toBe(false)
    expect(result.reason).toMatch(/not an attribute/)
  })

  it('refuses a foreign namespace such as xlink:href', () => {
    const src =
      'data:image/svg+xml,' +
      encodeURIComponent(
        '<svg viewBox="0 0 10 10" xmlns:xlink="http://www.w3.org/1999/xlink">' +
          '<use xlink:href="#a"/></svg>'
      )
    expect(readInlineSvg(src).ok).toBe(false)
  })

  it('refuses markup with no svg root at all', () => {
    const src = `data:image/svg+xml,${encodeURIComponent('<rect width="4" height="4"/>')}`
    const result = readInlineSvg(src)
    expect(result.ok).toBe(false)
    expect(result.reason).toMatch(/no <svg> root/)
  })

  it('refuses a payload that is not valid base64', () => {
    const result = readInlineSvg('data:image/svg+xml;base64,not-base64!!!')
    expect(result.ok).toBe(false)
  })

  it('accepts the ;utf8 parameter the same way as a bare data URI', () => {
    const src =
      'data:image/svg+xml;utf8,' +
      encodeURIComponent('<svg viewBox="0 0 10 10"><rect width="4" height="4" fill="currentColor"/></svg>')
    expect(readInlineSvg(src).ok).toBe(true)
  })

  /* `svg` names only the document's own root — a diagram carries no viewport
     of its own to reason about, and the vocabulary's own comment says so. */
  it('refuses a nested <svg>, not just an unrecognised element', () => {
    const src =
      'data:image/svg+xml,' +
      encodeURIComponent('<svg viewBox="0 0 1 1"><svg><rect fill="currentColor"/></svg></svg>')
    const result = readInlineSvg(src)
    expect(result.ok).toBe(false)
    expect(result.reason).toMatch(/not a diagram element/)
  })

  it('refuses a diagram with more elements than the budget allows', () => {
    const rects = Array.from({ length: 600 }, () => '<rect width="1" height="1" fill="currentColor"/>')
    const src =
      'data:image/svg+xml,' +
      encodeURIComponent(`<svg viewBox="0 0 10 10">${rects.join('')}</svg>`)
    const result = readInlineSvg(src)
    expect(result.ok).toBe(false)
    expect(result.reason).toMatch(/too many elements/)
  })
})
