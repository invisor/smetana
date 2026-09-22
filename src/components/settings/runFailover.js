/* The global run-failover choices. Rust validates the saved copy in
   `settings/model.rs`; these small pure rules keep an edit coherent before the
   next settings announcement returns from the main window. */
export const WAIT_MINUTES = [0, 1, 5, 10, 15, 30]
export const DEFAULT_WAIT_MINUTES = 5

export function waitOptions() {
  return WAIT_MINUTES.map((value) => ({
    value,
    label: value === 0 ? 'Immediately' : `${value} minute${value === 1 ? '' : 's'}`
  }))
}

export function isWaitMinutes(value) {
  return typeof value === 'number' && WAIT_MINUTES.includes(value)
}

/* Keep all supported agents visible, including ones not installed. The store
   knows the catalogue dynamically, so an older or malformed event cannot
   remove an agent the current build supports. */
export function priorityOrder(priority, agentIds) {
  const known = new Set(agentIds)
  const seen = new Set()
  const ordered = Array.isArray(priority)
    ? priority.filter((id) => typeof id === 'string' && known.has(id) && !seen.has(id) && seen.add(id))
    : []
  for (const id of agentIds) {
    if (!seen.has(id)) ordered.push(id)
  }
  return ordered
}

export function movePriority(priority, agentIds, id, direction) {
  const ordered = priorityOrder(priority, agentIds)
  const from = ordered.indexOf(id)
  const to = from + direction
  if (from < 0 || to < 0 || to >= ordered.length) return ordered
  ;[ordered[from], ordered[to]] = [ordered[to], ordered[from]]
  return ordered
}
