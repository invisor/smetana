/* Which dialogs are OS windows of their own, how wide each one is, and the one
   rule that closes a window whose reason for being open has gone.

   Pure, with no Vue and no DOM in it, for the reason the rest of this family is
   pure: a `.vue` file is the one thing no test in this repository can reach, so
   the whole of a rule lives outside the component that draws it. The map from a
   kind to a component is therefore *not* here — it is in `DialogWindow.vue`,
   which is the only place that may import one.

   A dialog window stands on some ground: the project it belongs to, and often a
   task, a column or a branch besides. It is open because that ground exists, and
   when the ground goes the window has nothing left to be about. In the app
   window that never had to be said, because the scrim meant nothing could move
   underneath; a window somebody can push aside and click past has no such
   promise. */

/* The seven, and the width each one's window gets. Every one of them drew at
   `Modal`'s default 440 as a modal, and there is no reason for that to change
   just because the frame did — the numbers are here so that a dialog which
   outgrows it has somewhere to say so. */
const REGISTRY = {
  run: { width: 440, ground: ['project'] },
  'new-task': { width: 440, ground: ['project', 'column'] },
  'new-branch': { width: 440, ground: ['project', 'branch'] },
  'promote-column': { width: 440, ground: ['project', 'column'] },
  'setup-project': { width: 440, ground: ['project'] },
  'delete-task': { width: 440, ground: ['project', 'issue'] },
  'ready-task': { width: 440, ground: ['project', 'issue'] }
}

export const DIALOG_KINDS = Object.keys(REGISTRY)

export function isDialogKind(kind) {
  return Object.prototype.hasOwnProperty.call(REGISTRY, kind)
}

export function dialogWidth(kind) {
  return REGISTRY[kind]?.width ?? 440
}

export function dialogGround(kind) {
  return REGISTRY[kind]?.ground ?? ['project']
}

/* What each kind is called in a sentence, and what each sort of ground is called
   when it goes. Both are here rather than in the view for the same reason the
   rule is: this is copy a test can read.

   Sentence case, like everything else on screen. */
const DIALOG_NOUN = {
  run: 'run',
  'new-task': 'new task',
  'new-branch': 'new branch',
  'promote-column': 'promote column',
  'setup-project': 'project setup',
  'delete-task': 'delete',
  'ready-task': 'move to ready'
}

const REASON_CLAUSE = {
  project: 'the project changed',
  issue: 'the task it was about no longer exists',
  column: 'the column it was about is no longer on the board',
  branch: 'the branch it started from is gone'
}

export function stalenessMessage(kind, reason) {
  const noun = DIALOG_NOUN[kind] ?? 'dialog'
  const clause = REASON_CLAUSE[reason] ?? 'what it was about is gone'
  return `The ${noun} dialog closed: ${clause}.`
}

/* Which of the open dialog windows have lost their ground.
   `world` is what the app window holds right now:
   `{ project, issues: Set, columns: Set, branches: Set }`. */
export function staleDialogs(open, world) {
  return open
    .filter(({ kind, ground }) => Boolean(stalenessOf(kind, ground, world)))
    .map((dialog) => dialog.kind)
}

/* Why one dialog is stale, or null if it is not. Exported because the caller
   needs the reason for the sentence, and computing it twice would be two rules
   to keep in step.

   The project is checked first and on its own, because a project that has moved
   invalidates every other kind of ground at once — the ids in a different
   tracker are a different vocabulary, not a smaller one.

   The other three are checked only when the ground actually names one. A field
   that is absent is not a field that went: a dialog standing on the project
   alone would otherwise be closed by every world whose set of columns happens
   not to hold `undefined`. */
export function stalenessOf(kind, ground, world) {
  const needs = dialogGround(kind)
  if (needs.includes('project') && ground.project !== world.project) return 'project'
  if (needs.includes('issue') && ground.issue != null && !world.issues.has(ground.issue)) {
    return 'issue'
  }
  if (needs.includes('column') && ground.column != null && !world.columns.has(ground.column)) {
    return 'column'
  }
  if (needs.includes('branch') && ground.branch != null && !world.branches.has(ground.branch)) {
    return 'branch'
  }
  return null
}
