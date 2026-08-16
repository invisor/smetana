/* What a branch row's right-click menu offers, as a rule rather than as a
   template.

   The `projectMenu.js` / `gitActions.js` / `taskMenu.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the component is a
   rule nothing checks.

   It holds the three things a branch row has always been able to do — two of
   them used to be buttons that appeared on the row under the pointer, and the
   third was the row's own click, which had no name anywhere on screen and now
   has one. That is the whole reason the menu carries a verb it did not have to:
   a menu is where a person goes to find out what a place can do, and a place
   whose main action is missing from its own menu reads as a place that cannot
   do it.

   The fourth is the one thing here that is not about a branch that exists:
   cutting a new one from this row's commit. It is last and in a group of its
   own, because it is the only item that leaves the list longer than it found
   it.

   **The reason is a caption, not a suffix per row.** `projectMenu.js` writes its
   refusal into each label because its items are refused for different reasons
   and `ContextMenu` gives a row no tooltip and no `title`. Here one fact refuses
   a whole group at once, so the sentence sits above the group as a caption, said
   once.

   **Two refusals of different reach**, which is the whole shape of this rule. A
   run or an operation already going refuses everything, caption at the top. The
   branch being the one already checked out refuses only the three verbs about
   moving between branches — a new branch cut *from where you are standing* is
   the ordinary case, not an edge one — so that caption heads those three and the
   last group stays live below the separator. What says how far a caption reaches
   is the greying under it: the live row is visibly not part of the group. */

/* What refuses the whole menu, in order of what is worth saying. Both mean "not
   now" rather than "not this row", which is what puts either at the very top. */
function frozen({ allowed, busy }) {
  if (!allowed) return 'A run is going in this project'
  if (busy) return 'Git is working in this repository'
  return null
}

export function branchMenuItems({ current = false, allowed = true, busy = false } = {}) {
  const held = frozen({ allowed, busy })
  /* The three verbs about moving between branches. Every one of them is a no-op
     on the branch already checked out: git answers a merge with "Already up to
     date", and a checkout of where you are with nothing at all. */
  const moving = Boolean(held) || current
  const caption = held ?? (current ? 'Already on this branch' : null)

  return [
    ...(caption ? [{ type: 'label', label: caption }] : []),
    /* The labels are the ones the two buttons carried as their accessible
       names, word for word. They were already written for someone who cannot
       see the row, which is the same sentence a menu row needs. */
    { kind: 'checkout', label: 'Switch to this branch', icon: 'git-branch', disabled: moving },
    /* A separator, because the two below are not the same kind of act as the
       one above: switching branches is where you are, merging and rebasing
       change what the branch you are on contains. */
    { type: 'separator' },
    { kind: 'merge', label: 'Merge into the current branch', icon: 'git-merge', disabled: moving },
    {
      kind: 'rebase',
      label: 'Rebase the current branch onto this',
      icon: 'git-graph',
      disabled: moving
    },
    { type: 'separator' },
    /* No ellipsis on a row that opens a dialog, though the convention is a good
       one: nothing else in this app spends it — "Set up" opens a dialog, so does
       "Discard worktree" — and one row keeping a convention nobody else keeps
       reads as a typo rather than as a signal. */
    {
      kind: 'new-branch',
      label: 'New branch from this',
      icon: 'git-branch-plus',
      disabled: Boolean(held)
    }
  ]
}
