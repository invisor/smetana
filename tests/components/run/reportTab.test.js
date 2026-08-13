import { describe, expect, it } from 'vitest'
import { REPORTS_DIR, isReportPath, reportTabPath } from '../../../src/components/run/reportTab.js'

describe('isReportPath', () => {
  it('accepts a report the run wrote', () => {
    expect(isReportPath('.smetana/reports/2026-08-12-143155.html')).toBe(true)
  })

  it('accepts the disambiguated name two runs in one second produce', () => {
    expect(isReportPath('.smetana/reports/2026-08-12-143155-2.html')).toBe(true)
  })

  it('refuses an ordinary file, however it is named', () => {
    expect(isReportPath('src/index.html')).toBe(false)
    expect(isReportPath('reports/2026-08-12-143155.html')).toBe(false)
  })

  it('refuses a folder whose name merely starts the same way', () => {
    // What the trailing slash on REPORTS_DIR is for, and the trap a tidy-up of
    // the constant falls straight into.
    expect(isReportPath('.smetana/reports-old/2026-08-12-143155.html')).toBe(false)
  })

  it('refuses anything else in the same folder, so only documents open as one', () => {
    expect(isReportPath('.smetana/reports/notes.txt')).toBe(false)
  })

  it('refuses a path climbing out of the folder', () => {
    expect(isReportPath('.smetana/reports/../../etc/passwd.html')).toBe(false)
  })

  it('refuses a document in a folder under the reports folder', () => {
    expect(isReportPath('.smetana/reports/old/2026-08-12-143155.html')).toBe(false)
  })

  it('refuses the folder itself, which names no document', () => {
    expect(isReportPath(REPORTS_DIR)).toBe(false)
  })

  it('says no to nothing at all rather than throwing', () => {
    expect(isReportPath('')).toBe(false)
    expect(isReportPath(null)).toBe(false)
    expect(isReportPath(undefined)).toBe(false)
  })

  it('says no to the two pinned tabs, which are names and not paths', () => {
    expect(isReportPath('terminal')).toBe(false)
    expect(isReportPath('kanban')).toBe(false)
  })
})

describe('reportTabPath', () => {
  const ROOT = '/Users/you/Projects/smetana'
  const REPORT = `${ROOT}/.smetana/reports/2026-08-12-143155.html`

  it('takes the project off the front of what the worker wrote', () => {
    expect(reportTabPath(REPORT, ROOT)).toBe('.smetana/reports/2026-08-12-143155.html')
  })

  it('does not mind a root with a trailing separator', () => {
    expect(reportTabPath(REPORT, `${ROOT}/`)).toBe('.smetana/reports/2026-08-12-143155.html')
  })

  it('reads a Windows path, where the two vocabularies use different separators', () => {
    const root = 'C:\\Users\\you\\smetana'
    const report = 'C:\\Users\\you\\smetana\\.smetana\\reports\\2026-08-12-143155.html'
    expect(reportTabPath(report, root)).toBe('.smetana/reports/2026-08-12-143155.html')
  })

  it('refuses a document belonging to another project rather than guessing', () => {
    expect(reportTabPath(REPORT, '/Users/you/Projects/other')).toBe(null)
    // The trailing separator again, from the other side: a sibling folder whose
    // name merely starts with this one's is not inside it.
    expect(reportTabPath('/Users/you/Projects/smetana-old/.smetana/reports/a.html', ROOT)).toBe(null)
  })

  it('refuses anything under the project that is not a report', () => {
    expect(reportTabPath(`${ROOT}/src/index.html`, ROOT)).toBe(null)
    expect(reportTabPath(`${ROOT}/.smetana/reports/notes.txt`, ROOT)).toBe(null)
  })

  it('says no to nothing at all rather than throwing', () => {
    expect(reportTabPath(null, ROOT)).toBe(null)
    expect(reportTabPath(REPORT, null)).toBe(null)
    expect(reportTabPath(REPORT, '')).toBe(null)
  })
})
