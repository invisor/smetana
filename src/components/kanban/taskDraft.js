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

/* Whether this draft may be put back right now.

   Three conditions, and the last two are one worry seen from both sides: a
   project switch does not deliver a board instantly, and a window opened before
   its column exists is closed again by the very watcher that opened it, with a
   second notice about a window nobody ever saw.

   `columns` alone does not settle that, which is the part worth reading twice.
   The active project changes the moment somebody clicks a row, and the board
   answering for it lands a couple of seconds later — so in between, the columns
   on screen are the *previous* project's, and they will very often carry a
   `ready` of their own. Asking them is then asking the wrong board a question it
   is happy to answer. `boardArrived` is the caller's word for "what is on screen
   came back after the switch, not before it"; without it this rule reads a
   leftover as an arrival.

   `columns` is taken as either a list or a `Set`, and that is not generality
   for its own sake: the app window builds a `world` for `stalenessOf` forty
   lines away whose `columns` is a `Set`, and the shapes are close enough that
   the two will be offered to each other. A `Set` read as a list would answer
   "the board has no such column" for ever, silently, and no gate in this
   repository could see it. Answering both is a line; being wrong here is a
   window that never comes back and nobody knowing why. */
export function canRestore(draft, { project, columns, column, boardArrived }) {
  if (draftIsEmpty(draft)) return false
  if (!project || draft.project !== project) return false
  if (!boardArrived) return false
  if (columns instanceof Set) return columns.has(column)
  return Array.isArray(columns) && columns.includes(column)
}
