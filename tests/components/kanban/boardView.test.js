import { describe, expect, it } from 'vitest'
import {
  columnChoices,
  columnLabel,
  columnNames,
  mergeOrder,
  readKanban,
  toggleColumn,
  visibleColumns,
  withinInterval
} from '../../../src/components/kanban/boardView.js'

const NOW = Date.parse('2026-08-13T12:00:00Z')
const ago = (ms) => new Date(NOW - ms).toISOString()
const DAY = 24 * 60 * 60 * 1000

const task = (id, updatedAt) => ({ id, updatedAt })
const column = (status, ...tasks) => ({ status, tasks })
const names = (columns) => columns.map((c) => c.status)
const ids = (columns, status) => columns.find((c) => c.status === status).tasks.map((t) => t.id)

describe('withinInterval', () => {
  it('lets everything through when no window was chosen', () => {
    expect(withinInterval(task('a', ago(400 * DAY)), 'all', NOW)).toBe(true)
  })

  it('measures a day, a week and thirty days', () => {
    const fresh = task('fresh', ago(2 * 60 * 60 * 1000))
    const threeDays = task('three-days', ago(3 * DAY))
    const twoWeeks = task('two-weeks', ago(14 * DAY))
    const twoMonths = task('two-months', ago(60 * DAY))

    expect(withinInterval(fresh, 'day', NOW)).toBe(true)
    expect(withinInterval(threeDays, 'day', NOW)).toBe(false)

    expect(withinInterval(threeDays, 'week', NOW)).toBe(true)
    expect(withinInterval(twoWeeks, 'week', NOW)).toBe(false)

    expect(withinInterval(twoWeeks, 'month', NOW)).toBe(true)
    expect(withinInterval(twoMonths, 'month', NOW)).toBe(false)
  })

  it('shows a task whose date is missing or unreadable', () => {
    // Of the two ways to be wrong, one card too many costs a glance and one
    // card too few costs somebody's work.
    expect(withinInterval(task('none', undefined), 'day', NOW)).toBe(true)
    expect(withinInterval(task('none', null), 'day', NOW)).toBe(true)
    expect(withinInterval(task('junk', 'yesterday-ish'), 'day', NOW)).toBe(true)
    expect(withinInterval({ id: 'bare' }, 'day', NOW)).toBe(true)
  })

  it('shows a task stamped in the future', () => {
    expect(withinInterval(task('ahead', new Date(NOW + DAY).toISOString()), 'day', NOW)).toBe(true)
  })

  it('treats an interval it has never heard of as no window at all', () => {
    expect(withinInterval(task('old', ago(400 * DAY)), 'decade', NOW)).toBe(true)
  })
})

describe('readKanban', () => {
  it('is the board as it is today when there is nothing stored', () => {
    expect(readKanban(undefined)).toEqual({
      columns: 'all',
      alwaysShow: [],
      interval: 'all',
      unlimited: []
    })
  })

  it('takes the shipped value for a field off its closed list', () => {
    const rules = readKanban({ columns: 'some', interval: 'fortnight', unlimited: ['ready'] })
    expect(rules.columns).toBe('all')
    expect(rules.interval).toBe('all')
    expect(rules.unlimited, 'the neighbouring field must survive').toEqual(['ready'])
  })

  it('drops blanks, duplicates and anything that is not a name', () => {
    expect(readKanban({ alwaysShow: ['ready', 'ready', '', 7, null, 'done'] }).alwaysShow).toEqual([
      'ready',
      'done'
    ])
  })
})

