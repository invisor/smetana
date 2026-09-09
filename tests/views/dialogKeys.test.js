import { describe, expect, it } from 'vitest'
import { dialogVerb } from '../../src/views/dialogKeys.js'

/* A press, in the six fields the rule reads and no others. */
const press = (key, extra = {}) => ({
  key,
  defaultPrevented: false,
  metaKey: false,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  ...extra
})

describe('dialogVerb', () => {
  it('closes the window on Escape', () => {
    expect(dialogVerb(press('Escape'))).toBe('close')
  })

  it('says nothing about any other key', () => {
    for (const key of ['Enter', 'Tab', 'Escapee', 'a', ' ']) {
      expect(dialogVerb(press(key))).toBe(null)
    }
  })

  it('assumes a way out when nothing has said otherwise', () => {
    // A kind with no component draws an empty state and no Modal at all, and
    // that window has to be closable: it is the symptom of a mechanism whose
    // other failure is a window nobody can see.
    expect(dialogVerb(press('Escape'), {})).toBe('close')
    expect(dialogVerb(press('Escape'), { closable: undefined })).toBe('close')
  })

  it('refuses to be the way around a dialog that has taken its way out away', () => {
    // While the discard is running, the cross and Cancel are both gone; Escape
    // is answered and does nothing, rather than becoming a third exit.
    expect(dialogVerb(press('Escape'), { closable: false })).toBe('refuse')
  })

  it('leaves a press something inside the dialog has already answered', () => {
    // A dropdown, a pointer menu or the review window's branch list closes on
    // its own Escape and cancels the press on the way out. One Escape undoes
    // one thing: the panel goes and the window it is drawn in stays.
    expect(dialogVerb(press('Escape', { defaultPrevented: true }))).toBe(null)
    expect(dialogVerb(press('Escape', { defaultPrevented: true }), { closable: false })).toBe(null)
  })

  it('refuses a modified press rather than ignoring the modifier', () => {
    for (const held of ['metaKey', 'ctrlKey', 'altKey', 'shiftKey']) {
      expect(dialogVerb(press('Escape', { [held]: true }))).toBe(null)
    }
  })

  it('reads a press it was handed nothing of as belonging to nobody', () => {
    expect(dialogVerb()).toBe(null)
  })
})
