---
paths:
  - "src-tauri/src/vcs/**"
  - "src-tauri/src/window.rs"
  - "src/components/git/**"
  - "src/stores/vcs.js"
  - "src/stores/compare.js"
  - "src/views/CompareWindow.vue"
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

**A configured list cannot grow on its own, and the panel says so rather than staying quiet about
it.** Somebody clones a repository into their project from a terminal, and for a project with a
`[project].repos` it never appears — the refresh button re-reads the file and gets the same list,
which is correct and stays. What was wrong was the silence, so `repos.rs` carries a second pure rule
beside `names`: which folders in the listing the configuration does not name. `discover` reads the
listing in **both** arms now and answers `ProjectRepos { repos, unlisted }`, which is the cost bought
back deliberately — one `read_dir` plus a `.git` stat per entry on every window focus, exactly what
every project *without* a configuration already pays. In the unconfigured arm the answer is empty by
construction, never by a branch, which is what keeps it from being a second concept. `stores/vcs.js`
holds it as `unlisted` and clears it everywhere `repos` is cleared — the error path and `reset()` —
so a read that failed never leaves a sentence standing about a directory nobody looked at.
`GitPanel` draws it at the foot of the repositories section, in `--row-h` rows so the arithmetic
above is untouched: a caption, one muted mono name each, and the `settings-2` gear that opens the
**same** setup dialog the project row's menu opens. Nothing / one name / several is
`components/git/unlistedRepos.js`, pure and tested. **The panel gains no verb of its own**: writing
`.smetana/project.toml` from here was rejected on the file's own contents — it is comments and prose
throughout, and any round trip through `toml::to_string` destroys all of it — so the setup agent
stays the only thing in this app that writes it. Merging the two arms was rejected too: the
configured list is what `runs::commands::target_branches` merges into, and offering a repository runs
know nothing about would trade one silence for a louder lie.

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
than inventing a palette is the point; claiming the two lists match everywhere would not be true. The
icon between the letter and the name is borrowed the same way — `src/catppuccinIcon.js`, the tree's
own table — and it is the third mark on a row that already carries a staged tick and a coloured
letter, which is the most this row can hold. Unlike the other two it is in colours this app did not
choose, and the cost is measured: on a modified `.js` the status letter and the icon are within one
degree of hue of each other. It was accepted with the set; if this row is ever trimmed back, that
glyph is the first thing to go.
Each section has **its own empty state and they say different things** — no git on this machine
(naming what was looked for), no repository in this folder, nothing uncommitted in this repository:
one blank area for all three would be a panel saying nothing three different ways. Freshness is
window focus (`catchUp`), the project switch (`projects.js`, after the new layout has landed, since
the remembered repository lives in it) and the refresh button in the panel header. **No watcher, and
do not add one**: a third watcher subsystem would fire on every write inside `node_modules` and
`target`, and the price of the sweep is named — while an agent works, this list is as stale as the
file tree beside it.

The list is **read from outside this panel too**: `dirtyCount`, the scope bar's uncommitted-files
counter (`.claude/rules/git-head.md`). It is deliberately nothing more than
`tree.changes.length` — every kind, staged and unstaged and untracked and conflicted alike — so that
the number in the bar is the number of rows here and can be checked by looking rather than by
counting. It is `null` and never `0`
for a tree that could not be read, the same opposition `tree` itself keeps, and the bar draws no
counter at all for it. Its freshness is this store's freshness and there is no second mechanism: the
counter ages with the list, which is the price named in the paragraph above, and a watcher added for
the bar's sake would be the watcher this panel refuses.

**The three sections fold and two of them are dragged**, and the rule is `components/git/sectionHeights.js`
— pure, tested, of the `gitActions.js` family; `SectionHeader.vue` is the caption, which is a real
`<button>` so the keyboard and `aria-expanded` come for free, and `shell/Resizer.vue` is the strip
between them, at `orientation="horizontal"`. The state is **global**, in `settings.layout.gitSections`
rather than under the project: how tall somebody likes their branch list is a habit of reading, the
same argument `kanban` is global on. A folded caption **keeps its count** — folding the branches away
says "do not draw me the list", not "stop telling me there are nine".

**One of those three folds is overruled on the way in.** Arriving on the Git tab with anything
uncommitted in the selected repository draws Changes open whatever is stored, because that list is
what somebody came back for; inside the visit it folds on a press and stays folded until they leave
the tab and return. The rule is `components/git/changesFold.js` — pure, tested, of the
`sectionHeights.js` family. **What it deliberately does not do is write `changesOpen: true` on the
way in**: a stored `false` would then survive only a clean tree, and folding this section away would
stop meaning anything as a preference. So the visit is two fields held in `DesktopApp.vue` — an
override and an arm — and neither reaches `settings.json` at all, which is the one place the global
fold above is not the whole story.

