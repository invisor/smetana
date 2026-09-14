/* Which agent "Attach to agent" reaches, and whether it may fire at all.

   Extracted out of `DesktopApp.vue` because a `.vue` file is the one thing no
   test in this repository can reach, and because the rule used to be wrong:
   `attachTarget` there read `terminalState.activeId` — the PTY store's own
   selection — while the agents panel's actual highlight, `activeAgentRow`, is
   a driven conversation's row for as long as one is aimed at. The two agree
   only by accident, which is what smetana-2p84 is. With no live PTY session
   the old field answered `null` and the row was refused with a live agent
   sitting one column over; with one, the row lit and typed the path into that
   PTY session while the person was looking at a conversation somewhere else.

   The fix is to ask the one list the panel actually draws from —
   `orderedAgentRows` in the caller, PTY and driven rows already merged and
   already in this design system's status words (`components/agent/drivenRows.js`
   is where that merge and that translation happen) — about the one row the
   panel highlights, `activeAgentRow`. This module takes neither store: it is
   handed the merged rows and the selected id, and it is handed them as plain
   data, so a test can build both by hand. */

import { ENDED, drivenSessionOf } from '../agent/drivenRows.js'

function isLive(row) {
  return Boolean(row) && !row.starting && !ENDED.includes(row.state)
}

/* Whether there is an agent to pick at all, across both roads — the
   population `selectedAttachTarget` is narrowed out of by the selection. It
   decides nothing about whether the row is off; it decides which of the
   menu's two reasons says so (`fileMenu.js`'s `NOTHING_TO_ATTACH_TO` against
   `NOTHING_SELECTED`). */
export function hasLiveAgent(rows = []) {
  return rows.some(isLive)
}

/* The row `activeAgentRow` names, resolved to where a path actually has to
   land — or `null`, when there is nothing to land it in.

   `selectedId` is `activeAgentRow`'s own value: a driven row's prefixed key
   (`drivenRowId`) while the panel's highlight is a conversation, or the PTY
   store's `activeId` otherwise — the same field the panel itself is drawn
   from, which is the whole point. `rows` is `orderedAgentRows`, the merged
   list both kinds of row already live in.

   `null` for a selection naming no row at all (nothing picked yet), for one
   that is not live (finished, failed, still starting), and for a restored row
   with no session behind it — `isLive` refuses all three the same way,
   without asking which one it was.

   The one branch is `drivenSessionOf`: a row whose id is this window's driven
   prefix answers with the conversation the worker holds it under, and the
   caller delivers to that id by a different door than a PTY target — `pty`
   writes bytes into a pseudoterminal, `driven` lands the path in that
   conversation's own draft (`conversationFor(id).draft` in
   `stores/conversation.js`), never a submitted turn — see `attachToAgent` in
   `DesktopApp.vue` for why. This module decides only which agent and which
   road; the delivery itself is the caller's. */
export function selectedAttachTarget({ selectedId = null, rows = [] } = {}) {
  if (selectedId === null) return null
  const row = rows.find((candidate) => candidate.id === selectedId)
  if (!isLive(row)) return null
  const conversation = drivenSessionOf(selectedId)
  return conversation !== null
    ? { road: 'driven', id: conversation }
    : { road: 'pty', id: selectedId }
}
