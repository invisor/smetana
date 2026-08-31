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

   Beside the switch sit the two items that only read: comparing this branch
   with the one the repository is on, which opens a window and touches the
   repository not at all, and asking an agent to review what it changed. Both
   are verbs about a different branch like the switch above them, which is why
   they are in that group and not in the writes' one.

   **Compare shows and Review judges**, and the two rows stay two rows for that
   reason. The comparison is a diff viewer against HEAD and nothing else; the
   review picks a reference branch and a branch to check — in one repository or
   in several, each side local or `origin` — and puts an agent on the difference
   with a written report at the end. Folding them together was considered and
   dropped: the compare window is built throughout on the pair being
   (HEAD, one branch), from the shas it resolves once to what its own header
   says, and a second base would change every one of those decisions in a window
   that does its own job perfectly well.

   Beside the comparison sits the one item that touches git not at all: marking
   this branch so the panel keeps it above the tree. It is a write to
   `settings.json` and a fact about how somebody reads this list, which is why
   nothing refuses it — not a run, not an operation in flight, and not the row
   being the branch already checked out.

   Beside the favourite sits the one item that reaches nothing at all: putting
   this branch's **whole** name on the clipboard, because it is wanted for a git
   command somewhere else. The row draws a leaf under a folder heading, so the
   name a person can see is not the name they need; this is where the whole one
   comes from.

   The last three are the ones that are not about a branch as it stands: cutting
   a new one from this row's commit, renaming this one, and deleting it. They
   change whether a branch exists at all or what it answers to, and the delete
   sits in a group of its own rather than with the other two. `Delete this
   branch` is last because it is the only item here that loses work, and a
   destructive row is worth the separator that keeps a roughly aimed pointer off
   it. It is also refused differently — cutting from where you are standing is
   the ordinary case, renaming where you are standing is what `git branch -m`
   is for, and deleting where you are standing is impossible — so one group
   holding all three would grey a third of itself and leave the caption reaching
   over two live rows into a dead one.

   That last property is one this file used to have whole and now has in part,
   which is worth saying plainly rather than leaving to be discovered. Once the
   favourite arrived in the first group there is no arrangement in which the
   greyed rows are one unbroken run: `Add to favourites` is live under every
   caption there is. So the caption is read as being true of whatever is greyed
   below it rather than of a contiguous block, and the items are grouped by what
   they *are* — read, write, create, destroy — with the greying following.

   **The reason is a caption, not a suffix per row.** One fact refuses a whole
   group at once, so the sentence sits above the group and is said once.
   `projectMenu.js` used to be the counter-example and is now the second case of
   the same rule: it suffixed its one refusal onto both of its refused labels,
   and `ContextMenu` — which clips a row rather than wrapping it, and gives it
   no tooltip and no `title` — cut the sentence off mid-word on each of them. A
   suffix is for a menu whose rows are refused for *different* reasons; neither
   of these two ever was.

   **Three refusals of different reach**, which is the whole shape of this rule.
   A run or an operation already going refuses everything that writes, caption at
   the top. The branch being the one already checked out refuses the three verbs
   about moving between branches and the comparison beside them — a new branch
   cut *from where you are standing* is the ordinary case, not an edge one — so
   that caption heads those four and the last group stays live below the
   separator. The third reach is the narrowest and it arrived with the
   comparison: that item reads and writes nothing, so `held` does not reach it at
   all, and a caption saying "not now" can therefore stand over one row that is
   still live. What says how far a caption reaches is the greying under it: the
   live row is visibly not part of the group.

   The favourite is the **fourth** reach and the narrowest of the lot: nothing
   refuses it at all. Its neighbour the comparison still reads the repository,
   so a branch has no difference from itself to draw and the current row refuses
   it; this one writes a preference and reads nothing, so it stays live on every
   row in every state.

   Copying the name shares that reach without widening it either, and it has the
   plainest claim to it of the three: it writes nothing anywhere, not even a
   preference, so there is nothing for a run, an operation in flight or the tick
   on this row to refuse.

   The review is the **fifth**, and it shares the fourth's reach rather than
   widening it: nothing refuses it either. It reads, it writes only inside
   `.smetana/`, and it takes no git lock — so neither a run, nor an operation in
   flight, nor the row being the branch already checked out has anything to
   refuse. The current branch does not refuse it the way it refuses the
   comparison: a review whose two sides are the same ref is a table somebody can
   look at and change, and whether it is worth running is visible in the window
   itself rather than decided by a grey menu row. */

