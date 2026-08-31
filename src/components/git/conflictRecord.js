/* What the panel knows about a conflicted tree, out of what git said, what the
   tree shows, and what was already held.

   Pure, of the `conflictsFirst.js` family and here for the same reason: a
   `.vue` file is the one thing no test in this repository can reach, and a
   store is where this rule would otherwise sit unwatched.

   There are two sources for one record and that is the whole of the problem it
   solves. The **press** that started a merge or a rebase knew both branches,
   because a person picked the second one off a row. The **probe** that reads a
   tree afterwards knows the operation exactly and the branches only as far as
   a git process will say — a stopped rebase leaves the branch it is going onto
   readable nowhere at all. So a refresh must not overwrite a name with
   nothing. */

/**
 * The record to hold now, or `null` for a repository with nothing to answer.
 *
 * `files` are the unmerged paths the tree shows **now**, and they are never
 * borrowed: a resolved path leaving the list is the one thing about a conflict
 * that changes while the dialog is open.
 *
 * A held name is borrowed only where the probe answered nothing, and only from
 * a record about the **same repository and the same operation**. Both halves
 * are load-bearing: a name from another repository would put one project's
 * branch in another's sentence, and a merge's `theirs` carried into a rebase
 * would name the wrong side of it — a rebase puts the branches on the opposite
 * sides of every sentence the dialog and the prompt write.
 */
export function conflictRecord({ repo, files, progress, previous }) {
  if (!files.length || !progress) return null
  const held =
    previous && previous.repo === repo && previous.op === progress.op ? previous : null
  return {
    repo,
    op: progress.op,
    ours: progress.ours ?? held?.ours ?? null,
    theirs: progress.theirs ?? held?.theirs ?? null,
    files
  }
}
