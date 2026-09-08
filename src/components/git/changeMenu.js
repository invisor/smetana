/* What a row of the Changes section offers on a secondary click.

   The `branchMenu.js` / `fileMenu.js` / `gitActions.js` family: pure, no Vue
   and no DOM, which is the whole reason it is a file of its own — no test in
   this repository can reach a `.vue`, so a rule left inside the component is a
   rule nothing checks. It is also one half of a pair nothing joins
   mechanically: these `kind` strings become named events in `ChangeList.vue`
   and land in `DesktopApp.vue`, the seam `fileMenu.js` and `onFileAction`
   already have, where a row renamed on one side draws perfectly and does
   nothing at all when pressed. The test pins this side.

   **Six rows, and the first of them is the gesture the row already had.** A
   click on a change opens its diff, and that verb had no name anywhere on
   screen until this menu; `branchMenu.js` states the rule it is here for — a
   menu is where somebody goes to find out what a place can do, so a place whose
   main action is missing from its own menu reads as a place that cannot do it.

   Beside it are the four that VS Code's own menu on this row was borrowed from.
   `Open file` is the working copy as an ordinary editor tab, which is the other
   half of what a person wants from a changed file and is not reachable from
   this panel at all otherwise. `Reveal in Finder` is the file tree's own row,
   word for word and through the same noun. `Copy path` and `Copy relative path`
   are that menu's pair, with the same labels and the same meaning — relative to
   the **project** root and not to the repository, since that is what the pair
   means everywhere else in this app and a second reading of two identical
   labels is how the two lists start disagreeing.

   **What was refused from that menu, so it is not proposed again.** *Stage
   changes*: the commit takes the whole tree — `commit_all` runs `git add --all`
   — so a row offering to stage one file would promise a choice the commit
   cannot make, which `ChangeList.vue`'s own header already fixes. *Open file
   (HEAD)*: that is the left-hand pane of the diff `Open changes` opens. *Open
   with*, *Open on remote*, *Share*, *File history*, *Stash* and *Copy changes
   (patch)*: there is no subsystem behind any of them.

   **The sixth row is the one that loses work**, and it is the whole of what
   this menu writes: *Discard changes*, in a group of its own at the very foot,
   behind a separator. `Delete this branch` sits the same way in `branchMenu.js`
   and for the same reason — a menu opened by a roughly aimed pointer must not
   put its one destructive row against the edge of the group above it. It is the
   only row here that reaches git at all, and the only one that needs a window
   to ask first.

   **So this menu now has two reaches of refusal rather than one.** Five rows
   read, and the fourth reach `branchMenu.js` describes is still theirs: a run
   in the project and an operation already going leave every one of them live.
   The sixth writes the working tree, so it takes exactly the refusal every
   write in this panel takes, in `frozen`'s own two sentences — lifted from
   `branchMenu.js` rather than written again, because two spellings of the same
   state read as two states.

   **And the two reaches are said in two different ways, which is not an
   inconsistency.** The five carry their reasons as a suffix on the label,
   because they are refused for *different* facts about the row and no caption
   can serve that. The sixth is refused by one fact about the whole repository,
   which is exactly the case a caption is for, so `frozen`'s sentence stands
   above its group and the row underneath says nothing about it. The one
   exception is the conflicted row, whose reason **is** about the row, and it is
   a suffix like the other five: a conflicted tree is the conflict dialog's
   business — abort or resolve — and putting one path back to HEAD behind its
   back would leave the merge half undone. Rust refuses it a second time, for
   the reason `vcs_discard` records: this window has no scrim and the tree can
   move while it stands. */

import { fileManagerName } from '../files/fileMenu.js'
import { frozen } from './branchMenu.js'

/* The three reasons, in the form `fileMenu.js` chose and for its reason:
   `ContextMenu` clips a row rather than wrapping it and gives it no tooltip and
   no `title`, so a reason kept anywhere else is a reason nobody reads. A suffix
   per row rather than one caption above a group — which is `branchMenu.js`'s
   shape — because the rows here are refused for *different* reasons, which is
   exactly the case a caption cannot serve. */

/* `--untracked-files=normal` is git's own default and this panel's, so an
   untracked directory arrives as one record with a trailing slash. There is no
   file behind that name: nothing to diff and nothing to open. It is still a
   place on the disk, so the reveal and both copies stay live. */
