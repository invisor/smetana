---
name: merging
description: Use when a task's work is reviewed and has to reach the target branch — integrating one task's worktrees, resolving by class, running the gates, and fast-forwarding, one task at a time
---

# Merging one task

This is the mechanics of getting one task's worktrees into the target branch. **The
process that called this one owns the policy**: what to do when something needs a
judgement call (ask, or park and carry on), whether worktrees are removed and the issue
closed afterwards, and what one task's failure means for the rest of a batch. If you were
not told, stop and ask — do not invent it.

Two things are unconditional and are marked as such below. They exist because **a clean
`git merge` can still be wrong**: git merges text, and it has no opinion about two
branches that each generated a file which was supposed to be unique, or a lockfile whose
clean textual merge resolves to a dependency set neither branch ever installed.

Read `.smetana/project.toml` first. `[project].repos` gives the merge order,
`[repo.<name>].gates` gives what green means, `[merge].regenerate` gives the paths that
are never merged by hand, and `[merge].hazards` is prose written by somebody who knows
this codebase — read it before every merge, not once.

Below, **STOP** means "hand this to the caller's policy". Never guess past one.

## The merge lock — one lead in a target branch at a time

Everything below assumes it is the only thing moving the target branch, and with two
runs going in one project nothing in git makes that true: two leads merging at the same
moment leave a half-merged target rather than a clean refusal. The lock is an issue in
the project's own tracker — bd is already the shared, atomic, cross-process store every
run talks to — marked by the `smetana-lock` label, which is what keeps it from ever
being taken as work: `provisioning` skips the label when claiming, and so does the
app's own queue.

**Claim it before the first task's Step 0, hold it across every task of the batch, and
release it after the last — on every way out.** A single task merged on its own is a
batch of one. A gate that went red, a task that parked, a STOP that ends the phase:
whatever ends this batch's merging, the release below is the last thing done on the way
out. The one exit that cannot release — the run being killed — is what the staleness
rule exists for.

Find it by its label:

```bash
bd list -l smetana-lock --json
```

Nothing there → create it, then **list again and take the lowest id**: two leads racing
the creation converge on one lock, and the loser's duplicate is inert — it carries the
label, so nothing ever takes it as work.

```bash
bd create --title "Merge lock" --type chore --priority 4 -l smetana-lock \
  --silent --body-file - <<'EOF'
The merge lock (label: smetana-lock). A lead claims this issue before its first merge
into the target branch and releases it after its last. It is coordination, not work —
never implement it, never close it.
EOF
```

The claim is the whole of the mutual exclusion — atomic, and refused when a different
actor holds it:

```bash
bd update <lock-id> --claim
```

- **Accepted** → merge. A re-claim of a lock your own actor already holds is accepted
  too, so recovering over your own claim is safe.
- **Refused** ("already claimed by <actor>") → another lead is mid-merge. **Wait and
  retry** — once a minute is enough; a merge phase is minutes, not hours. When the
  claim finally lands, carry on — and **the report must say this batch waited on the
  lock, and roughly how long**.
- **Refused past the staleness rule** → break it, below.

**Staleness: 60 minutes.** A run killed mid-merge leaves the lock claimed forever, and
the evidence is `started_at` in `bd show <lock-id> --json` — the moment the standing
claim was taken. A whole batch's merge phase with gates fits well under an hour in this
project, so a claim older than 60 minutes is a dead run's. Break it — release, then
claim:

```bash
bd update <lock-id> --status open --assignee ""
bd update <lock-id> --claim
```

**Breaking a lock is a report line, never a silent step**: name the actor it was taken
from and how old the claim was. The claim after the break can still be refused —
another waiting lead may land first — and that refusal goes back to waiting, not to
another break: the new claim carries a fresh `started_at`, so the ordinary 60-minute
rule re-arms and applies to it like any other.

The break is release-then-claim and bd has no compare-and-swap, so the pair is not
atomic: two waiters seeing the same stale lock can interleave, and the second release
silently unseats the first's fresh claim — and a broken holder that was alive after
all resumes merging unaware. **So holdership is re-verified, never assumed**: after
any accepted claim that followed a break, and after any pause inside the merge phase —
a STOP answered, a wait of any kind — `bd show <lock-id> --json` and confirm the
assignee is your own actor before (re)entering a task's Step 0; not yours → back to
waiting. This shrinks the window to seconds rather than closing it, and that is the
honest limit of what bd offers.

Release — unconditional, the same on success and on failure (`open` because only an
open issue is claimable; an empty assignee so nothing still names a holder):

```bash
bd update <lock-id> --status open --assignee ""
```

The lock serializes leads, not repositories: one claim covers the whole batch, however
many repositories `[project].repos` lists.

## Step 0 — What is being merged, and in what order

The task's slug is `<issue-id>-<short-kebab-title>`. Find its worktrees by walking
`[project].repos` **in the order the config lists them** — that order is the config's
statement about what depends on what, and merging against it is why a consumer sees a
contract that has already landed:

```bash
set=""
for repo in <repos, in config order>; do
  [ -d "<root>/$repo/.worktrees/$slug" ] && set="$set $repo"
done
```

No worktree anywhere → STOP ("no worktree for this task"). Otherwise run Steps 1–5 for
each repository in `$set`, one at a time, from inside its worktree.

## Step 1 — Preconditions, re-derived per repository

**Re-derive every per-repository variable at the top of each iteration.** Carrying
`root`, `branch` or `main_checkout` over from the previous repository — or the previous
task — is the single easiest way to merge the wrong thing into the wrong place.

1. `root="$(git rev-parse --show-toplevel)"`. It must contain `/.worktrees/`; if not,
   you are not where you think you are → STOP.
