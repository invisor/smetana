/* `invoke` for the stores in ../src/stores/, as the phone has it: the call
   leaves for the desktop through the Rust core's `bridge_invoke` and comes back
   as the desktop's own answer. See vite.config.js for how a store ends up here.

   This is the one file that imports the real `@tauri-apps/api/core`; the swap
   in the Vite config is limited to importers under src/stores/, which is what
   keeps this import from resolving to itself. */
import { invoke as tauriInvoke } from '@tauri-apps/api/core'

/* A few commands a store calls that are not on the bridge's allowlist, by the
   allowlisted command that answers the same question.

   `tracker_resync` asks the desktop to re-read bd from disk and answers with a
   snapshot. A phone has no business making the desktop do that, and the
   allowlist refuses it; `tracker_snapshot` returns the same shape from what the
   desktop already holds, which is what a phone that reconnected wants. The
   store keeps calling the name it knows. */
const ALIASES = { tracker_resync: 'tracker_snapshot' }

export function invoke(cmd, args = {}) {
  return tauriInvoke('bridge_invoke', { cmd: ALIASES[cmd] ?? cmd, args })
}

/* The rest of what the real module exports and the stores do not use is not
   re-exported on purpose: a store that starts to want, say, `Channel` should fail
   to build here rather than quietly get the desktop's. */
