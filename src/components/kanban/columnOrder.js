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
