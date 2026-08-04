import { describe, expect, it } from 'vitest'
import { moveColumn, orderColumns } from '../../../src/components/kanban/columnOrder.js'

const board = (...statuses) => statuses.map((status) => ({ status, tasks: [] }))
const names = (columns) => columns.map((column) => column.status)

describe('orderColumns', () => {
  it('leaves bd order alone when nothing was ever rearranged', () => {
    const columns = board('ready', 'running', 'done')
    expect(orderColumns(columns, [])).toBe(columns)
    expect(orderColumns(columns, undefined)).toBe(columns)
    expect(orderColumns(columns, null)).toBe(columns)
  })

  it('draws the columns in the stored sequence', () => {
    const stored = ['done', 'ready', 'running']
    expect(names(orderColumns(board('ready', 'running', 'done'), stored))).toEqual(stored)
  })

  it('does not depend on the order bd happens to hand them over in', () => {
    const stored = ['done', 'ready', 'running']
    expect(names(orderColumns(board('running', 'done', 'ready'), stored))).toEqual(stored)
  })

  it('appends a column the stored order has never heard of', () => {
    const columns = board('ready', 'running', 'triage', 'done')
    expect(names(orderColumns(columns, ['done', 'ready', 'running']))).toEqual([
      'done',
      'ready',
      'running',
      'triage'
    ])
  })

  it('keeps bd order among the columns it appends', () => {
    const columns = board('hooked', 'deferred', 'ready')
    expect(names(orderColumns(columns, ['ready']))).toEqual(['ready', 'hooked', 'deferred'])
  })

  it('passes over a stored name that matches no column', () => {
    const columns = board('ready', 'done')
    expect(names(orderColumns(columns, ['done', 'gone', 'ready']))).toEqual(['done', 'ready'])
  })

  it('holds the place of a status that comes back', () => {
    const stored = ['done', 'pinned', 'ready']
    expect(names(orderColumns(board('ready', 'done'), stored))).toEqual(['done', 'ready'])
    expect(names(orderColumns(board('ready', 'pinned', 'done'), stored))).toEqual(stored)
  })

  it('takes the first mention of a name repeated in a damaged order', () => {
    const columns = board('ready', 'running', 'done')
    expect(names(orderColumns(columns, ['done', 'ready', 'done', 'running']))).toEqual([
      'done',
      'ready',
      'running'
    ])
  })

  it('does not mutate what it was given', () => {
    const columns = board('ready', 'running', 'done')
    orderColumns(columns, ['done', 'running', 'ready'])
    expect(names(columns)).toEqual(['ready', 'running', 'done'])
  })
})

describe('moveColumn', () => {
  const order = ['ready', 'running', 'done']

  it('moves a column forward', () => {
    expect(moveColumn(order, 0, 2)).toEqual(['running', 'done', 'ready'])
  })

  it('moves a column back', () => {
    expect(moveColumn(order, 2, 0)).toEqual(['done', 'ready', 'running'])
  })

  it('swaps neighbours', () => {
    expect(moveColumn(order, 0, 1)).toEqual(['running', 'ready', 'done'])
  })

  /* The caller tells "nothing happened" from "something did" by reference, and
     leans on it: a move that changes nothing must not be committed as a drag. */
  it('gives back the very array when the column is already there', () => {
    expect(moveColumn(order, 1, 1)).toBe(order)
  })

  it('gives back the very array when an index is out of range', () => {
    expect(moveColumn(order, -1, 1)).toBe(order)
    expect(moveColumn(order, 0, 3)).toBe(order)
    expect(moveColumn(order, 0, -1)).toBe(order)
    expect(moveColumn(order, 3, 0)).toBe(order)
  })

  it('does not mutate what it was given', () => {
    moveColumn(order, 0, 2)
    expect(order).toEqual(['ready', 'running', 'done'])
  })
})
