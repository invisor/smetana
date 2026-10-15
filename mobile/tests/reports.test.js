import { describe, expect, it } from 'vitest'
import { reportRows, reportTitle } from '../src/reports.js'

describe('the report list', () => {
  it('is called by its title, else its file, else a plain name', () => {
    expect(reportTitle({ title: ' Batch report ', file: 'x.html' })).toBe('Batch report')
    expect(reportTitle({ title: null, file: 'old.html' })).toBe('old.html')
    expect(reportTitle({ title: '', file: '' })).toBe('Run report')
    expect(reportTitle(undefined)).toBe('Run report')
  })

  it('lists the newest first, each with a name and a date', () => {
    const rows = reportRows([
      { path: '/a', title: 'Older', stamp: '2026-10-14T11:00:00Z' },
      { path: '/b', title: 'Newer', stamp: '2026-10-15T11:00:00Z', summary: 'Three tasks' }
    ])
    expect(rows.map((row) => row.title)).toEqual(['Newer', 'Older'])
    expect(rows[0].summary).toBe('Three tasks')
    expect(rows[0].date).toMatch(/15 Oct/)
    expect(rows[1].summary).toBe('')
  })

  it('draws a report with no date rather than dropping it', () => {
    const [row] = reportRows([{ path: '/c', title: 'Undated', stamp: null }])
    expect(row.title).toBe('Undated')
    expect(row.date).toBe('')
  })

  it('answers nothing for no list', () => {
    expect(reportRows(undefined)).toEqual([])
  })
})
