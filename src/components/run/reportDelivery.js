/* Where a run's own account goes the moment the run is over: into a tab in
   front of the person, into the bell to wait for them, or nowhere at all.

   Another of the `branchChoice.js` family — the whole of one rule, pure, with no
   Vue, no DOM and no Tauri in it. A `.vue` file is the one thing no test in this
   repository can reach, and the acting half of this lives in `DesktopApp.vue`
   because opening a tab is the only thing that view can do and no store can.

   **One setting is the whole of the policy**, and that is deliberate rather
   than incidental: `notifications.showReport` on the General tab decides
   whether a finished run says anything at all. What stood here before asked
   whether the person had that run's own agent selected at the moment it
   stopped, and the answer to "why did my report not open this time" was a
   window state nobody could see — so the check is gone rather than kept under
   the switch, since leaving it would have made the switch one condition of two
   and the complaint would still stand with it on.

   **Delivery is one of the three, never two of them.** A card in the bell is
   the app asking to be visited; a tab already in front of somebody is the
   visit; and off is neither. Which is why the caller marks the run delivered
   whatever this answers, and why `delivered` is a parameter rather than state
   kept here: this file decides, it does not remember.

   What the switch cannot cancel is physics rather than policy. With no document
   — a run that fell over before writing one — there is nothing for a tab to
   open, so a switched-on app still leaves the bell's card, which says how the
   run ended rather than merely linking to a file. Switched off there is still
   nothing at all. */

/* `tab`, `bell`, `none`, or `null` for a run there is nothing to say about yet.
   `bell` is not an instruction — `syncRunCards` has already made that card from
   the same stopped run — it is this rule declining the tab and saying which
   delivery is therefore standing. `none` is the opposite: the card exists for
   the same reason and the caller has to take it back. */
export function deliveryFor(run, show, delivered) {
  /* Only an ending is delivered. Every other state is a run still going, and a
     shape this rule cannot read is not an ending it may announce. */
  if (run?.state?.kind !== 'stopped') return null
  /* `loadRun` replaces the list wholesale on window focus and on a project
     switch, so the same ending arrives here again and again; without this it
     would open its tab every time. */
  if (delivered?.has(run.token)) return null

  /* Off means off in both deliveries. A card is a way of showing the report
     too — it is a button onto the same document — so leaving it up would be
     answering a person who asked not to be shown their reports with a smaller
     version of the thing they declined. The sound is a separate answer and
     keeps playing, and the run bar still says the run has stopped. */
  if (!show) return 'none'

  /* A tab is the one delivery that cannot happen without a file: with no
     document the bell's card is already the whole of what there is to say, and
     it draws no button for exactly this reason. */
  return run.summary?.report ? 'tab' : 'bell'
}
