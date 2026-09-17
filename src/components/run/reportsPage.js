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
export const ORDERS = ['newest', 'oldest']

/* The shipped values, mirroring Rust's `ReportsSettings::default()`. */
export const REPORTS_DEFAULTS = { perPage: 20, order: 'newest' }

/* What the two `Select`s at the top of the tab offer, labels included — the
   tab draws these rather than writing its own, so the vocabulary and the
   words about it cannot drift apart. */
export const PAGE_SIZE_CHOICES = PAGE_SIZES.map((size) => ({ value: size, label: `${size} per page` }))
export const ORDER_CHOICES = [
  { value: 'newest', label: 'Newest first' },
  { value: 'oldest', label: 'Oldest first' }
]

/* By `stamp`, and then by `file` for two reports of the same second — the
   tiebreak `claim_report`'s own `-<n>` suffix exists to be told apart by.
   The tiebreak stays ascending regardless of `order`: `order` is a person's
   answer to "which end do I read from", and reversing the tiebreak along with
   it would make a second report of one second sort *before* the first only
   half the time, for no reason anybody chose. Anything but `'oldest'` reads
   as newest-first, the same total-default `formatStamp` and `pageOf` below
   give an unrecognised value. */
export function sortReports(rows, order) {
  const oldest = order === 'oldest'
  return [...rows].sort((a, b) => {
    if (a.stamp !== b.stamp) return a.stamp < b.stamp === oldest ? -1 : 1
    return a.file < b.file ? -1 : a.file > b.file ? 1 : 0
  })
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

/* The same `Intl` shape `TaskInspector.vue` reads bd's own dates with — day,
   short month, year, hour and minute in `en-GB`'s order — so a stamp reads the
   same way wherever a date appears in this app. `stamp` here is local time
   with no offset (`"YYYY-MM-DDTHH:MM:SS"`, `reports.rs`'s own shape), which
   `Date.parse` reads as local for exactly that reason — there is no timezone
   to get wrong between the two. Anything that will not parse is shown as it
   arrived, the same refusal `formatDate` in `TaskInspector.vue` makes. */
const STAMP_FORMAT = new Intl.DateTimeFormat('en-GB', {
  day: '2-digit',
  month: 'short',
  year: 'numeric',
  hour: '2-digit',
  minute: '2-digit'
})

export function formatStamp(stamp) {
  if (!stamp) return stamp
  const parsed = Date.parse(stamp)
  return Number.isNaN(parsed) ? stamp : STAMP_FORMAT.format(parsed)
}

/* The grid the list's header and every row share, so the two can never drift
   apart into two different rulers. Seven columns — stamp, title, scope, then
   the four counts — and every unit is `fr`/`auto`/`minmax`, never a pixel:
   the row is a computed style object like everything else in this system, and
   a literal width here would be exactly the hardcoded value that rule
   forbids. The four counts share one `auto` each rather than a fixed width,
   since a mono digit column is only ever as wide as `total`'s longest
   duration. */
export const COLUMNS = 'auto minmax(0, 2fr) minmax(0, 1fr) auto auto auto auto'
