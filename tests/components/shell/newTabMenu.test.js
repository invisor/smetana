import { describe, expect, it } from 'vitest'
import { NEW_TAB_ITEMS } from '../../../src/components/shell/newTabMenu.js'

/* The + button's menu. Four rows, and the words are the point: this is the one
   place in the app that offers a shell at all, and the row a person looks for
   when they want an agent has to read the same as the one in the Agents panel.
   A `.vue` file is out of reach here, which is why the rows are a module. */
describe('the new-tab menu', () => {
  /* The order is the design and not an accident: the task first, then the two
     rows that open a tab, then the review. Pinned exactly, so a fourth row
     appended anywhere but the end fails here rather than on somebody's screen. */
  it('offers exactly four things, a task, an agent, a terminal and a review', () => {
    expect(NEW_TAB_ITEMS.map((item) => item.kind)).toEqual([
      'task',
      'agent',
      'terminal',
      'review'
    ])
    expect(NEW_TAB_ITEMS.map((item) => item.label)).toEqual([
      'New task',
      'New agent',
      'New terminal',
      'New review'
    ])
    /* The glyph per row, and not merely that each row has one: `square-check`
       is the mark `kanban/issueType.js` already draws bd's `task` type with, so
       the row and the card it files agree on what a task looks like, and
       `search-check` is the glyph the branch row's own `Review this branch…`
       carries, so the two doors into one window are marked the same way. */
    expect(NEW_TAB_ITEMS.map((item) => item.icon)).toEqual([
      'square-check',
      'bot',
      'terminal',
      'search-check'
    ])
  })

  /* Sentence case, like every label in this system, and not "New Agent". */
  it('is written in sentence case', () => {
    for (const item of NEW_TAB_ITEMS) {
      expect(item.label).toBe(item.label[0].toUpperCase() + item.label.slice(1).toLowerCase())
    }
  })

  /* Every row is walkable: nothing here is a separator, a heading or greyed
     out, so `MenuButton`'s keyboard reaches all three. Whether the menu can be
     used at all is the button's own `disabled`. */
  it('has no row that cannot be picked', () => {
    for (const item of NEW_TAB_ITEMS) {
      expect(item.type).toBeUndefined()
      expect(item.disabled).toBeUndefined()
      // A glyph each, and all four are registered in core/icons.js.
      expect(['bot', 'terminal', 'square-check', 'search-check']).toContain(item.icon)
    }
  })
})
