/* What a card's play may do and what it would start, as rules rather than lines in a component.

   The `statusPatch.js` / `lockCascade.js` family: pure, no Vue and no DOM. The desktop's board
   (`DesktopApp.vue`) and the phone's task sheet both decide whether a card's Run row is live and
   which scope it would send, and a rule written twice is a rule that drifts with no gate to see it,
   so the one copy lives here. */
import { needsReady } from '../run/readyPromote.js'
import { isParked } from './parked.js'

const list = (issues) => (issues instanceof Map ? [...issues.values()] : [...(issues ?? [])])

/* bd's words for a finished issue, which the front end maps to the Done column (`toUiStatus` in
   `stores/tracker.js`: `closed` becomes `done`, and `done` stays itself). */
const isFinished = (status) => status === 'closed' || status === 'done'

/* What is left to do under an issue: its children that are not done, because a run would not take
   a done one. Parenthood is the parent-child relation and nothing else; the parent's own type and
   status have no part in it. `issues` is a Map or an array of bd issues. */
export const openChildren = (issues, id) =>
  list(issues).filter((issue) => issue.parent === id && !isFinished(issue.status))

/* The scope a card's play would start: an issue with children is run as its children, and the
   parent itself is never the work. One function so the play's grey and the click's scope are about
   the same run. */
export const cardScope = (issues, id) => ({ kind: openChildren(issues, id).length ? 'epic' : 'task', id })

/* Whether the project's run configuration lets a play be offered at all: a damaged one is let
   through on purpose, since the run dialog is the surface wide enough to quote the parser.
   `state` is `runsState.config.state`: `ok`, `broken` or `missing`. */
export const runOffered = (state) => state === 'ok' || state === 'broken'

/* Whether a card's Run row is live (`offered` is `runOffered`'s answer). A blocker is a reason to
   refuse only for a status the run would have to move (`needsReady`): an `in_progress` or
   `ready_to_merge` card is let through whatever it waits on. */
export const runnable = (card, offered) =>
  Boolean(offered) &&
  card.status !== 'done' &&
  card.status !== 'blocked' &&
  !(card.blockedBy && needsReady(card.bdStatus)) &&
  !isParked(card.bdStatus)
