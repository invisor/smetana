/* Which dialogs are OS windows of their own, how wide each one is, and the one
   rule that closes a window whose reason for being open has gone.

   Pure, with no Vue and no DOM in it, for the reason the rest of this family is
   pure: a `.vue` file is the one thing no test in this repository can reach, so
   the whole of a rule lives outside the component that draws it. The map from a
   kind to a component is therefore *not* here — it is in `DialogWindow.vue`,
   which is the only place that may import one.

   A dialog window stands on some ground: the project it belongs to, and often a
   task, a column or a branch besides. It is open because that ground exists, and
   when the ground goes the window has nothing left to be about. In the app
   window that never had to be said, because the scrim meant nothing could move
   underneath; a window somebody can push aside and click past has no such
   promise. */

/* Every kind, and the width each one's window gets. All but one draw at
   `Modal`'s default 440, which is what they drew at as modals, and there is no
   reason for that to change just because the frame did — the numbers are here
   so that a dialog which outgrows it has somewhere to say so, and
   `review-changes` is the first and so far the only one that does. */
const REGISTRY = {
  run: { width: 440, ground: ['project'] },
  'new-task': { width: 440, ground: ['project', 'column'] },
  'new-branch': { width: 440, ground: ['project', 'repo', 'branch'] },
  /* Deleting one. The same ground as cutting one and for the same reasons: the
     repository because every write in `stores/vcs.js` resolves which one it
     runs in at the moment it is pressed, and the branch because this window is
     entirely about a branch that exists — one deleted from a terminal while
     this stands open leaves nothing to answer about. */
  'delete-branch': { width: 440, ground: ['project', 'repo', 'branch'] },
  /* Renaming one. The same ground again and for the same two reasons: the
     repository because `renameBranch` in `stores/vcs.js` resolves which one it
     runs in from `vcsState.selected` at the moment Rename is pressed, and the
     branch because this window is entirely about a branch that exists under the
     name it opened on — one renamed or deleted from a terminal while this
     stands open leaves nothing for `git branch -m` to move. */
  'rename-branch': { width: 440, ground: ['project', 'repo', 'branch'] },
  'promote-column': { width: 440, ground: ['project', 'column'] },
  'setup-project': { width: 440, ground: ['project'] },
  /* Everything about one project that is not the board: `[defaults]` in the
     project's own `project.toml`, and the caveman level this machine uses while
     that project is open. The same ground as the setup window and for the same
     reason, which the second half only sharpens: the file belongs to the
     project, so a window left standing over a project somebody has clicked away
     from would save four numbers into the wrong repository — and, since
     `settings.project` is the active project's entry, would write a level into
     it as well. A window of its own rather than one more tab in the settings
     window, and the split is by subject rather than by file: this one is about
     one project, that one is about the machine, and a per-project row over
     there could only mean "whichever project the app window happens to have
     open". */
  'project-settings': { width: 440, ground: ['project'] },
  'delete-task': { width: 440, ground: ['project', 'issue'] },
  'ready-task': { width: 440, ground: ['project', 'issue'] },
  /* Deleting a Claude Code transcript. Its ground is the project and nothing
     else, and that is a decision rather than a gap: the other sorts of ground
     are sets this window keeps and can watch — the issues, the columns, the
     branches — and there is no set of sessions to watch. The list is read off
     disk when the tab is opened and deliberately never watched (see
     `sessions/mod.rs`), so a transcript that goes while this dialog stands open
     is not something the app can notice, and inventing a clause for it would be
     a check that never fires. What answers that case instead is the delete
     itself: `sessions_delete` says the transcript is no longer on disk, and the
     person reads it as a sentence rather than as a window vanishing. */
  'delete-session': { width: 440, ground: ['project'] },
  /* Choosing what an agent reviews: one pair of refs for the project — a
     reference branch and a branch to check, each either the local branch or
     what `origin` has — and the repositories it reaches, any of which may keep
     a pair of its own instead.

     **720 and not 440**, which is what the width field was put here for. It was
     bought for a table of four controls per row and is spent differently now:
     the branch list opens in the flow, a row full width, so a name like
     `feature/smetana-4nsa-remote-branches-repo` is read rather than guessed at,
     and a row of the table carries a repository, its path, its pair and what it
     is doing at once. The number is written twice — here and as `Modal`'s
     `:width` inside the component, which is what `?view=gallery` draws it at —
     and the two have to agree.

     Its ground is the project and nothing else, and that is a decision rather
     than a gap. This window is about the project's repositories as a set rather
     than about any one of them, so a repository going is a row leaving the
     table and not a window that has lost its reason to be open — which is the
     opposite of `new-branch` and `delete-branch` beside it, whose every write
     resolves against whichever repository the Git panel has selected at the
     moment it is pressed. Nothing here is resolved that way: each row carries
     the repository it is about. And the branch is not ground either, for the
     same reason — a row whose branch has gone is a pair git will refuse in its
     own words, which is a better answer than a half-filled table vanishing. */
  'review-changes': { width: 720, ground: ['project'] }
}

