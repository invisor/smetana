import { describe, expect, it } from 'vitest'
import { NEW_TAB_ITEMS } from '../../../src/components/shell/newTabMenu.js'

/* The + button's menu. Two rows, and the words are the point: this is the one
   place in the app that offers a shell at all, and the row a person looks for
   when they want an agent has to read the same as the one in the Agents panel.
   A `.vue` file is out of reach here, which is why the rows are a module. */
describe('the new-tab menu', () => {
  it('offers exactly two things, an agent and a terminal', () => {
    expect(NEW_TAB_ITEMS.map((item) => item.kind)).toEqual(['agent', 'terminal'])
    expect(NEW_TAB_ITEMS.map((item) => item.label)).toEqual(['New agent', 'New terminal'])
  })

  /* Sentence case, like every label in this system, and not "New Agent". */
  it('is written in sentence case', () => {
    for (const item of NEW_TAB_ITEMS) {
      expect(item.label).toBe(item.label[0].toUpperCase() + item.label.slice(1).toLowerCase())
    }
  })

  /* Every row is walkable: nothing here is a separator, a heading or greyed
     out, so `MenuButton`'s keyboard reaches both. Whether the menu can be used
     at all is the button's own `disabled`. */
  it('has no row that cannot be picked', () => {
    for (const item of NEW_TAB_ITEMS) {
      expect(item.type).toBeUndefined()
      expect(item.disabled).toBeUndefined()
      // A glyph each, and both are registered in core/icons.js.
      expect(['bot', 'terminal']).toContain(item.icon)
    }
  })
})
