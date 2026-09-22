/* The words unique to an agent-failover boundary in `RunBar.vue`.

   This is deliberately a small pure module: the footer component is not
   directly mounted in the unit suite, while these states must not quietly
   regress into the generic pause wording. */

export function failoverLabel(state) {
  switch (state?.kind) {
    case 'waiting_for_agent':
      return `Waiting for ${state.agent} (${state.pct}%)`
    case 'waiting_for_any_agent':
      return 'Waiting for any available agent'
    case 'switching_agent':
      return `Switching from ${state.from} to ${state.to}`
    default:
      return null
  }
}

export function failoverDetail(state) {
  if (!['waiting_for_agent', 'waiting_for_any_agent'].includes(state?.kind)) return null
  return state.resets ? `resets ${state.resets}` : 're-checking every minute'
}
