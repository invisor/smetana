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

/* Questions a store asks that have an answer on this side of the link, so that they are
   never put to the desktop at all.

   Each is a question about the desktop's own machine or about what a window of the
   desktop remembers, and none is on the allowlist: `tracker_access_repair` asks what a
   refused folder needs (the answer is a button in the desktop's window), and
   `terminal_marks` is the rail's dots. The values are what the stores already treat as
   "nothing to say" - 'unavailable' is the form that offers nothing, an empty list draws
   no rows. (`terminal_restorable`, the offers to resume an agent, is the server's now.) */
const LOCAL = {
  tracker_access_repair: 'unavailable',
  terminal_marks: []
}

export function invoke(cmd, args = {}) {
  if (cmd in LOCAL) return Promise.resolve(structuredCloneOf(LOCAL[cmd]))
  return tauriInvoke('bridge_invoke', { cmd: ALIASES[cmd] ?? cmd, args })
}

/* A copy, so a store that pushes onto what it was given does not change the answer the
   next caller gets. JSON is enough: every value above is plain data. */
const structuredCloneOf = (value) => JSON.parse(JSON.stringify(value))

/* The rest of what the real module exports and the stores do not use is not
   re-exported on purpose: a store that starts to want, say, `Channel` should fail
   to build here rather than quietly get the desktop's. */