export const DIALOG_KINDS = Object.keys(REGISTRY)

export function isDialogKind(kind) {
  return Object.prototype.hasOwnProperty.call(REGISTRY, kind)
}

export function dialogWidth(kind) {
  return REGISTRY[kind]?.width ?? 440
}

export function dialogGround(kind) {
  return REGISTRY[kind]?.ground ?? ['project']
}

/* What each kind is called in a sentence, and what each sort of ground is called
   when it goes. Both are here rather than in the view for the same reason the
   rule is: this is copy a test can read.

   Sentence case, like everything else on screen. */
const DIALOG_NOUN = {
  run: 'run',
  'new-task': 'new task',
  'new-branch': 'new branch',
  'delete-branch': 'delete branch',
  'rename-branch': 'rename branch',
  'promote-column': 'promote column',
  'setup-project': 'project setup',
  'project-settings': 'project settings',
  'delete-task': 'delete',
  'ready-task': 'move to ready',
  'delete-session': 'delete session',
  'review-changes': 'review changes'
}

const REASON_CLAUSE = {
  project: 'the project changed',
  repo: 'the Git panel moved to another repository',
  issue: 'the task it was about no longer exists',
  column: 'the column it was about is no longer on the board',
  /* "It was about" and not "it started from", which is what this said while
     `new-branch` was the only window standing on a branch. That clause now has
     to serve a window whose branch is the one being deleted rather than the one
     being cut from, and a delete dialog announcing that the branch it started
     from is gone would name a relationship it never had. The wider wording is
     true of both. */
  branch: 'the branch it was about is gone'
}

/* `kept` is the New task dialog's, and it is an argument rather than something
   worked out from the kind: what the app window holds after this window goes is
   a draft that was reported before the switch, and whether there is one is a
   fact only the caller has. A promise made from the kind alone would be made to
   somebody who typed a sentence and switched inside the reporting debounce. */
export function stalenessMessage(kind, reason, kept = false) {
  const noun = DIALOG_NOUN[kind] ?? 'dialog'
  const clause = REASON_CLAUSE[reason] ?? 'what it was about is gone'
  const notice = `The ${noun} dialog closed: ${clause}.`
  return kept ? `${notice} What you wrote is kept.` : notice
}

/* Which of the open dialog windows have lost their ground.
   `world` is what the app window holds right now:
   `{ project, repo, issues: Set, columns: Set, branches: Set }`. */
export function staleDialogs(open, world) {
  return open
    .filter(({ kind, ground }) => Boolean(stalenessOf(kind, ground, world)))
    .map((dialog) => dialog.kind)
}

/* Why one dialog is stale, or null if it is not. Exported because the caller
   needs the reason for the sentence, and computing it twice would be two rules
   to keep in step.

   The project is checked first and on its own, because a project that has moved
   invalidates every other kind of ground at once — the ids in a different
   tracker are a different vocabulary, not a smaller one.

   The repository is checked the same way, by equality against the one the Git
   panel has selected, and **not** by membership of the list of repositories the
   project holds. The case this clause exists for is a repository that is still
   perfectly present and simply no longer the selected one: the panel's writes
   resolve which repository they run in at the moment they are pressed, so a
   dialog left standing over a repository somebody has clicked away from would
   cut its branch in the wrong one. A name like `main` exists in both, so no
   clause about branches could have caught it.

   The other three are checked only when the ground actually names one. A field
   that is absent is not a field that went: a dialog standing on the project
   alone would otherwise be closed by every world whose set of columns happens
   not to hold `undefined`. */
export function stalenessOf(kind, ground, world) {
  const needs = dialogGround(kind)
  if (needs.includes('project') && ground.project !== world.project) return 'project'
  if (needs.includes('repo') && ground.repo !== world.repo) return 'repo'
  if (needs.includes('issue') && ground.issue != null && !world.issues.has(ground.issue)) {
    return 'issue'
  }
  if (needs.includes('column') && ground.column != null && !world.columns.has(ground.column)) {
    return 'column'
  }
  if (needs.includes('branch') && ground.branch != null && !world.branches.has(ground.branch)) {
    return 'branch'
  }
  return null
}
