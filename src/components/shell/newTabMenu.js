/* What the + button beside the board tab offers.

   The `projectMenu.js` / `taskMenu.js` family: pure, no Vue and no DOM, which is
   the whole reason it is a file of its own — no test in this repository can
   reach a `.vue`, so a rule left inside the component is a rule nothing checks.
   The second reason is that there are two call sites, the app and the gallery,
   and two copies of the words would be two answers to what the menu says.

   Two rows and no third. Which harness runs is not asked here: the agent comes
   from `settings.json`, exactly as it does for the "+ New agent" row of the
   Agents panel, and this row is the same act by another door. A shell is asked
   nothing at all — where it opens is the project root, and what it does is
   whatever gets typed into it.

   Not a function, unlike its two relatives: neither row is ever greyed out or
   reworded. What decides whether the menu can be used at all is whether a
   project is open, and that is the button's own `disabled` — a state about the
   control rather than about either row. */
export const NEW_TAB_ITEMS = [
  { kind: 'agent', label: 'New agent', icon: 'bot' },
  { kind: 'terminal', label: 'New terminal', icon: 'terminal' }
]
