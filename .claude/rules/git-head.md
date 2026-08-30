---
paths:
  - "src-tauri/src/git.rs"
  - "src/stores/git.js"
---

# The branch in the scope bar

The bar over everything names the active project and the branch it is on. The branch comes from
`src-tauri/src/git.rs` — one file, the same no-worker shape as `files/` and for a stronger version of
the same reason: `git rev-parse` would spawn a process for one line git already keeps in plain form
on disk. So `.git/HEAD` is read directly, and a `.git` that is a file rather than a directory is
followed to the linked worktree's own HEAD, which is where a worktree's branch actually lives.

Nothing in that file is an error. A folder outside git, an unreadable `.git`, a HEAD in an
unrecognised shape all mean the same thing to the bar — no branch to show, drawn as `—`. A detached
HEAD is not silently dressed up as a branch: `Head` keeps `branch` and `detached` apart, and
`DesktopApp.vue` labels the short hash as detached. Freshness is window focus and switching projects,
the same answer the file tree gives; `src/stores/git.js` guards against its own stale response the
way `terminals.js` does, so the bar cannot name one project's branch under another project's name.

The same file holds the no-spawn rule at the one place it is genuinely inconvenient — a branch list
is not one line the way `HEAD` is, it is `refs/heads` walked for loose refs, `packed-refs` for the
ones git has folded away, and each branch's own reflog under `logs/refs/heads`: three reads, still
cheaper than a process, all from the common directory so a linked worktree offers its whole list. The
reflog is what orders the result rather than the alphabet, because the branch somebody merges into
every day is nowhere in particular alphabetically; a branch with no reflog anywhere falls outside the
recency group entirely, into the alphabetical tail a fresh clone leaves nearly everything in. Nothing
in that reading is an error either: a folder outside git offers an empty list rather than a failure,
and a repository whose only branch has no commits yet has no ref file at all — a merge-target field
offering nothing would be worse than one offering the single branch that exists.

A run's dialog reaches those same three sources through `branches_with_recency`, which sorts and
dedups the names before stamping each with its own reflog time — the ordering itself is left undone,
deliberately, because it is `by_recency`'s rule and not a second one written here. `combine` is the
pure function that applies it: it folds several repositories' lists into one, splits complete from
partial, and calls `by_recency` once on each group. Its one new judgement is where a branch's
freshness comes from across repositories — `develop` opened an hour ago in `backend` and a month ago
in `admin` is an hour old, because it is one branch to the person merging into it, and taking the
first repository's answer, or the least of them, would bury the branch somebody is actually in.
`BranchOption { name, missing_in }` is what a folded list is made of: a name, and the repositories
from `[project].repos` that do not have it, in the order those repositories were given. An empty
`missing_in` means every one of them has it.

**There is a third reader of those same refs, and it asks a narrower question**: `task_work(project,
id)` answers where one tracker task's work was left — its branch, and the commit at the tip of it —
for the note a run writes when it gives a dead batch's claim back (`runs::service::release_claims`,
smetana-0t4). It is the only place in this file that knows anything about the tracker, and what it
knows is one convention rather than a lookup: `provisioning` requires a task's branch to be
`<fix|feature>/<id>-<short-kebab-title>`, with the id in the slug precisely so that a branch found
afterwards can be *proved* to belong to the task looking for it. `task_branch` encodes exactly that
and is pure over `(name, sha)` pairs — the last path segment is the id, or the id and a hyphen, so
`fix/smetana-1ab-follow-up-to-smetana-0t4` and `fix/smetana-0t4x-…` are both refused. The commit
comes from the ref itself: `parse_packed_heads` keeps each packed branch's sha beside its name
(`parse_packed_refs` is now a projection of that one parser rather than a second pass over the same
lines), a loose ref file is read for its object name and wins over the packed copy the way it does
for git itself, and a symbolic ref is not a commit and answers nothing. The leaf property above is
untouched: the caller passes a `&Path` per repository and walks `[project].repos` itself, so nothing
here reads project configuration, and a folder outside git simply has no work to name.

