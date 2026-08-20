/* What order the cards inside a column sit in — which today is a question about
   exactly one column, done.

   The same shape as `columnOrder.js` and `boardView.js` beside it, and pulled
   out for the same reason: pure, no Vue and no DOM, which is what makes it the
   one part of the ordering a test can reach. `stores/tracker.js` applies it in
   `boardColumns`, so the board and everything else reading that computed get
   one order rather than each arranging the cards for itself.

   Every other column keeps the order it has, which is the order the store's
   `Map` yields and nothing more. That is deliberate rather than unfinished: the
   question the done column answers — what was finished most recently — has no
   counterpart in ready or running, where a person's own priorities decide and
   the tracker has no date that stands for them. */

/* The design system's word, not bd's: `boardColumns` has already translated
   `closed` through `toUiStatus` by the time a column is handed here. */
const DONE = 'done'

/* When this card was finished, as a number to sort on.

   `closedAt` is the answer to the question the column asks. `updatedAt` stands
   in when it cannot be read, because the field is optional in the model
   (`src-tauri/src/tracker/model.rs`) and a rule with a hole in it would leave a
   card wherever the `Map` happened to put it. `null` is neither being readable,
   and it sends the card to the bottom rather than off the board — losing a card
   over an unreadable date costs more than drawing it out of place. */
function finishedAt(task) {
  const closed = Date.parse(task?.closedAt ?? '')
  if (!Number.isNaN(closed)) return closed
  const updated = Date.parse(task?.updatedAt ?? '')
  return Number.isNaN(updated) ? null : updated
}

/* Ties break on the id, ascending, and the stability of
   `Array.prototype.sort` is not enough on its own here: a batch merge closes
   several tasks within one second, and the incoming order is the `Map`'s, which
   is the snapshot's order after a `tracker_resync` and the order things were
   upserted in during a session. Two cards would swap places by themselves
   between one and the other. The id decides nothing anybody cares about — it is
   chosen for being the same every time. */
function byFinished(a, b) {
  const left = finishedAt(a)
  const right = finishedAt(b)
  if (left !== right) {
    if (left === null) return 1
    if (right === null) return -1
    return right - left
  }
  const leftId = String(a?.id ?? '')
  const rightId = String(b?.id ?? '')
  return leftId < rightId ? -1 : leftId > rightId ? 1 : 0
}

/* One column's cards, in the order they are drawn. Anything but done gets back
   the very array it came in with, by reference — the same identity
   `moveColumn` keeps, and for a caller the cheapest possible "nothing
   happened". */
export function orderCards(status, tasks) {
  if (status !== DONE || !Array.isArray(tasks)) return tasks
  return [...tasks].sort(byFinished)
}