A visit is any move of `project.sideTab` onto `'git'` — a press or a line of code — plus the app
starting and the project changing with Git already the open tab. Those last two are what the
predicate is really for: nothing in this app assigns that tab `'git'` today, every programmatic
switch goes to `'agents'`, so the interesting arrivals are the ones nobody clicked. One predicate
with no exceptions, because a rule that told a press apart from an assignment would be visible
nowhere on screen. The arm is what makes any of it work against a `loadRepos` nobody awaits, and it
is the **first known answer of the visit** that settles it — the moment `vcsState.tree` is
*replaced*, which `loadStatus` always does rather than writing into the object already in hand.
Deliberately not "the moment it stops being `null`", which is how the task that built this described
it: that is true of one project sitting still, and false of the case the rule exists for, since on a
switch the tree goes from the departing project's object straight to the arriving one's and passes
through `null` not at all. So the watch is on the identity, and a count would not serve — it fires
only when the number moves, and six changes in one project followed by six in the next would arm a
visit for good. Everything follows from that. A refresh
under somebody already sitting on the tab, by focus or by the panel's own button, unfolds nothing,
because the visit was spent long before it; a clean tree that goes dirty mid-visit is the same case
and also draws nothing new; and a read that failed leaves the visit still waiting rather than reading
as a clean tree, the `null`-and-never-`0` opposition again. The press on the caption stores the
inverse of what is **drawn** rather than of what is stored, or the first press under a forced-open
section would write `true` and fold nothing, and it spends the visit in both directions — a
`vcs_status` still in flight must not reopen what was just folded.

**A count about another project is not an answer, and the project switch is the case that proves
it.** `moveTo` sets the active project synchronously and only reaches `loadRepos` after an awaited
layout read, and `loadRepos` deliberately leaves `vcsState.tree` standing rather than clearing it —
so for a moment the store holds the departing project's tree under the arriving project's name. A
visit deciding from it would draw Changes open over an empty list one way and, the other way, spend
itself on a clean tree that the arriving project's changes can then never open, which is this
feature failing in exactly the entry case nothing on screen would explain. So the count is read only
when the store is about this project and settled: `vcsState.project`, which is the guard token every
call in `stores/vcs.js` already checks itself against, and `loading`, which is no such thing — a
flag nothing in that store guards on, wanted here for the narrower window where `loadRepos` has
claimed the new project before its first `await` and the tree in hand is still the previous one.
Anything else arms the visit instead, and an arm is never wrong here, only slower. The predicate is
`answeredCount` in `components/git/changesFold.js` rather than a condition in the view, which is
this family's whole reason: the one defect this feature shipped with lived in the half that was
inside the `.vue`, where no test in this repository could reach it.

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
which `shell/projectMenu.js` now does too: it suffixed its one refusal onto each of its two refused
labels until the sentence was found clipped mid-word, a clause per row being for a menu whose rows
are refused for *different* reasons. Here one fact refuses all three — the branch is
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

**The branch the repository is on is the first row, always.** `branchTree.js` lifts it out of the
tree before the tree is built, so it is on screen whatever the reflog said and whatever fold its name
would otherwise put it behind — the row Pull and Push in the caption are about, and the one fact
somebody opens this section to read, was reachable only by unfolding a `feature/` heading before this.
It draws its **whole** name, since there is no heading above it carrying the prefix, and it carries
`SectionHeader`'s own hairline under it so the list proper reads as starting below. That rule sits
inside the row's `--row-h` and adds no height to it — `box-sizing: border-box` — so the arithmetic
over `BRANCH_ROWS` is untouched. It is lifted rather than copied: the tree below never holds it, a
heading it was the whole of is not drawn at all, and `folderBehind` passes over it for the same
reason, since a fold cannot be hiding a row that is at the top. It scrolls with everything else — what
was asked for is an order, and a row pinned against the top of a box capped at a handful of rows would
spend one of them on every scroll.

Which folders are unfolded is per project — `settings.project.branchFolders`, beside the file tree's
`expanded` — because a `feature/…` convention belongs to a repository where the section heights above
belong to a person. **`null` and `[]` are different states and the field is an `Option` in Rust for
exactly that**: `null` is "nobody has chosen here" and unfolds the folder the current branch is in;
`[]` is somebody having folded them all, and stays. That seed was about the tick, back when a fold
could take the current branch off the screen; what is left of the argument now that it cannot is the
rest of that folder — the branches beside the one being worked on are the ones most likely to be
wanted next. With a plain list there would be no way to fold the last folder away — the empty list would
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
`--no-verify`: a repository's hooks are part of what committing means there, and the price is
`run::WRITE_CEILING` — a hook is somebody else's program, so the commit is allowed five minutes and is
then stopped, with git given the chance to take `index.lock` back off the disk on its way out.

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

