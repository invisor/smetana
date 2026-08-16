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

**The three sections fold and two of them are dragged**, and the rule is `components/git/sectionHeights.js`
— pure, tested, of the `gitActions.js` family; `SectionHeader.vue` is the caption, which is a real
`<button>` so the keyboard and `aria-expanded` come for free, and `shell/Resizer.vue` is the strip
between them, at `orientation="horizontal"`. The state is **global**, in `settings.layout.gitSections`
rather than under the project: how tall somebody likes their branch list is a habit of reading, the
same argument `kanban` is global on. A folded caption **keeps its count** — folding the branches away
says "do not draw me the list", not "stop telling me there are nine".

A caption carries `divided`, the hairline above it, and the repositories deliberately do not: every
section here is `--row-h` and quiet, so with nothing between them the three ran together into one
column of rows and neither the captions nor the blocks under them read as blocks at all. It is the
same hairline `Panel` draws under its title, and the topmost caption goes without because the project
list above this panel already ends in one — two of them meeting would draw a 2px line. The rule sits
on the caption and not on the `Resizer` above it, since a folded section has no resizer and would
lose its separator exactly where the captions are stacked tightest. It is drawn inside the height,
which `box-sizing: border-box` is what makes true: a rule that added a pixel to two of the three
captions would put the measured row a pixel away from the drawn ones for the whole of the arithmetic
below.

The arithmetic is in **rows and not pixels**, so a stored height follows `--row-h` through both
densities and the app-wide font size, and a drag stops on a row boundary instead of leaving half of
one under the fold. The one pixel measurement is at the edge, in the component: a header *is* a row,
so it is what a row is measured by, and `getComputedStyle` is no use for it — `--row-h` is a `calc()`
over an unregistered custom property and comes back unevaluated, the trap `terminal/theme.js`
records. A `ResizeObserver` watches the panel and that header, because the window moves the first and
the density and font size move the second without re-rendering anything.

Which section absorbs the leftover is **derived, never stored**: the changes while they are unfolded,
then the repositories, then the branches. The filler's own separator is not drawn — there is nothing
on its side of the strip to take height from — which is also what fixes the direction of the other
two for good: the repositories are always above the filler and the branches always below it, in every
configuration of folds. Dragging to nothing stops at the minimum rather than folding, unlike a side
panel: a panel's rail had to invent a fold out of the drag because it carries no other affordance,
while a chevron here is always on screen, and a fold made by a gesture whose separator then
disappears is a one-way door. Double click gives a section back to its content.

**The stored number and the drawn number are two numbers**, the rule `panelWidths.js` states one axis
over, and here it is load-bearing rather than tidy: letting CSS shrink a section below the number it
holds was tried, and it turns the section's own drawn height into the next drag's starting point, so
every attempt to pull it up walks the stored number *down*. So a dragged section is `0 0 auto` at a
height clamped against the panel it is in now, and only a drag writes back. Three numbers earn their
keep in that clamp and all three were paid for by a defect: the filler's floor is honoured in the
clamp and deliberately **not** repeated as a `minHeight`, which took its floor out of the sections
above and drew a 260px panel's repository list as a clipped strip; the ceiling is **floored, not
rounded**, since a panel with room for 6.8 rows has room for 6 and the seventh comes back out of
whichever section had no claim; and a section nobody has dragged is owed one whole row — it gives way
on its own, but giving way to nothing draws the same sliver.

**The writes a branch row offers are in its right-click menu**, and the row itself draws a name and a
mark and nothing else. Merging and rebasing used to be two `IconButton`s that appeared under
the pointer — a control per row per verb, in a column that also carries a file tree, a change list
and a commit box — and they are `components/git/branchMenu.js`'s items now, beside a third the row
had all along without a name anywhere on screen: the checkout its own click performs. A menu is where
somebody goes to find out what a place can do, so a place whose main action is missing from its menu
reads as a place that cannot do it.

What that costs is a gesture nobody is told about: nothing on the row says the two verbs exist. It is
the deliberate trade, and the only thing paying it back is that the project list one level up answers
the same gesture the same way. **The refusal is one caption above the rows, not a clause on each**,
which is where this parts company with `shell/projectMenu.js`: there the items are refused for
different reasons and each has to say its own, while here one fact refuses all three — the branch is
the one already checked out, a run holds the repository, or git is mid-operation — and the caption
says it once. The current branch wins that caption even under a run, because somebody right-clicking
the row with the tick is asking about that row. The menu opens on **every** branch row including the
refused ones: a gesture that answers on some rows and does nothing on others reads as a broken row
rather than a refused one.

