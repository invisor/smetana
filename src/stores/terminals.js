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

export const terminalState = reactive({
  sessions: [],
  activeId: null,
  ready: false,
  project: null,
  lastError: null
})

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

export const agentRows = computed(() =>
  terminalState.sessions.map((session) => ({
    id: session.id,
    name: `${session.agent}-${session.id}`,
    state: toUiState(session),
    question: session.question,
    elapsed: formatElapsed(now.value - Date.parse(session.startedAt))
  }))
)

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
   and write errors. Two kinds are enough here too: reading (list, attach,
   detach, resize) and writing (create, remove, write). */
const ERRORS = {
  read: {
    title: 'Could not read the terminal',
    description: 'The session list may be out of date. It will catch up on the next change.'
  },
  write: {
    title: 'Could not complete the action',
    description: 'Nothing was created, removed, or sent.'
  }
}

function report(kind, error) {
  console.error(`[terminal] ${kind} failed:`, error)
  terminalState.lastError = ERRORS[kind]
}

/* The number of the last chunk delivered for the active session. A gap means
   a lost event — then we take the truth whole, the way the tracker takes a
   snapshot on a generation gap. */
let seq = 0
let attaching = null

function upsert(session) {
  if (terminalState.project && session.project !== terminalState.project) return
  const index = terminalState.sessions.findIndex((s) => s.id === session.id)
  if (index === -1) {
    terminalState.sessions.push(session)
    terminalState.sessions.sort((a, b) => a.id - b.id)
  } else {
    terminalState.sessions[index] = session
  }
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
  try {
    const sessions = project ? await invoke('terminal_list', { project }) : []
    if (terminalState.project !== project) return
    terminalState.sessions = sessions
    if (!terminalState.sessions.some((s) => s.id === terminalState.activeId)) {
      terminalState.activeId = terminalState.sessions.at(-1)?.id ?? null
    }
    terminalState.lastError = null
  } catch (err) {
    if (terminalState.project !== project) return
    report('read', err)
  }
}

/* The one write that still rejects: its caller is a later task turning a
   failed spawn into something the human sees, and an agent asked for that
   never appeared needs to say why — swallowing the error here would leave
   nothing to show. */
export async function createSession(project) {
  try {
    const session = await invoke('terminal_create', { project })
    upsert(session)
    terminalState.activeId = session.id
    terminalState.lastError = null
    return session
  } catch (err) {
    report('write', err)
    throw err
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
  if (id == null) return
  try {
    await invoke('terminal_detach', { id })
    terminalState.lastError = null
  } catch (err) {
    report('read', err)
  }
}

export async function send(id, data) {
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
  try {
    await invoke('terminal_resize', { id, cols, rows })
    terminalState.lastError = null
  } catch (err) {
    report('read', err)
  }
}
