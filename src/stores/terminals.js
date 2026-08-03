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
  project: null
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

export function formatElapsed(ms) {
  const minutes = Math.floor(ms / 60000)
  const hours = Math.floor(minutes / 60)
  return hours ? `${hours}h ${String(minutes % 60).padStart(2, '0')}m` : `${minutes}m`
}

/* Ticks once every thirty seconds: the time in an agent's row is measured in
   tens of minutes, and second-level precision would serve nobody there. */
const now = ref(Date.now())
setInterval(() => (now.value = Date.now()), 30000)

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
  await listen('terminal:state', (event) => upsert(event.payload))
  await listen('terminal:output', (event) => {
    const { id, seq: next, data } = event.payload
    if (id !== terminalState.activeId) return
    if (next !== seq + 1) {
      attach(id)
      return
    }
    seq = next
    push(decode(data))
  })
  terminalState.ready = true
}

export async function loadSessions(project) {
  terminalState.project = project
  terminalState.sessions = project ? await invoke('terminal_list', { project }) : []
  if (!terminalState.sessions.some((s) => s.id === terminalState.activeId)) {
    terminalState.activeId = terminalState.sessions.at(-1)?.id ?? null
  }
}

export async function createSession(project) {
  const session = await invoke('terminal_create', { project })
  upsert(session)
  terminalState.activeId = session.id
  return session
}

export async function removeSession(id) {
  await invoke('terminal_remove', { id })
  terminalState.sessions = terminalState.sessions.filter((s) => s.id !== id)
  if (terminalState.activeId === id) terminalState.activeId = terminalState.sessions.at(-1)?.id ?? null
}

/* Attaching hands back the whole ring, and the subscriber must repaint from
   scratch: whatever it was showing before is either another session's past,
   or a piece of this same session already folded into the snapshot. */
export async function attach(id) {
  terminalState.activeId = id
  const current = invoke('terminal_attach', { id })
  attaching = current
  const { data, seq: at } = await current
  if (attaching !== current) return
  seq = at
  push(decode(data), { reset: true })
}

/* Detach names the session it is leaving. Without a name it would clear the
   pointer unconditionally, and switching agents is two separate IPC calls
   with no ordering guarantee at the worker: the old session's detach
   arriving after the new session's attach would leave the worker with no
   active session, and output for the session the human is looking at would
   silently stop arriving. No error, no event — the terminal just goes
   still. */
export async function detach(id) {
  if (id == null) return
  if (terminalState.activeId === id) terminalState.activeId = null
  await invoke('terminal_detach', { id })
}

export function send(id, data) {
  return invoke('terminal_write', { id, data })
}

/* What to send is the profile's knowledge, not the panel's: one CLI wants a
   digit followed by a newline, another wants arrow keys and Enter. */
export function answer(id, option) {
  return send(id, option.send)
}

export function resize(id, cols, rows) {
  return invoke('terminal_resize', { id, cols, rows })
}
