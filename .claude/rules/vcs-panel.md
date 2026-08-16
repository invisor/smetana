---
paths:
  - "src-tauri/src/vcs/**"
  - "src/components/git/**"
  - "src/stores/vcs.js"
---

# The Git panel: what only the binary can answer

`src-tauri/src/vcs/` is the one place in the tree that runs `git` as a process, and `git.rs`
(`.claude/rules/git-head.md`) is untouched by it. **The split is by mechanism, not by subject**: `git.rs` is what can be read off the
disk, `vcs/` is what only git itself can do — the state of a working tree, and later a diff, a
checkout, a merge. Folding them together fails in one direction by dragging a process spawn into a
file whose own header forbids one, and in the other by making the scope bar's branch pay for a
process on every window focus. `vcs/mod.rs` says so in its header, because a reader who asks where
git lives in this app finds two answers.

| file | what it does |
|---|---|
| `model.rs` | `Repo`, `Change`, `ChangeKind`, `WorkingTree`, `Branch`, `OpKind`, `MergeOutcome`, `VcsError`, the **pure** parse of `git status --porcelain=v2 -z --branch` and the reading of a conflict off it; the tests are here |
| `repos.rs` | what a project is made of — the pure rule, split from the directory read |
| `run.rs` | the only file that touches the OS |
| `commands.rs` | thin `#[tauri::command]`s, shaped like `files/`'s |

There is **no worker**, for the reason `files/` has none: `git status` costs tens of milliseconds
against a bd call's two seconds, and the module owns no snapshot — the front end holds the list.
Concurrent writes are serialised by git's own `index.lock`, whose refusal is shown as it is. The
machine-readable form and never the human one: `--porcelain=v2`'s output is documented and stable
where `git status`'s prose moves between versions, and `-z` is not tidiness — a path may hold a
space and it may hold a newline, and the non-`-z` form answers that by quoting, which would be a
second parser to get wrong. A rename is **two** records, the path and then the path it came from, so
reading it as one puts the original into the next record's slot and every change after a rename is
nonsense. An unrecognised record is skipped rather than refused: losing one row beats losing the
panel.

What a project is made of is one rule with two arms (`repos.rs`): `[project].repos` from
`.smetana/project.toml` when it is there and non-empty, in its own order, and otherwise the root
itself plus every directory **one level** below it that git can see. That second arm is the addition,
and it is for the folder holding five sibling repositories that nobody has set up for runs yet —
asking only the root would name the accidental repository that container happens to be, which is the
defect the run dialog already paid for once. It stops at one level on purpose: deeper is not a
fallback but a search, and it would find every vendored dependency with a `.git` in it. A name that
resolves to nothing readable is left out rather than shown broken, the rule `git::combine` keeps.
Each row's branch comes from `git::head` — a file read, so the whole list costs **no process at
all**.

`run.rs` builds the child's environment from `shell_env::path()`, exactly as `runs/preflight.rs` and
`terminal/pty.rs` do, and for the reason recorded there. `GIT_OPTIONAL_LOCKS=0` on every call, reads
and writes alike, so looking at a status never takes `index.lock` out from under an agent working in
the same tree — it suppresses only the locks git takes on its own account, an index refresh it did
not have to do, so the merge and the rebase still take the locks their own work needs. The
working directory is `current_dir` and not `-C`, so an odd character in a path never has to survive
being an argument. A missing `git` is `VcsError::NoGit` and never an empty list — anything
unobservable reads as "no", loudly (`runs/browser.rs`) — and a non-zero exit carries git's **own
stderr untouched**, because the person reading it knows git.

On the front end `src/stores/vcs.js` sits beside `git.js` and mirrors that same split; it is guarded
against its own stale response the way `git.js`, `terminals.js` and `runs.js` are. Which repository
is selected is remembered per project as `selectedRepo` in `settings.json`, validated in
`settings/model.rs` like every other field, and a stored path no longer in the list is silently
replaced by the first — a stored value is a hint, never the truth, the rule `columnOrder.js` states.
`components/git/` draws it: `GitPanel.vue` over `RepoList.vue`, `ChangeList.vue` and
`BranchList.vue`, with the pure
`changeStatus.js` saying what a change is captioned with. Four of its eight kinds — modified, added,
deleted, untracked — take the `--git-*` token the file tree already marks that file with
(`files/FileTreeRow.vue`), which is the whole of the agreement between the two: renamed, copied and
type-changed have no token there and take the neutral `--type-plain-fg`, and a conflict shares
`--git-conflict` while the letters differ, `C` here against the tree's `!`. Borrowing the four rather
than inventing a palette is the point; claiming the two lists match everywhere would not be true.
Each section has **its own empty state and they say different things** — no git on this machine
(naming what was looked for), no repository in this folder, nothing uncommitted in this repository:
one blank area for all three would be a panel saying nothing three different ways. Freshness is
window focus (`catchUp`), the project switch (`projects.js`, after the new layout has landed, since
the remembered repository lives in it) and the refresh button in the panel header. **No watcher, and
do not add one**: a third watcher subsystem would fire on every write inside `node_modules` and
`target`, and the price of the sweep is named — while an agent works, this list is as stale as the
file tree beside it.

