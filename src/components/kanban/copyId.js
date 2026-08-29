/* What the tooltip on a task's id says, for the whole of the three-value
   vocabulary the copying speaks in — and how long the app says any of it for.

   Out of the two components that draw an id — the card on the board and the
   inspector's header — because it was written twice over and the two copies
   were free to drift into different words for the same operation with both
   suites green. A `.vue` file is the one thing no test in this repository can
   reach, and these three strings are three of this feature's acceptance
   criteria, so the whole of the rule lives here where something mechanical can
   read it.

   Under `kanban/` rather than at the top of `src/`: it is a rule about one part
   of the interface, which is where this tree keeps that kind. No Vue and no DOM
   in it.

   `COPIED_MS` below is the one thing here that the rest of the interface reads,
   and it is here rather than beside any of them for the ordinary reason: this
   file is where the copy confirmation's vocabulary already lived, and the
   duration is the rest of the same policy. A session row's menu
   (`components/agent/sessionMenu.js`) imports it, and so does the composable
   that waits it out (`components/core/copyFeedback.js`) — both a reach across
   two groups, and both the smaller of the two costs, the other being the number
   written out three times over. It stays here rather than moving in with the
   behaviour so that this module keeps the property its own header claims: pure,
   with no Vue in it, which is the whole of why a test can reach it. */
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

/* How long a confirmation stands before the control goes back to inviting the
   press again.

   **One number for every copy in the app**, and it was three until this line
   existed: `COPIED_ID_MS` in `views/DesktopApp.vue`, a bare `1200` in
   `views/Gallery.vue`, and a third in the session menu's own rule. Nothing
   mechanical joined them, and the gallery is this project's only verification of
   anything under `src/components/` — so a duration that moved in the app alone
   would leave the harness confirming a copy at a speed the product no longer
   uses, and the next person checking this by eye measuring the wrong thing.

   Long enough to be read without being looked for, short enough that a second
   copy a moment later is not waiting on the first. It is a duration rather than
   a token because nothing in `tokens/motion.css` is about how long a *sentence*
   stays on screen — those are transitions, and this is a dwell. */
export const COPIED_MS = 1200
