/* Which columns of the board the phone draws, in which order, and how much of each.

   The rules are the desktop's own and are not restated: the sequence is `orderColumns`
   (`components/kanban/columnOrder.js`, the per-project order kept in the settings) and the
   visibility is `visibleColumns` (`components/kanban/boardView.js`, the global `kanban`
   settings), composed in the same order `DesktopApp.vue` composes them - the sequence first,
   so that it does not depend on which columns happen to be shown today. This module is only
   that composition, so that a screen has nothing to decide. */
import { orderColumns } from '../../src/components/kanban/columnOrder.js'
import { visibleColumns } from '../../src/components/kanban/boardView.js'

export function phoneColumns(boardColumns, columnOrder, kanban, now = Date.now()) {
  return visibleColumns(orderColumns(boardColumns ?? [], columnOrder ?? []), kanban, now)
}

/* A phone draws a column's cards one under another, and a done column can hold hundreds. The
   first screenfuls are shown and the rest are one press away. The order of the cards is the
   board's (`cardOrder.js` has already put the newest finished first), so what is hidden is the
   oldest. */
export const PREVIEW = 20

export function shownCards(tasks, expanded) {
  const list = tasks ?? []
  if (expanded || list.length <= PREVIEW) return { cards: list, hidden: 0 }
  return { cards: list.slice(0, PREVIEW), hidden: list.length - PREVIEW }
}

/* The column an issue stands in, in the design system's words, or `undefined` when the board
   does not draw it. A task waiting on an unfinished blocker is `open` in bd and stands in the
   computed Blocked column, so the status to show is the column's and not the issue's. */
export function columnOf(boardColumns, id) {
  for (const column of boardColumns ?? []) {
    if (column.tasks.some((task) => task.id === id)) return column.status
  }
  return undefined
}