## The remote: what is behind, and the two verbs that reach it

**Network is the second way this module runs git, and it is a second function rather than a flag.**
What makes it its own pair of functions is the environment rather than the number: nobody pressed it
(a window came back into focus) and there is nobody for git to ask (there is no terminal on this
process, so a prompt for a password is at best a dialog from some other program and at worst a wait
with no end). So `git_network` and `git_network_attempt` add `GIT_TERMINAL_PROMPT=0`,
`SSH_ASKPASS_REQUIRE=never`, `GIT_SSH_COMMAND="ssh -o BatchMode=yes"` and a **60 second deadline**.
`StrictHostKeyChecking` is deliberately left alone: what a machine trusts is not this function's
business.

**Every local call has a ceiling too, and there are two of them.** `run.rs` used to record having
none as a decision — a commit hook that hangs was started by somebody watching the screen, and
stopping their build would be this app inventing a policy for a repository it knows nothing about.
That argument held while the module only read; it lost the day the module started writing. There is
no worker in `vcs/` and no queue, so a git that never returns is an IPC call that never answers: the
button stays inert, nothing on screen says why, and the way out is restarting the app. Somebody
standing over a hook can watch it in their own terminal; nobody can press a dead button, and a call
that comes back can be pressed again. So a call site says which kind of call it is by the function it
names — **`run::git_read` under `READ_CEILING` (30 s), `run::git_write` and `run::git_attempt` under
`WRITE_CEILING` (300 s)**, with `git_maybe` and `git_bytes` reads by construction. Thirty seconds is
two orders of magnitude over the 220 ms a cold `git status` measures on this repository. Five minutes
is what the first repository this ships against declares for itself: `core.hooksPath = .beads/hooks`,
where every hook wraps `bd hooks run` in `timeout ${BEADS_HOOK_TIMEOUT:-300}`. **The two ways of
being wrong are not symmetric, and that is what sets the number** — a ceiling too low is a hard
failure with no way round it from inside the app (the commit is killed, pressed again, killed again,
and committing there is impossible), while a ceiling too high costs one wait on a hang that should
not have happened and ends in an error somebody can act on. A ceiling turns "forever" into
"eventually"; it is not a performance budget for somebody else's hook. It is affordable at that
length only because the work is off the runtime: a stuck button no longer takes a tokio worker with
it.

**A stopped write is not a clean no-op, and there is no door in the panel to what it leaves.**
`WRITE_CEILING` governs `checkout`, `merge` and `rebase` as well as the commit, and a merge or a
rebase killed part-way leaves `MERGE_HEAD`, or `rebase-merge`/`rebase-apply` depending on which
backend the repository is configured for, on the disk with a half-updated tree behind it. `ConflictModal` does not open on that: it opens on `MergeOutcome::Conflict`, which is a
tree read *after* an operation that finished. So what a person gets is the timeout sentence and a
repository mid-operation, and the way back is git in a terminal. That door is deliberately not built
— naming the state is the whole of what is owed here.

**A held `index.lock` is not one of the hangs this protects against**, and the correction is worth
keeping: git does not wait on that lock, it refuses immediately (measured on 2.34.1 —
`fatal: Unable to create '...index.lock': File exists.` at exit 128), which reaches the panel as an
ordinary refusal in git's own words. The lock matters on the other side of the ceiling, as the thing
a killed git would have left behind.

**The stop is SIGTERM first, then SIGKILL, and the grace is not politeness.** git removes the
`*.lock` files it holds from a signal handler and re-raises; SIGKILL cannot be caught, so a write
killed outright leaves `.git/index.lock` behind and every later git command in that repository
refuses with "Another git process seems to be running" until somebody deletes the file by hand —
a worse state than the hang the ceiling exists to end. Measured on git 2.34.1 against a `pre-commit`
hook that sleeps: SIGTERM to the group leaves no lock, SIGKILL leaves one. **The two-second grace is
slept through whole and the child is not watched during it**, which reads like waste and is not: git
dies promptly on SIGTERM, so a version that returned the moment it was reaped would skip the SIGKILL
in exactly the case it exists for — a hook that ignored the signal and is still running. Waiting for
the reap and signalling afterwards is not the alternative either, since a reaped pid can be recycled
and the group named would be somebody else's.

