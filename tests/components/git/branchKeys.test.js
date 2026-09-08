import { describe, expect, it } from 'vitest'
import { branchVerb, isFilterChord } from '../../../src/components/git/branchKeys.js'

const press = (fields) => ({ metaKey: false, ctrlKey: false, altKey: false, shiftKey: false, ...fields })

describe('branchVerb', () => {
  it('walks the list with the vertical arrows, whatever kind of row it is on', () => {
    expect(branchVerb(press({ code: 'ArrowUp' }))).toBe('up')
    expect(branchVerb(press({ code: 'ArrowDown' }))).toBe('down')
    expect(branchVerb(press({ code: 'ArrowUp' }), { folder: true, expanded: true })).toBe('up')
    expect(branchVerb(press({ code: 'ArrowDown' }), { folder: true, expanded: false })).toBe('down')
  })

  it('closes an open folder with the left arrow', () => {
    expect(branchVerb(press({ code: 'ArrowLeft' }), { folder: true, expanded: true })).toBe('fold')
  })

  it('goes out to the heading with the left arrow anywhere else', () => {
    expect(branchVerb(press({ code: 'ArrowLeft' }), { folder: false })).toBe('parent')
    /* A folder already closed has nothing left to close, so the press means
       what it means on a branch row: go out one level. */
    expect(branchVerb(press({ code: 'ArrowLeft' }), { folder: true, expanded: false })).toBe('parent')
  })

  it('opens a closed folder with the right arrow', () => {
    expect(branchVerb(press({ code: 'ArrowRight' }), { folder: true, expanded: false })).toBe('unfold')
  })

  it('answers nothing to the right arrow where there is nothing to go into', () => {
    expect(branchVerb(press({ code: 'ArrowRight' }), { folder: false })).toBe(null)
    expect(branchVerb(press({ code: 'ArrowRight' }), { folder: true, expanded: true })).toBe(null)
  })

  it('switches on Enter, which is what the double click does', () => {
    expect(branchVerb(press({ code: 'Enter' }))).toBe('switch')
    expect(branchVerb(press({ code: 'Enter' }), { folder: true, expanded: true })).toBe('switch')
  })

  it('opens the menu on Shift+F10 and on the context menu key', () => {
    expect(branchVerb(press({ code: 'F10', shiftKey: true }))).toBe('menu')
    expect(branchVerb(press({ code: 'ContextMenu' }))).toBe('menu')
  })

  it('leaves every arrow alone under a command, a control or an alt key', () => {
    expect(branchVerb(press({ code: 'ArrowDown', metaKey: true }))).toBe(null)
    expect(branchVerb(press({ code: 'ArrowUp', ctrlKey: true }))).toBe(null)
    expect(branchVerb(press({ code: 'ArrowLeft', altKey: true }), { folder: true, expanded: true })).toBe(null)
    expect(branchVerb(press({ code: 'Enter', metaKey: true }))).toBe(null)
    expect(branchVerb(press({ code: 'F10', shiftKey: true, ctrlKey: true }))).toBe(null)
  })

  it('answers nothing to Shift on any key but F10', () => {
    expect(branchVerb(press({ code: 'ArrowDown', shiftKey: true }))).toBe(null)
    expect(branchVerb(press({ code: 'Enter', shiftKey: true }))).toBe(null)
    expect(branchVerb(press({ code: 'ContextMenu', shiftKey: true }))).toBe(null)
  })

  it('answers nothing for a key it has never heard of', () => {
    expect(branchVerb(press({ code: 'Space' }))).toBe(null)
    expect(branchVerb(press({ code: 'KeyF' }))).toBe(null)
    expect(branchVerb(press({ code: 'Escape' }))).toBe(null)
    expect(branchVerb(press({ code: 'F10' }))).toBe(null)
  })

  it('answers nothing for an event with nothing in it', () => {
    expect(branchVerb()).toBe(null)
    expect(branchVerb({})).toBe(null)
    expect(branchVerb({}, {})).toBe(null)
  })
})

describe('isFilterChord', () => {
  it('reads the find chord off the command key and off the control key', () => {
    expect(isFilterChord(press({ code: 'KeyF', metaKey: true }))).toBe(true)
    expect(isFilterChord(press({ code: 'KeyF', ctrlKey: true }))).toBe(true)
  })

  it('refuses the letter pressed on its own', () => {
    expect(isFilterChord(press({ code: 'KeyF' }))).toBe(false)
  })

  it('refuses the chord with Shift or Alt held as well', () => {
    expect(isFilterChord(press({ code: 'KeyF', metaKey: true, shiftKey: true }))).toBe(false)
    expect(isFilterChord(press({ code: 'KeyF', ctrlKey: true, altKey: true }))).toBe(false)
  })

  it('refuses every other key', () => {
    expect(isFilterChord(press({ code: 'KeyK', metaKey: true }))).toBe(false)
    expect(isFilterChord(press({ code: 'Enter', metaKey: true }))).toBe(false)
    expect(isFilterChord()).toBe(false)
    expect(isFilterChord({})).toBe(false)
  })
})
