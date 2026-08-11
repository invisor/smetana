/* When `.smetana/project.toml` may have changed under this window — expressed
   as a value that changes exactly then, so a plain `watch` over it is the whole
   of that freshness. Another of the `branchChoice.js` family: pure, no Vue and
   no DOM, because a `.vue` file is the one thing no test in this repository can
   reach and this rule is what the mark on a project row hangs off.

   The configuration is written by an agent in this window's own terminal tab,
   and that is the one writer window focus never notices: somebody watching a
   setup agent work never leaves and never comes back, so `catchUp` does not
   fire and neither does a project switch. Something has to notice from inside,
   and the only thing that moves is the session's own state.

   So the signal is the set of the project's sessions that are still working.
   `starting` and `running` are an agent with the keyboard; every other state
   the worker has — `idle`, `needs-you`, `exited` — is an agent that has stopped
   for a moment, which is the first moment a file it was writing can be read.
   Leaving the list counts the same way, so a session removed by hand, or one
   dropped when `loadSessions` replaces the array wholesale, moves the key as
   surely as one that exits. And a state this front end has never heard of reads
   as "not working", which errs toward re-reading a small file too often rather
   than toward a mark that will not clear.

   Every session of the project rather than the setup session alone, and that
   width is honest rather than lazy: a configuration is a file, and the app does
   not know which agent wrote it. Narrowing it back to one session id is exactly
   the shape of the defect this replaces (smetana-0ag) — a watcher created
   inside an event handler, on one id, which tore itself down for good on its
   first callback for another project or for a session already gone from the
   list, with nothing anywhere to re-establish it. A key over a set cannot be
   lost: it is recomputed from whatever the store holds now.

   Scoped to one project, and that scope is the guard the fix must not widen: a
   session in another project moves that project's key and never this one's, so
   a setup running in A can neither clear nor set the mark on B. With no project
   there is nothing to be fresh about, and the answer is the same empty key for
   any list at all. */
const WORKING = new Set(['starting', 'running'])

export function workingKey(sessions, project) {
  if (!project) return ''
  return (sessions ?? [])
    .filter((session) => session && session.project === project && WORKING.has(session.state))
    .map((session) => session.id)
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0))
    .join(' ')
}
