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
  /* Which server: `local`, or a remote one's key. See `onServerChange`. */
  target: 'local',
  server: null,
  /* A local server older than the bundled one, and what replacing it would end. */
  outdated: null,
  /* A `when-idle` restart was accepted and is waiting for the work to finish. */
  restartScheduled: false,
  /* Whether the first answer has been heard: until then nothing is known. */
  ready: false
})

/* The server is on another machine. Paths are then the server's, not this machine's: the native
   folder dialog, the file manager and the system's "open this file" are the wrong tools for them
   (`serverFiles.js` is what the front end does instead). `local` defaults to true, and so does every
   state that is not a connection to a server at all, so the build with its back end in-process and
   a browser never read as remote. */
export const isRemote = computed(() => connection.mode === 'split' && connection.local === false)

/* The data on screen is older than the server's truth. */
export const stale = computed(() => connection.mode === 'split' && connection.state !== 'connected')

const hooks = new Set()

/* Registers a store's snapshot read, to be run after every new connection. Answers the way to
   stop. A hook's failure is its own: it is logged and the others still run. */
export function onReconnect(hook) {
  hooks.add(hook)
  return () => hooks.delete(hook)
}

const serverHooks = new Set()

/* Registers what must happen when the connection is to a *different server* than the one the data on
   screen came from (the person chose another in Settings), before the stores read their snapshots
   again. The project list, the open project and the settings a window holds belong to the server they
   were read from: carried over, a path on this machine would be written into the new server's project
   list, and a board of the new server would be drawn under the old one's project. A hook takes the
   new server's own. Hooks run one after another and before the ordinary resync; a failing one is
   logged and the rest still run. Answers the way to stop. */
export function onServerChange(hook) {
  serverHooks.add(hook)
  return () => serverHooks.delete(hook)
}

async function changeServer() {
  for (const hook of [...serverHooks]) {
    try {
      await hook()
    } catch (err) {
      console.error('[connection] a store could not take the new server in:', err)
    }
  }
}

async function resync() {
  const results = await Promise.allSettled([...hooks].map((hook) => hook()))
  for (const result of results) {
    if (result.status === 'rejected') console.error('[connection] a store could not resync:', result.reason)
  }
}

let seenEpoch = 0
let heard = false
/* The server the last connection that was acted on was to; null until there is one. */
let servedTarget = null

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
  connection.target = typeof view.target === 'string' ? view.target : 'local'
  connection.server = view.server && typeof view.server === 'object' ? view.server : null
  connection.outdated = view.outdated && typeof view.outdated === 'object' ? view.outdated : null
  if (newEpoch) connection.restartScheduled = false
  connection.ready = true
  if (first) {
    /* Mounted onto a connection that is already up: its loads were answered by it. */
    seenEpoch = mode === 'split' && view.state === 'connected' ? epoch : 0
    if (seenEpoch) servedTarget = connection.target
    return
  }
  if (mode === 'split' && view.state === 'connected' && epoch !== seenEpoch) {
    seenEpoch = epoch
    const changed = servedTarget !== null && servedTarget !== connection.target
    servedTarget = connection.target
    if (changed) changeServer().then(resync)
    else resync()
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

/* ---- which server this device uses ---------------------------------------- */

/* `{ kind: 'local' }`, `{ kind: 'remote', node, relay }`, or `{ kind: 'in-process' }` in the
   build that has no choice to make. A browser has nobody to ask and is in-process. */
export async function readServerTarget() {
  try {
    const answer = await invoke('server_target')
    return answer?.target ?? { kind: 'in-process' }
  } catch (err) {
    console.debug('[connection] nobody to ask which server this is (a browser):', err)
    return { kind: 'in-process' }
  }
}

/* What the server on this machine is doing that moving away from it would leave running: the list
   of live work, empty when there is none or when this device is not using it. */
export async function localServerWork() {
  try {
    const work = await invoke('server_local_work')
    return Array.isArray(work) ? work : []
  } catch (err) {
    console.debug('[connection] the local server was not asked about its work:', err)
    return []
  }
}

/* Back to the server on this machine. Rejects with the shell's words. */
export async function useLocalServer() {
  try {
    await invoke('server_use_local')
  } catch (err) {
    throw new Error(refusalOf(err))
  }
}

/* Pairs with the server a desktop ticket names and moves this device to it. `stopLocal` asks the
   local server to stop once its work is done (it ends nothing); without it the local server is left
   running. Answers `{ target, localStop }`; rejects with the shell's words, and a rejection means the
   device is where it was. The pairing waits for the person to confirm at the server, so this can
   take a minute or two. */
export async function useRemoteServer(ticket, stopLocal) {
  try {
    return await invoke('server_use_remote', { ticket, stopLocal: Boolean(stopLocal) })
  } catch (err) {
    throw new Error(refusalOf(err))
  }
}

/* An error as text: Tauri rejects with the command's own string, and an `Error` has its message. */
export function refusalOf(err) {
  if (typeof err === 'string') return err
  if (err && typeof err === 'object' && typeof err.message === 'string') return err.message
  return String(err)
}
