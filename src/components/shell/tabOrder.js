/* What order the centre's tabs sit in, and which one is next to which. Two
   owners meet here the way they meet in `kanban/columnOrder.js`: the row's
   contents are the app's — the board, the Agent tab, the open files, the diffs,
   the shells — and the sequence alone is the settings'. So this file is the
   reconciliation between them, and it is pure, with no Vue and no DOM in it,
   which is what makes the whole of the rule reachable by a test at all.

   One thing here has no parallel on the board: **a leading run that does not
   move.** The board and the Agent tab are `kind: 'pinned'`, they come first, and
   nothing may be put to the left of them — which is also what keeps the
   "+ New agent, terminal or task" button beside them, since `TabBar.vue` finds
   that slot by the same leading run. The rule lives here rather than in the
   component for exactly that reason: two readings of "which tabs are pinned"
   would be two answers about where that button goes. */

/* Where the movable part of the row starts: the length of the leading run of
   pinned tabs. Read off the list rather than counted anywhere, the same way
   `TabBar.vue` reads it — `stores/tabs.js` is the one place that decides which
   tabs are pinned, and this is that decision being obeyed rather than repeated. */
const pinnedCount = (tabs) => {
  const at = tabs.findIndex((tab) => tab.kind !== 'pinned')
  return at === -1 ? tabs.length : at
}

/* A stored order is a hint, never the truth. A tab is not conjured into the row
   by a line in a settings file, and a tab opened since the last drag has to
   appear even though no stored order names it.

   So: tabs the stored order knows are drawn in its sequence, and the rest go
   after them in the row's own order — which is what makes a newly opened tab,
   file or diff or terminal, appear at the end. Ids in the stored order that
   match nothing are simply passed over, not pruned: after a restart only the
   file tabs come back, and the entries naming yesterday's diffs and shells cost
   nothing at all. `stores/tabs.js` rewrites the field whole on the next drag,
   so the file cleans itself up rather than being cleaned on the way in.

   The pinned run is sliced off first and put back untouched, so a pinned id
   that found its way into the stored order — a hand-edited file, an older build
   — matches nothing and is ignored, which is the same sentence as "Kanban and
   the Agent tab do not move". */
export function orderTabs(tabs, stored) {
  const lead = pinnedCount(tabs)
  if (lead === tabs.length) return tabs
  if (!Array.isArray(stored) || !stored.length) return tabs

  const rank = new Map()
  for (const id of stored) if (!rank.has(id)) rank.set(id, rank.size)

  const known = []
  const fresh = []
  for (let i = lead; i < tabs.length; i += 1) {
    ;(rank.has(tabs[i].id) ? known : fresh).push(tabs[i])
  }
  known.sort((a, b) => rank.get(a.id) - rank.get(b.id))

  return [...tabs.slice(0, lead), ...known, ...fresh]
}

/* One tab moved from one index to another, in the indices of the movable part
   of the row — the pinned run is not in this list, which is what makes "no tab
   can be put to the left of it" arithmetic rather than a check.

   Out-of-range indices and a move to where the tab already is give back the very
   array that came in, by reference: the caller leans on that to tell "nothing
   happened" from "something did" without comparing contents, exactly as
   `moveColumn` is leaned on. */
export function moveTab(order, from, to) {
  const last = order.length - 1
  if (from === to || from < 0 || from > last || to < 0 || to > last) return order

  const next = [...order]
  next.splice(to, 0, next.splice(from, 1)[0])
  return next
}

/* Which tab takes over when this one is closed: the neighbour on the right,
   then the one on the left, then nobody — and the caller answers the last case,
   because the board is the store's fallback and not a fact about a list.

   It is a statement about **the row as drawn**, which is why it is one function
   and not three. `closeTab`, `closeDiff` and `closeTerminalTab` each used to
   write it out over their own list, and that was only ever right while the row
   and those lists were the same thing: after a drag the closed tab would hand
   the choice to whoever happened to sit beside it in `openTabs` rather than to
   whoever sits beside it on screen.

   `ids` is the movable part of the row and deliberately not the whole of it.
   With the pinned run in the list there would always be a neighbour — the board
   is never absent — and the caller's fallback could never be reached, which
   would quietly turn "close the last file tab" into "go to the Agent tab". */
export function neighbourIn(ids, id) {
  const at = ids.indexOf(id)
  if (at === -1) return null
  return ids[at + 1] ?? ids[at - 1] ?? null
}
