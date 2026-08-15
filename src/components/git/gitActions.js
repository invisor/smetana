/* Whether the Git panel may change the working tree it is looking at, and the
   sentence a person reads when it may not.

   Pure, with no Vue and no DOM in it — the family `branchChoice.js`,
   `boardView.js` and `parked.js` belong to, and for the reason that family
   exists: a `.vue` file is the one thing no test in this repository can reach,
   so the whole of a rule lives outside the component that draws it.

   **The rule is about runs and nothing else.** A run is the app driving itself
   overnight — nobody is watching, and a batch may be halfway through a merge —
   so a branch switched under it is a night's work lost to one click. An agent
   session a person started themselves is the opposite case: they started it,
   they can see it in the tab next door, and a panel dead for as long as one
   terminal is open would be dead nearly all the time. So the only thing this
   reads is the project's runs, and that is what keeps the sessions out by
   construction rather than by a condition somebody could add later.

   The verdict is a pair rather than a bare string, because the two halves are
   read in two places — the control's `disabled` and the tooltip over it — and
   `browserTools.js` collapses them into one only because there the tooltip is
   the whole of what a blocked toggle says. Here a disabled row still draws.

   `allowed: true` with a reason, or `false` without one, are both a control
   contradicting its own explanation, so neither is expressible: the reason is
   `null` exactly when the answer is yes. */

/* The one state that is not a live run. Everything else — running, paused,
   a word this build has never heard of, a run whose state cannot be read at
   all — counts as going. That direction is deliberate and it is the cheap one:
   being wrong this way costs a button somebody has to wait a moment for, and
   being wrong the other way costs the run. It is the same instinct `tracker.js`
   keeps with bd's statuses and `runs.js` keeps by holding a `Run` whole — a
   state this front end has not heard of must never quietly read as one it
   has. */
const STOPPED = 'stopped'

const isGoing = (run) => run?.state?.kind !== STOPPED

/* What a checkout under a run would cost, in the words of the thing it would
   cost it. The count is in the sentence because a project holds several runs at
   once (smetana-5hf): "a run" over three of them would read as though stopping
   one would free the panel. */
const COST = 'A checkout under a batch that is mid-merge would lose its work.'

export function gitActions(runs) {
  /* `?? []` is defensive and not a window the store can produce: `runsState.runs`
     starts as `[]` and is set back to `[]` on a project switch before `loadRun`
     refills it, so it is never absent in the app. It costs nothing and keeps
     this a total function over whatever a caller — a gallery frame, a test —
     hands it. */
  const going = (runs ?? []).filter(isGoing).length
  if (going === 0) return { allowed: true, reason: null }
  const subject = going === 1 ? 'A run is going' : `${going} runs are going`
  return { allowed: false, reason: `${subject} in this project. ${COST}` }
}
