import { describe, expect, it } from 'vitest'
import { PREVIEW, columnOf, phoneColumns, shownCards } from '../src/board.js'

const card = (id, updatedAt = '2026-10-15T10:00:00Z') => ({ id, title: id, updatedAt })
const board = [
  { status: 'ready', tasks: [card('a')] },
  { status: 'running', tasks: [] },
  { status: 'needs-you', tasks: [card('b')] },
  { status: 'done', tasks: [card('c', '2026-01-01T00:00:00Z')] }
]
const NOW = Date.parse('2026-10-15T12:00:00Z')
const names = (columns) => columns.map((column) => column.status)

describe('the columns the phone draws', () => {
  it('draws every column in the board\'s own order when nothing is set', () => {
    expect(names(phoneColumns(board, [], undefined, NOW))).toEqual(['ready', 'running', 'needs-you', 'done'])
  })

  it('puts the stored order first and the columns it does not know after, in the board\'s order', () => {
    expect(names(phoneColumns(board, ['needs-you', 'ready'], undefined, NOW))).toEqual([
      'needs-you',
      'ready',
      'running',
      'done'
    ])
  })

  it('leaves out the empty columns the desktop would leave out', () => {
    const kanban = { columns: 'non-empty' }
    expect(names(phoneColumns(board, [], kanban, NOW))).toEqual(['ready', 'needs-you', 'done'])
  })

  it('keeps an empty column the settings say to always show', () => {
    const kanban = { columns: 'non-empty', alwaysShow: ['running'] }
    expect(names(phoneColumns(board, [], kanban, NOW))).toContain('running')
  })

  it('judges emptiness after the period, as the desktop does', () => {
    const kanban = { columns: 'non-empty', interval: 'week' }
    expect(names(phoneColumns(board, [], kanban, NOW))).toEqual(['ready', 'needs-you'])
  })

  it('sequence does not depend on which columns are hidden', () => {
    const order = ['done', 'running', 'needs-you', 'ready']
    const kanban = { columns: 'non-empty' }
    expect(names(phoneColumns(board, order, kanban, NOW))).toEqual(['done', 'needs-you', 'ready'])
  })

  it('answers an empty board for a board that has not arrived', () => {
    expect(phoneColumns(undefined, undefined, undefined, NOW)).toEqual([])
  })
})

describe('how much of a column is drawn', () => {
  const many = (n) => Array.from({ length: n }, (_, i) => card(`t${i}`))

  it('draws a short column whole', () => {
    expect(shownCards(many(PREVIEW), false)).toEqual({ cards: many(PREVIEW), hidden: 0 })
  })

  it('draws the first cards of a long one and says how many are left', () => {
    const shown = shownCards(many(PREVIEW + 5), false)
    expect(shown.cards).toHaveLength(PREVIEW)
    expect(shown.cards[0].id).toBe('t0')
    expect(shown.hidden).toBe(5)
  })

  it('draws all of it once it is expanded', () => {
    expect(shownCards(many(PREVIEW + 5), true)).toEqual({ cards: many(PREVIEW + 5), hidden: 0 })
  })

  it('draws nothing for no tasks', () => {
    expect(shownCards(undefined, false)).toEqual({ cards: [], hidden: 0 })
  })
})

describe('which column a task is in', () => {
  it('names the column and not the status bd keeps', () => {
    expect(columnOf(board, 'b')).toBe('needs-you')
    expect(columnOf(board, 'c')).toBe('done')
  })

  it('names none for a task the board does not draw', () => {
    expect(columnOf(board, 'zzz')).toBeUndefined()
    expect(columnOf(undefined, 'a')).toBeUndefined()
  })
})
