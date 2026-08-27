/* What the whole-column promote is called, at each of the three moments it has.

   The `columnOrder.js` / `taskMenu.js` / `parked.js` family: pure, with no Vue
   and no DOM in it, and a file of its own for the reason that family exists —
   a `.vue` file is the one thing no test in this repository can reach, so a
   rule left inside the component that draws it is a rule nothing checks.

   It is out here rather than inside `PromoteColumnModal.vue` because two
   places need the same sentence and neither may guess at the other's. The
   dialog draws it as its heading; the app window announces it to the OS frame
   of the window the dialog lives in (`DesktopApp.vue`, `openPromote`), because
   the frame is the desktop's and nothing on the window's side of the wire knows
   what a dialog is called. Written twice, the pluralisation and the branch
   below would be two rules to keep in step, and the frame would be the one to
   go quietly wrong. */

/* Plural by count, and exported because the dialog's body needs the same
   phrase for the failures it reports. */
export function taskCount(n) {
  return `${n} ${n === 1 ? 'task' : 'tasks'}`
}

/* The question stops being a question once it has been answered: what is left
   to say by then is what happened, and a title still asking whether to move
   them reads as though nothing had.

   `failed` is `null` while nothing has been attempted, which is what separates
   the two halves — a zero is a run that lost none and is a report like any
   other. */
export function promoteTitle({ count = 0, moved = 0, failed = null } = {}) {
  if (failed != null) return `Moved ${moved} of ${count}`
  return `Move ${taskCount(count)} to ready?`
}
