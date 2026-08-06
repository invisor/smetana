/* Agent sessions on the front end. Components know only this store; only it
   knows Tauri exists.

   The split follows cost: session state arrives as events for every session
   at once — it is cheap, and needed even for an agent nobody is looking at.
   Output bytes flow only for the active session, and nothing here keeps
   them: their consumer is xterm.js, and the truth lives in the ring in
   Rust. */
import { computed, reactive, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { settings } from './settings.js'

export const terminalState = reactive({
  sessions: [],
  /* Starts the worker has not answered yet. A session takes about a second to
     come back — a spawn, and on the first one of a run a login shell as well —
     and until it does there is nothing in `sessions` to draw, so the panel drew
     its empty state at the very moment somebody had just asked for an agent.
     Kept beside the sessions rather than inside them: everything else here
     treats that list as "what the worker has", and a row with no process
     behind it must not reach code that would send it an id. */
  starting: [],
  activeId: null,
  ready: false,
  project: null,
  lastError: null
})

/* Whether an id names a start rather than a session. Ticket ids are strings and
   the worker's are numbers, so the two can never be confused for one another —
   which is what lets `activeId` carry either without anything having to ask
   which kind it is holding before comparing it. */
export const isStarting = (id) => typeof id === 'string'

/* The starts that belong to the project on screen. The same rule `upsert`
   applies to a session, and it is needed for the same reason: a spawn takes
   about a second, a person can switch project inside it, and a row for an agent
   starting somewhere else drawn under this project's name is exactly the
   confusion the session list refuses to allow. The start itself is not
   cancelled — it is somebody's agent and it keeps coming up; it simply belongs
   to the panel it was asked for. */
const visibleStarts = () =>
  terminalState.starting.filter(
    (ticket) => !terminalState.project || ticket.project === terminalState.project
  )

/* The session's internal state and the design system's status are different
   vocabularies, and the translation lives here, the way bd's status
   translation lives in tracker.js. */
export function toUiState(session) {
  if (session.state === 'exited') return session.exitCode === 0 ? 'done' : 'failed'
  if (session.state === 'starting') return 'running'
  if (session.state === 'idle') return 'ready'
  return session.state
}

/* Negative input is ordinary, not a bug to let through: the clock below
   ticks every thirty seconds, so a session created between ticks has a
   startedAt in the future of the time this row is measured against. Floor of
   a negative number rounds away from zero, which is how a fresh agent
   showed "-1h -1m". An agent's age is never less than nothing. */
export function formatElapsed(ms) {
  const minutes = Math.max(0, Math.floor(ms / 60000))
  const hours = Math.floor(minutes / 60)
  return hours ? `${hours}h ${String(minutes % 60).padStart(2, '0')}m` : `${minutes}m`
}

/* Ticks once every thirty seconds: the time in an agent's row is measured in
   tens of minutes, and second-level precision would serve nobody there.
   Started lazily from initTerminals(), not at module scope: the module
   loads once for a window's lifetime in the app, but the test harness
   reloads it per test, and an interval nobody clears would outlive every
   test that started one. */
const now = ref(Date.now())
let clockStarted = false

function startClock() {
  if (clockStarted) return
  clockStarted = true
  setInterval(() => (now.value = Date.now()), 30000)
}

/* Sessions first, in the worker's own order, then whatever is still starting.
   A new session always takes the highest id, so a start belongs at the bottom
   both before and after it lands, and the row a person is watching does not
   move under them when it becomes real.

   A ticket's name is the agent alone: the rest of the name is the id, and this
   row does not have one yet — inventing a number here would be the one lie a
   placeholder must not tell, since the very next thing a person does with a row
   is match it against a terminal. What it says instead of an elapsed time is
   what it is doing, which is also why it needs no separate state: `running`
   already draws the live dot, and `starting` in the corner says the rest.

   The agent named is the configured one, and the worker may start another —
   `agents::pick` falls back to whatever is installed. It corrects itself the
   moment the session arrives, which is the same second or so this row exists
   for at all. */
export const agentRows = computed(() => [
  ...terminalState.sessions.map((session) => ({
    id: session.id,
    name: `${session.agent}-${session.id}`,
    state: toUiState(session),
    question: session.question,
    elapsed: formatElapsed(now.value - Date.parse(session.startedAt))
  })),
  ...visibleStarts().map((ticket) => ({
    id: ticket.id,
    name: ticket.agent,
    state: 'running',
    question: null,
    elapsed: 'starting',
    starting: true
  }))
])

/* Exactly one output subscriber exists at a time — the terminal view. A Set,
   not a single field, so unsubscribing never depends on who mounted last. */
const sinks = new Set()

export function subscribeOutput(cb) {
  sinks.add(cb)
  return () => sinks.delete(cb)
}

function push(bytes, meta = {}) {
  for (const sink of sinks) sink(bytes, meta)
}

function decode(base64) {
  const binary = atob(base64)
  const bytes = new Uint8Array(binary.length)
  for (let i = 0; i < binary.length; i += 1) bytes[i] = binary.charCodeAt(i)
  return bytes
}

/* Worker errors are diagnostics: their text speaks the worker's language and
   is meant for whoever fixes things, not whoever is waiting on a session.
   The interface gets a short explanation of what didn't work; the raw text
   stays in the console — the same split tracker.js makes between its read
   and write errors. Two kinds cover most of it here too: reading (list,
   attach, detach, resize) and writing (create, remove, write).

   The third entry is keyed on the kind the Rust error itself carries rather
   than on ours, and it earns the exception by being the one failure in this
   list a person can act on. "Nothing was created" is true of a machine with
   no agent installed and tells them nothing — and since filing a task is now
   an agent session rather than a write into the tracker, that is no longer a
   missing convenience but the only way to put a card on the board. It is a
   function because the names of the agents looked for belong to Rust:
   agents::IDS is the only copy of that list and the error carries it in its
   message, so nothing here has to hold a second one. */
const ERRORS = {
  read: {
    title: 'Could not read the terminal',
    description: 'The session list may be out of date. It will catch up on the next change.'
  },
  write: {
    title: 'Could not complete the action',
    description: 'Nothing was created, removed, or sent.'
  },
  noAgent: (looked) => ({
    title: 'No coding agent is installed',
    description: `Smetana looked for ${looked} on your PATH. It starts one to file a task and to edit an issue, so install one and try again.`
  })
}

function report(kind, error) {
  console.error(`[terminal] ${kind} failed:`, error)
  /* A cause the worker named wins over the generic pair, when this store has
     words for it. The typeof guard, rather than a plain lookup, is what keeps
     a future Rust kind called `read` or `write` from resolving to the generic
     entry's own object and being called with it. */
  const cause = ERRORS[error?.kind]
  terminalState.lastError = typeof cause === 'function' ? cause(error.message) : ERRORS[kind]
}

/* The number of the last chunk delivered for the active session. A gap means
   a lost event — then we take the truth whole, the way the tracker takes a
   snapshot on a generation gap. */
let seq = 0
let attaching = null
/* Ticket ids only have to be unique within this window's lifetime, and nothing
   ever reads the number. */
let tickets = 0

/* Answers whether the session ended up in the list, which is not the same as
   whether it exists: one belonging to another project is dropped on purpose,
   and the caller has to know that so it does not point the selection at a row
   nobody can see. */
function upsert(session) {
  if (terminalState.project && session.project !== terminalState.project) return false
  const index = terminalState.sessions.findIndex((s) => s.id === session.id)
  if (index === -1) {
    terminalState.sessions.push(session)
    terminalState.sessions.sort((a, b) => a.id - b.id)
  } else {
    terminalState.sessions[index] = session
  }
  return true
}

export async function initTerminals() {
  startClock()
  await listen('terminal:state', (event) => upsert(event.payload))
  await listen('terminal:output', (event) => {
    const { id, seq: next, data } = event.payload
    if (id !== terminalState.activeId) return
    if (next !== seq + 1) {
      // Fired from an event listener, not awaited by anyone: attach() no
      // longer throws (it reports instead), but the .catch() stays as a
      // second line of defence — a lost event must never surface as an
      // unhandled rejection, whatever else about this function changes.
      attach(id).catch(() => {})
      return
    }
    seq = next
    push(decode(data))
  })
  terminalState.ready = true
}

/* A project switch can start while an earlier call is still awaiting its
   invoke — a click on a different project row, or the activePath watcher
   firing again before the first call's response lands. The response that
   arrives second is not necessarily the one that was asked for second, so
   whichever call's request no longer matches terminalState.project when it
   wakes up has lost the race and must drop its result outright, not merge
   it: a stale response written into `sessions` while `project` already
   names the new project would map old session ids onto a different
   project's agents, and clicking a row's remove button would kill the
   wrong project's process with no error anywhere — the same class of loss
   `stale` guards against in the files layer. There is no way to tell which
   of a stale response's rows are still valid without asking again, and
   asking again is exactly what the next loadSessions call already does. */
export async function loadSessions(project) {
  terminalState.project = project
  /* What the list already held when the question went out. Anything that
     appeared while it was travelling is younger than the answer, and the
     answer is not entitled to speak about it — see below. */
  const asked = new Set(terminalState.sessions.map((s) => s.id))
  try {
    const sessions = project ? await invoke('terminal_list', { project }) : []
    if (terminalState.project !== project) return
    /* A session started while this request was in the air is not in the reply:
       the worker had not made it yet when it was asked. Replacing the list
       wholesale would drop that session and, through the selection repair
       below, the person's place in it too — an agent started seconds ago would
       vanish from the panel and the terminal would say "No agent selected",
       with nothing anywhere to say why and nothing due to arrive that would put
       it back. So a row that both is missing from the reply and was not there
       when the reply was asked for is carried over instead of dropped. A row
       that was there before the question and is not in the answer really is
       gone, and goes. */
    const late = terminalState.sessions.filter(
      (s) => !asked.has(s.id) && !sessions.some((fresh) => fresh.id === s.id)
    )
    terminalState.sessions = sessions
    for (const session of late) upsert(session)
    if (!selected()) {
      terminalState.activeId = terminalState.sessions.at(-1)?.id ?? null
    }
    terminalState.lastError = null
  } catch (err) {
    if (terminalState.project !== project) return
    report('read', err)
  }
}

/* Whether what `activeId` names is still something a person can be looking at:
   a session in the list, or a start that has not answered yet. A start counts —
   it is the newest thing in the panel and the reason the human is watching at
   all, and repairing the selection past it would hand the terminal back to some
   older agent one moment before the new one arrives. */
function selected() {
  const id = terminalState.activeId
  if (id == null) return false
  if (isStarting(id)) return visibleStarts().some((ticket) => ticket.id === id)
  return terminalState.sessions.some((s) => s.id === id)
}

/* The one write that still rejects: its caller turns a failed spawn into
   something the human sees, and an agent asked for that never appeared needs
   to say why — swallowing the error here would leave nothing to show.

   The intent, not a prompt: which words reach the agent depends on which agent
   it is, and that decision lives in Rust, in agents/. The store's job is to
   say what the session is for. */
export async function createSession(project, intent = { kind: 'bare' }) {
  const ticket = { id: `start-${(tickets += 1)}`, agent: settings.agent, project }
  terminalState.starting.push(ticket)
  /* Where the selection goes back to if nothing starts. Not "the last session"
     — that is a repair, and this is a person's own place in the panel, which a
     failed start has no business moving. */
  const before = terminalState.activeId
  terminalState.activeId = ticket.id
  try {
    const session = await invoke('terminal_create', { project, agent: settings.agent, intent })
    const kept = upsert(session)
    /* The handover, and the whole point of the ticket. Only if nobody has moved
       since: a person who picked another agent while this one was starting has
       said what they want to look at, and an answer arriving afterwards does not
       overrule them. `kept` is the other half — a project switch during the
       start leaves this session belonging to somewhere else, and pointing the
       selection at a row that is not in the panel would black the terminal out
       with no way back to it. */
    if (terminalState.activeId === ticket.id) terminalState.activeId = kept ? session.id : before
    terminalState.lastError = null
    return session
  } catch (err) {
    /* Nothing started, so nothing is drawn: the row goes with the ticket in the
       `finally` below and the selection goes back where it was. The caller keeps
       its dialog open on this rejection and the toast says why — an agent that
       never existed must not be left on screen looking startable. */
    if (terminalState.activeId === ticket.id) terminalState.activeId = before
    report('write', err)
    throw err
  } finally {
    terminalState.starting = terminalState.starting.filter((t) => t.id !== ticket.id)
  }
}

export async function removeSession(id) {
  try {
    await invoke('terminal_remove', { id })
    terminalState.sessions = terminalState.sessions.filter((s) => s.id !== id)
    if (terminalState.activeId === id) terminalState.activeId = terminalState.sessions.at(-1)?.id ?? null
    terminalState.lastError = null
  } catch (err) {
    report('write', err)
  }
}

/* Attaching hands back the whole ring, and the subscriber must repaint from
   scratch: whatever it was showing before is either another session's past,
   or a piece of this same session already folded into the snapshot. */
export async function attach(id) {
  terminalState.activeId = id
  /* A start has no ring, no seq and no id the worker would recognise: asking
     about it would come back as `no session` and report a failure at the one
     moment nothing has failed. The view attaches again on its own when the
     selection becomes a real id, which is what the ticket exists to lead to. */
  if (isStarting(id)) return
  const current = invoke('terminal_attach', { id })
  attaching = current
  try {
    const { data, seq: at } = await current
    if (attaching !== current) return
    seq = at
    push(decode(data), { reset: true })
    terminalState.lastError = null
  } catch (err) {
    // A newer attach already overtook this one; its outcome, not this
    // rejection, is what the store and the screen should reflect.
    if (attaching !== current) return
    report('read', err)
  }
}

/* activeId carries two different meanings, and detach is only allowed to
   touch one of them. It is "which agent the human selected" — that has to
   survive leaving the terminal tab, because the agent list highlights its
   row from this same field, and switching tabs must not un-pick it. It is
   also "which session the worker is currently streaming to this window",
   and that is what a view's unmount ends: the worker must stop pushing
   output nobody is listening to.
   The id argument is what keeps that stop from misfiring: switching agents
   is two separate IPC calls with no ordering guarantee at the worker, so a
   detach must name the session it is leaving. Without a name — or if this
   function cleared activeId unconditionally — the old session's detach
   arriving after the new session's attach would leave the worker with no
   active session, and output for the session the human is now looking at
   would silently stop arriving. No error, no event — the terminal just
   goes still. Selection is not the transport's to forget, though: it stays
   whatever it was, so the next mount can reattach to it. */
export async function detach(id) {
  if (id == null || isStarting(id)) return
  try {
    await invoke('terminal_detach', { id })
    terminalState.lastError = null
  } catch (err) {
    report('read', err)
  }
}

/* Typing into a pane whose agent has not started is dropped rather than queued,
   and that is the same answer `terminal_create` gives by handing the prompt over
   as an argument instead of writing it after the spawn: there is no input to
   write to yet, and bytes sent into one go nowhere with nothing to say they
   did. */
export async function send(id, data) {
  if (isStarting(id)) return
  try {
    await invoke('terminal_write', { id, data })
    terminalState.lastError = null
  } catch (err) {
    report('write', err)
  }
}

/* What to send is the profile's knowledge, not the panel's: one CLI wants a
   digit followed by a newline, another wants arrow keys and Enter. */
export function answer(id, option) {
  return send(id, option.send)
}

export async function resize(id, cols, rows) {
  if (isStarting(id)) return
  try {
    await invoke('terminal_resize', { id, cols, rows })
    terminalState.lastError = null
  } catch (err) {
    report('read', err)
  }
}
