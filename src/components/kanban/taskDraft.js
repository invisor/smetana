/* A task somebody has started writing and not filed, and when one may be put
   back on screen.

   Pure, with no Vue and no DOM in it, for the reason the rest of this family is
   pure: a `.vue` file is the one thing no test in this repository can reach, so
   the whole of the rule lives outside the component that draws it.

   A draft is one record, and every field of it is a field `NewTaskModal`
   already sends when the task is filed:

     { project, text, issue_type, priority, brainstorm, spec, plan,
       images: [absolute path], parent: { id, title } | null }

   `project` is stamped on by the app window — the path the window was opened
   for, not whichever project is in front of somebody when the draft comes
   back. `parent` carries the title as well as the id, unlike the one `submit`
   sends: the dialog draws it, and the app window is the side that has it. */

/* Nothing typed and nothing attached. An untouched window that was opened and
   switched away from is not a loss, and reopening one would be noise. */
export function draftIsEmpty(draft) {
  if (!draft) return true
  const typed = typeof draft.text === 'string' ? draft.text.trim() : ''
  return typed.length === 0 && !(draft.images?.length > 0)
}

/* Whether this draft may be put back right now. Two conditions, and the second
   is the one that is easy to miss: a project switch does not deliver a board
   instantly, and a window opened before its column exists is closed again by
   the very watcher that opened it, with a second notice about a window nobody
   ever saw. */
export function canRestore(draft, { project, columns, column }) {
  if (draftIsEmpty(draft)) return false
  if (!project || draft.project !== project) return false
  return Array.isArray(columns) && columns.includes(column)
}