const NO_FILE_BEHIND_FOLDER = 'no file behind a folder'

/* Whether a record names a directory rather than a file, and **the one spelling
   of that question in this front end**. What asks it, named rather than
   counted, because a number written here is wrong the first time somebody adds
   a reader and does not notice: this menu greys its first two rows,
   `ChangeList.vue`'s click refuses to open a diff, `changeKeys.js` refuses
   Enter through the same answer, `icon()` draws a folder glyph instead of a
   file's, `label()` keeps the trailing slash on the drawn name, and
   `DesktopApp.vue` cuts that slash before joining an absolute path — six, and
   `.claude/rules/vcs-panel.md` carries the same list.

   Written out at each of those it is one character away from a menu row greyed
   over a click that is still live, or a row drawing a file icon under a greyed
   `Open changes`, with nothing on screen to say which is right —
   `BranchList.vue` records the same care in its own words about Enter and the
   double click reaching one `activate`.

   One place in `ChangeList.vue` deliberately keeps its own trailing-slash test
   and is not in that list: `directory()`, which normalises a string before a
   lexical split the way `dirname` does in `src/paths.js`. It is about the shape
   of the string rather than about whether there is a file behind the row, so
   widening what counts as a folder record must not reach it.

   Exported from here rather than from `ChangeList.vue` because this module is
   the one a test can reach, and because the greying is where the rule is first
   written down. */
export const isFolderRecord = (path = '') => path.endsWith('/')
/* A deleted file is gone from the working tree, so there is nothing for the
   editor to read and nothing for the file manager to select. The diff is the
   one thing that still means something — it is what shows the deletion. */
const GONE_FROM_TREE = 'the file is gone from the working tree'
/* A repository named in `[project].repos` may sit anywhere at all, so a change
   inside one can be outside the project root entirely. `openFile` takes a path
   relative to that root and `files_read` refuses anything outside it, and a
   relative path there would be a lie rather than a refusal — `relativeTo`
   answers `null`, which is an ordinary answer here and not a failure. */
const OUTSIDE_PROJECT = 'outside the project'
/* The one refusal of the discard that is a fact about the row rather than about
   the repository, which is why it is a suffix where `frozen`'s two are a
   caption. A conflicted tree belongs to `ConflictModal` and its two doors — an
   abort, or a resolution that finishes what git started — and a single path put
   back to HEAD from under either would leave the operation half undone with
   nothing on screen saying which half. */
const RESOLVE_FIRST = 'resolve the conflict first'

/* The label, with the first applicable reason on it and no reason at all when
   none applies. The order is stated rather than incidental: one row can be
   refused twice — `Open file` on an untracked folder in a repository outside
   the project — and the more specific fact is the one worth saying. A folder is
   the most specific, since it is about the row itself rather than about where
   the row is or what git did to it. */
const refuse = (label, reasons) => {
  const said = reasons.find(Boolean)
  return said ? `${label} — ${said}` : label
}

/**
 * The rows a change offers, given facts rather than a store.
 *
 * `path` is relative to the repository, as `vcs_status` answers it, and a
 * trailing slash is the untracked-folder record. `kind` is Rust's `ChangeKind`
 * through serde, so `deleted` is the spelling and not `delete`.
 * `insideProject` is whether `relativeTo(root, abs)` has an answer — a fact
 * about the repository rather than about the row, since every file in a
 * repository is inside the project exactly when the repository is. `userAgent`
 * is read for one noun, the way `fileMenuItems` reads it.
 *
 * `allowed` and `busy` are the two facts every write in this panel is refused
 * by — `gitActions(runs).allowed` and whether the store holds an operation —
 * and they reach the **last row alone**. They arrive as the two booleans rather
 * than as the verdict object and the `{ op, branch }` record, because the
 * sentence is `frozen`'s and neither the reason nor the operation is drawn
 * anywhere on this menu: what is wanted here is the answer, not the report.
 */