The panel writes three times and they share one rule and one field apiece. A branch row checks out,
merges into the current branch or rebases the current branch onto it; `gitActions.js` — pure, tested,
of the `branchChoice.js` family — is the whole of when any of them may be offered, and it reads the
project's **runs** and nothing else, so a session a person started themselves never dims the panel
while a batch mid-merge always does. `busy` (`{ op, branch }`) is what makes it one at a time, and
`writeError` carries git's stderr with the `op` that earned it, since a block reading "did not switch
branch" over a refused merge would name an operation nobody asked for.

**A conflict is an outcome and not a failure, and it is read off the tree rather than off the
message.** `git merge`'s prose moves between versions where an unmerged record in `--porcelain=v2`
does not, so a non-zero exit is not an answer by itself: `run::git_attempt` hands the refusal back
instead of raising it, the tree is read through the very call `vcs_status` uses, and unmerged records
decide. Unmerged paths are `MergeOutcome::Conflict`; nothing unmerged is `VcsError::Git` with git's
own stderr, untouched.

**The tree is read twice — before as well as after — and the first read is what makes that rule
true.** git refuses to *start* either operation in a tree that already has unmerged entries ("Merging
is not possible because you have unmerged files", exit 128) and changes nothing, so those same
records are still in the porcelain afterwards; an "after" read alone reports somebody else's conflict
as this operation's. What that costs is not hypothetical: leaving a tree conflicted is this app's own
designed exit from the dialog, so one click later the modal would name a merge git never began, and
its Abort would run `git merge --abort` against whatever really is in progress and throw away
resolutions somebody had already staged. `model::new_conflicts` is the rule, pure and measured
against a real refusal. The price is one `git status` in front of an operation that rewrites the
working tree.

An unreadable "before" attributes nothing either — not knowing what was there is not evidence that
nothing was — and **what that arm costs is worse than it sounds, which is why it is written down
measured**: `refusal()` carries git's stderr, and a *merge* conflict writes nothing to stderr at all
(its "CONFLICT (content): …" goes to stdout), so a merge conflict lost to that arm draws "Git did not
merge" over an empty message block. A rebase keeps its words there, since `error: could not apply …`
does go to stderr. Neither draws the conflicted files: `write()` in `stores/vcs.js` sets `writeError`
in its catch and returns, where the refresh is on the success path, so the tree stays as the panel
last read it until the next window focus or a press of refresh. It is still the cheap side of the
trade — the other side offers an Abort that destroys somebody else's staged work — but a cost
recorded lower than the real one is what invites the arm to be inverted later.

**And the rule is a comparison of two moments rather than a lock.** An agent that starts a
conflicting merge in the same tree between the pre-read and the spawn leaves the "before" clean and
the "after" unmerged, and its conflict is attributed to us exactly as the one-read version attributed
every one. The window is the tens of milliseconds between two `git status` calls against a failure
that used to be certain, and no arithmetic over those two lists closes it: only asking git what is
*in progress* would — a `MERGE_HEAD` / `rebase-merge` probe — which is a file read in the module
whose header forbids one, and deliberately not taken.

**What the app then offers is two doors and no third**, because there is no merge editor here and
this epic adds none: `ConflictModal.vue` has no close button, and `overlays/Modal.vue` closes on
neither Escape nor the scrim, so `closable: false` is the whole of it. A conflicted tree behind a
closed dialog is a state this panel promises to show and cannot draw. **Abort** is `git merge
--abort` or `git rebase --abort` — nothing was committed, so nothing is lost — and git's refusal of
the abort is drawn *inside* the dialog, since a message behind a dialog with no dismiss is one nobody
can see. **Resolve with an agent** is `Intent::ResolveConflict`, the same idiom "Ask agent to edit"
and "Answer questions" already use, with the tree left exactly as git left it.

That intent carries the whole of the moment — the repository, which of the two operations, both
branches and every conflicted path — where `ResolveTask` deliberately carries almost nothing: a
parked task's questions are in the issue and bd can be asked again, while a stopped rebase leaves
HEAD detached, so the branch it moved off is readable nowhere afterwards (which is why `ours` is read
*before* git is asked). The operation rides as `op` and not `kind`, because `Intent`'s own serde tag
is `kind`. `SessionWork::ResolveConflict` keeps the repository and the branch coming in and leaves
the paths behind, the way `NewTask` leaves its images. **No skill was added to the library for it**
and none is named in the prompt: `smetana:merging` is the neighbouring process and the wrong one — it
is about a *task's* worktrees, its gates and its fast-forward — so the instruction rides as prose in
`prompt.rs`, which says exactly two things: resolve the conflict, and **finish** the merge or the
rebase, never `--abort`. That last is a named refusal rather than a silence, because an agent that
tidies up by aborting has undone the only thing it was asked to do and leaves a clean tree behind,
which is the one way this fails that looks like success.
