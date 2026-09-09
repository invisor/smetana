/* Which order the board's columns sit in. Two owners meet here: bd owns which
   columns exist, the settings own only the sequence — so this file is the
   reconciliation between them, and it is pure, with no Vue and no DOM, which is
   what makes it the one part of the reordering a test can reach at all.

   The same split as the panel widths: what settings.json keeps is what a person
   dragged to, what the board draws is that reconciled against the columns bd
   actually has now. */

/* A stored order is a hint, never the truth — a status bd no longer has cannot
   be conjured onto the board by a line in a settings file, and a status bd grew
   since the last visit has to appear even though no stored order names it.

   So: columns the stored order knows are drawn in its sequence, and the rest go
   after them in bd's own order. Appended rather than dropped, because a column
   nobody has arranged yet still holds issues; appended rather than slotted back
   into bd's position, because there is no honest position to slot it into once
   the neighbours have been moved by hand.

   Names in the stored order that match nothing are simply passed over, not
   pruned. A status that comes back — a custom one deleted and recreated, a
   project reopened — finds the place it was left in. */
export function orderColumns(columns, stored) {
  if (!Array.isArray(stored) || !stored.length) return columns

  const rank = new Map()
  for (const status of stored) if (!rank.has(status)) rank.set(status, rank.size)

  const known = []
  const fresh = []
  for (const column of columns) (rank.has(column.status) ? known : fresh).push(column)
  known.sort((a, b) => rank.get(a.status) - rank.get(b.status))

  return [...known, ...fresh]
}

/* One column moved from one index to another, as both a drag and an arrow key
   see it. Out-of-range indices and a move to where the column already is give
   back the very array that came in, by reference — the caller leans on that to
   tell "nothing happened" from "something did" without comparing contents. */
export function moveColumn(order, from, to) {
  const last = order.length - 1
  if (from === to || from < 0 || from > last || to < 0 || to > last) return order

  const next = [...order]
  next.splice(to, 0, next.splice(from, 1)[0])
  return next
}

/* Which way the "move this whole column into the queue" button points. The
   queue is a column like any other and can be dragged to either side of the
   one being emptied, so an arrow drawn always-right says "into pinned" on half
   the boards there are — which is a lie about where the tasks go.

   A rule about the order of the columns, so it lives here beside the other two
   rather than in the header that draws it: a `.vue` file is the one thing no
   test in this repository can reach.

   `columns` is the drawn board, in the shape the rest of this file takes
   (objects with a `status`). Either status missing is an ordinary outcome, not
   an error — a project can hide the queue by a view setting, and a column that
   is not on screen has no side to point at — so the answer is `'right'`, which
   is what the button drew before this rule existed. */
export function promoteSide(columns, from, to) {
  if (!Array.isArray(columns)) return 'right'

  const at = (status) => columns.findIndex((column) => column.status === status)
  const source = at(from)
  const target = at(to)
  if (source < 0 || target < 0) return 'right'

  return target < source ? 'left' : 'right'
}
