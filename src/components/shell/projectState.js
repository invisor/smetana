/* How a project's agent state is said in words. Two call sites — the rail's
   tooltip, which is the one place `live` and `loud` are told apart by something
   other than hue, and the panel header's summary line — so the words are here
   rather than in either of them: two copies of a sentence are two sentences
   within the month.

   Pure, no Vue, for the reason the `projectMenu.js` family is: nothing in this
   repository can test a `.vue`.

   The counts come from `projectStates` in stores/terminals.js. `undefined` is
   ordinary and means nobody has any sessions there, which is what a window that
   has just opened sees for every project. */

/** "1 agent waiting on you" | "2 agents running" | "idle". */
export function stateLabel(row) {
  /* Waiting is counted before running for the reason `projectStates` lets
     `loud` win over `live`: a project with an agent waiting on a person is the
     reason the rail exists, and another agent getting on with its work there
     must not be what the tile says instead. */
  if (row?.loud) {
    return row.loud === 1 ? '1 agent waiting on you' : `${row.loud} agents waiting on you`
  }
  if (row?.live) {
    return row.live === 1 ? '1 agent running' : `${row.live} agents running`
  }
  return 'idle'
}

/** The panel header's one line: "develop · 1 agent running". */
export function projectSummary(branch, row) {
  /* An empty branch is dropped rather than joined: a line opening with a
     separator reads as a missing word, and a project whose head has not been
     read yet is the ordinary case for the first moment of a window. */
  return [branch, stateLabel(row)].filter(Boolean).join(' · ')
}