`bounded` and `terminate` are the one part of `run.rs` under test, and the tests are the shape of
those two rules: a script that floods either pipe, one that outstays its deadline and is then looked
for in the process table (a zombie still answers `kill(pid, 0)`, a reaped child does not), one whose
grandchild ignores SIGTERM so the kill behind the grace is the only thing that can reach it, and two
that drive **real git in a real repository** — a `textconv` helper that hangs a read, and a
`pre-commit` hook that hangs a commit, after which the test commits again to prove the repository is
not left refusing.

**How every one of those waits is done is `bounded`, and it is deliberately not the poll-and-kill
loop in `agents::oneshot::ask` it started as.** That loop reads both pipes only once the child is gone, and
its own comment names the precondition that makes it safe: the output is bounded, one line asked
for. Nothing here is. `git pull` writes a merge diffstat of one line per changed file and
`git fetch --prune` a line per updated ref, so past the 64 KiB a pipe holds git blocks in `write`,
`try_wait` never answers `Some`, and the deadline kills a git that had **already written the merge
commit** — under a sentence saying this app stopped it. So **every pipe that is opened is drained on
a thread of its own** while the wait happens, and a caller with nothing to do with standard output
says so (`Capture::Discard`) and it is never opened at all — which is why `git_network` and
`git_write` return `()`. A read asks for it and pays for the second thread.

The stop is a kill **and** a reap, and both halves are load-bearing. `Child` has no reaping `Drop`,
so a `kill` with no `wait` behind it leaves a defunct process for the lifetime of the app — and this
is the call a five-minute sweep makes, so a blackholed route is a hundred zombies a day against
`kern.maxprocperuid`, after which the app cannot fork at all and nothing on screen says why. And the
signal goes to the **process group**, as `runs/preflight.rs::terminate` does and for its reason: the
process actually blocked is often a child of git's — `ssh` or `git-remote-https` on a fetch, the hook
itself on a commit — and killing git alone leaves it holding both the connection and the stderr pipe.
That twin stays on a bare SIGKILL deliberately: a declared command holds no lock file of ours, and a
person pressing stop must not wait two seconds for a program that has already been asked once.

**Every command in `vcs/` runs its work in `spawn_blocking` and none of it in the body of the
`async fn`** — `off_the_runtime`, or `off_the_runtime_or_empty` for the three that are documented as
never refusing and so have no error to hand back a failed join. This started as the three networked
commands alone, the first that could take a minute by design; it is the general rule now that every
call has a length this app has committed to waiting. Every IPC call in the app — the file tree, the
editor, the tracker, the terminals — shares the runtime these commands are polled on, so a git that
is merely slow would otherwise take workers out of everything else on screen with nothing saying
why. `vcs_suggest_message` keeps its own wrapper, since what comes back from it is
`OneshotError`.

An expired ceiling is **`VcsError::Timeout`, its own variant with its own `kind()` of `"timeout"`**,
and never a `Git { stderr }` with an empty message. git said nothing; this app decided, and the
sentence says so ("Smetana stopped git after 300 seconds — it had not finished"). One variant for all
three ceilings, and the sentence says only what is true of all three: it named the remote while only
the networked calls could produce it, which would now be a lie about a commit hook, and a second
variant would have carried this same `kind` — nothing on the front end could have told them apart,
and the operation is already named by the panel's own heading over the message. It **does** reach the
editor's table of refusals now (`ERRORS` in `stores/files.js`), since a diff's left-hand side is
`vcs_file_at_head` and that is three git calls; `timeout` is a key there for that reason, and without
it the fallback would draw "Could not read this file." over a git this app stopped.

**Where a branch stands against its upstream is `vcs_tracking`, a separate command, and folding it
into `vcs_branches` would end two documented properties of that one.** `vcs_branches` spawns no
process at all (three file reads through `git.rs`), which is why the branch list can be re-read on
every window focus, and it cannot refuse, which is why a project holding a folder that is not a
repository still draws a list. One `git for-each-ref --format='%(refname:short)%00%(upstream:short)%00%(upstream:track,nobracket)' refs/heads`
answers for the whole list in one process, and the front end merges the two answers by name — a
branch in one and not the other draws no mark until the next refresh, which is the freshness this
panel already promises everywhere else. Rejected: the `# branch.ab` line of `git status
--porcelain=v2 --branch`, which is already parsed and free, and answers only about the current
branch, where the mark is wanted on every row. The parse is a pure function in `model.rs` with its
tests, tolerant in the direction `parse_status` is: an unrecognised `track` string counts as zero
rather than raising, because this runs against whatever git is on somebody's machine and a row with
no mark beats a panel with no rows. A newline is a safe record separator here where it would not be
for a path — `git check-ref-format` forbids control characters in a ref name.

