import { describe, expect, it } from 'vitest'
import {
  COLUMNS,
  ORDER_CHOICES,
  ORDERS,
  PAGE_SIZE_CHOICES,
  PAGE_SIZES,
  formatStamp,
  pageOf,
  sortReports
} from '../../../src/components/run/reportsPage.js'

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
  it('matches the format TaskInspector.vue reads bd dates with', () => {
    // en-GB, day/short-month/year, hour:minute — Node's ICU spells September
    // "Sept" for the short form, which is what the acceptance criteria's own
    // example shows.
    expect(formatStamp('2026-09-13T02:40:00')).toBe('13 Sept 2026, 02:40')
  })

  it('returns an unparseable value as it arrived', () => {
    expect(formatStamp('not-a-date')).toBe('not-a-date')
  })

  it('passes through a missing stamp rather than throwing', () => {
    expect(formatStamp(null)).toBe(null)
    expect(formatStamp(undefined)).toBe(undefined)
    expect(formatStamp('')).toBe('')
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

  it('offers exactly two orders, labelled in prose', () => {
    expect(ORDERS).toEqual(['newest', 'oldest'])
    expect(ORDER_CHOICES).toEqual([
      { value: 'newest', label: 'Newest first' },
      { value: 'oldest', label: 'Oldest first' }
    ])
  })
})

describe('COLUMNS', () => {
  it('is a grid template with no pixel literal in it', () => {
    expect(COLUMNS).not.toMatch(/\d+px/)
    expect(COLUMNS.split(' ').length).toBeGreaterThanOrEqual(5)
  })
})
