import { describe, expect, it } from 'vitest'
import { orderCards } from '../../../src/components/kanban/cardOrder.js'

const card = (over = {}) => ({
  id: 'bd-1',
  title: 'A card',
  closedAt: '2026-08-01T10:00:00Z',
  updatedAt: '2026-08-01T10:00:00Z',
  ...over
})

const ids = (tasks) => tasks.map((task) => task.id)

describe('orderCards', () => {
  it('puts the done column newest first', () => {
    const tasks = [
      card({ id: 'bd-old', closedAt: '2026-07-30T09:00:00Z' }),
      card({ id: 'bd-new', closedAt: '2026-08-02T15:06:00Z' }),
      card({ id: 'bd-mid', closedAt: '2026-08-01T11:10:00Z' })
    ]

    expect(ids(orderCards('done', tasks))).toEqual(['bd-new', 'bd-mid', 'bd-old'])
  })

  /* A batch merge closes several tasks inside one second, and the order they
     arrive in differs between a session and a resync — so the tie has to be
     broken on something that never moves. */
  it('breaks an equal closing time on the id, ascending', () => {
    const at = '2026-08-02T15:06:00Z'
    const tasks = [card({ id: 'bd-c', closedAt: at }), card({ id: 'bd-a', closedAt: at }), card({ id: 'bd-b', closedAt: at })]

    expect(ids(orderCards('done', tasks))).toEqual(['bd-a', 'bd-b', 'bd-c'])
    expect(ids(orderCards('done', [...tasks].reverse()))).toEqual(['bd-a', 'bd-b', 'bd-c'])
  })

  it('falls back to the update time when the closing time is missing or unreadable', () => {
    const tasks = [
      card({ id: 'bd-1', closedAt: '2026-08-01T10:00:00Z' }),
      card({ id: 'bd-2', closedAt: null, updatedAt: '2026-08-03T10:00:00Z' }),
      card({ id: 'bd-3', closedAt: 'not a date', updatedAt: '2026-08-02T10:00:00Z' })
    ]

    expect(ids(orderCards('done', tasks))).toEqual(['bd-2', 'bd-3', 'bd-1'])
  })

  /* Out of place is cheaper than off the board. */
  it('sends a card with neither date readable to the bottom rather than dropping it', () => {
    const tasks = [
      card({ id: 'bd-lost', closedAt: null, updatedAt: null }),
      card({ id: 'bd-also-lost', closedAt: 'nonsense', updatedAt: 'nonsense' }),
      card({ id: 'bd-dated', closedAt: '2026-07-01T10:00:00Z' })
    ]

    expect(ids(orderCards('done', tasks))).toEqual(['bd-dated', 'bd-also-lost', 'bd-lost'])
  })

  it('leaves every other column exactly as it was, by reference', () => {
    const tasks = [card({ id: 'bd-2' }), card({ id: 'bd-1' })]

    for (const status of ['ready', 'running', 'blocked', 'awaiting-review']) {
      expect(orderCards(status, tasks)).toBe(tasks)
    }
  })
})