**"There is something to pull" is `behind > 0` against that branch's own upstream, and nothing
else.** Not "origin has moved on": a branch with no upstream is not orange (there is nothing to pull
from), and neither is one whose upstream was deleted on the remote (`gone`), where the honest state
is that the other end is gone rather than ahead. `gone` is its own field rather than an absent
`upstream` because the two are opposite facts — never pushed against pushed and then deleted — and
the panel refuses a pull for different reasons in different words.

**The orange is `--git-modified`, the token the file tree and the change list one section above
already draw "differs from what is committed" in**; "differs from origin" is the same sentence one
step further out, and borrowing it is what keeps the panel one vocabulary. Deliberately **not**
`--status-needs-you`: that hue is budgeted at one or two rows on a screen, and a branch list can
hold ten branches that are behind, so spending the loud colour here would end the rule that loud
means a person is needed. And **never colour alone** — `↓N` is drawn beside the name in the same
token, so the mark survives a monochrome screen and anybody who does not separate those two hues.
Ahead is `↑N` in the neutral `--type-plain-fg` and does **not** colour the row: what was asked for
was the branch with something to bring in, and colouring both would leave the two indistinguishable
at a glance. A branch both ahead and behind carries both marks and takes the colour. The counts keep
their own token while a run mutes the rows — they are a fact about the remote rather than an offer —
but the *name* gives its colour up with the row, since one name in orange over a panel nobody may
press would be saying a press was possible.

**A folded folder carries a bare `↓` for the branches it is hiding**, with no number of its own.
Without it the feature would be invisible in exactly the repositories that need it — one `feature/`
folder holding thirty branches, folded, which is the ordinary state of this list. No number, because
the heading already carries the count of what it holds and a second number beside it reads as a
subtotal of the first; what tells the two apart is that the count is `--text-muted` and the arrow is
not.

**Pull and Push belong to the branch the repository is on, so they live in the Branches caption and
not in a row's menu**, which is where every other write in this panel lives. On nine rows out of ten
the item would be refused, and a menu here answers about the row it was opened on. The caption also
gives these two the one thing merge and rebase do without: something on screen saying they exist.
The structural cost is real — `SectionHeader.vue` **is** a `<button>`, and a button inside a button
is invalid HTML that also folds the section on the way through — so the caption grew an `actions`
slot drawn **beside** the caption button inside a wrapper, and `--row-h`, `flexShrink: 0` and the
`divided` hairline moved onto that wrapper, because the wrapper is the element `GitPanel` measures a
row by (`sectionHeights.js` is untouched, and a drag still stops on a row boundary). Whether the
slot was filled is a **function argument rather than a `computed` over `useSlots()`**: a slot's
presence is not a reactive dependency, so a cached answer would go on insetting a caption whose
controls have since gone.

**Beside them is a third control, and it is about the repository rather than about the branch**: a
`git fetch` somebody presses for, `Check the remote`, drawn first of the three. It is there because
of what the other two do when there is nothing to do — Pull is refused when the branch is level and
Push when it is ahead of nothing — so the state a person most wants to ask about is exactly the
state in which the caption had nothing left to press. The count they are reading is only as fresh
as the last sweep that worked, and with `git.autoFetch` off there has been no sweep at all: a fact
somebody decides on has to be a fact they can ask about again. It stays on a detached HEAD, where
neither verb is drawn, since a repository is still a repository with no branch checked out in it.

The check is **two `Button`s under one `Tooltip`**, one per state, and that is a defect's shape rather
than a preference: `Button` draws its slot as `<span v-if="$slots.default">`, and a slot function is
present whether or not a `v-if` inside it renders anything, so a single button carrying the spinner
in its slot kept an empty span as a flex child — spending its `gap` on nothing, coming out 6px wider
than the arrows beside it, and snapping back to their width the moment a fetch started, which slid
the caption's count and both arrows sideways. Interaction is a surface step and never a shift. The
glyph decides which of the two is drawn; `fetchAction`'s verdict decides `disabled` on both, so the
rule stays in one place.

The three controls are `Button` in `ghost`, icon-only, each inside its own `Tooltip`, and that is
**deliberately not `IconButton`**, the icon-only control everywhere else in this app. `IconButton`
carries a `Tooltip` of its own around its `label`, and a refused button needs a wrapper tooltip — a
native disabled button raises no pointer events of its own, so an explanation living inside it is
the one thing a person cannot reach. Nested, the two opened together on hover: the name above the
glyph and the reason beside it, two panels over a caption 152 pixels wide. So the wrapper is the
only tooltip, in both states — the control's own name when it may be pressed, the sentence saying
why when it may not, with the 400 ms delay `Tooltip`'s own note reserves for prose somebody is
crossing on the way to something else. The accessible name `IconButton` would have enforced is
passed by hand as `aria-label`.

