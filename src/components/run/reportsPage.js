/* Pagination and sort order for the Reports tab — pure over the rows
   `run_reports` hands back, no Vue and no DOM, the shape the whole `reports.js`
   family of modules follows and the one part of this feature a test can
   reach.

   Two settings live here, both global (`reports` in `settings.json`, mirroring
   `ReportsSettings` in `settings/model.rs`): how many rows a page holds and
   which end of the list is read first. Neither is per project, on `kanban`'s
   own argument: a habit of reading, not a fact about one repository. */

/* The two closed lists, written out a second time in
   `src-tauri/src/settings/model.rs`'s `ReportsSettings::validate`, under the
   same obligation `boardView.js`'s doubling carries: what this offers must be
   a subset of what Rust accepts, or a value Rust refuses reverts to the
   default on the next save with nothing on screen to say so. */
export const PAGE_SIZES = [20, 50, 100]
export const ORDERS = ['newest', 'oldest', 'longest']

/* The shipped values, mirroring Rust's `ReportsSettings::default()`. */
export const REPORTS_DEFAULTS = { perPage: 20, order: 'newest' }

/* What the two `Select`s at the top of the tab offer, labels included — the
   tab draws these rather than writing its own, so the vocabulary and the
   words about it cannot drift apart. */
export const PAGE_SIZE_CHOICES = PAGE_SIZES.map((size) => ({ value: size, label: `${size} per page` }))
export const ORDER_CHOICES = [
  { value: 'newest', label: 'Newest first' },
  { value: 'oldest', label: 'Oldest first' },
  { value: 'longest', label: 'Longest first' }
]

/* Newest first, then the `file` tiebreak, ascending whatever was asked —
   the tiebreak `claim_report`'s own `-<n>` suffix exists to be told apart by,
   and it stays ascending regardless of `order`: `order` is a person's answer
   to "which end do I read from", and reversing the tiebreak along with it
   would make a second report of one second sort *before* the first only half
   the time, for no reason anybody chose. Shared by `longest` for rows of one
   length. */
function byStamp(a, b, oldest) {
  if (a.stamp !== b.stamp) return a.stamp < b.stamp === oldest ? -1 : 1
  return a.file < b.file ? -1 : a.file > b.file ? 1 : 0
}

/* `longest`: by `seconds`, descending, with an unknown length (`null`, the
   row's word for "the document did not say") after every known one — a dash
   sorted as zero would put an unread run among the shortest, which is a claim
   about it nobody made. Equal lengths fall back to newest first. Anything but
   `'oldest'` or `'longest'` reads as newest-first, the same total-default
   `formatStamp` and `pageOf` below give an unrecognised value. */
export function sortReports(rows, order) {
  if (order === 'longest') {
    return [...rows].sort((a, b) => {
      const as = a.seconds ?? -1
      const bs = b.seconds ?? -1
      if (as !== bs) return bs - as
      return byStamp(a, b, false)
    })
  }
  const oldest = order === 'oldest'
  return [...rows].sort((a, b) => byStamp(a, b, oldest))
}

/* One page of the sorted list. `page` is clamped into `1..=pages`, and `pages`
   is never less than 1 — an empty list is `page 1 of 1` with no rows, not a
   division by zero and not a page 0 nothing can point back from. Changing
   `perPage` or `order` is the caller's business: this only answers for
   whatever `page` it was asked about, which is why `ReportList.vue` resets its
   own `page` to 1 whenever either of them, or `rows`, changes. */
export function pageOf(rows, { order, perPage, page }) {
  const sorted = sortReports(rows, order)
  const total = sorted.length
  const pages = Math.max(1, Math.ceil(total / perPage))
  const clamped = Math.min(Math.max(1, page), pages)
  const start = (clamped - 1) * perPage
  return { rows: sorted.slice(start, start + perPage), page: clamped, pages, total }
}

/* Day and short month, no year — the year is noise on every row of a list
   read newest first — and the time apart from it, so the row can colour the
   two differently. Same `en-GB` shape `TaskInspector.vue` reads bd's own
   dates with, split rather than joined. `stamp` here is local time with no
   offset (`"YYYY-MM-DDTHH:MM:SS"`, `reports.rs`'s own shape), which
   `Date.parse` reads as local for exactly that reason — there is no timezone
   to get wrong between the two. */
const DATE_FORMAT = new Intl.DateTimeFormat('en-GB', { day: '2-digit', month: 'short' })
const TIME_FORMAT = new Intl.DateTimeFormat('en-GB', { hour: '2-digit', minute: '2-digit' })

/* `{ date, time }` rather than one string: the row draws the two in different
   colours. An unparseable stamp keeps its own text as `date` and answers no
   `time` at all, the same refusal `formatDate` in `TaskInspector.vue` makes;
   a missing stamp answers both halves empty rather than throwing. */
export function formatStamp(stamp) {
  if (!stamp) return { date: '', time: '' }
  const parsed = Date.parse(stamp)
  if (Number.isNaN(parsed)) return { date: stamp, time: '' }
  return { date: DATE_FORMAT.format(parsed), time: TIME_FORMAT.format(parsed) }
}

/* The duration bar is data ink scaled to the page: the longest run on screen
   is always full width. At least 1 so a page of unknowns divides by nothing. */
export function maxSeconds(rows) {
  return rows.reduce((max, row) => Math.max(max, row.seconds ?? 0), 1)
}

/* A fill width in percent, never under 2 so a short run still shows, and
   `null` for an unknown length — no bar, the same silence as the dash. */
export function barPercent(seconds, max) {
  if (seconds === null || seconds === undefined) return null
  return Math.max(2, Math.round((seconds / max) * 100))
}

/* The grid the list's header and every row share, so the two can never drift
   apart into two different rulers. Seven columns — When, Summary (the
   flexible one, the sentence a row is for), Scope (a quarter of what is left,
   ellipsised), then the four counts — and every unit is `fr`/`auto`/`minmax`,
   never a pixel: the row is a computed style object like everything else in
   this system, and a literal width here would be exactly the hardcoded value
   that rule forbids. */
export const COLUMNS = 'auto minmax(0, 3fr) minmax(0, 1fr) auto auto auto auto'
