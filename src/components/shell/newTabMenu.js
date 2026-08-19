/* What the + button beside the pinned tabs offers.

   The `projectMenu.js` / `taskMenu.js` family: pure, no Vue and no DOM, which is
   the whole reason it is a file of its own — no test in this repository can
   reach a `.vue`, so a rule left inside the component is a rule nothing checks.
   The second reason is that there are two call sites, the app and the gallery,
   and two copies of the words would be two answers to what the menu says.

   Three rows, and the third is a different kind of thing from the first two.
   `agent` and `terminal` each open a tab in the centre column; `task` opens the
   new-task dialog over whatever tab is already there. That is why it comes
   last — a dialog pushed above the two rows that are used every day would move
   their keyboard order for the sake of the one that has another door already,
   the `+` above the `ready` column on the board. It is deliberately the same
   dialog rather than one of its own: two ways of filing a task would be two
   dialogs drifting apart within the month. No separator marks the boundary
   either — over three rows it divides the menu louder than it divides the
   meaning.

   Which harness runs is not asked here: the agent comes from `settings.json`,
   exactly as it does for the "+ New agent" row of the Agents panel, and this
   row is the same act by another door. A shell is asked nothing at all — where
   it opens is the project root, and what it does is whatever gets typed into
   it.

   Not a function, unlike its two relatives: no row is ever greyed out or
   reworded. What decides whether the menu can be used at all is whether a
   project is open, and that is the button's own `disabled` — a state about the
   control rather than about any row. */
export const NEW_TAB_ITEMS = [
  { kind: 'agent', label: 'New agent', icon: 'bot' },
  { kind: 'terminal', label: 'New terminal', icon: 'terminal' },
  { kind: 'task', label: 'New task', icon: 'square-check' }
]
