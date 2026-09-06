/* What the app knows about the harnesses it can run: their names, what each of
   them can be asked to do, and what each may be run on. The single copy of that is Rust's — `agents::IDS`
   and the `Profile` methods behind `agents::catalogue` — and this store is how
   it reaches a row being drawn.

   Read **once at startup** and never again. That is the property four
   hand-written lists in this tree were keeping — one of agent labels in
   `settings/AgentSettings.vue`, two of ids that resume and fork in
   `agent/sessionMenu.js`, one of ids that clear in `agent/agentMenu.js`. Each
   existed because the answer has to be known while a menu row is drawn, and a
   row greyed a round trip later is a row somebody has already pressed. Nothing
   here changes while the app runs — the set of shipped harnesses is fixed at
   build time — so one read is the whole of it.

   A read that fails leaves the list empty, and an empty list greys every row
   that depends on a capability. That is the safe direction: Rust refuses an
   unsupported verb with a sentence anyway (`NoResume`, `NoFork`, `NoClear`), so
   being wrong here costs a row nobody can press rather than a command written
   into somebody's prompt. */
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export const agents = ref([])

export async function initAgents() {
  try {
    const rows = await invoke('agents_catalog')
    agents.value = Array.isArray(rows) ? rows : []
  } catch {
    agents.value = []
  }
}

/* Whether this harness can be asked for this verb — one of `resume`, `fork`,
   `clear`, `usage`, `batch`, `oneshot`, which are the fields
   `agents::Capabilities` carries.

   An id nobody ships — a hand-edited `settings.json`, or nothing configured at
   all — answers `false` for everything, which is the answer the greying already
   gave it. */
export function can(id, capability) {
  const row = agents.value.find((agent) => agent.id === id)
  return Boolean(row?.capabilities?.[capability])
}

/* The label a person reads, or the raw id when the catalogue has never heard of
   it: naming an unknown harness as one of ours would be the app claiming
   something it does not know.

   There is deliberately **no `modelsOf` beside this**, though a row carries its
   models too. `agentLabel` is here because two unrelated callers want it and
   neither has the row in hand; the model list has exactly one reader — the
   Models group's `modelOptions` in `components/settings/agentRoles.js` — which
   is handed `agents.value` whole and does the lookup there. Adding the wrapper
   would have put "the models of this harness id" in two places, and the tested
   copy would have been the one nothing runs. What must not happen instead is
   that rule reaching in here: `agentRoles.js` is one of the pure modules, and
   importing this store would pull Vue and Tauri into a family defined by having
   neither. */
export function agentLabel(id) {
  return agents.value.find((agent) => agent.id === id)?.label ?? id
}