**`git.rs` no longer answers the dialog.** `runs::commands::target_branches` does, because "what may
this run merge into" is a question about a run rather than about one directory: it reads
`.smetana/project.toml` itself, through `config::load`, and walks `[project].repos`, calling
`branches_with_recency` once per repository and folding the results through `combine`. `git.rs` keeps
its shape — a leaf, no worker, no spawn — and no code in it reads project configuration: `combine`
takes a list of `(name, branches)` pairs and never learns where they came from. The config is read
inside that one command rather than taken from the front end, and that is the design rather than a
shortcut: `runs.js` holds its own copy of the config, filled by its own `project_config` call, and
the run dialog is shown before that call has landed — the whole of `smetana-6gs` and `smetana-o8r`,
where the branch-filling rule ran once against a list that was not there yet. A repository list
threaded down from the front end would be the same race wearing a different name; reading both facts
inside the one command leaves no order between them to get wrong.

What the field draws from that is two groups, headed "Everywhere" and "Not everywhere", and no
captions at all when nothing is partial — which is every single-repository project, and therefore the
common case. A name in `[project].repos` that resolves to nothing readable, a missing folder or one
with no `.git`, is left out of the coverage question entirely rather than counted as missing every
branch: the alternative reads worse in exactly the case that matters, since one typo in the config
would make every branch partial, empty the field's top group, and bury the real question behind a
fault that has nothing to do with it. This is what closed a defect with no issue behind it, and the
shape of it is worth keeping: a project of four repositories living under one folder had the dialog
asking not any of the four but the fifth repository that folder itself happened to be, so `develop` —
present in all four — read as a branch nobody had, and the run went out telling the agent to cut
`develop` from the current branch in every one of those four repositories, though each already had it
with its own history.

**Refs are shared and HEAD is per-worktree, and conflating the two is `smetana-5t7`.** A linked
worktree's git directory — whatever its `.git` file points at, `.git/worktrees/<name>` — holds only
the per-checkout half: `HEAD`, `ORIG_HEAD`, the index, `logs/HEAD`. Everything a branch list is made
of lives in the *common* directory instead, named by a `commondir` file sitting next to that git
directory, and `parse_commondir` resolves it — relative (git's usual `../..`) against the git
directory rather than the checkout, absolute taken as-is, and missing meaning an ordinary clone that
*is* its own common directory. So `refs/heads/`, `packed-refs` and `logs/refs/heads/` are all read
from that one resolved place, while `HEAD` stays where it is. Before that, opening a linked worktree
as a project offered exactly one branch in the run dialog — the branch the worktree was already on,
which is the single branch nobody needs to merge into — and the reflog ordering did not work at all,
since the log directory was not there either. Live-checked against this repository's own linked
worktree: the same list as the main checkout, in the same reflog order, with HEAD still reading
per-worktree.

The counters used to sit next to it and are along the bottom of the window now — `shell/StatusFooter.vue`,
which took the project's own state off the window's title bar. Both are live, and neither of them is
this store's. The uncommitted files are `dirtyCount` in `stores/vcs.js` — the length of the change list the Git panel draws for the
repository selected there, so the number in the strip is the number of rows in the panel. The running
agents are `liveAgentCount` in `stores/terminals.js` — the sessions that have not exited, plus the
starts the worker has not answered for yet. Both are computeds in their stores rather than in
`DesktopApp.vue`, which is a rule about testability and not about tidiness: no test in this
repository can reach a `.vue` file.

One consequence belongs here rather than there, because it is about this branch: **the branch and
the file count can be about different repositories.** The branch is the project root's HEAD, read here;
the count is whichever repository the Git panel has selected, which in a project of several is often
not the root. It was accepted knowingly — a count summed over every repository would be a `git status`
apiece, and the number would then match no list on screen at all.
