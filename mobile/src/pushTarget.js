/* Where tapping a notification was meant to go.

   The core gets the target from the push (decrypted from the blob) and offers it twice: as the
   event `push:target` while the app is running, and through `take_push_target` for a tap that
   started the app. This holds the latest one and nothing more; turning it into a screen is
   ./nav.js (`routeForTarget`), and spending it is screens/Workspace.vue. */
import { reactive } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

export const pushTarget = reactive({ value: null })

let listening = false

export async function initPushTarget() {
  if (!listening) {
    listening = true
    await listen('push:target', (event) => {
      if (typeof event.payload === 'string') pushTarget.value = event.payload
    })
  }
  const pending = await invoke('take_push_target')
  if (typeof pending === 'string') pushTarget.value = pending
}

/* Called by a screen once it has acted on the target. */
export function clearPushTarget() {
  pushTarget.value = null
}
