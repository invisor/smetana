/* The one line in the status footer saying what this project is doing right
   now. It was the scope bar's until the window's title bar went back to
   answering "where am I working" alone; the sentence itself did not change.
   Present tense only, and empty when there is nothing to say — an empty
   headline draws nothing at all and the strip closes up, rather than reserving
   a gap for a sentence most projects will not have most of the time.

   Pure, and a module of its own for the reason the `projectMenu.js` family is:
   no test in this repository can reach a `.vue`, so a rule left inside the
   component that draws it is a rule nothing checks.

   The design handoff also captioned an idle project "last run 2h ago" and
   "never run". Neither is written here: `runsState.runs` holds what this window
   has been told since it opened, so after a restart a project that ran all
   night would be captioned "never run" — a caption that is confidently wrong is
   worse than no caption, and this app has made that mistake once already with a
   log pane fed from a fixture. */

/* A run counts as under way while it has not stopped, which makes this the
   eighth place in the front end comparing against the serde tag of
   `RunState::Stopped` in src-tauri/src/runs/model.rs. The comparison is that
   way round rather than a list of the four live states (`preflight`,
   `working`, `deciding`, `paused`) on purpose: a state this front end has not
   heard of belongs to a run that is still going, and enumerating the live ones
   would silently read a new one as an ended run. A paused run is under way too
   — the allowance it is waiting on is the run's own business, and the `RunBar`
   segment beside this sentence is where that detail is said. */
const STOPPED = 'stopped'

/**
 * What the status footer says about this project.
 *
 * @param {object} input
 * @param {{live: number, loud: number} | undefined} input.row
 *   how many of this project's agents are waiting on the person and how many
 *   are otherwise alive — `agentCounts` from stores/terminals.js, which is the
 *   active project's own session list with the person's shells already taken
 *   out. Missing is ordinary and means nobody has an agent there, which is what
 *   every project reads as in a window that has just opened. Agents only: a
 *   shell reaching `needs-you` on a completion beep is not somebody being
 *   waited on, and this sentence says "agent".
 * @param {Array<{state?: {kind?: string}}>} [input.runs]
 *   every run this window knows of for the project, as `runsState.runs` keeps
 *   them. A list rather than one run because a project holds several at once,
 *   and the newest of them being over says nothing about the others.
 * @returns {{text: string, level: 'loud'|'live'|'quiet'}} `level` is the design
 *   system's attention vocabulary, and an empty `text` is `quiet`.
 */
export function headline({ row, runs = [] } = {}) {
  if (row?.loud) {
    return {
      text: row.loud === 1 ? '1 agent needs you' : `${row.loud} agents need you`,
      level: 'loud'
    }
  }
  /* `run &&` first: a hole in the list must not read as a run whose state
     nobody has told us, which is what a bare `!==` against a missing `kind`
     would come to. */
  if (runs?.some((run) => run && run.state?.kind !== STOPPED)) {
    return { text: 'Run under way', level: 'live' }
  }
  if (row?.live) {
    return {
      text: row.live === 1 ? '1 agent running' : `${row.live} agents running`,
      level: 'live'
    }
  }
  return { text: '', level: 'quiet' }
}