What either button says and whether it may be pressed is `components/git/tracking.js` — pure,
tested, of the `gitActions.js` family, for the reason that family exists. It does not repeat
`gitActions.js`: whether this panel may write at all is that file's one verdict and arrives here as
an argument, since a second copy of a rule is the half that drifts. **Both verbs are refused when
the branch is level with its upstream**, in a sentence each, and that is one rule read twice rather
than two rules: a control offering an act with no effect is a control somebody presses to find out.
Pull was live in that state once, on the argument that pressing it is how a person makes the count
they are reading current — right about the need and wrong about the control, since what it
describes is a fetch, and the fetch now has a button of its own in the same caption. **The check
takes neither the tracking record nor the runs verdict**: `git fetch` writes remote-tracking refs
and touches neither the working tree nor the index, which is the argument that already keeps the
background sweep going under a batch, and the one state that refuses it is one already in flight —
where the control spins rather than explains, `loader-circle` at `--attn-live`, the idiom the branch
rows already use over a write. Push has a second shape rather than a second control: a branch with no
upstream — the ordinary state of one cut by `New branch from this` — is published, which is
`git push --set-upstream origin HEAD` and a different name for the control: **Publish branch**,
since "push" for a branch the remote has never heard of says less than what is about to happen. A
branch whose upstream was deleted is the same act. **A name here is not a caption**: both controls
are icon-only — two arrows — so `Pull 3`, `Push 4` and `Publish branch` are what the tooltip says,
what `aria-label` carries and what a screen reader announces, and nothing of any of them is drawn
beside the glyph. That is the same bargain every other icon-only control in this app makes, and it
is why the wrapper is a `Tooltip` in both states rather than only in the refused one: a control
whose name is a glyph always has something to say.

Which of the two forms it is, is decided on the front end from the tracking record the button was
drawn from — `publishes` in `components/git/tracking.js`, which `stores/vcs.js` reads as well, so
that the word on the control and the arguments git is run with cannot come apart. A stale answer is
harmless both ways: `-u` against a branch that has since gained an upstream sets the same one
again, and a plain push of one that has since lost it comes back refused in git's own words.
**Never `--force`, and never `--force-with-lease`** — the same default `vcs_checkout` documents,
and the argument is stronger here, since the refusal force would drive over is the one protecting
somebody else's commits.

**Pull is a merge, and the abort follows from that.** `vcs_pull` runs `git pull --no-rebase
--no-edit` through the very machinery `vcs_merge` uses — the tree read before and after,
`model::new_conflicts` deciding, `MergeOutcome::Conflict` opening the existing `ConflictModal` with
`OpKind::Merge`, whose `git merge --abort` is the call that puts this tree back. `--no-rebase` is
named explicitly rather than left to the config: with `pull.rebase` set in somebody's, one button
would merge in one repository and rebase in the next, and the dialog would offer the abort of an
operation that never started. Rejected: `--ff-only`, which is simplest and cannot conflict, but
leaves a diverged branch with no move at all inside the app — there are no remote branches in this
list to merge from — and `--rebase`, whose stopped state is a detached HEAD, which is the state this
panel draws worst. The store records the conflict with `op: 'merge'` for the same reason, since
`OpKind` knows two words and `pull` is not one of them.

**Freshness is a background `git fetch --prune`, throttled and silent.** It goes out on window focus
(from `catchUp`, where the file tree and the branch list already catch up), on the project
change, **and on a one-minute tick while the window is open**, and the project half is keyed on **the repository the panel settled on** rather than on the
project path: which repository a project shows is decided an invoke later, so a fetch fired on the
path itself would ask about the repository being left and stamp that one's throttle. Watching the
selection covers a person picking another repository too, which is the same question one row along.
The store holds the cost down — the setting first, then once every five minutes per repository, then
one call in flight per repository, since a second would queue behind a network call that may run for
a minute. **Nothing waits for it**: the panel draws from what is already known, and when the answer
lands the tracking read is repeated and the marks change underneath. The interval is a constant and
not a setting, because a person can reasonably decide whether this happens at all and cannot
reasonably decide whether four minutes beats five.

The tick is the one of the three that is not somebody doing something, and it is there because
whether a colleague pushed is the one thing in this window that changes with nobody touching this
machine. Everything else `catchUp` refreshes is local, so a window left alone cannot have it move;
a window left on the board for an afternoon would otherwise spend the afternoon believing a branch
is level because the last answer said so — and that number is what dims Pull. **A minute is the
tick and not the interval**: what decides when a socket actually opens is the same five-minute
throttle per repository, and ticking under it is what makes the timer robust the two ordinary ways
a timer goes wrong — a laptop that slept through six ticks fetches once on waking rather than six
times, and a tick landing a second before the throttle expires is followed by another a minute
later rather than by a whole interval of waiting. It lives in `DesktopApp.vue` beside the focus
listener and not in the store, for the reason the focus sweep is wired there: a store that started
timers of its own would open sockets from a module nobody mounted.

