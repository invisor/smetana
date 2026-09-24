/* The visible Crew hierarchy. Provider ids never reach this module: rows are
   already Smetana-owned ids from the backend, which keeps Vue keys stable while
   a provider reconciles a child or reports it finished.

   This is deliberately separate from agentOrder.js. A Crew tree describes
   containment and live routing, while the ordinary Agents list describes a
   person's flat, persisted arrangement; applying the latter to children would
   let a drag contradict a provider's parentage. */

const terminal = (state) => state === 'done' || state === 'failed'

export function canMessage(node) {
  return Boolean(node?.canMessage) && !terminal(node?.state)
}

/* Parent-before-child depth-first rows. Orphans and cycles are made visible at
   the root instead of disappearing: an incomplete provider update must never
   turn a live agent into an unreachable composer target. */
export function crewRows(nodes) {
  const list = Array.isArray(nodes) ? nodes.filter((node) => node?.id != null) : []
  const byId = new Map(list.map((node) => [node.id, node]))
  const children = new Map()
  const roots = []
  for (const node of list) {
    if (node.parent == null || !byId.has(node.parent) || node.parent === node.id) roots.push(node)
    else {
      const group = children.get(node.parent) ?? []
      group.push(node)
      children.set(node.parent, group)
    }
  }
  const seen = new Set()
  const out = []
  const visit = (node, depth) => {
    if (seen.has(node.id)) return
    seen.add(node.id)
    out.push({ ...node, depth, canMessage: canMessage(node) })
    for (const child of children.get(node.id) ?? []) visit(child, depth + 1)
  }
  for (const root of roots) visit(root, 0)
  // A cycle has no root. Preserve it at the end rather than pretending it did
  // not exist; the next structured update repairs parentage in the backend.
  for (const node of list) visit(node, 0)
  return out
}
