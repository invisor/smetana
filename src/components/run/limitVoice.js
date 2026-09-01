/* Which of several paused runs says why they are all standing still.

   Pure and outside the component for the reason `stopReason.js` next to it is:
   a `.vue` file is the one thing no test in this repository can reach, and
   "which segment writes the sentence" is exactly the sort of rule that goes
   wrong quietly — the way it went wrong to begin with, as two runs paused
   seconds apart and the footer wrote the same sentence twice, differing only in
   the minute the harness happened to name.

   The subscription is one per machine, so the sentence about it is one per
   footer. It goes to the **first paused run in the list**, which is the oldest:
   `runsState.runs` holds them oldest first and the footer already draws them in
   that order, so the sentence lands on the leftmost of the paused segments and
   does not move as later runs come and go.

   The other paused segments are not removed. Each one owns its own Stop button,
   and that button belongs to that run and to nothing else — dropping the segment
   to be rid of the duplicated sentence would take a control with it. What they
   lose is the words. */

/* The token of the run that speaks, or `null` when none of them is paused.

   Anything that is not a live list answers `null`, which draws no sentence at
   all rather than one on every segment: a footer that cannot say which run
   speaks is a footer where every run would claim to. */
export function limitVoice(runs) {
  if (!Array.isArray(runs)) return null
  const speaker = runs.find((run) => run?.state?.kind === 'paused')
  return speaker ? speaker.token : null
}