**A pressed check is the same call, loud, and past both guards it has.** `fetchNow` in the store
ignores the setting — that switch is about what this app does on its own, and a press is not that,
which is what the setting's own prose already promised — and ignores the throttle, and resets it,
since five minutes is a budget for calls nobody asked for. What it keeps is the one-in-flight hold,
which is also what dims the button and spins it — and that hold is the promise rather than a flag,
so a press landing while the sweep is still out **joins that call** instead of being refused by it.
A guard would have made exactly the press this button exists to prevent: nothing spins, nothing is
said, nothing happens. The joined call stays silent on failure, since nobody pressed *that* one. Its failure lands in the panel's refusal block with
`op: 'fetch'` and the title **Git did not reach the remote** — the one entry in that table for
something that is not a write to the tree, because a fetch somebody pressed for fails the way a
pressed write fails. `vcsState.fetching` is deliberately not `busy`: `busy` is what holds the branch
rows inert, and a fetch freezes no row it could affect. The background sweep sets neither, so a
panel never starts twirling over a decision the person did not make.

**A background fetch that fails says nothing on screen**, sets neither `error` nor `writeError`, and
goes to the console alone. The block over the branch list reads "Git refused this operation" and
there was no operation — nobody pressed anything — so a laptop with no network would spend all day
accusing the panel of a failure that is the machine's ordinary state. The stamp is written **even on
failure**, so an unreachable remote is asked once every five minutes rather than on every focus.
A pressed Pull or Push is the opposite and is loud like every other write here: it goes through the
same `write` helper, takes the same `busy`, and its refusal lands in the same block carrying its own
`op`. Fetch also stays **live under a run** while Pull and Push are dimmed with the rest: it writes
remote-tracking refs and touches neither the working tree nor the index, which is the argument that
already keeps the commit box's sparkle alive under a batch.

Whether any of it happens at all is one setting, the root `git.autoFetch` in `settings.json`,
shipped on, with a switch on the settings window's General tab (`.claude/rules/settings.md`). It
exists for a metered connection, a VPN that is not always up and a key with a passphrase that would
otherwise fail on every sweep. With it off the app makes no network call of its own at all, and both
buttons go on working when pressed. In `npm run dev` the marks come from `mockBackend.js`, which
answers `vcs_tracking` with one branch behind, one ahead, one level and one nobody has pushed;
`vcs_fetch` is deliberately **not** answered there, so the browser exercises the silent failure
rather than pretending a remote was reached — and, since the check reached the caption, the loud
one as well: pressing it in `npm run dev` draws the refusal block, which is the only place that
block's fetch title can be seen at all.

## Comparing a branch with the one you are on

The fifth item on a branch row opens a window — a window and not a modal, so it can be dragged out
beside the board it is about and left there — listing the files one branch differs from the current
one by, with the picked file's two sides drawn by the same `DiffView` the change list already opens a
diff in. `vcs_compare` and `vcs_file_at_rev` in `commands.rs` are the whole of the back end,
`stores/compare.js` holds what the window knows, and `components/git/CompareList.vue` inside
`views/CompareWindow.vue` draw it. Nothing here writes: no staging, no cherry-picking, no
conflict, and no working tree either — comparing branches is about commits, and what is uncommitted
is the change list this panel already has.

**"What has this branch changed" has two honest answers, and they disagree every time the current
branch has moved since the two split.** *From where they diverged* is `git diff <merge-base>
<branch>`, what a pull request shows and the ordinary reading of "what is new here". *Direct* is
`git diff HEAD <branch>`, how the two trees stand against each other right now — which also draws
every file only the current branch touched, backwards, as a change being undone. **Both are offered,
on a two-position switch above the file list, and the diverged reading is the default**: neither is
wrong, each is the wrong one half the time, and a product that picks silently makes the other half
unexplainable to somebody who has no way to ask which it picked. The switch sits over the rows rather
than in the window's chrome, because it changes what the list holds and a control that changes a list
belongs with it. `Mode::parse` reads anything it does not recognise as the default rather than
refusing it — a mode is a switch on a screen, not a state worth a sentence. The one thing it will not
do is fall back quietly: two histories with no commit in common have no point they diverged from, and
that is `VcsError::Unrelated` in its own words, because a diff computed from a base nobody asked for,
drawn under a switch that says otherwise, is the one failure this window could actually mislead
somebody with.

