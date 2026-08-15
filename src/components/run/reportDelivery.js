/* Where a run's own account goes the moment the run is over: into a tab in
   front of the person, or into the bell to wait for them.

   Another of the `branchChoice.js` family — the whole of one rule, pure, with no
   Vue, no DOM and no Tauri in it. A `.vue` file is the one thing no test in this
   repository can reach, and the acting half of this lives in `DesktopApp.vue`
   because opening a tab is the only thing that view can do and no store can.

   The question it answers is "was the person watching *this* agent", and the
   answer is the selection and nothing else. Not window focus: somebody who left
   the app with an agent selected comes back to that agent, and a document
   waiting in a tab is what they came back for. Not the centre tab either —
   `activeId` deliberately survives leaving the terminal, because it is what
   `AgentList.vue` highlights a row from, and a person reading a file with their
   agent selected is still watching that agent.

   **Delivery is one or the other, never both.** A card in the bell is the app
   asking to be visited; a tab already in front of somebody is the visit. Which
   is why the caller marks the run delivered when this answers `tab`, and why
   `delivered` is a parameter rather than state kept here: this file decides, it
   does not remember. */

/* `tab`, `bell`, or `null` for a run there is nothing to say about yet.
   `bell` is not an instruction — `syncRunCards` has already made that card from
   the same stopped run — it is this rule declining the tab and saying which
   delivery is therefore standing. */
export function deliveryFor(run, selected, delivered) {
  /* Only an ending is delivered. Every other state is a run still going, and a
     shape this rule cannot read is not an ending it may announce. */
  if (run?.state?.kind !== 'stopped') return null
  /* `loadRun` replaces the list wholesale on window focus and on a project
     switch, so the same ending arrives here again and again; without this it
     would open its tab every time. */
  if (delivered?.has(run.token)) return null

  /* Two absent sessions are not one agent. A run from a worker too old to name
     the session it worked in carries no id, a window with nothing selected
     carries no id, and the obvious `run.last_session === selected` reads that
     pair as a match and opens a tab neither of them asked for. */
  const worked = typeof run.last_session === 'number' && run.last_session === selected

  /* A tab is the one delivery that cannot happen without a file: with no
     document the bell's card is already the whole of what there is to say, and
     it draws no button for exactly this reason. */
  return worked && run.summary?.report ? 'tab' : 'bell'
}
