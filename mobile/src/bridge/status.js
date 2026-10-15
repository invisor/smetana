/* What the connection to the desktop is doing, as the Rust core reports it in
   `bridge:status`, and what the page does when that changes.

   Plain module, no Tauri import beyond the two calls it makes: the decision of
   what a status means (`isOnline`, `blocksCalls`) is in ./states.js, where a
   test can reach it. */
import { reactive } from 'vue'
import { invoke as tauriInvoke } from '@tauri-apps/api/core'
import { listen as tauriListen } from '@tauri-apps/api/event'
import { isOnline } from './states.js'

export const STATUS_EVENT = 'bridge:status'

export const bridge = reactive({
  paired: null,
  status: { state: 'connecting' },
  version: '',
  dismissedRecommended: false
})

const onlineHooks = []

/* Called every time the connection becomes usable: the first time and after
   every reconnect, with `again` true from the second on. What it is for is
   reading the snapshots: missed deltas are not replayed, so a phone that came
   back reads everything the stores hold again. */
export function whenOnline(hook) {
  onlineHooks.push(hook)
}

let everOnline = false

function apply(status) {
  const was = isOnline(bridge.status)
  bridge.status = status
  if (isOnline(status) && !was) {
    const again = everOnline
    everOnline = true
    for (const hook of onlineHooks) {
      Promise.resolve(hook({ again })).catch((err) => console.warn('[bridge] an online hook failed:', err))
    }
  }
}

export function applyStatus(status) {
  apply(status)
}

/* Reads what the core knows and subscribes to what it will say. Subscribing is
   first, so a change between the two cannot be lost; the read then only fills
   in what happened before. */
export async function initStatus() {
  await tauriListen(STATUS_EVENT, (event) => apply(event.payload))
  const now = await tauriInvoke('bridge_state')
  bridge.paired = now.paired
  bridge.version = now.version
  if (now.paired) apply(now.status)
}

export async function pair(ticket) {
  await tauriInvoke('pair', { ticket })
  bridge.paired = true
  await connect()
}

export async function connect() {
  apply({ state: 'connecting' })
  await tauriInvoke('connect')
}

export async function unpair() {
  await tauriInvoke('unpair')
  bridge.paired = false
  bridge.status = { state: 'connecting' }
}

export function dismissRecommended() {
  bridge.dismissedRecommended = true
}