**Both endpoints are resolved once and everything afterwards is read by sha.** `vcs_compare` answers
`Comparison { left, right, files }` where the two are object names and never the names they were
asked for by, and `vcs_file_at_rev` takes a revision rather than a branch. What asking a name twice
would have cost is not a race worth ignoring here but the ordinary case in this app: an agent commits
into this very tree while the window stands open, and the file list would then belong to one commit
and the bytes on screen to another, with nothing anywhere saying so. It is the same rule
`file_at_head` has always kept by handing `cat-file` an object name instead of `HEAD:<path>` a second
time, one scope wider — and it is what `tests/stores/compare.test.js` pins from the other side, where
a read whose `rev` is `'feature'` or `'HEAD'` is the assertion that fails.

It settles the injection question for free as well. The front end never composes a revision of its
own; it sends back a sha this module gave it. So `is_object_name` can be as narrow as hex and nothing
else and lose nothing, and `VcsError::BadRevision` is a fault rather than a state worth drawing —
unreachable from the app. Without it there is no `--` to hide behind in the middle of `{rev}:{path}`,
and a leading dash would be read as a flag.

**The right-hand side is resolved through the full `refs/heads/<branch>`; the left-hand side is the
literal `HEAD`.** The prefix is what stops a name that arrived from the front end being read as a
flag or as an ambiguous rev, and this list holds whatever a person has called a branch. The literal
`HEAD` costs nothing and answers a detached checkout for free: a project opened at one compares
against where it is standing, with no name to invent and no second case to write. `file_at_rev` is
the generalised `file_at_head` rather than a copy of it, so the 2 MiB ceiling, the binary sniff and
the UTF-8 refusal are written once — the property `file_at_head`'s own header always claimed, that a
file the editor refuses above 2 MiB must not arrive through a second door.

**One compare window per app, label `compare`, and not one per pair.** It is built exactly as the
settings window is — a child of the main window so it cannot sink behind the board, the one bundle
under `index.html?view=compare`, an open one focused and re-aimed rather than rebuilt — and it is why
`close_settings_with_main` is `close_children_with_main` now and takes both labels down with the app
window. A comparison left standing after that window is gone is one nothing can ever re-aim, and it
would keep the app from exiting on its last window.

A window per `(repo, branch)` pair, so two comparisons could stand side by side, was considered and
dropped. Its labels are generated, and `src-tauri/capabilities/default.json` names windows literally
— so that version needs a **glob** in `windows` where the file today lists three names, and **a
window not named in that file reaches no core plugin at all**: it fails as a page that cannot talk to
anything, rather than as an error somebody could read. Widening the app's permission surface with a
pattern is a large change to make for a want nobody has expressed, and it diverges from the only
precedent for a second window in this tree. If side-by-side comparisons ever are wanted, the glob is
the change and this paragraph is where to start reading.

The pair travels twice, for the reason the settings window's section does: a window being built reads
it off the URL, an open one never sees a new URL, so the `compare:show` event is the only way to
re-aim it — and both halves are needed for one press to work in both states. `compare_query`
percent-encodes both rather than validating them the way `tab_query` validates a section name: a
section is a short identifier from a closed list, while a repository is an absolute path and a branch
name may hold almost anything a ref allows, and dropping either would leave a window with nothing to
compare. Freshness is window focus and the mode switch, the answer the rest of this app gives, and
there is no watcher.

**The menu item is the third reach of refusal in `branchMenu.js`, and it is the narrowest of the
three.** That file's header described two — the whole menu, refused while a run holds the project or
git is mid-operation, and the three moving verbs, refused on the branch already checked out. This one
reads and writes nothing at all, so `held` does not reach it: **the caption at the top may say "not
now" while the row directly under it stays live and opens the window**. It is the same argument that
keeps Fetch pressable under a run and leaves a folder heading undimmed beside greyed rows — a row
refused for a reason that does not apply to it is the menu saying something untrue about itself. What
still refuses it is the row being the branch already checked out, since a branch has no difference
from itself to draw, so that caption now heads four rows rather than three.

Two smaller decisions worth not re-opening. `CompareChange` is its own struct rather than the
existing `Change`: `staged` and `unstaged` are facts about a working tree and have no answer between
two commits, and two fields that are always `false` are two fields somebody will one day read as one.
And `parse_name_status` reads a record as **one field or three** — `R` and `C` carry a similarity
score on the letter and are followed by two paths, from and then to — because read as one field each,
a single rename puts every record after it one field out of step, which is the defect `--porcelain=v2`
already cost this module once. `DiffView`'s column captions became props for this window and default
to what they were hardcoded as, so the diff tab passes neither and did not change; the store's
`missingLeft` is bound to the prop's existing name `missingAtHead`, which means what it has always
meant — the left side has no such file.
