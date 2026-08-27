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
/* Null-prototype, and that is the whole of why it is not a plain object
   literal. The fallback below is `??`, which only catches nullish, so an
   inherited key would be answered rather than fallen back on: over
   `Object.prototype` this table returns a function for `constructor`,
   `toString` and `valueOf`, and an object for `__proto__`. Nothing on screen
   can reach it — the three producers of this state emit `''`, `'copied'` or
   `'failed'` and nothing else — but the fallback is stated here as a contract
   about *anything*, and a contract with four holes in it is worse than no
   contract at all. Borrowing nothing from anywhere is what makes the sentence
   below true. */
const LABEL = Object.assign(Object.create(null), {
  copied: 'Copied',
  failed: 'Could not copy'
})

/* `''` is the ordinary state and the one everything starts in: nothing has been
   asked yet, so the panel says what a click would do. Anything this file has
   never heard of falls there too — an unknown state is not worth a sentence of
   its own, and the invitation is never wrong. */
export function copyLabel(state) {
  return LABEL[state] ?? 'Copy id'
}
