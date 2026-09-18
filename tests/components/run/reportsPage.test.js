import { describe, expect, it } from 'vitest'
import {
  COLUMNS,
  ORDER_CHOICES,
  ORDERS,
  PAGE_SIZE_CHOICES,
  PAGE_SIZES,
  barPercent,
  formatStamp,
  maxSeconds,
  pageOf,
  reportScopeText,
  sortReports
} from '../../../src/components/run/reportsPage.js'
import { scopeLabel } from '../../../src/components/run/runScopes.js'

const row = (stamp, file) => ({ stamp, file })

describe('sortReports', () => {
  it('sorts newest first by default', () => {
    const rows = [row('2026-09-15T10:00:00', 'a.html'), row('2026-09-17T10:00:00', 'b.html')]
    expect(sortReports(rows, 'newest').map((r) => r.file)).toEqual(['b.html', 'a.html'])
  })

  it('sorts oldest first when asked', () => {
    const rows = [row('2026-09-17T10:00:00', 'b.html'), row('2026-09-15T10:00:00', 'a.html')]
    expect(sortReports(rows, 'oldest').map((r) => r.file)).toEqual(['a.html', 'b.html'])
  })

  it('breaks a tied stamp by file, ascending, whichever order was asked', () => {
    // Two reports of the same second share one stamp; only `file` (the
    // `-<n>` suffix `claim_report` adds) tells them apart.
    const rows = [
      row('2026-09-17T10:00:00', '2026-09-17-100000-2.html'),
      row('2026-09-17T10:00:00', '2026-09-17-100000.html')
    ]
    expect(sortReports(rows, 'newest').map((r) => r.file)).toEqual([
      '2026-09-17-100000-2.html',
      '2026-09-17-100000.html'
    ])
    expect(sortReports(rows, 'oldest').map((r) => r.file)).toEqual([
      '2026-09-17-100000-2.html',
      '2026-09-17-100000.html'
    ])
  })

  it('does not mutate the array it was handed', () => {
    const rows = [row('2026-09-15T10:00:00', 'a.html'), row('2026-09-17T10:00:00', 'b.html')]
    const original = [...rows]
    sortReports(rows, 'newest')
    expect(rows).toEqual(original)
  })
})

describe('sortReports longest first', () => {
  const r = (stamp, file, seconds) => ({ stamp, file, seconds })

  it('puts the longest run first and an unknown length last', () => {
    const rows = [
      r('2026-09-17T10:00:00', 'b.html', 60),
      r('2026-09-15T10:00:00', 'a.html', null),
      r('2026-09-16T10:00:00', 'c.html', 8040)
    ]
    expect(sortReports(rows, 'longest').map((x) => x.file)).toEqual(['c.html', 'b.html', 'a.html'])
  })

  it('breaks an equal length by newest stamp, then by file', () => {
    const rows = [
      r('2026-09-15T10:00:00', 'a.html', 60),
      r('2026-09-17T10:00:00', '2026-09-17-100000-2.html', 60),
      r('2026-09-17T10:00:00', '2026-09-17-100000.html', 60)
    ]
    expect(sortReports(rows, 'longest').map((x) => x.file)).toEqual([
      '2026-09-17-100000-2.html',
      '2026-09-17-100000.html',
      'a.html'
    ])
  })
})

describe('pageOf', () => {
  const rows94 = Array.from({ length: 94 }, (_, i) => row(`2026-09-${String(i + 1).padStart(2, '0')}T00:00:00`, `${i}.html`))

  it('gives five pages of twenty for ninety-four reports, the last with fourteen', () => {
    const first = pageOf(rows94, { order: 'newest', perPage: 20, page: 1 })
    expect(first.pages).toBe(5)
    expect(first.total).toBe(94)
    expect(first.rows).toHaveLength(20)

    const last = pageOf(rows94, { order: 'newest', perPage: 20, page: 5 })
    expect(last.rows).toHaveLength(14)
    expect(last.page).toBe(5)
  })

  it('clamps a page past the end to the last page', () => {
    const out = pageOf(rows94, { order: 'newest', perPage: 20, page: 99 })
    expect(out.page).toBe(5)
    expect(out.rows).toHaveLength(14)
  })

  it('clamps a page below one to the first page', () => {
    const out = pageOf(rows94, { order: 'newest', perPage: 20, page: 0 })
    expect(out.page).toBe(1)
  })

  it('answers page 1 of 1 with no rows for an empty list', () => {
    const out = pageOf([], { order: 'newest', perPage: 20, page: 1 })
    expect(out.page).toBe(1)
    expect(out.pages).toBe(1)
    expect(out.total).toBe(0)
    expect(out.rows).toEqual([])
  })

  it('honours the chosen page size', () => {
    const out = pageOf(rows94, { order: 'newest', perPage: 50, page: 1 })
    expect(out.pages).toBe(2)
    expect(out.rows).toHaveLength(50)
  })
})

