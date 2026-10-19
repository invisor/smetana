import { describe, expect, it } from 'vitest'
import { SELECTION_MENU_W, selectionMenuItems } from '../../../src/components/conversation/selectionMenu.js'

describe('selectionMenuItems', () => {
  it('offers quote, copy and a new task, in that order', () => {
    expect(selectionMenuItems({ canQuote: true, canFile: true }).map((it) => it.kind ?? it.type)).toEqual([
      'quote', 'copy', 'separator', 'new-task'
    ])
  })
  it('greys quote when there is no composer to put it in', () => {
    const [quote] = selectionMenuItems({ canQuote: false, canFile: true })
    expect(quote.disabled).toBe(true)
  })
  it('greys the task row while the dialog is open, and never copy', () => {
    const items = selectionMenuItems({ canQuote: true, canFile: false })
    expect(items.find((it) => it.kind === 'new-task').disabled).toBe(true)
    expect(items.find((it) => it.kind === 'copy').disabled).toBeFalsy()
  })
  it('names a width a number', () => {
    expect(typeof SELECTION_MENU_W).toBe('number')
  })
})
