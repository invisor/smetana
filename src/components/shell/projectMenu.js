/* What a project row's right-click menu offers, as a rule rather than as a
   template.

   The `taskMenu.js` / `branchChoice.js` / `columnOrder.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the component is a
   rule nothing checks.

   Three of the four items are actions the row already carries, and the menu is
   not a copy of the row's visibility rules for them. Each of those rules has
   two halves — the gear appears while the project is active *and* has no
   configuration, the plus while it is active *and* the agents panel is open —
   and the menu keeps only the first half of each. Dropping the second half is
   the point rather than an oversight: a person who asked for this menu asked
   for the verb, and a row that offers a button only in one panel is answering a
   different question about where the button should sit.

   The fourth, `settings`, has no counterpart on the row at all: a 28px tile has
   room for a monogram and a dot, and editing `[defaults]` in the project's own
   `.smetana/project.toml` had no door in the app before this menu. */

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

/* Two more refusals stood here, both the settings item's alone: "Set this
   project up first" over an active project with no `.smetana/project.toml`, and
   "This project's configuration will not parse" over one whose file is damaged.
   Both are gone and the item is live in either state, and the reason is what
   that window holds now.

   It edits two things rather than one: `[defaults]` in the project's own file,
   and this machine's caveman level for this project, which lives in
   `settings.json` and has nothing to do with the file. So a project with no
   file, or with a damaged one, would have been shut out of a preference it had
   before — it used to be a row on the Agents tab of the settings window, which
   asks nothing about a project's configuration. The window says which of the
   two states it is in, in its own words and where there is room for them
   (`configNotice` in `components/run/projectDefaults.js`), and draws no Save
   over a file it cannot fill a form from. A caption here would now be refusing
   a window that has something to offer.

   What is left is the one fact this menu can still act on, and it is
   `ELSEWHERE`'s: another project's row. */

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
      /* Editing `[defaults]` in the project's own file, and the caveman level
         this machine uses in it, without starting anything. The setup item
         above is the other verb about that file and is not a substitute for
         this one: it costs a session and takes no instruction, which is the
         right price for "this project grew a fourth repository" and the wrong
         one for "run three tasks at a time, not five". */
      kind: 'settings',
      label: 'Project settings',
      /* Not `settings-2`, which the setup item above already carries: two
         adjacent rows under one glyph read as one row with a stutter, and these
         two verbs are genuinely different — one adjusts a value, the other
         starts an agent. */
      icon: 'sliders-horizontal',
      /* Live wherever the window is pointed, whatever state the project's file
         is in — see the note above the removed captions. */
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