**The fourth item is the one that leaves the list longer than it found it**: `New branch from this`,
cut from the row that was clicked and never from HEAD, which is the whole reason it belongs to a row
rather than to the section header. It is last, in a group of its own, and it is the item that made
`branchMenu.js`'s refusals grow two different reaches — a run or an operation in flight refuses it
like everything else, but *being on the branch* does not, since cutting a branch from where you are
standing is the ordinary case. So "already on this branch" heads the three moving verbs and stops
there, and what says how far a caption reaches is the greying under it.

The name comes from `NewBranchModal.vue`, and the rule under it is `components/git/branchName.js` —
git's own documented refusals (`git help check-ref-format`), plus the one name the list already
holds. **It is deliberately allowed to be narrower than git and never wider**: `vcs_create_branch`
runs the real command and git's refusal comes back in git's words like every other refusal here, so a
name this rule passes and git rejects is an ordinary outcome the panel already draws, while a name it
refuses and git would have taken is a door somebody cannot open. What it buys is the moment before:
the button goes dead on the character that broke the name, with one line saying which rule that was,
instead of a dialog closing onto a red block behind it.

The dialog closes the moment `Create` is pressed rather than waiting on git, which is the shape of
every write in this panel — the spinner lands on the row the branch is cut **from**, since that is a
row already drawn and the new branch has none until the refresh. `busy` is keyed on that same branch
for the same reason. Its checkbox is `git switch -c` against `git branch`, and they are two commands
rather than a flag on one: the first writes the working tree and carries uncommitted work across, the
second writes one ref and touches the tree not at all. Somebody who cleared the box asked for the
second, and creating-then-switching-back would be two writes and a window where the tree sits
somewhere nobody asked for. `switch` and not `checkout -b`, because `checkout` takes pathspecs and
this panel has already paid for that once (see `vcs_checkout`'s note about the missing `--`); it
wants git 2.23 or newer.

The panel it opens is `overlays/PointerMenu.vue`, which is `MenuButton` anchored to a point instead
of to a trigger — teleported out of the document because every list here sits inside something with
`overflow`, flipped above the pointer when there is no room below, closed on a scroll anywhere
underneath. All of it was inline in `shell/ProjectList.vue` while that was the only caller; this made
it the second, and the component is the one copy both now use.

**Branch names group into folders**, the way GitLens does it: everything before a slash is a heading
and a row draws only the leaf, which is the width it buys back — under one heading the prefix is on
every row and the tail is the half that identifies a branch. The whole name still travels on the row,
because that is what the three writes are given. The rule is `components/git/branchTree.js` — pure,
tested, of the same family — and it nests as deeply as the name does rather than splitting once:
`fix/legacy/…` is already in this tree, and a rule that split at the first slash would draw a branch
still called `legacy/warehouse-geocode`.

**A folder stands where its most recent branch stood**, and that is the whole of how the grouping
keeps the promise `BranchList` opens with. The list arrives in `git::by_recency`'s order because the
branch somebody merges into every day is nowhere in particular alphabetically; grouping is a re-sort,
and this is the one arrangement of it that leaves what was worked on last at the top, heading or row.
Names with no slash in them — `main`, `develop`, half of any repository — stay exactly where they
were and never become a folder. A folded folder leaves its branches out of the list altogether and
its count is the only thing saying they are there. A heading is a `<button>`, like the section caption
above it, and is deliberately **not dimmed while a run blocks the three writes**: unfolding is
reading, and a heading greyed out beside rows greyed out because they cannot be pressed would be
saying something untrue about itself.

Which folders are unfolded is per project — `settings.project.branchFolders`, beside the file tree's
`expanded` — because a `feature/…` convention belongs to a repository where the section heights above
belong to a person. **`null` and `[]` are different states and the field is an `Option` in Rust for
exactly that**: `null` is "nobody has chosen here" and unfolds the folder the current branch is in, so
the tick saying where you are is on screen the first time; `[]` is somebody having folded them all,
and stays. With a plain list there would be no way to fold the last folder away — the empty list would
read as the first case and come back unfolded on the next start. `branchTree.js` resolves a press
against that seed and hands the panel a whole new list, which is what writes the seed out on the
first press. Nothing reopens a folder afterwards, and it does not need to: the only way to press a
branch row is to see it, so a branch checked out from this panel was in a folder that was open.

## Committing, and the message somebody does not have to write

**The commit takes the whole tree, and the button says so.** `vcs_commit` runs `git add --all` and
then `git commit -m`, so what a press takes is exactly the list the section is drawing, untracked
files included — the ordinary case here, where a change set is often mostly new files and `git commit
-a` would leave every one of them behind. The cost is stated rather than hidden: somebody who staged
one hunk by hand loses that distinction, and this app has no staging of its own to express it with.
Hence the count on the button, `commitLabel` — a button reading only "Commit" would leave the one
surprising thing about it unsaid. The empty message is refused **before** the add and not left to
git, whose own refusal is in good words but arrives with the index already rewritten behind it. No
`--no-verify`: a repository's hooks are part of what committing means there, and the price is the one
this module pays everywhere — there is no timeout in `run.rs`, so a hook that hangs hangs the panel,
exactly as a merge driver that hangs already does.

**The draft is per repository, in memory, and never in `settings.json`.** A project is often several
repositories and the sentences are about different work, so `vcsState.messages` is keyed by path; the
file holds what somebody *chose* about the app, and a half-typed sentence restored three days later
is not that. It survives folding the section away, since the panel is handed it rather than holding
it, and it survives a refusal — a commit git declined is one somebody is about to try again, and the
sentence is the thing they would otherwise type twice. It does not survive the project changing,
where it would be a message about other work sitting one keystroke from being committed here.

**The sparkle button is a read, and everything about it follows from that.** `vcs_suggest_message`
runs `git diff HEAD`, gathers the untracked paths separately — they are in no diff at all, so a
change set of nothing but new files would otherwise be described as empty — and hands the lot to
`agents::oneshot`, which is `claude -p` with no PTY: the same spawn `runs/usage.rs` makes, with the
login shell's `PATH` and a deadline with a kill behind it. Being a read is why it stays live while a
run holds the three writes (the line `BranchList` already draws for a folder heading), why it uses
its own `suggesting` flag rather than `busy`, and why its failure is a quiet line under the field
rather than the panel's "Git refused this operation" block, which would name a party that was never
asked. Its guard is the pair, project and repository, and that one earns its keep: an answer landing
after a switch would drop one repository's commit message into another's field.

What the harness can be asked is the harness's own business, so it rides on `Profile::oneshot_args`
beside `usage_command`, and the front end never learns which agent is configured — the button is
drawn for everybody and a harness with no non-interactive form says so in a sentence
(`OneshotError::Unsupported`) rather than being hidden. `commit_prompt` and `clean` are pure and
carry their tests in `oneshot.rs`: the patch is cut at 48 K with the cut **announced**, since a model
told the whole diff was there when it was not will describe the half it saw as the whole change, and
what comes back is taken as its first non-empty line with fences and quotation marks stripped —
belt and braces, because the instruction asks for one bare line and models add the fence anyway.

The layout is VS Code's, and each half of it is a decision rather than a copy. The **sparkle sits
inside the field**, at its right edge, because that leaves the commit button the whole width and the
whole width is what somebody aims at without looking — and because the two buttons are not the same
kind of thing anyway: one is about the sentence in the box it sits in, the other about the tree. The
field's placeholder carries the shortcut and **the branch the commit would land on**
(`messagePlaceholder`, pure and tested), which is the width's best use in a panel people work several
branches from, given a commit is the write with no undo here; `⌘` or `Ctrl` follows the platform, and
a detached HEAD names no branch rather than naming nothing. The field is a plain box and deliberately
not a `<label>`: a label would forward a click on the sparkle to the textarea, which is a press that
moves the focus on its way to doing nothing.

The box is drawn **stuck** to the top of the change list rather than pinned above it, and that is one
scroller rather than two: `sectionHeights.js` is untouched by it, and a panel too short for the box
scrolls to the button instead of clipping it off under the section boundary — which is what the
pinned version did at 260px. When it may be pressed, and the one sentence it says when it may not, is
`components/git/commitBox.js`, pure and tested, of the `gitActions.js` family.

The panel's writes share one rule and one field apiece. A branch row checks out, merges into the
current branch, rebases the current branch onto it or cuts a new branch from it, and the commit box
takes the tree;
`gitActions.js` — pure, tested, of the `branchChoice.js` family — is the whole of when any of them
may be offered, and it reads the project's **runs** and nothing else, so a session a person started
themselves never dims the panel while a batch mid-merge always does. `busy` (`{ op, branch }`) is
what makes it one at a time, and `writeError` carries git's stderr with the `op` that earned it,
since a block reading "did not switch branch" over a refused merge would name an operation nobody
asked for. The branch in `busy` may be **null**, and only for the commit: every other write names a
branch and guards it itself, while a commit is about the tree and on a detached HEAD there is
no branch to name — a guard in the shared path turned that into a button that did nothing and said
nothing.

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
