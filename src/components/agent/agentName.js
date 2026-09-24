/* A person's own name for an agent row, and where it wins.

   Pure, no Vue and no Tauri, for `agentOrder.js`'s reason: the rule has to be
   reachable by a test, and `AgentList.vue` is the one thing no test here can
   reach. Two rules and no more.

   `withAgentName` is the write: `settings.json`'s `agentNames` (per project,
   keyed by conversation id — `agentOrder.js` says why never by the worker's
   session number) with one entry changed. A new object rather than a mutation,
   because the caller assigns it back to the settings store and a mutation of
   the stored object would bypass the watch that saves it. An empty value
   removes the entry: a name of nothing is no name, and the automatic title
   shows again. No conversation id is a refusal, and the same map comes back
   untouched — the menu has already greyed the verb with `nothing to remember
   it by`, this is only the rule saying so where a test can see it.

   `nameAgentRows` is the read, applied once at the seam where the panel's
   rows are assembled (`orderedAgentRows` in `views/DesktopApp.vue`), after
   the merge and before the order: a row whose conversation has a name gets
   that name as its `label`, and nothing else on the row changes — the mono
   ids stay beside it, `title` stays what the worker said so a cleared name
   falls back to it. A row with no conversation, or one nobody named, is the
   very same object. */

export function withAgentName(names, conversation, value) {
  if (!conversation) return names
  const next = { ...(names ?? {}) }
  const name = typeof value === 'string' ? value.trim() : ''
  if (name) next[conversation] = name
  else delete next[conversation]
  return next
}

export function nameAgentRows(rows, names) {
  const map = names ?? {}
  return (rows ?? []).map((row) => {
    const name = row?.conversation ? map[row.conversation] : undefined
    return name ? { ...row, label: name } : row
  })
}