describe('formatStamp', () => {
  it('splits a stamp into a short date with no year and a time', () => {
    // en-GB, day/short-month — Node's ICU spells September "Sept".
    expect(formatStamp('2026-09-13T02:40:00')).toEqual({ date: '13 Sept', time: '02:40' })
  })

  it('returns an unparseable value as the date, with no time', () => {
    expect(formatStamp('not-a-date')).toEqual({ date: 'not-a-date', time: '' })
  })

  it('answers empty strings for a missing stamp rather than throwing', () => {
    expect(formatStamp(null)).toEqual({ date: '', time: '' })
    expect(formatStamp(undefined)).toEqual({ date: '', time: '' })
    expect(formatStamp('')).toEqual({ date: '', time: '' })
  })
})

describe('the duration bar', () => {
  it('scales to the longest run on the page and never drops below two percent', () => {
    const rows = [{ seconds: 8040 }, { seconds: 60 }, { seconds: null }]
    const max = maxSeconds(rows)
    expect(max).toBe(8040)
    expect(barPercent(8040, max)).toBe(100)
    expect(barPercent(60, max)).toBe(2)
    expect(barPercent(0, max)).toBe(2)
  })

  it('draws no bar for an unknown length and survives a page of unknowns', () => {
    expect(maxSeconds([{ seconds: null }])).toBe(1)
    expect(maxSeconds([])).toBe(1)
    expect(barPercent(null, 1)).toBe(null)
    expect(barPercent(undefined, 1)).toBe(null)
  })
})

describe('the closed lists front end and Rust must agree on', () => {
  it('offers exactly three page sizes, each with its own label', () => {
    expect(PAGE_SIZES).toEqual([20, 50, 100])
    expect(PAGE_SIZE_CHOICES).toEqual([
      { value: 20, label: '20 per page' },
      { value: 50, label: '50 per page' },
      { value: 100, label: '100 per page' }
    ])
  })

  it('offers exactly three orders, labelled in prose', () => {
    expect(ORDERS).toEqual(['newest', 'oldest', 'longest'])
    expect(ORDER_CHOICES).toEqual([
      { value: 'newest', label: 'Newest first' },
      { value: 'oldest', label: 'Oldest first' },
      { value: 'longest', label: 'Longest first' }
    ])
  })
})

describe('COLUMNS', () => {
  it('is a grid template of exactly seven tracks with no pixel literal in it', () => {
    expect(COLUMNS).not.toMatch(/\d+px/)
    // `minmax(0, 1fr)` carries a space of its own, so a track is counted by
    // collapsing each `(...)` to one word first rather than by a bare split.
    const tracks = COLUMNS.replace(/\([^)]*\)/g, 'X').split(' ')
    expect(tracks.length).toBe(7)
  })
})

describe('reportScopeText', () => {
  it('answers null for the row’s own dash', () => {
    expect(reportScopeText(null)).toBe(null)
    expect(reportScopeText(undefined)).toBe(null)
    expect(reportScopeText('')).toBe(null)
  })

  it('carries the queue’s own words through unabbreviated', () => {
    expect(reportScopeText('the queue')).toEqual({ isQueue: true, text: 'the queue' })
  })

  it('answers the bare id for a task or an epic, never the word in front of it', () => {
    expect(reportScopeText('task smetana-1wgi')).toEqual({ isQueue: false, text: 'smetana-1wgi' })
    expect(reportScopeText('epic smetana-8fzc')).toEqual({ isQueue: false, text: 'smetana-8fzc' })
  })

  it('draws a shape it does not recognise exactly as it arrived', () => {
    expect(reportScopeText('backend, admin and the design-system port all at once')).toEqual({
      isQueue: false,
      text: 'backend, admin and the design-system port all at once'
    })
  })

  it('is runScopes.js’s scopeLabel’s exact inverse, so nothing but this test notices if the two drift', () => {
    // The two live in different modules and each has its own suite pinning
    // its own side against a literal string — rename `epic` on either side
    // and both would stay green while the Scope column silently went back
    // to drawing the whole wire string. This is what would go loud instead.
    const queue = { kind: 'queue' }
    const task = { kind: 'task', id: 'smetana-1wgi' }
    const epic = { kind: 'epic', id: 'smetana-8fzc' }
    for (const scope of [queue, task, epic]) {
      const wire = scopeLabel(scope)
      const back = reportScopeText(wire)
      expect(back.isQueue).toBe(scope.kind === 'queue')
      expect(back.text).toBe(scope.kind === 'queue' ? wire : scope.id)
    }
  })
})
