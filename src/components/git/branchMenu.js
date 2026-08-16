/* What a branch row's right-click menu offers, as a rule rather than as a
   template.

   The `projectMenu.js` / `gitActions.js` / `taskMenu.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the component is a
   rule nothing checks.

   It holds the three things a branch row has always been able to do. Two of
   them used to be buttons that appeared on the row under the pointer; the third
   was the row's own click, which had no name anywhere on screen and now has
   one. That is the whole reason the menu carries a verb it did not have to:
   a menu is where a person goes to find out what a place can do, and a place
   whose main action is missing from its own menu reads as a place that cannot
   do it.

   **The reason is one caption, not three suffixes.** `projectMenu.js` writes its
   refusal into each label because its items are refused for different reasons
   and `ContextMenu` gives a row no tooltip and no `title`. Here all three are
   refused by the same fact — the branch is the one already checked out, a run
   holds the repository, or git is in the middle of something — so the sentence
   is a caption above the group it is about, said once. */

/* Why every item is greyed, in order of what is most worth saying. The current
   branch comes first even under a run: a person right-clicking the row with the
   tick is asking about that row, and "a run is going" would answer a question
   they did not ask — the run blocks the other rows too, and those say so. */
function refusal({ current, allowed, busy }) {
  if (current) return 'Already on this branch'
  if (!allowed) return 'A run is going in this project'
  if (busy) return 'Git is working in this repository'
  return null
}

export function branchMenuItems({ current = false, allowed = true, busy = false } = {}) {
  const reason = refusal({ current, allowed, busy })
  const disabled = Boolean(reason)

  return [
    ...(reason ? [{ type: 'label', label: reason }] : []),
    /* The labels are the ones the two buttons carried as their accessible
       names, word for word. They were already written for someone who cannot
       see the row, which is the same sentence a menu row needs. */
    { kind: 'checkout', label: 'Switch to this branch', icon: 'git-branch', disabled },
    /* A separator, because the two below are not the same kind of act as the
       one above: switching branches is where you are, merging and rebasing
       change what the branch you are on contains. */
    { type: 'separator' },
    { kind: 'merge', label: 'Merge into the current branch', icon: 'git-merge', disabled },
    { kind: 'rebase', label: 'Rebase the current branch onto this', icon: 'git-graph', disabled }
  ]
}
