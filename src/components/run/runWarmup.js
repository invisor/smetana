/* "The run has been started but has no session yet", as a rule rather than as
   two `kind === ...` checks in two `.vue` files.

   The `stopReason.js` family: pure, no Vue and no DOM. `run_start` answers as
   soon as the worker has noted the request, and for the seconds after that the
   run sits in `preflight` (bringing the project up) and `deciding` (reading the
   board) until the first batch has a session. The run bar and the agents panel
   both need to say "still going" for exactly that window, and reading it from
   one place is what keeps them from disagreeing.

   `deciding` also happens between batches while earlier sessions are still
   listed. That is fine for the run bar, where a turning glyph over "Reading the
   board" is honest; the agents panel additionally checks that it has no rows. */
const WARMING = ['preflight', 'deciding']

export function warmingUp(run) {
  return WARMING.includes(run?.state?.kind)
}
