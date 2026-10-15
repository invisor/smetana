/* What state a driven session is in, worked out on the phone from its events.

   The desktop's front end learns a session's state from `session:state`, which is not an
   allowed event: the bridge carries `session:events` and nothing about state. Without a state
   the conversation panel cannot tell a session that is working (Stop, not Send) from one that
   waits, and the Agents list cannot say which agent is waiting on a person. The state is a
   fold of the journal, though - `session::model::state_of` is exactly that - and this is the
   same fold, so the phone can say the event the desktop would have said.

   One difference from the desktop's, stated rather than hidden: `state_of` is also told
   whether the child process is alive, and the phone cannot know that from events. A session
   the snapshot reported as ended (`exited`, `failed`) stays ended; one that dies afterwards
   goes on reading as it did until the next snapshot. */

const ENDED = ['exited', 'failed']

/* The fold's state, as the accumulator `step` carries it. */
function fresh() {
  return { openTurn: false, pending: new Set(), textQuestion: false, seen: false, dead: null, last: null }
}

function step(acc, event) {
  acc.seen = true
  switch (event.kind) {
    case 'turn-start':
      acc.openTurn = true
      acc.textQuestion = false
      break
    case 'result':
    case 'turn-failed':
      acc.openTurn = false
      break
    case 'permission':
      acc.pending.add(event.id)
      break
    case 'permission-answered':
      acc.pending.delete(event.id)
      break
    case 'text-question':
      acc.textQuestion = true
      break
    default:
      break
  }
}

export function stateOf(acc) {
  if (acc.dead) return acc.dead
  if (acc.pending.size > 0 || acc.textQuestion) return 'needs-you'
  if (acc.openTurn) return 'running'
  return acc.seen ? 'ready' : 'starting'
}

/* The accumulators, by session id. */
export function createTracker() {
  const sessions = new Map()
  return {
    /* A snapshot: the whole journal, and the state the desktop reported with it. */
    seed(id, events, state) {
      const acc = fresh()
      for (const event of events ?? []) step(acc, event)
      if (ENDED.includes(state)) acc.dead = state
      acc.last = stateOf(acc)
      sessions.set(id, acc)
      return acc.last
    },
    /* A batch of events: the new state when it differs from the last one said, else `null`.
       Also `null` for a session nobody has seeded - there is no journal to fold the batch
       into, and a state worked out from a fragment would be a guess. */
    absorb(id, events) {
      const acc = sessions.get(id)
      if (!acc) return null
      for (const event of events ?? []) step(acc, event)
      const state = stateOf(acc)
      if (state === acc.last) return null
      acc.last = state
      return state
    },
    forget(id) {
      sessions.delete(id)
    },
    has: (id) => sessions.has(id)
  }
}

/* The one tracker of the page, shared by the two bridge modules: the call that learns a
   snapshot (./core.js) and the pump that learns the events after it (./event.js). */
export const sessions = createTracker()
