import { describe, expect, it } from 'vitest'
import { fileTreeVerb } from '../../../src/components/files/fileTreeKeys.js'

const MAC = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15'
const WINDOWS = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36'
const LINUX = 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36'

const press = (fields) => ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...fields })

describe('fileTreeVerb', () => {
  it('reads the four clipboard verbs off the command key', () => {
    expect(fileTreeVerb(press({ code: 'KeyC', metaKey: true }))).toBe('copy')
    expect(fileTreeVerb(press({ code: 'KeyX', metaKey: true }))).toBe('cut')
    expect(fileTreeVerb(press({ code: 'KeyV', metaKey: true }))).toBe('paste')
    expect(fileTreeVerb(press({ code: 'KeyD', metaKey: true }))).toBe('duplicate')
  })

  it('reads the same four off the control key, for a machine without a command key', () => {
    expect(fileTreeVerb(press({ code: 'KeyC', ctrlKey: true }))).toBe('copy')
    expect(fileTreeVerb(press({ code: 'KeyD', ctrlKey: true }))).toBe('duplicate')
  })

  it('answers nothing for a letter pressed on its own', () => {
    expect(fileTreeVerb(press({ code: 'KeyC' }))).toBe(null)
    expect(fileTreeVerb(press({ code: 'KeyV' }))).toBe(null)
  })

  it('leaves the chord alone when Shift or Alt is held as well', () => {
    expect(fileTreeVerb(press({ code: 'KeyC', metaKey: true, shiftKey: true }))).toBe(null)
    expect(fileTreeVerb(press({ code: 'KeyV', metaKey: true, altKey: true }))).toBe(null)
  })

  it('answers nothing for a key it has never heard of', () => {
    expect(fileTreeVerb(press({ code: 'KeyQ', metaKey: true }))).toBe(null)
    expect(fileTreeVerb(press({ code: 'ArrowDown' }))).toBe(null)
    expect(fileTreeVerb(press({ code: 'Escape' }))).toBe(null)
  })

  it('renames on F2 on every platform', () => {
    expect(fileTreeVerb(press({ code: 'F2' }), { userAgent: MAC })).toBe('rename')
    expect(fileTreeVerb(press({ code: 'F2' }), { userAgent: WINDOWS })).toBe('rename')
    expect(fileTreeVerb(press({ code: 'F2' }), { userAgent: LINUX })).toBe('rename')
  })

  it('renames on Enter on a Mac, where that is what Finder does', () => {
    expect(fileTreeVerb(press({ code: 'Enter' }), { userAgent: MAC })).toBe('rename')
  })

  it('leaves Enter alone everywhere else, where it means open', () => {
    expect(fileTreeVerb(press({ code: 'Enter' }), { userAgent: WINDOWS })).toBe(null)
    expect(fileTreeVerb(press({ code: 'Enter' }), { userAgent: LINUX })).toBe(null)
    expect(fileTreeVerb(press({ code: 'Enter' }))).toBe(null)
  })

  it('leaves a modified F2 and a modified Enter alone', () => {
    expect(fileTreeVerb(press({ code: 'F2', metaKey: true }), { userAgent: MAC })).toBe(null)
    expect(fileTreeVerb(press({ code: 'Enter', metaKey: true }), { userAgent: MAC })).toBe(null)
    expect(fileTreeVerb(press({ code: 'Enter', shiftKey: true }), { userAgent: MAC })).toBe(null)
  })

  it('answers nothing for an event with nothing in it', () => {
    expect(fileTreeVerb()).toBe(null)
    expect(fileTreeVerb({})).toBe(null)
  })
})
