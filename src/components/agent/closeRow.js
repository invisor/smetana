/* What the cross on an agent row does, as data: which of five acts a row asks
   for, and the ids each one needs. Pure, with no Vue, no DOM and no store in it,
   so the whole of the rule is reachable by a test and the desktop's panel and
   the phone's list can share one copy of it instead of two that drift.

   The caller hands each answer to the store verbs it names; this file only says
   which. The acts, and the order in which they are tried, are the ones
   `removeAgentRow` in `views/DesktopApp.vue` always had:

   - `crew-root` — a Crew lead; closing it closes the whole crew.
   - `crew-child` — a member of a crew; its cross does nothing, the lead owns it.
   - `driven` — a conversation: close it, forget the session, and forget the
     record the registry holds under its conversation id (`recorded`, or `null`
     for a session that never had one).
   - `offered` — an offline row: there is no session, only a record, and its id
     is the conversation's.
   - `pty` — an ordinary terminal session. */
import { drivenSessionOf } from './drivenRows.js'

export function closeActsFor(row) {
  const id = row?.id
  if (typeof id === 'string' && id.startsWith('crew:')) {
    const [, root, node] = id.split(':')
    return root === node ? { kind: 'crew-root', root: id } : { kind: 'crew-child' }
  }
  const session = drivenSessionOf(id)
  if (session !== null) return { kind: 'driven', session, recorded: row.conversation ?? null }
  if (row?.restored) return { kind: 'offered', conversation: id }
  return { kind: 'pty', id }
}
