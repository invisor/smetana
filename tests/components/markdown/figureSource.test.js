import { describe, expect, it } from 'vitest'
import {
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
})