export function changeMenuItems({
  path = '',
  kind = '',
  insideProject = true,
  userAgent = '',
  allowed = true,
  busy = false
} = {}) {
  const folder = isFolderRecord(path) ? NO_FILE_BEHIND_FOLDER : null
  const deleted = kind === 'deleted' ? GONE_FROM_TREE : null
  const outside = insideProject ? null : OUTSIDE_PROJECT
  /* The last group's own two facts, and neither of them reaches a row above:
     `held` is about the repository and is said once in a caption, `conflicted`
     is about this row and rides on the label. */
  const held = frozen({ allowed, busy })
  const conflicted = kind === 'conflicted' ? RESOLVE_FIRST : null

  return [
    /* The click's own verb, named. It survives a deleted file deliberately: a
       diff of a deletion is the whole left-hand side against nothing, which is
       what somebody opening it wants to see. */
    {
      kind: 'open-changes',
      label: refuse('Open changes', [folder]),
      icon: 'git-compare',
      disabled: Boolean(folder)
    },
    {
      kind: 'open-file',
      label: refuse('Open file', [folder, deleted, outside]),
      icon: 'file',
      disabled: Boolean(folder || deleted || outside)
    },
    /* A separator, because the two above are about this window and the one
       below leaves it. */
    { type: 'separator' },
    /* The file tree's own row, through the same noun and the same glyph: one
       file shown in one file manager should not be two different sentences
       depending on which list it was reached from. */
    {
      kind: 'reveal',
      label: refuse(`Reveal in ${fileManagerName(userAgent)}`, [deleted]),
      icon: 'folder-open',
      disabled: Boolean(deleted)
    },
    { type: 'separator' },
    /* The pair that reaches nothing at all — not git, not the disk, not
       `settings.json` — so `Copy path` is the one row here that nothing can
       ever refuse. It is written out as a constant `false` rather than left
       off, `branchMenu.js`'s line: every other row carries the field, and a row
       missing it reads as a row somebody forgot rather than as one nothing can
       refuse. */
    { kind: 'copy-path', label: 'Copy path', icon: 'copy', disabled: false },
    {
      kind: 'copy-relative-path',
      label: refuse('Copy relative path', [outside]),
      icon: 'copy',
      disabled: Boolean(outside)
    },
    /* A separator, and it is doing what `branchMenu.js`'s last one does: the
       five rows above read and this one destroys, and a pointer that missed by
       a row must not land on the only act in this panel with nothing to undo
       it. */
    { type: 'separator' },
    /* The caption over that group, and only when there is one. It heads a group
       of one row, so there is no question of how far it reaches — the greying
       under it is the whole of what it is about. */
    ...(held ? [{ type: 'label', label: held }] : []),
    /* `undo-2` is the glyph the review window already spends on "put this back
       the way the rule had it", which is the same sentence one subsystem over,
       and `tone: 'danger'` is what says this row is not like the five above it.
       Both halves matter: `Delete this branch` is red in `branchMenu.js` and
       this row loses more than that one can — a branch's commits survive in the
       repository until they are collected, and an uncommitted change does not
       exist anywhere else at all. */
    {
      kind: 'discard',
      label: refuse('Discard changes', [conflicted]),
      icon: 'undo-2',
      tone: 'danger',
      disabled: Boolean(held || conflicted)
    }
  ]
}

/* How wide the panel of these rows may get, exported for `FILE_MENU_W`'s reason
   and living here for the same one: the ceiling is a fact about the longest
   **label**, and the labels are this module's. `ChangeList.vue` opens the panel
   with it and `Gallery.vue` draws the same rows at the same width, rather than
   the number being written out by hand in both.

   It is a ceiling and not a width — `ContextMenu` sizes itself by its widest
   row and clips anything past this with an ellipsis, giving that row no tooltip
   and no `title` to recover the rest from — so what it has to hold is the
   longest sentence a row can carry, and the panel never grows to reach it in
   the ordinary case. That sentence is `Reveal in file manager — the file is
   gone from the working tree`, 63 characters: the reveal on a deleted file on
   Linux, where the noun is the longest of the three. At the 5.7px a character
   `FILE_MENU_W`'s own measurement comes to, plus the 70px of chrome that file
   records, it wants 430. Room over it costs nothing; clipping the one sentence
   that says why a row is off costs the whole reason the sentence is there.

   Prefixed rather than `MENU_W`, the way `FILE_MENU_W` and `sessionMenu.js`
   prefix their own: two menus sharing one number is how a panel gets moved by a
   rewording somewhere else. */
export const CHANGE_MENU_W = 440
