/* `listen` and `emit` for the stores in ../src/stores/, as the phone has them.

   Events from the desktop arrive as one Tauri event, `bridge:event`, carrying
   `{ name, payload }`; this file fans it out to whoever listened to `name`, and
   tells the Rust core which names to subscribe to (`bridge_listen`). The core
   remembers them and subscribes again after a reconnect, so a store never has to.

   A name the desktop does not offer (a store's own cross-window event, say) is
   refused by the core; it is then a local event: `emit` reaches its listeners
   in this page and nothing else, which is all a one-window app has. */
import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen as tauriListen } from '@tauri-apps/api/event'
import { isAllowedEvent } from '../allowed.js'

export const BRIDGE_EVENT = 'bridge:event'

const handlers = new Map()
const local = new Set()
let pump = null

function dispatch(name, payload) {
  for (const handler of [...(handlers.get(name) ?? [])]) {
    handler({ event: name, id: 0, payload })
  }
}

function ensurePump() {
  pump ??= tauriListen(BRIDGE_EVENT, (event) => {
    const message = event.payload
    if (message && typeof message.name === 'string') {
      dispatch(message.name, message.payload)
    }
  })
  return pump
}

export async function listen(name, handler) {
  await ensurePump()
  let set = handlers.get(name)
  if (!set) {
    set = new Set()
    handlers.set(name, set)
  }
  set.add(handler)
  /* A name off the allowlist is a local event without asking the core: the core would
     refuse it, and there is no reason to put the question. */
  if (set.size === 1 && !isAllowedEvent(name)) local.add(name)
  if (set.size === 1 && !local.has(name)) {
    try {
      await tauriInvoke('bridge_listen', { event: name })
    } catch {
      local.add(name)
    }
  }
  return async () => {
    set.delete(handler)
    if (set.size === 0) {
      handlers.delete(name)
      if (!local.has(name)) {
        try {
          await tauriInvoke('bridge_unlisten', { event: name })
        } catch {
          // The core forgets a subscription when the connection goes anyway.
        }
      }
    }
  }
}

export async function once(name, handler) {
  let off = null
  const wrapped = (event) => {
    off?.()
    handler(event)
  }
  off = await listen(name, wrapped)
  return off
}

export async function emit(name, payload) {
  dispatch(name, payload)
}

/* Forgets every listener. Tests rebuild the module graph instead; this is for
   a page that is being torn down. */
export function reset() {
  handlers.clear()
  local.clear()
  pump = null
}
