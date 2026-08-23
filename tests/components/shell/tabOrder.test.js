import { describe, expect, it } from 'vitest'
import { moveTab, neighbourIn, orderTabs } from '../../../src/components/shell/tabOrder.js'

/* The row as `stores/tabs.js` builds it: a leading run of pinned tabs, then
   everything a project brought. Only `id` and `kind` are read here, which is the
   whole of what this rule is about. */
const pinned = (...ids) => ids.map((id) => ({ id, kind: 'pinned' }))
const row = (...ids) => ids.map((id) => ({ id, kind: 'file' }))
const ids = (tabs) => tabs.map((tab) => tab.id)

describe('orderTabs', () => {
  it('leaves the row alone when nothing was ever rearranged', () => {
    const tabs = [...pinned('kanban'), ...row('a.js', 'b.js')]
    expect(orderTabs(tabs, [])).toBe(tabs)
    expect(orderTabs(tabs, undefined)).toBe(tabs)
    expect(orderTabs(tabs, null)).toBe(tabs)
  })

  it('leaves a row of nothing but pinned tabs alone', () => {
    const tabs = pinned('kanban', 'terminal')
    expect(orderTabs(tabs, ['kanban'])).toBe(tabs)
  })

  it('draws the movable tabs in the stored sequence', () => {
    const tabs = [...pinned('kanban'), ...row('a.js', 'b.js', 'c.js')]
    expect(ids(orderTabs(tabs, ['c.js', 'a.js', 'b.js']))).toEqual([
      'kanban',
      'c.js',
      'a.js',
      'b.js'
    ])
  })

  /* The whole point of the field: the row is one sequence rather than four
     glued lists, so a shell can stand between two files and a diff in front of
     all of them. */
  it('mixes the kinds, since the order is the row and not the lists behind it', () => {
    const tabs = [
      ...pinned('kanban', 'terminal'),
      ...row('a.js', 'b.js'),
      { id: '\u0000diff:/r\u0000x.rs', kind: 'diff' },
      { id: '\u0000term:2', kind: 'terminal' }
    ]

    const stored = ['\u0000diff:/r\u0000x.rs', 'a.js', '\u0000term:2', 'b.js']

    expect(ids(orderTabs(tabs, stored))).toEqual(['kanban', 'terminal', ...stored])
  })

  it('never moves the pinned run and never puts anything in front of it', () => {
    const tabs = [...pinned('kanban', 'terminal'), ...row('a.js', 'b.js')]

    // A stored order naming the pinned ids, and naming them last at that.
    expect(ids(orderTabs(tabs, ['b.js', 'a.js', 'kanban', 'terminal']))).toEqual([
      'kanban',
      'terminal',
      'b.js',
      'a.js'
    ])
  })

  it('appends a tab the stored order has never heard of, in the row order', () => {
    const tabs = [...pinned('kanban'), ...row('a.js', 'b.js', 'c.js', 'd.js')]
    expect(ids(orderTabs(tabs, ['c.js']))).toEqual(['kanban', 'c.js', 'a.js', 'b.js', 'd.js'])
  })

  it('passes over a stored id that matches no tab', () => {
    const tabs = [...pinned('kanban'), ...row('a.js', 'b.js')]
    expect(ids(orderTabs(tabs, ['b.js', '\u0000term:9', 'a.js']))).toEqual([
      'kanban',
      'b.js',
      'a.js'
    ])
  })

  /* After a restart only the file tabs come back, so every diff and terminal in
     the stored order names nothing — and the files still stand where they were
     left. */
  it('holds the places of the file tabs across a restart that lost the rest', () => {
    const stored = ['\u0000term:2', 'b.js', '\u0000diff:/r\u0000x.rs', 'a.js']
    const tabs = [...pinned('kanban'), ...row('a.js', 'b.js')]

    expect(ids(orderTabs(tabs, stored))).toEqual(['kanban', 'b.js', 'a.js'])
  })

  it('takes the first mention of an id repeated in a damaged order', () => {
    const tabs = [...pinned('kanban'), ...row('a.js', 'b.js', 'c.js')]
    expect(ids(orderTabs(tabs, ['c.js', 'a.js', 'c.js', 'b.js']))).toEqual([
      'kanban',
      'c.js',
      'a.js',
      'b.js'
    ])
  })

  it('does not mutate what it was given', () => {
    const tabs = [...pinned('kanban'), ...row('a.js', 'b.js', 'c.js')]
    orderTabs(tabs, ['c.js', 'b.js', 'a.js'])
    expect(ids(tabs)).toEqual(['kanban', 'a.js', 'b.js', 'c.js'])
  })
})

describe('moveTab', () => {
  const order = ['a.js', 'b.js', 'c.js']

  it('moves a tab forward', () => {
    expect(moveTab(order, 0, 2)).toEqual(['b.js', 'c.js', 'a.js'])
  })

  it('moves a tab back', () => {
    expect(moveTab(order, 2, 0)).toEqual(['c.js', 'a.js', 'b.js'])
  })

  it('swaps neighbours', () => {
    expect(moveTab(order, 0, 1)).toEqual(['b.js', 'a.js', 'c.js'])
  })

  /* The gesture tells "nothing happened" from "something did" by reference, and
     leans on it: a drag that changed nothing must not be committed. */
  it('gives back the very array when the tab is already there', () => {
    expect(moveTab(order, 1, 1)).toBe(order)
  })

  it('gives back the very array when an index is out of range', () => {
    expect(moveTab(order, -1, 1)).toBe(order)
    expect(moveTab(order, 0, 3)).toBe(order)
    expect(moveTab(order, 0, -1)).toBe(order)
    expect(moveTab(order, 3, 0)).toBe(order)
  })

  it('does not mutate what it was given', () => {
    moveTab(order, 0, 2)
    expect(order).toEqual(['a.js', 'b.js', 'c.js'])
  })
})

describe('neighbourIn', () => {
  const order = ['a.js', 'b.js', 'c.js']

  it('gives the neighbour on the right', () => {
    expect(neighbourIn(order, 'b.js')).toBe('c.js')
  })

  it('gives the neighbour on the left for the last tab', () => {
    expect(neighbourIn(order, 'c.js')).toBe('b.js')
  })

  /* Nobody, rather than the board: what a row with nothing left falls back to is
     the store's answer, and this function has never heard of the board. */
  it('gives nobody for the only tab in the row', () => {
    expect(neighbourIn(['a.js'], 'a.js')).toBe(null)
    expect(neighbourIn([], 'a.js')).toBe(null)
  })

  it('gives nobody for a tab that is not in the row', () => {
    expect(neighbourIn(order, 'gone.js')).toBe(null)
  })

  /* The kinds are nothing to it: the row is one sequence, so the tab that takes
     over is whichever one is drawn beside the closed one. */
  it('does not care what kind of tab the neighbour is', () => {
    const mixed = ['a.js', '\u0000term:2', '\u0000diff:/r\u0000x.rs']
    expect(neighbourIn(mixed, 'a.js')).toBe('\u0000term:2')
    expect(neighbourIn(mixed, '\u0000term:2')).toBe('\u0000diff:/r\u0000x.rs')
  })
})
