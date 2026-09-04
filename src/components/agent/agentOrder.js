/* What order the agents panel's rows sit in, and which of them are held above
   the rest. Two owners meet here the way they meet in `kanban/columnOrder.js`
   and `shell/tabOrder.js`: what rows exist is the app's — the live sessions,
   the starts, and the records the last run of the app left behind — and the
   sequence alone is the settings'. So this file is the reconciliation between
   them, and it is pure, with no Vue and no DOM in it, which is what makes the
   whole of the rule reachable by a test at all.

   **One flat zone.** Any row may be dragged past any other: a live session, one
   still starting, an offline record. That was asked directly and answered
   directly — whether a process stands behind a row is not the person's
   question. The consequence is taken knowingly: a new agent appears at the end
   of the list and may well sit below yesterday's rows. That is what "the order
   belongs to the person" means.

   The one thing that does not move is the leading block of pinned rows, and
   that is the shape `tabOrder.js` already has for the board and the Agent tab.
   The difference is who decides membership: there it is the kind of the tab and
   nothing can change it, here it is a person's own choice, kept per project in
   `settings.json`.

   **The key is the conversation id and never the session number.** `SessionId`
   is a counter in the terminal worker that starts at 1 on every launch, and no
   process survives a restart — so an order or a pin kept under it would hand
   yesterday's place to whichever agent happened to be started second today. The
   conversation id is the one name for a session that means the same thing
   tomorrow: the app chooses it at the spawn, `.smetana/agents.json` keys the
   restorable record by it, and the offline row offered back after a restart
   carries the very same one. */

/* What a row is known by while this window is open.

   The conversation id where there is one, and the row's own id where there is
   not — a run's batch, a fork, a start ticket, a harness that cannot be told an
   id. A row of the second sort takes part in the order for as long as the
   window lives and is written to no file, which is what `conversationsOf`
   below is for: what reaches `settings.json` is conversation ids alone.

   The fallback cannot collide with a real key: a restored row's own id *is* its
   conversation id, and every other id is a number or a start ticket, neither of
   which a stored order can hold. */
export const agentKey = (row) => row?.conversation ?? row?.id ?? null

/* Whether this row is one of the pinned ones.

   Asked of the conversation id alone, never of `agentKey`: a row with no
   conversation cannot be pinned, because a pin outliving the session is the
   entire point of pinning and there would be nothing to remember it by. */
export function isPinned(row, pinned) {
  const id = row?.conversation
  return Boolean(id) && (pinned ?? []).includes(id)
}

/* The rows in the order they are to be drawn.

   A stored order is a hint, never the truth. A row is not conjured into the
   panel by a line in a settings file, and an agent started since the last drag
   has to appear even though no stored order names it. So: rows the stored order
   knows are drawn in its sequence, and the rest go after them in the panel's
   own order — which is what makes a newly started agent appear at the end. Ids
   in the stored order that match nothing are simply passed over here, so a
   conversation that is not on screen today is never the reason a row goes
   missing.

   **Passed over is not kept**, and the difference matters to whoever reads the
   file: `conversationsOf` below is fed the rows standing at the moment of a
   drag, so the next drag writes the field whole and every unmatched id goes
   with it. That is wanted rather than tolerated — a conversation leaves
   `.smetana/agents.json` when its agent exits on its own or somebody closes the
   row, and it is not coming back, so an entry naming one is dead weight.

   Then the pinned block is lifted out in front, and what decides *its* order is
   the same stored sequence — so dragging one pinned row past another works
   exactly as dragging anything else does. A pinned row the stored order has
   never heard of falls in behind those it has, in the order the pins were put
   on. That is the whole reason the two lists are separate fields rather than
   one: a drag rewrites the order and leaves the pins alone, pinning rewrites
   the pins and leaves the order alone, and an unpinned row therefore drops back
   into the place the order still remembers for it instead of landing at the
   end.

   With nothing stored and nothing pinned the rows come back by reference, which
   the caller leans on the way it leans on `moveColumn`'s: it is how "this
   project has never been arranged" is told from "arranged, and this is what it
   came to". */
export function orderAgents(rows, stored, pinned) {
  const list = Array.isArray(rows) ? rows : []
  const sequence = Array.isArray(stored) ? stored : []
  const pins = Array.isArray(pinned) ? pinned : []
  if (!sequence.length && !pins.length) return rows

  const rank = new Map()
  for (const id of sequence) if (!rank.has(id)) rank.set(id, rank.size)

  let out = list
  if (rank.size) {
    const known = []
    const fresh = []
    for (const row of list) (rank.has(agentKey(row)) ? known : fresh).push(row)
    known.sort((a, b) => rank.get(agentKey(a)) - rank.get(agentKey(b)))
    out = [...known, ...fresh]
  }
  if (!pins.length) return out

  const pinRank = new Map()
  for (const id of pins) if (!pinRank.has(id)) pinRank.set(id, pinRank.size)
  const held = []
  const rest = []
  for (const row of out) (isPinned(row, pins) ? held : rest).push(row)
  if (!held.length) return out

  /* The stored sequence first, and the pins' own order behind it — offset past
     the whole of the sequence so the two can never interleave. */
  const placeOf = (row) => {
    const key = agentKey(row)
    return rank.has(key) ? rank.get(key) : rank.size + pinRank.get(row.conversation)
  }
  held.sort((a, b) => placeOf(a) - placeOf(b))
  return [...held, ...rest]
}

/* One row moved from one index to another, in the indices of the panel as it is
   drawn. Out-of-range indices and a move to where the row already is give back
   the very array that came in, by reference: the caller leans on that to tell
   "nothing happened" from "something did" without comparing contents, exactly
   as `moveTab` and `moveColumn` are leaned on. */
export function moveAgent(order, from, to) {
  const last = order.length - 1
  if (from === to || from < 0 || from > last || to < 0 || to > last) return order

  const next = [...order]
  next.splice(to, 0, next.splice(from, 1)[0])
  return next
}

/* What of a drawn order reaches `settings.json`: the conversation ids, in the
   order the rows stand in, and nothing else. A row with none is passed over
   rather than written under its session number — that number is the very thing
   this file refuses to key anything by.

   This is also what prunes the field: the answer is built from the rows on
   screen, so whatever the stored order named and no row matches is gone on the
   next drag. See the header for why that is the wanted behaviour here and not
   the loss it would be on the board. */
export function conversationsOf(rows) {
  return (rows ?? []).map((row) => row?.conversation).filter(Boolean)
}

/* One agent pinned or unpinned, as a new list. Appended at the end, so the
   stored order is the order the pins were put on; `orderAgents` reads it that
   way for the rows no dragged order names.

   A row with no conversation id gives the list back unchanged — the menu greys
   that row rather than offering it, and this is the same refusal standing where
   the write happens, the way `toggleFavorite` guards an empty name. */
export function togglePin(pinned, conversation) {
  const pins = pinned ?? []
  if (!conversation) return [...pins]
  if (pins.includes(conversation)) return pins.filter((id) => id !== conversation)
  return [...pins, conversation]
}
