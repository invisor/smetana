/* Whether starting a run over one task has to put that task into Ready first,
   and the sentence saying so before anybody presses the button.

   The `runScopes.js` / `branchChoice.js` / `promoteTitle.js` family: pure, with
   no Vue and no DOM in it, and a file of its own for that family's reason — a
   `.vue` file is the one thing no test in this repository can reach, so a rule
   left inside the component that draws it is a rule nothing checks. Three
   readers here, which is the other half of why the words are not in the dialog:
   `DesktopApp.vue` asks whether to write the status before `startRun`,
   `RunModal.vue` asks what to say about it, and `runnableTask` asks whether a
   blocked card is one the run would have had to move at all. Written out
   separately, the first two could drift into a dialog promising a move the
   start does not make, which is worse than saying nothing at all.

   The hole it closes: `runs/queue.rs`'s `snapshot` puts only `open` into
   `ready`, so a run aimed at a card standing anywhere else found nothing to do
   and stopped in the same breath it started — `QueueEmpty`, and a report with
   nothing in it. The card menu already refuses `done`, `blocked` and `parked`
   (`runnableTask` in `DesktopApp.vue`), which left `deferred`, `pinned`,
   `hooked`, `human-check` and every status a project invented for itself
   offering a run that could not run. */

/* bd's word for what the board calls Ready, and this is the front end's one
   copy of it — `kanban/parked.js` owns the string for the same reason it owns
   `parked`. */
import { READY } from '../kanban/parked.js'

/* The two statuses that are not `open` and still must not be written.
   `snapshot` puts both into `unfinished`, where the run's recovery phase picks
   them up on its own, so moving one back to `open` would pull a live session's
   claim out from under it. They are bd's own spellings, matching
   `runs/queue.rs`'s `IN_PROGRESS` and `READY_TO_MERGE` — the same copy across
   the language boundary that `parked.js` keeps, and it fails the same quiet
   way if one side is renamed alone. */
const HELD = ['in_progress', 'ready_to_merge']

/* True for everything this file has not been told about, deliberately: a
   project's own custom status is an ordinary outcome here rather than an error,
   and it is exactly the case the defect was about. The closed list is the three
   statuses a run can already work with, and nothing else.

   The status alone, which is why this is not the whole answer — see below — and
   also why the card menu reads this one rather than `promotesToReady`: there the
   question is whether a blocked card is one a run would have to *move*, and
   `in_progress` and `ready_to_merge` are not, blocker or no blocker. */
export function needsReady(bdStatus) {
  return bdStatus !== READY && !HELD.includes(bdStatus)
}

/* Whether writing `open` would actually put the task where a run can take it,
   and the one predicate both halves of this feature read.

   A status needing the move is only half the question, and the other half cost
   a status. Blocked is a **column and not a status anybody writes**: bd keeps a
   dependent issue at `open` and the board computes the column from its
   unfinished blockers (`boardColumns` in `stores/tracker.js`), so an issue that
   is `deferred`, `pinned` or custom **and** waiting on an unfinished blocker is
   bucketed under its own status and never under Blocked. The card therefore
   offers a run, and `snapshot` in `runs/queue.rs` would still refuse it —
   `OPEN if within_floor(...) && !blocked(...)` — so the promote would have
   overwritten `deferred` with `open`, unrecoverably, and bought the same
   `QueueEmpty` the promote exists to prevent.

   `blocked` is the caller's reading of the board, not a status: what counts as
   a blocker is one unfinished `blocks` dependency, which `dependencyEdges` in
   `stores/tracker.js` works out on the front end and `queue::blocked` works out
   again in Rust, both treating a closed blocker and a blocker absent from the
   board as satisfied. It stays an argument rather than something read here for
   this family's usual reason: a rule with no Vue and no store in it is a rule a
   test can reach. */
export function promotesToReady(bdStatus, blocked = false) {
  return !blocked && needsReady(bdStatus)
}

/* The line the run dialog carries above its fields, or '' where there is
   nothing to say.

   The id is named because the dialog is the last cheap moment to see the aim
   was wrong, and a promise to move "this task" would be a promise about
   whichever card the person thinks they clicked.

   A status the window does not have says nothing rather than promising a move:
   `startTheRun` writes only where it holds the issue, so a card deleted under
   an open dialog is the worker's refusal to give, not this line's. Nothing but
   a task scope speaks either — an epic run and a queue run change no statuses,
   and neither does the promote of a whole column, which is `PromoteColumnModal`
   and asks its own question.

   A blocked task says nothing for the same reason and it is the same rule
   rather than a second one: `promotesToReady` is what the start reads, so a
   sentence surviving where the write does not would be this dialog promising a
   move that never comes — a card told it was going to Ready and landing in
   Blocked. Silence rather than a sentence about the blocker: the card menu
   refuses the run on such a card outright (`runnableTask` in `DesktopApp.vue`),
   so what is left here is the seconds between a blocker being reopened and this
   window hearing about it, and a dialog that grew an explanation in that
   window would be explaining a state nobody arrived in on purpose. */
export function readyPromoteNote(scope, bdStatus, blocked = false) {
  if (scope?.kind !== 'task' || !bdStatus || !promotesToReady(bdStatus, blocked)) return ''
  return `Starting moves ${scope.id} to ready — a run takes its work from that column and nowhere else.`
}
