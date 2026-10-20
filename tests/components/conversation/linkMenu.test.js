import { describe, expect, it } from 'vitest'
import { LINK_MENU_W, linkAbsolutePath, linkMenuItems } from '../../../src/components/conversation/linkMenu.js'

const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15'
const WIN = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
const LINUX = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15'

describe('linkMenuItems', () => {
  it('offers open, reveal, a separator and copy path, in that order', () => {
    expect(linkMenuItems({ userAgent: MAC }).map((it) => it.kind ?? it.type)).toEqual([
      'open', 'reveal', 'separator', 'copy-path'
    ])
  })
  it('labels the rows in sentence case', () => {
    const items = linkMenuItems({ userAgent: MAC })
    expect(items.find((it) => it.kind === 'open').label).toBe('Open')
    expect(items.find((it) => it.kind === 'copy-path').label).toBe('Copy path')
  })
  it('names the file manager after the platform', () => {
    const label = (userAgent) => linkMenuItems({ userAgent }).find((it) => it.kind === 'reveal').label
    expect(label(MAC)).toBe('Reveal in Finder')
    expect(label(WIN)).toBe('Reveal in Explorer')
    expect(label(LINUX)).toBe('Reveal in file manager')
    expect(label(undefined)).toBe('Reveal in file manager')
  })
  it('gives every row an icon', () => {
    for (const it of linkMenuItems({ userAgent: MAC }).filter((row) => row.kind)) {
      expect(typeof it.icon).toBe('string')
    }
  })
  it('names a width a number', () => {
    expect(typeof LINK_MENU_W).toBe('number')
  })
})

describe('linkAbsolutePath', () => {
  it('joins a relative path onto the project root', () => {
    expect(linkAbsolutePath('/Users/me/proj', 'src/a.js')).toBe('/Users/me/proj/src/a.js')
  })
  it('leaves an absolute path as it is, inside the project or out of it', () => {
    expect(linkAbsolutePath('/Users/me/proj', '/tmp/x.txt')).toBe('/tmp/x.txt')
    expect(linkAbsolutePath('/Users/me/proj', '/Users/me/proj/a.js')).toBe('/Users/me/proj/a.js')
  })
  it('leaves an absolute path alone when there is no root', () => {
    expect(linkAbsolutePath('', '/tmp/x.txt')).toBe('/tmp/x.txt')
    expect(linkAbsolutePath(null, 'src/a.js')).toBe('src/a.js')
  })
})