2. `branch="$(git rev-parse --abbrev-ref HEAD)"`;
   `main_checkout="$(git worktree list --porcelain | awk '/^worktree /{print $2; exit}')"`.
3. The worktree is clean (`git status --porcelain` empty) → otherwise STOP. **Never
   stash silently**; uncommitted work in a worktree means somebody is not finished.
4. The main checkout is clean and on the target branch → otherwise STOP.
5. The target branch is not behind its remote:
   `git fetch origin <target> 2>/dev/null || true`, then
   `git log --oneline <target>..origin/<target>`. Anything listed → STOP. Being *ahead*
   of the remote is normal after earlier merges in the same batch and never trips this.

## Step 2 — Merge the target branch into the branch

Inside the worktree: `git merge <target>`. Clean → Step 4. Otherwise resolve **by
class**, and only these classes:

- **Registration-style conflicts** — two sides each adding an entry to a list, a
  registry, a route table, an index of modules. Keep both sides' intent: something
  registered on each side must survive on both. `git add`.
- **Anything needing a judgement about what the program should do** → STOP. Do not
  guess. "Only mechanical conflicts resolve themselves" is the rule, and a conflict is
  mechanical only when both intentions can be kept without choosing between them.
- **Lockfiles** — never hand-merge one. Take the target's copy, reconcile the manifest
  by hand (union of dependencies, higher version where they clash), then re-run the
  repository's install so the lockfile is regenerated against what the manifest now
  says. Stage manifest and lockfile together.
- **Anything matched by `[merge].regenerate`** — never hand-merge it either. Take the
  target's version, run that entry's `command`, stage the result. If the command depends
  on something being up and serving the merged code, verify that first; a regeneration
  against a stale service produces a file that looks right and is not → STOP rather than
  stage it.
- **Anything `[merge].hazards` describes** — do what it says. It is there because
  somebody was bitten.

Stage each resolved file. **Do not commit yet.**

## Step 3 — The unconditional check for what git does not flag

Run this after **every** merge, whether or not git reported a conflict. The dangerous
case is precisely the one git resolves silently: two branches that each generated a new
file in a numbered or ordered sequence off the same base. Git keeps both, the sequence
now has two entries claiming the same position, and nothing anywhere says so.

`[merge].hazards` is where this project's instances of that are written down. Read it,
check each one, and treat "the prose does not mention my case" as a reason to look
harder rather than a clearance.

Where the answer is to regenerate rather than to merge: record what the branch
contributed before you throw it away, regenerate from the target's state, and then
**check that nothing hand-authored was lost**. A generated schema can be regenerated; a
hand-written data step in the same directory cannot, and quietly dropping one is worse
than any conflict. If anything hand-authored would be lost → STOP, naming each file, and
say how it can be recovered from the branch.

## Step 4 — Gates

**Refuse to commit a conflicted tree.** Committing conflict markers bakes them into the
merge commit, and Step 5 then fast-forwards them into the target branch:

```bash
unresolved="$(git diff --name-only --diff-filter=U)"
[ -n "$unresolved" ] && { echo "$unresolved"; }   # -> STOP, do not commit
```

Otherwise commit the integration, then run every command in `[repo.<name>].gates`, in
order, inside the worktree. A repository with no gates declared has none — that is a
fact about the project, and it belongs in your report, not a reason to invent one.

Any gate red → STOP, with the output. Nothing has to be undone: the integration lives on
the branch, and the target branch has not been touched yet.

## Step 5 — Fast-forward into the target branch

The branch now contains the target plus the resolved work, so this cannot conflict:

```bash
git -C "$main_checkout" merge --no-ff "$branch" -m "merge: $branch into <target>"
```

`--no-ff` keeps each task as one revertible merge commit. **Nothing is pushed here** —
pushing is not this process's business, and the caller's final report is where the human
is reminded.

## When a multi-repository task fails halfway

If an earlier repository of this task already reached Step 5 and a later one of the
**same task** stops, the target branch is holding half a task. Sequential merging
guarantees the merge commit is still at the tip, so roll each merged repository back
before handing over:

```bash
git -C "<root>/<repo>" reset --hard HEAD~1
```

Then hand the whole task to the caller's policy as one unit. Half a task in a branch
that everything else is cut from is worse than no task.

## One task at a time

Never overlap two tasks' merges. Step 3's regeneration reads the state of the target
branch, and a second task landing in the middle of that makes the regenerated output a
mixture nobody asked for. Finish one task completely — Steps 0 through 5, every
repository — before starting the next.

## Removing worktrees, when the caller's policy says to

One precondition, and it is unconditional:

**Stand the task's workers down before removing anything.** Somebody you spawned stays
resumable after their task merges and closes, and their shell's default directory is
often a *sibling* task's worktree. Remove their tree while they can still run commands,
and their next `cd <worktree> && …` fails into that fallback — the rest of the block then
reads, or writes, another task's checkout, with no error anywhere. Shut them down if the
harness allows it; otherwise send a final message — "task merged and closed, worktree
being removed, run no further commands" — and never remove a worktree while its worker is
mid-command.

Then, deliberately without `--force`:

```bash
for repo in $set; do
  git -C "<root>/$repo" worktree remove "<root>/$repo/.worktrees/$slug" \
    || echo "WARN: could not remove $repo/.worktrees/$slug — left in place"
done
```

A dirty or locked worktree refusing removal is a line in the report, never a stop.

## What the caller has to tell the human

This process does none of the following, and every one of them is somebody's next step:
pushing the target branch, re-installing dependencies in the main checkout when they
changed, and applying whatever a regenerated artefact needs applying (a migration
against a shared database is the usual one — and if the branch's earlier version was
already applied there, it has to be reconciled by hand, because the number it used no
longer exists).
