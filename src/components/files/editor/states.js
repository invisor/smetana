/* The editor's state per tab: the document, the caret, the edit history and
   the scroll position. A plain Map, deliberately outside Vue's reactivity —
   reactive() would wrap the EditorState in a Proxy, and CodeMirror compares its
   objects by identity, so a substituted object would break its transactions.

   The scroll position is stored as a separate number: EditorState does not hold
   it — it is a property of the DOM, not of the document.

   The price of this decision is a third copy of the text in memory per open tab
   (tabs.js already holds text and original). It is accepted knowingly: a code
   editor cannot lose the edit history on every tab switch. */
const states = new Map()

/* Named peek, not take: the entry survives being read. The same path is read
   twice per switch — first by the watcher in FileEditor.vue, then, if the state
   goes back into the cache, by the next transition — and both need the same
   entry, not what was left of it after the first read. */
export function peekState(path) {
  return states.get(path) ?? null
}

export function putState(path, state, scrollTop) {
  states.set(path, { state, scrollTop })
}

/* The cleanup follows the tab list rather than a close event: that way one
   rule covers closing a tab, switching project, and a path that fell out of the
   list because the file no longer reads. */
export function keepOnly(paths) {
  const live = new Set(paths)
  for (const path of states.keys()) {
    if (!live.has(path)) states.delete(path)
  }
}
