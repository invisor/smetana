import { describe, expect, it } from 'vitest'
import { canPasteInto } from '../../../src/components/files/fileClipboard.js'

describe('canPasteInto', () => {
  it('refuses with a reason when the clipboard is empty', () => {
    expect(canPasteInto({ clipboard: null, folder: 'src' })).toEqual({ ok: false, reason: 'empty' })
  })

  it('refuses a clipboard record that holds no paths at all', () => {
    expect(canPasteInto({ clipboard: { paths: [], mode: 'copy' }, folder: 'src' })).toEqual({
      ok: false,
      reason: 'empty'
    })
  })

  it('allows a folder that is neither the source nor under it', () => {
    const clipboard = { paths: ['src/a'], mode: 'copy' }
    expect(canPasteInto({ clipboard, folder: 'lib' })).toEqual({ ok: true, reason: null })
  })

  it('refuses a folder into itself', () => {
    const clipboard = { paths: ['src/a'], mode: 'copy' }
    expect(canPasteInto({ clipboard, folder: 'src/a' })).toEqual({ ok: false, reason: 'intoSelf' })
  })

  it('refuses a folder into its own descendant', () => {
    const clipboard = { paths: ['src/a'], mode: 'copy' }
    expect(canPasteInto({ clipboard, folder: 'src/a/b' })).toEqual({ ok: false, reason: 'intoSelf' })
  })

  it('does not mistake a sibling with a longer name for a descendant', () => {
    // `src/ab`.startsWith('src/a') is true and the two are siblings. The
    // separator is what tells the two questions apart.
    const clipboard = { paths: ['src/a'], mode: 'copy' }
    expect(canPasteInto({ clipboard, folder: 'src/ab' })).toEqual({ ok: true, reason: null })
  })

  it('allows a paste into the root', () => {
    const clipboard = { paths: ['src/a'], mode: 'copy' }
    expect(canPasteInto({ clipboard, folder: '' })).toEqual({ ok: true, reason: null })
  })

  it('answers the same for a cut as for a copy: where it can go does not depend on the mode', () => {
    const folder = 'src/a/b'
    expect(canPasteInto({ clipboard: { paths: ['src/a'], mode: 'cut' }, folder })).toEqual(
      canPasteInto({ clipboard: { paths: ['src/a'], mode: 'copy' }, folder })
    )
  })

  it('refuses when any one of several paths would swallow the folder', () => {
    // The record carries an array although the tree selects one entry, so that
    // multiple selection does not change the shape later.
    const clipboard = { paths: ['lib/x', 'src/a'], mode: 'copy' }
    expect(canPasteInto({ clipboard, folder: 'src/a/b' })).toEqual({ ok: false, reason: 'intoSelf' })
  })

  it('answers without being given anything at all', () => {
    expect(canPasteInto()).toEqual({ ok: false, reason: 'empty' })
  })
})
