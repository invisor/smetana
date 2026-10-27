/* The rows of the Agents screen: the desktop's own rule for what an agent row is, over the
   sessions the phone can see.

   The rows are `mergeAgentRows` of the terminal store's rows (PTY sessions) and the driven
   ones, built the way `views/DesktopApp.vue` builds them, then ordered with `orderAgents` -
   newest first, the order the agents panel keeps when nothing has been dragged. A phone never
   drags and pins nothing. What this adds is the phone's two small questions: how a row opens
   (a conversation or a read-only terminal) and which row shouts.

   The phone sees what the server lists (`session_list`, `terminal_list`), offline rows
   included: they are drawn dim and offered for resuming, as on the desktop. */
import { mergeAgentRows, drivenSessionOf } from '../../src/components/agent/drivenRows.js'
import { orderAgents } from '../../src/components/agent/agentOrder.js'
import { attentionLevel } from '../../src/components/status/status.js'

/* A driven record as the agents panel's row builder wants it. `statusOf` and `elapsed` are the
   store's and the clock's, handed in so that this stays free of both. */
export function drivenRecords(sessions, project, { statusOf, elapsed }) {
  return (sessions ?? [])
    .filter((session) => session.project === project)
    .map((session) => ({
      id: session.id,
      project: session.project,
      state: statusOf(session.state),
      elapsed: elapsed(session.startedAt),
      startedAt: session.startedAt,
      conversation: session.conversation ?? null,
      work: session.work,
      title: session.title ?? null
    }))
}

export function agentRowsFor(terminalRows, driven) {
  const rows = mergeAgentRows(
    (terminalRows ?? []).filter((row) => !row.starting),
    driven
  )
  return orderAgents(rows, null, [])
}

/* How a row opens: the entry the navigation pushes. A driven row names its session; a PTY row is
   read-only, and its id is the terminal worker's. */
export function entryFor(row) {
  if (row.restored) return { resume: true }
  const driven = drivenSessionOf(row.id)
  return driven === null
    ? { screen: 'conversation', id: row.id, driven: false }
    : { screen: 'conversation', id: driven, driven: true }
}

/* The intent that reopens an offline row: the desktop's own `resumeSession`, word for word
   (`resumeSession` in views/DesktopApp.vue), without `fork`. */
export const resumeIntentFor = (row) => ({
  kind: 'resumeSession',
  id: row.conversation,
  cwd: row.cwd,
  title: row.title ?? null
})

/* A row waiting on a person is the one the screen is for. */
export const loud = (row) => attentionLevel(row.state) === 'loud'