/* What refuses the whole menu, in order of what is worth saying. Both mean "not
   now" rather than "not this row", which is what puts either at the very top. */
function frozen({ allowed, busy }) {
  if (!allowed) return 'A run is going in this project'
  if (busy) return 'Git is working in this repository'
  return null
}

export function branchMenuItems({
  current = false,
  allowed = true,
  busy = false,
  favorite = false
} = {}) {
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
    /* It reads. A run in this project and an operation in this repository both
       refuse everything that writes, and this writes nothing — so the caption
       above may say "not now" while this row stays live under it, which is a
       third reach in a file that had two. What still refuses it is the row
       being the branch already checked out: a branch has no difference from
       itself to draw. */
    { kind: 'compare', label: 'Compare with the current branch', icon: 'git-compare', disabled: current },
    /* The other reader, and the one item in this menu that carries an ellipsis.
       That is a deliberate exception and it is the only one: the note on
       `New branch from this` below says this app spends no ellipsis at all, on
       the grounds that one row keeping a convention nobody else keeps reads as
       a typo. It is kept here by a person's own choice, made with that
       objection in front of them — every other row in this menu is over in a
       second, and this one opens a form and then starts an agent, which is a
       difference worth saying on the row. If the convention is ever levelled
       across the app, this is the row it was broken for.

       Nothing refuses it: it reads, it writes only inside `.smetana/`, and it
       takes no git lock. Not even the current branch, unlike the comparison
       above — a review of a branch against itself is a table with both sides on
       one ref, which somebody can see and change in the window. */
    {
      kind: 'review',
      label: 'Review this branch…',
      icon: 'search-check',
      disabled: false
    },
    /* The one item in this menu that asks git for nothing. It writes a line in
       `settings.json` and moves a row up the list, so there is nothing for a
       run, an operation in flight or the tick on this row to refuse. `disabled`
       is written out as a constant `false` rather than left off: every other
       verb here carries the field, and a row missing it reads as a row somebody
       forgot rather than as one nothing can refuse.
       The label is the act and not the state: a row already marked offers the
       way back out, which is the whole of what tells somebody the mark is
       theirs to remove. */
    {
      kind: 'favorite',
      label: favorite ? 'Remove from favourites' : 'Add to favourites',
      icon: 'star',
      disabled: false
    },
    /* Beside it, and the only item in this menu that reaches neither git nor
       `settings.json`: it puts the branch's name on the clipboard because
       somebody needs it for a git command somewhere else. The **whole** name
       and never the leaf the row draws — `fix/spike` is what a command takes,
       and `spike` is a string that would fail somewhere the panel cannot see.

       Refused by nothing at all, which is the favourite's reach and for a
       stronger reason: this writes nothing anywhere. Not a run, not an
       operation in flight, and not the row being the branch already checked
       out — the name of the branch you are standing on is exactly as copyable
       as any other. */
    { kind: 'copy-name', label: 'Copy branch name', icon: 'copy', disabled: false },
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
    },
    /* In that same group and above the separator, because it is the other item
       that changes what a branch is called rather than what it contains — and
       it is refused exactly as its neighbour is, by `held` and by nothing else.
       The row with the tick is deliberately live: `git branch -m` renames the
       branch HEAD is on and HEAD travels with the ref, so a typo in the name of
       the branch somebody is working in is the ordinary case rather than the
       edge one. That is the whole reason it is not down in the delete's group,
       where `current` refuses everything. */
    {
      kind: 'rename',
      label: 'Rename this branch',
      icon: 'pencil',
      disabled: Boolean(held)
    },
    { type: 'separator' },
    /* Its own group at the foot, and the separator above it is doing two things
       at once. It keeps the destructive item apart from the one that creates,
       which is worth a line on its own in a menu opened by a gesture people aim
       roughly; and it keeps this row's greying off that one's, since this is
       refused on the current branch where `New branch from this` above it is
       not. That is a claim about these two rows and no more — the header above
       says why the menu as a whole no longer has an unbroken run of greyed rows
       to protect. Refused here in the menu and refused again in Rust: the window
       that asks the question is a window of its own, and HEAD can move while it
       stands. */
    {
      kind: 'delete',
      label: 'Delete this branch',
      icon: 'trash-2',
      disabled: moving
    }
  ]
}
