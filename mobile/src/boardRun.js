/* Where the board starts a run and whether it may: a pure rule, with no Vue in it, so that the
   one decision the Ready column's play makes can be tested without a screen.

   The desktop starts the queue from the play in the Ready column's header (`ColumnHeader`
   `runnable`), and the phone does the same. The play is offered only while the project's run
   configuration lets a run be offered at all, and it is grey, with the desktop's own reason,
   while a run over the queue is already going. */
import { runOffered } from '../../src/components/kanban/runScope.js'
import { scopeBusyReason } from '../../src/components/run/runScopes.js'

/* The column the play stands in. */
export const RUN_FROM = 'ready'

/* `configState` is `runsState.config.state`, `runs` is `runsState.runs`. `runnable` says whether
   the play is drawn; `blockedReason` is the empty string when it can be pressed. */
export function queueRun(configState, runs) {
  const runnable = runOffered(configState)
  return { runnable, blockedReason: runnable ? scopeBusyReason({ kind: 'queue' }, runs ?? []) : '' }
}
