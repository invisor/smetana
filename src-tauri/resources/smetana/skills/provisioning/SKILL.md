---
name: provisioning
description: Use when picking tracker work up and preparing somewhere to do it — pinning the tracker, claiming a task, reading its spec, and cutting one worktree per repository the task touches
---

# Picking work up and provisioning for it

The mechanics of turning a task on the board into a place where somebody can work on
it. **What is not decided here is what happens when something is unclear** — the process
that called this one says whether to ask or to park, and it says so explicitly. If you
have not been told, you have been called wrong: stop and say so rather than guessing.

Everything below is per project, and the project told you what it is: read
`.smetana/project.toml` before anything else. `[project].repos` names the repositories
and their order; `[repo.<name>]` carries that repository's `setup`, `gates` and
`env_files`; `[defaults].target_branch` is what work is cut from and merged back into
unless the run overrode it.

## Pin the tracker first

bd finds `.beads/` by walking up from the working directory, and a worktree is not under
the project root. Capture it once, while you are still at the root, and keep it exported
for every `bd` call in this session:

```bash
export BEADS_DIR="<project root>/.beads"
```

Every later `bd` failing with "no beads database found" means this was lost — re-export
it rather than running bd from somewhere else.

## Find and claim work

Finding and claiming are one step, not two: `.beads/` is shared, another run may be
reading the same board, and a window between seeing a task and taking it is exactly
where two runs pick up the same issue. The claim is the lock — `--claim` sets the
assignee and moves the issue to `in_progress` in one call, and bd refuses a claim held
by a different actor. Claim before provisioning, never after. Which form the claim
takes is decided by the scope you were given.

**The queue** — nothing narrower than "whatever is ready": take and claim in one atomic
call, one task at a time, repeated until you have as many as the batch allows:

```bash
bd ready --claim --json
```

It claims the first ready issue — open, unblocked, highest priority first, with the
custom statuses this process uses already excluded — and answers with it, so what comes
back is already yours and there is no window at all. An empty answer means no ready
work, which is an outcome, not a failure.

**The merge lock is on the board and is never work.** `merging`'s lock section says
what it is; what matters here is that it sits `open` under the `smetana-lock` label, so
`bd ready` returns it and the atomic form above can hand it to you as if it were a
task. An answer carrying that label was not work: put it back at once, and take the
rest of this batch through the listing form instead — list, drop everything labelled
`smetana-lock`, claim by id:

```bash
bd update <id> --status open --assignee ""   # put the lock back
```

In the listing form the same rule is one more drop, next to the scope's own: anything
carrying `smetana-lock` is never claimed, whatever the scope.

**A narrower scope** — an issue id, an epic whose children are the work, or a priority
floor — cannot go through the atomic form, because the first ready issue is not
necessarily one of yours. List first: `bd ready --json -n 50`, drop what the scope
excludes (a floor is applied by you, not by bd — drop everything with `priority` above
it), then claim each task you take by id:

```bash
bd update <id> --claim
```

**A refused claim is an ordinary outcome, not an error.** "Already claimed by
smetana-run-42" means another run got there first: skip the task and take the next — do
not retry it, do not park it, and do not report it as a failure. A re-claim of an issue
you already hold is accepted, so recovery over your own claims is safe.

Say plainly when nothing survives the scope; that too is an outcome, not a failure.

## Read the spec

`bd show <id> --json`. **The description is the spec** — read all of it, including
acceptance criteria and design notes. A description that is empty, or that does not say
what "done" would look like, is not something to start on: apply the caller's policy.

`filing-a-task` is the other end of this, and it guarantees the shape: bd's own
`--validate` refuses to create a task whose description is missing the sections its type
requires, so `## Acceptance Criteria` (or `## Success Criteria` on an epic) is present on
anything filed since. **Present is not the same as real**, and that is what you are
judging: criteria somebody could check, against criteria that restate the title. Any
task old enough to predate the rule has neither.

**Which repositories the task touches** comes from `repo:<name>` labels, where `<name>`
is one of the names in `[project].repos`. No label present means nobody has decided yet:
work it out from the spec and what each repository actually contains, then write the
answer back so the next reader does not repeat the guess —
`bd label add <id> repo:<name>`. Backfill only after the caller's policy has approved
the choice. A single-repository project is `repos = ["."]` and there is nothing to
decide.

## Names, computed once per task

- **Branch prefix** by type: a `bug` gets `fix/`, everything else `feature/`.
- **The slug must contain the issue id.** `slug="<id>-<short-kebab-title>"`, title
  clipped to about five words. Two tasks running in parallel will otherwise collide on a
  path, and worse, a worktree found at that path later cannot be proved to belong to the
  task looking for it. The id in the slug is what makes "reuse the worktree that is
  already there" safe.
- `branch="<prefix><slug>"`, and the worktree for repository `<repo>` goes at
  `<repo>/.worktrees/<slug>` — `.worktrees/<slug>` when the repository is `.`.

## Cut one worktree per repository

For each repository the task touches, in the order `[project].repos` gives:

1. **Make sure `.worktrees` is ignored** — `git -C <repo> check-ignore -q .worktrees`.
   If it is not, add the line to that repository's `.gitignore` and commit it before
   going further. A worktree directory tracked by its own repository turns every later
   `git status` into noise and can be committed by accident.
2. **Cut from the target branch**, not from whatever is checked out:
   `git -C <repo> worktree add "<wt>" -b "<branch>" <target-branch>`. The target branch
   is the run's, defaulting to `[defaults].target_branch`. It holds the siblings that
   already merged; cutting from anywhere else rebuilds the task against a state nothing
   else shares. The main checkout does not need to be clean for this — the worktree is
   cut from a branch, not from the working tree.
3. **Retry a lock.** Two creations racing produce an index or worktree lock error. Wait
   a second or two and try again, up to three times.
4. **A worktree already at that path belongs to this task** — the id in the slug
   guarantees it. Reuse it; do not delete it and start again, it may hold work.
5. **Bring it up.** Run `[repo.<name>].setup` in the fresh worktree if there is one.
   Dependencies are not shared between worktrees, so this is not optional when it exists.
6. **Copy in what git does not carry.** Each path in `[repo.<name>].env_files` is
   gitignored and therefore absent from a fresh worktree: copy it from the main checkout.
   A missing source file is worth one line in your report, not a stop.

**Provisioning is serialized on you.** Never hand `git worktree add` to somebody you
spawned. Creating them one at a time is the whole of the race protection, and it is
cheap — the work inside them is what takes time, not the cutting.

## What you hand to whoever does the work

Whoever writes the code has no tracker, no bd, and no way to ask a human. They get, in
text:

- the exact worktree path, and that they must never leave it or touch another one;
- the entire spec, pasted — they cannot look it up;
- the repository's `gates` from the config, as the commands they must get green
  themselves before reporting done;
- anything in `[merge].hazards` that bears on what they are about to change;
- where questions go: to you, and only to you.

Tell them to root every command at the worktree — `cd <worktree> || exit 1`, never a
bare `cd <worktree> && …`. With the bare form a failed `cd` short-circuits only its own
list; everything after it still runs, in whatever directory the shell fell back to,
reading and writing another task's checkout with no error anywhere.