describe('visibleColumns', () => {
  const board = () => [
    column('ready', task('r1', ago(2 * DAY)), task('r2', ago(90 * DAY))),
    column('running', task('n1', ago(1000))),
    column('deferred', task('d1', ago(90 * DAY))),
    column('done')
  ]

  it('draws the whole board on the shipped settings', () => {
    const drawn = visibleColumns(board(), undefined, NOW)
    expect(names(drawn)).toEqual(['ready', 'running', 'deferred', 'done'])
    expect(ids(drawn, 'ready')).toEqual(['r1', 'r2'])
  })

  it('drops the tasks outside the window', () => {
    const drawn = visibleColumns(board(), { interval: 'week' }, NOW)
    expect(ids(drawn, 'ready')).toEqual(['r1'])
    expect(ids(drawn, 'deferred')).toEqual([])
  })

  it('shows every task of a column named unlimited, whatever the window', () => {
    const drawn = visibleColumns(board(), { interval: 'day', unlimited: ['ready'] }, NOW)
    expect(ids(drawn, 'ready')).toEqual(['r1', 'r2'])
    expect(ids(drawn, 'deferred')).toEqual([])
  })

  it('takes an empty column off the board, and keeps one named alwaysShow', () => {
    const drawn = visibleColumns(board(), { columns: 'non-empty', alwaysShow: ['done'] }, NOW)
    expect(names(drawn)).toEqual(['ready', 'running', 'deferred', 'done'])

    const bare = visibleColumns(board(), { columns: 'non-empty' }, NOW)
    expect(names(bare)).toEqual(['ready', 'running', 'deferred'])
  })

  it('judges emptiness on what the window left, not on the whole board', () => {
    // A column the interval swept clean reads as empty and goes: the
    // alternative leaves visibly empty columns on screen, which is exactly
    // what the first setting is for.
    const drawn = visibleColumns(board(), { columns: 'non-empty', interval: 'week' }, NOW)
    expect(names(drawn)).toEqual(['ready', 'running'])
  })

  it('keeps a column swept clean by the window when alwaysShow names it', () => {
    const drawn = visibleColumns(
      board(),
      { columns: 'non-empty', interval: 'week', alwaysShow: ['deferred'] },
      NOW
    )
    expect(names(drawn)).toEqual(['ready', 'running', 'deferred'])
    expect(ids(drawn, 'deferred')).toEqual([])
  })

  it('leaves the board it was given alone', () => {
    const given = board()
    visibleColumns(given, { columns: 'non-empty', interval: 'day' }, NOW)
    expect(names(given)).toEqual(['ready', 'running', 'deferred', 'done'])
    expect(given[0].tasks).toHaveLength(2)
  })
})

describe('mergeOrder', () => {
  it('is the drawn order itself when the whole board is on screen', () => {
    expect(mergeOrder(['done', 'ready', 'running'], ['ready', 'running', 'done'])).toEqual([
      'done',
      'ready',
      'running'
    ])
  })

  it('keeps a hidden column where it stood', () => {
    // Without this the first drag strikes `deferred` out of the stored order,
    // and it reappears at the end of the board days later.
    const all = ['ready', 'deferred', 'running', 'done']
    expect(mergeOrder(['done', 'running', 'ready'], all)).toEqual([
      'done',
      'deferred',
      'running',
      'ready'
    ])
  })

  it('keeps several hidden columns in their own slots', () => {
    const all = ['a', 'hidden1', 'b', 'hidden2', 'c']
    expect(mergeOrder(['c', 'b', 'a'], all)).toEqual(['c', 'hidden1', 'b', 'hidden2', 'a'])
  })

  it('appends a drawn name the board order has never heard of', () => {
    expect(mergeOrder(['ready', 'fresh'], ['ready', 'done'])).toEqual(['ready', 'done', 'fresh'])
  })

  it('leaves the order alone when nothing was drawn', () => {
    expect(mergeOrder([], ['ready', 'done'])).toEqual(['ready', 'done'])
  })

  it('writes a name into every slot even when the board repeats one', () => {
    // Unreachable today — the board buckets its columns through a Map keyed on
    // status. Pinned because the failure would not stay in this file: an
    // undefined here reaches settings.json as a null and costs Rust the whole
    // project section.
    expect(mergeOrder(['a'], ['a', 'a'])).toEqual(['a', 'a'])
  })
})

describe('columnChoices', () => {
  it('splits the columns of this project from names saved elsewhere', () => {
    const choices = columnChoices(['ready', 'triage'], ['ready', 'running', 'done'])
    expect(choices.onBoard).toEqual([
      { name: 'ready', checked: true },
      { name: 'running', checked: false },
      { name: 'done', checked: false }
    ])
    expect(choices.elsewhere).toEqual([{ name: 'triage', checked: true }])
  })

  it('has no second group when everything stored is on the board', () => {
    expect(columnChoices(['ready'], ['ready', 'done']).elsewhere).toEqual([])
  })
})

describe('toggleColumn', () => {
  it('adds, removes and never grows a duplicate', () => {
    expect(toggleColumn(['ready'], 'done', true)).toEqual(['ready', 'done'])
    expect(toggleColumn(['ready', 'done'], 'ready', false)).toEqual(['done'])
    expect(toggleColumn(['ready'], 'ready', true)).toEqual(['ready'])
    expect(toggleColumn(undefined, 'ready', true)).toEqual(['ready'])
  })
})

describe('columnNames and columnLabel', () => {
  it('answers an empty list for anything that is not one', () => {
    expect(columnNames(null)).toEqual([])
    expect(columnNames('ready')).toEqual([])
  })

  it('reads a status as prose without inventing a word for it', () => {
    expect(columnLabel('needs-you')).toBe('needs you')
    expect(columnLabel('ready')).toBe('ready')
    expect(columnLabel(undefined)).toBe('')
  })
})
