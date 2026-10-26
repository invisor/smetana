/* The connection to the server, as the app draws it.

   One of the only files in `src/` that knows Tauri exists, and a store for the
   reason `app.js` and `updates.js` are: components must not import
   `@tauri-apps/api`, and this is the one thing in the app that is about the
   *absence* of the back end rather than its content.

   **Two builds, one shape.** The desktop shell is either the release, with the
   workers in its own process, or the Smetana Next build (`split`), which holds
   a connection to a `smetanad`. The first answers `connection_state` with
   `mode: 'in-process'` and a state that is always `connected`; so does
   `mockBackend.js` in a browser. Everything below reads `mode`, and nothing in
   the front end branches on which build it is.

   **What a state means for the rest of the app** (the design is
   `docs/superpowers/specs/2026-10-25-headless-server-design.md`, "Connection
   state is a first-class thing in the UI"):

   - Every store keeps the data it has. This file does not clear anything; the
     app draws the data dimmed and `components/shell/ConnectionBanner.vue`
     says why. That is "stale".
   - A write made while the connection is down is **refused by the shell** with
     a reason, which reaches the person through the store's own error path as
     any other refusal does. Nothing here queues it and nothing replays it.
   - When a connection is made, every store that registered with `onReconnect`
     reads its snapshot again. A new connection is a new `epoch`; the same one
     announced twice is not a second reason to read.

   The first `epoch` is the exception to "a new one resyncs": if the app mounted
   after the connection was already up, the stores' own first loads have just
   been answered and a second read would be waste. If it mounted while the
   connection was still being made, the first connection resyncs, because some
   load may have waited the shell's grace out and failed. */
import { computed, reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

/* `src-tauri/src/split/state.rs`'s `STATE_EVENT`, and the same string on both sides. */
export const STATE_EVENT = 'connection:state'

export const connection = reactive({
  mode: 'in-process',
  state: 'connected',
  epoch: 0,
  attempt: 0,
  reason: null,
  local: true,
  server: null,
  /* A local server older than the bundled one, and what replacing it would end. */
  outdated: null,
  /* A `when-idle` restart was accepted and is waiting for the work to finish. */
  restartScheduled: false,
  /* Whether the first answer has been heard: until then nothing is known. */
  ready: false
})

/* The data on screen is older than the server's truth. */
export const stale = computed(() => connection.mode === 'split' && connection.state !== 'connected')

const hooks = new Set()

/* Registers a store's snapshot read, to be run after every new connection. Answers the way to
   stop. A hook's failure is its own: it is logged and the others still run. */
export function onReconnect(hook) {
  hooks.add(hook)
  return () => hooks.delete(hook)
}

async function resync() {
  const results = await Promise.allSettled([...hooks].map((hook) => hook()))
  for (const result of results) {
    if (result.status === 'rejected') console.error('[connection] a store could not resync:', result.reason)
  }
}

let seenEpoch = 0
let heard = false

function adopt(view, { first = false } = {}) {
  if (!view || typeof view !== 'object' || typeof view.state !== 'string') return
  const mode = view.mode === 'split' ? 'split' : 'in-process'
  const epoch = Number.isFinite(view.epoch) ? view.epoch : 0
  /* A read and an event can land in either order, and a view older than the one held would put
     the previous connection's picture back. */
  if (connection.ready && mode === 'split' && epoch < connection.epoch) return
  const newEpoch = epoch !== connection.epoch
  connection.mode = mode
  connection.state = view.state
  connection.epoch = epoch
  connection.attempt = Number.isFinite(view.attempt) ? view.attempt : 0
  connection.reason = typeof view.reason === 'string' ? view.reason : null
  connection.local = view.local !== false
  connection.server = view.server && typeof view.server === 'object' ? view.server : null
  connection.outdated = view.outdated && typeof view.outdated === 'object' ? view.outdated : null
  if (newEpoch) connection.restartScheduled = false
  connection.ready = true
  if (first) {
    /* Mounted onto a connection that is already up: its loads were answered by it. */
    seenEpoch = mode === 'split' && view.state === 'connected' ? epoch : 0
    return
  }
  if (mode === 'split' && view.state === 'connected' && epoch !== seenEpoch) {
    seenEpoch = epoch
    resync()
  }
}

/* Subscribe, then ask — the order `updates.js` documents: an event that arrives before the read's
   answer is the newer of the two. Answers the way to stop listening. Neither half throws: in a
   browser there is no shell to ask, the state stays `in-process`, and nothing is drawn. */
export async function initConnection() {
  let stop = null
  try {
    stop = await listen(STATE_EVENT, (event) => {
      const first = !heard && !connection.ready
      heard = true
      adopt(event.payload, { first })
    })
  } catch (err) {
    console.debug('[connection] nobody to hear from (a browser, there is no Tauri):', err)
  }
  try {
    const view = await invoke('connection_state')
    if (!heard) adopt(view, { first: true })
  } catch (err) {
    console.debug('[connection] nobody to ask about the connection (a browser):', err)
  }
  connection.ready = true
  return stop
}

/* Try now, instead of at the end of the shell's wait. */
export async function retryConnection() {
  try {
    await invoke('connection_retry')
  } catch (err) {
    console.error('[connection] retry was refused:', err)
  }
}

/* Replace an older local server through the restart gateway. `now` carries the list the person was
   shown, which is the only thing that lets the server end live work, and it covers exactly that
   list; `when-idle` ends nothing and waits; `cancel` withdraws a wait. Answers the server's reply,
   or throws what it refused with. */
export async function restartServer(mode) {
  const confirmed = mode === 'now' ? (connection.outdated?.work ?? []) : null
  const reply = await invoke('server_restart', { mode, confirmed })
  if (reply?.outcome === 'scheduled') connection.restartScheduled = true
  if (reply?.outcome === 'cancelled') connection.restartScheduled = false
  if (reply?.outcome === 'needs-confirmation' && connection.outdated) {
    connection.outdated = { ...connection.outdated, work: reply.work ?? [] }
  }
  return reply
}
