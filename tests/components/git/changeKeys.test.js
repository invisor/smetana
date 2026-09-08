import { describe, expect, it } from 'vitest'
import { changeVerb } from '../../../src/components/git/changeKeys.js'

/* A press, in the five fields the rule reads and no others. */
const press = (code, mods = {}) => ({
  code,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...mods
})

const on = (code, mods) => changeVerb(press(code, mods), { openable: true })
const onFolder = (code, mods) => changeVerb(press(code, mods), { openable: false })

describe('changeVerb', () => {
  it('opens the diff on Enter, the same verb the click carries', () => {
    expect(on('Enter')).toBe('open')
  })

  it('opens the menu on the two presses that mean it', () => {
    // Shift+F10 is the platform's own chord and is the one that works
    // everywhere; the context-menu key is a PC keyboard's and a Mac has none.
    expect(on('F10', { shiftKey: true })).toBe('menu')
    expect(on('ContextMenu')).toBe('menu')
  })

  it('opens the menu on a row with nothing to diff, and refuses Enter there', () => {
    // An untracked directory arrives as one record with a trailing slash and
    // there is no file behind it. The menu still opens — a gesture that answers
    // on some rows and does nothing on others reads as a broken row.
    expect(onFolder('Enter')).toBe(null)
    expect(onFolder('F10', { shiftKey: true })).toBe('menu')
    expect(onFolder('ContextMenu')).toBe('menu')
  })

  it('refuses meta, control and alt rather than ignoring them', () => {
    // A handler that answered these would swallow presses it was never given.
    for (const held of ['metaKey', 'ctrlKey', 'altKey']) {
      expect(on('Enter', { [held]: true })).toBe(null)
      expect(on('ContextMenu', { [held]: true })).toBe(null)
      expect(on('F10', { [held]: true, shiftKey: true })).toBe(null)
    }
  })

  it('lets every shifted press but F10 straight through, so Shift+Tab still leaves', () => {
    // The caller cancels the default for a verb and never for anything else, so
    // `null` here is what keeps the row an ordinary tab stop.
    expect(on('Tab', { shiftKey: true })).toBe(null)
    expect(on('Enter', { shiftKey: true })).toBe(null)
    expect(on('ArrowDown', { shiftKey: true })).toBe(null)
  })

  it('has no arrow verbs at all, on either kind of row', () => {
    // Arrow navigation is a later task. Answering one of these now would be a
    // press that moves nothing, since there is no roving tabindex behind it.
    for (const code of ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight']) {
      expect(on(code)).toBe(null)
      expect(onFolder(code)).toBe(null)
    }
  })

  it('says nothing about Tab, Space, Escape or a letter', () => {
    for (const code of ['Tab', 'Space', 'Escape', 'KeyF', 'F2', 'Home', 'End']) {
      expect(on(code)).toBe(null)
    }
  })

  it('reads the key place on the board and never the character it produced', () => {
    // `event.key` is a Cyrillic character under a Russian layout and an
    // upper-case letter under Caps Lock. A rule reading it would fire in
    // neither case, so it is not read at all.
    expect(changeVerb({ key: 'Enter' }, { openable: true })).toBe(null)
    expect(on('Enter')).toBe('open')
  })

  it('answers null for a press it is handed nothing about', () => {
    expect(changeVerb()).toBe(null)
    expect(changeVerb({}, {})).toBe(null)
  })
})
