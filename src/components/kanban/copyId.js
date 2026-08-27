/* What the tooltip on a task's id says, for the whole of the three-value
   vocabulary the copying speaks in.

   Out of the two components that draw an id — the card on the board and the
   inspector's header — because it was written twice over and the two copies
   were free to drift into different words for the same operation with both
   suites green. A `.vue` file is the one thing no test in this repository can
   reach, and these three strings are three of this feature's acceptance
   criteria, so the whole of the rule lives here where something mechanical can
   read it.

   Under `kanban/` rather than at the top of `src/`: it is a rule about one part
   of the interface, which is where this tree keeps that kind. No Vue and no DOM
   in it. */
const LABEL = {
  copied: 'Copied',
  failed: 'Could not copy'
}

/* `''` is the ordinary state and the one everything starts in: nothing has been
   asked yet, so the panel says what a click would do. Anything this file has
   never heard of falls there too — an unknown state is not worth a sentence of
   its own, and the invitation is never wrong. */
export function copyLabel(state) {
  return LABEL[state] ?? 'Copy id'
}
