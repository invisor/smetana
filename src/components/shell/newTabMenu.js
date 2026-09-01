/* What the + button beside the pinned tabs offers.

   The `projectMenu.js` / `taskMenu.js` family: pure, no Vue and no DOM, which is
   the whole reason it is a file of its own — no test in this repository can
   reach a `.vue`, so a rule left inside the component is a rule nothing checks.
   The second reason is that there are two call sites, the app and the gallery,
   and two copies of the words would be two answers to what the menu says.

   Four rows, `task` first. The array is the order on the screen and the order
   the keyboard walks — `MenuButton` draws it as it stands — and where `task`
   stands in it is the product owner's decision (smetana-ep4s) rather than
   anything about the rows themselves. The argument that used to be written out
   here put it last; it was overruled and not refuted, so nothing is invented in
   its place: this paragraph says where the row is, and the one below says what
   is still worth knowing about the four.

   Two of them are a different kind of thing from the other two. `agent` and
   `terminal` each open a tab in the centre column; `task` and `review` open a
   dialog window over whatever tab is already there, and each of those two has
   another door already — the `+` above the `ready` column on the board for the
   task, and a branch row's own menu for the review. Both are deliberately the
   same dialogs those other doors open rather than ones of their own: two ways
   of filing a task, or of asking for a review, would be two dialogs drifting
   apart within the month. No separator marks the boundary — over four rows it
   divides the menu louder than it divides the meaning.

   `review` opens the branch-review window knowing no branch: the checked side
   of the pair is empty and there are no repositories under it yet, and picking
   a branch there fills the table. The branch row's menu opens the same window
   with that side already filled and a row for every repository that has the
   branch. The two doors differ in what they start with and in nothing else.

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
  { kind: 'task', label: 'New task', icon: 'square-check' },
  { kind: 'agent', label: 'New agent', icon: 'bot' },
  { kind: 'terminal', label: 'New terminal', icon: 'terminal' },
  /* The magnifier with a tick: the same glyph the branch row's own
     `Review this branch…` carries, so the two doors into one window are marked
     the same way. Kept in the `New …` shape the other three are in, since what
     it opens is a review that does not exist yet. */
  { kind: 'review', label: 'New review', icon: 'search-check' }
]
