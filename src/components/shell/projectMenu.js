/* What a project row's right-click menu offers, as a rule rather than as a
   template.

   The `taskMenu.js` / `branchChoice.js` / `columnOrder.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the component is a
   rule nothing checks.

   The menu holds the three actions the row already carries, and it is not a
   copy of the row's visibility rules. Each of those rules has two halves — the
   gear appears while the project is active *and* has no configuration, the plus
   while it is active *and* the agents panel is open — and the menu keeps only
   the first half of each. Dropping the second half is the point rather than an
   oversight: a person who asked for this menu asked for the verb, and a row
   that offers a button only in one panel is answering a different question
   about where the button should sit. */

/* Why the two project-scoped verbs are refused elsewhere, and where that reason
   is written. `ContextMenu` clips a row's label rather than wrapping it and
   gives a row no tooltip and no `title`, so a reason suffixed onto each label
   is a reason that runs off the end of the panel — which is what these two did,
   reading "Set up — switch to this pr…" at a ceiling wide enough for anything
   else this menu holds.

   **One fact refuses both verbs, so it is said once, above them.** That is
   `branchMenu.js`'s shape, and its note about the two files is now a
   description of the same rule rather than a contrast: a caption refuses a
   group, and how far it reaches is the greying under it — "Remove from list" is
   live below the separator and visibly not part of the group. A per-row suffix
   would be for a menu whose rows are refused for *different* reasons, and this
   one has never been that. */
const ELSEWHERE = 'Switch to this project first'

/* `configured` and `configBroken` are measured for the active project alone —
   probing every row would be a command per project for a mark nobody reads —
   so they are read here only when this row *is* that project. Anywhere else the
   setup item says the bare verb: "Set up" claims nothing about a file, where
   "Set up again" would be claiming another project's state.

   `canAddAgent` is taken because the caller has it and the row's own plus reads
   it, and it deliberately decides nothing here — see the note at the top about
   which half of each row rule the menu keeps. */
export function projectMenuItems({ active, configured, configBroken, canAddAgent }) {
  const here = Boolean(active)
  /* A file is there, parseable or not — which is the whole of what the setup
     dialog needs in order to choose its words, and why a damaged configuration
     reads "Set up again" like a working one. That damaged case is exactly what
     this menu exists for: the row draws no gear for it, and the route out used
     to be a button in the run dialog. */
  const existing = here && Boolean(configured || configBroken)

  return [
    ...(here ? [] : [{ type: 'label', label: ELSEWHERE }]),
    {
      kind: 'setup',
      label: existing ? 'Set up again' : 'Set up',
      icon: 'settings-2',
      /* What `SetupProjectModal` opens on: its copy differs between a project
         being set up for the first time and one being set up over. Carried on
         the item rather than worked out again by whoever handles the pick, so
         the words in the menu and the words in the dialog cannot disagree. */
      existing,
      disabled: !here
    },
    {
      kind: 'add-agent',
      label: 'New agent',
      icon: 'plus',
      disabled: !here
    },
    { type: 'separator' },
    {
      /* Live on every row, and the one item that is: removing a project from
         the list is about the list, not about the project the window is pointed
         at, and pruning a list is what people do to the rows they are *not*
         working in. It is also what the caption above does not reach, which is
         the whole reason the two groups are drawn apart. */
      kind: 'remove',
      label: 'Remove from list',
      icon: 'x',
      tone: 'danger'
    }
  ]
}
