import { describe, expect, it } from 'vitest'
import { canPasteInto, pasteSource } from '../../../src/components/files/fileClipboard.js'

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

  it('offers a paste anywhere for a path from outside the project, which is absolute', () => {
    // What the system clipboard holds after a copy in Finder. It is not a
    // prefix of any folder in the tree, so it greys nothing — the direction
    // this copy of the rule is allowed to disagree with Rust's in. The case it
    // cannot see, a folder from outside that holds the project, is
    // `refuse_into_self`'s.
    const clipboard = { paths: ['/elsewhere/pictures'], mode: 'copy' }
    expect(canPasteInto({ clipboard, folder: 'src/a' })).toEqual({ ok: true, reason: null })
    expect(canPasteInto({ clipboard, folder: '' })).toEqual({ ok: true, reason: null })
  })
})

describe('pasteSource', () => {
  it('falls back to the internal record when the system clipboard holds no files', () => {
    const internal = { paths: ['/p/src/a.txt'], mode: 'cut' }
    expect(pasteSource({ internal, system: { paths: [], mode: 'copy' } })).toEqual(internal)
  })

  it('prefers the internal record when both name the same paths, because only it knows about cut', () => {
    // A copy inside the tree writes to the system clipboard too, so this is
    // the ordinary case rather than the exotic one — and on macOS the system
    // side cannot say `cut` at all, so taking it would turn every move into a
    // copy.
    const internal = { paths: ['/p/src/a.txt'], mode: 'cut' }
    const system = { paths: ['/p/src/a.txt'], mode: 'copy' }
    expect(pasteSource({ internal, system })).toEqual(internal)
  })

  it('takes the system clipboard as a copy when the two disagree', () => {
    const internal = { paths: ['/p/src/a.txt'], mode: 'cut' }
    const system = { paths: ['/elsewhere/b.png'], mode: 'copy' }
    expect(pasteSource({ internal, system })).toEqual({ paths: ['/elsewhere/b.png'], mode: 'copy' })
  })

  it('keeps the mode the system clipboard states, where a platform states one', () => {
    // Windows writes `Preferred DropEffect` and Linux writes
    // `x-special/gnome-copied-files`; macOS states nothing and always reads as
    // a copy.
    const system = { paths: ['/elsewhere/b.png'], mode: 'cut' }
    expect(pasteSource({ internal: null, system })).toEqual({
      paths: ['/elsewhere/b.png'],
      mode: 'cut'
    })
  })

  it('answers nothing when neither holds anything', () => {
    expect(pasteSource({ internal: null, system: { paths: [], mode: 'copy' } })).toBe(null)
  })

  it('answers nothing when it is given nothing at all, which is what a refused read looks like', () => {
    expect(pasteSource()).toBe(null)
  })

  it('does not call two different lists the same because one is a prefix of the other', () => {
    const internal = { paths: ['/p/a.txt'], mode: 'cut' }
    const system = { paths: ['/p/a.txt', '/p/b.txt'], mode: 'copy' }
    expect(pasteSource({ internal, system })).toEqual({
      paths: ['/p/a.txt', '/p/b.txt'],
      mode: 'copy'
    })
  })

  it('reads a mode no platform states as a copy rather than passing it on', () => {
    const system = { paths: ['/elsewhere/b.png'], mode: 'link' }
    expect(pasteSource({ internal: null, system })).toEqual({
      paths: ['/elsewhere/b.png'],
      mode: 'copy'
    })
  })

  it('hands back a list of its own rather than the system clipboard\'s array', () => {
    const paths = ['/elsewhere/b.png']
    const chosen = pasteSource({ internal: null, system: { paths, mode: 'copy' } })
    paths.push('/elsewhere/c.png')
    expect(chosen.paths).toEqual(['/elsewhere/b.png'])
  })
})
