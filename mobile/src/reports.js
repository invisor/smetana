/* The run reports as the phone lists them: a name and a date, newest first. Opening one is out
   of scope - the full report is an HTML document on the desktop's disk and the bridge carries no
   file reads - so a row is all it will ever be.

   The ordering and the date are the desktop's own (`reportsPage.js`); what is decided here is
   what a row is called when the report has no title, which the desktop's list never has to say
   because it draws other columns beside it. */
import { formatStamp, sortReports } from '../../src/components/run/reportsPage.js'

export function reportTitle(row) {
  const title = typeof row?.title === 'string' ? row.title.trim() : ''
  if (title) return title
  const file = typeof row?.file === 'string' ? row.file.trim() : ''
  return file || 'Run report'
}

export function reportRows(rows) {
  return sortReports(rows ?? [], 'newest').map((row) => {
    const when = formatStamp(row.stamp)
    return {
      key: row.path ?? row.file ?? row.stamp,
      title: reportTitle(row),
      summary: typeof row.summary === 'string' ? row.summary : '',
      date: when.date,
      time: when.time
    }
  })
}
