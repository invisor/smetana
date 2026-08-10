---
name: running-tasks
description: Use when carrying tracker work through from the board to a merged branch — recovering interrupted work, claiming a batch, delegating, reviewing, merging in order, and deciding what happens when something is unclear
---

# Running tasks off the board

You are the lead for this run. **You do not write the feature code**: you decompose,
provision, delegate, review, merge, and close. You own every tracker command and every
`git worktree` operation in this run; nobody you spawn touches either.

Three other skills carry the mechanics, and this one carries the policy. Read each when
you reach it, and follow it exactly:

- `provisioning` — pinning the tracker, reading a spec, claiming, cutting worktrees
- `merging` — integrating one task and getting it into the target branch
- `live-checking` — verifying a merged task beyond its gates
- `reviewing` — what the reviewer you spawn is working from

Each of them says "the caller's policy decides" in places. **You are that caller**, and
the next section is the decision.

## The run's settings, and the one that changes everything

You were given: a **scope** (the whole ready queue, one epic, or one task), a **mode**, a
**target branch**, a **priority floor**, whether the live check runs, and whether
findings may be filed. The mode is the one that changes the shape of the run:

| mode | what happens where the mechanics say "the caller decides" |
|---|---|
| **Auto** | **Park it and carry on.** There is no human in this run. |
| **Supervised** | **Ask.** Your session turns to waiting, and the run stops until you are answered. |
| **Solo** | Ask freely; do the work yourself rather than delegating. One task, never an epic. |

Everything below is written for Auto and Supervised. **Solo** skips Phase R and Phase 1's
delegation entirely: claim the task, provision its worktrees, do the work, review your own
change against `reviewing`, and then take it through Phase 2 and Phase 3 exactly as
written.

### Parking, in Auto

Wherever any of the four skills says to stop, and wherever you hit something you cannot
resolve from the code, the spec or the skills:

1. `bd update <id> --status parked`
2. `bd note <id> "parked: <one concrete line>"`
3. Leave that task's worktrees where they are — somebody will want to look.
4. **Carry on with the rest of the batch.** One task parking never ends the run.

`parked` is a custom status, so `bd ready` never returns it and the run cannot spin on it.
It is deliberately not the built-in `blocked`, which is a hand-set flag for an impediment
outside the tracker and is never cleared by bd itself. A task waiting on another task is
neither of these: it stays `open` with a `blocks` dependency, `bd ready` hides it while
the blocker lives, and closing the blocker releases it with nothing to update. In Auto
you never ask a question of anybody — there is nobody there.

## Findings do not become work on their own authority

Reviewers, workers and live checks surface real defects that are out of scope for the
task in front of them. Surfacing them is correct. Turning each one into claimable work is
not: in the run these rules were written from, that made the queue self-feeding — 13
human-authored tasks became 105 in two days, one of them 61 descendants deep across 10
generations, and the run ended working through a queue of pure noise.

**Findings get recorded. Only some of them get filed, and none of them get picked up.**

### The lineage marker

Every task you file carries this as the first line of its description:

```
spawned-from: <parent-id> root: <root-id> depth: <N>
```

`<root-id>` is the human-authored ancestor. `<N>` is the parent's depth plus one. A task
with no such line is depth 0 — a human wrote it.

### What may be filed at all

| finding | what happens |
|---|---|
| reviewer BLOCKING, in scope | the worker fixes it in the same worktree — never a task |
| reviewer BLOCKING, genuinely out of scope | may be filed, subject to the depth budget |
| live check FAIL | filed as a follow-up, with its own generation guard (Phase 3) |
| reviewer SUGGESTION or QUESTION | **never a task** — digest only |
| "I noticed X nearby" from anyone | **never a task** — digest only |

A suggestion that feels important enough to file is a severity error on the reviewer's
part, not permission to file. Send it back as "BLOCKING or digest — pick one".

### The depth budget

- **Depth 3 or more — never file anything**, whatever the severity. Digest and move on.
- **Depth 1 or 2, BLOCKING only** — `bd create` is allowed, and the task must carry the
  label `spawned`, a priority **one step worse than its parent's** (floored at the
  lowest), the lineage marker as its first line, and `deferred` as its status.

Two calls, because **`bd create` has no `--status` flag** — passing one fails the whole
command with `unknown flag: --status` and files nothing:

```bash
id=$(bd create --title "<title>" --type <type> --priority <N> -l spawned \
       --validate --silent --body-file - <<'EOF'
<the lineage marker, then the spec>
EOF
)
bd update "$id" --status deferred
```

`deferred` is one of bd's own statuses and `bd ready` excludes it. **This is the rule that
breaks the loop**: a filed task waits there until a person moves it to `open`. You never
promote one — not in this run, not in a later one — and you never claim one.

**How it is written is `filing-a-task`'s business, and that skill applies here in full**
— the required sections, `--validate`, and above all its standard: whoever picks this up
can finish it without asking anybody. You are filing it having just read the code and
seen the defect; the person promoting it in a week has neither, and neither has the run
that eventually takes it. So the test is the same one that skill sets, applied a step
earlier: **if you cannot state acceptance criteria for a finding, it is a digest line and
not a task.** Not knowing enough to specify it is not a reason to file it thinly — it is
the finding failing the bar for being work at all.

When the run was started with findings switched off, nothing is filed at all: everything
goes to the digest.

### The digest

One note on the **root** task, appended to, never a new issue:

```bash
bd note <root-id> "digest: [<severity>] <one line> — from <task-id> (depth <N>)"
```

Note it even if the root is already closed. A closed task keeps its notes, and this is
exactly the trail a person reads when deciding what deserves promoting.

In **Supervised**, you have somebody to tell: put the same lines in your hand-back and let
them decide, rather than filing.

## Phase R — pick up what a killed run left behind

Runs get interrupted: a crash, a quit, a limit. What that strands is tasks at
`in_progress` (claimed, worktrees cut, work possibly unfinished) and at `ready_to_merge`
(reviewed but never merged). `bd ready` excludes both, so nothing will ever pick them up
on its own. **This runs before you take any new work.**

1. Pin the tracker, and set the custom statuses once — idempotent, and always the full
   set, because a partial value clobbers the others:
   `bd config set status.custom "ready_to_merge,parked"`.
2. `bd list --status in_progress --json`. Each one is an orphan. Finish them **in place**,
   one at a time:
   - Its slug is `<id>-<short-kebab-title>` and its worktrees are already at
     `<repo>/.worktrees/<slug>` — the id in the slug is what proves they are this task's.
     None found → park it ("in_progress with no worktree to resume").
   - Re-read the spec. Empty or vague → park it.
   - Resume Phase 1 in the **existing** worktrees. Brief the worker to **finish** the
     task — the tree may already hold partial work; it is not starting over. Then the
     review loop, same five-pass ceiling. Clean → `ready_to_merge`. Not clean after the
     fifth → park, with what the reviewer still objects to.
3. Leave anything already at `ready_to_merge` alone. Phase 2 takes it.
4. Only when nothing is `in_progress` any more, go on to Phase 0. Phase 2 then merges the
   recovered orphans and this run's new survivors together, in one ordered pass. An orphan
   a killed run had already merged fast-forwards to a no-op — that is the expected signal,
   not an error.

## Phase 0 — take the batch

1. **Scope.** The whole ready queue, one epic's children, or the single task you were
   given.
2. **Priority floor.** `bd ready` has no maximum-priority filter, so apply it yourself:
   `bd ready --json -n 50`, drop everything worse than the floor, take what you need from
   what survives. Nothing survives → say "no ready work above <floor>" and stop. That is
   an outcome.
3. **How many at once** — the number the run gave you, whatever it is: it is this run's
   choice and it wins over `[defaults].max_parallel_tasks`, upwards as well as down. Were
   you given none, the config's number is the answer.
4. Per task: claim it the way `provisioning` says — the whole queue atomically with
   `bd ready --claim`, a narrower scope by id, and a claim refused because another run
   holds it is skipped, not retried. Then load the spec through `provisioning`. Vague or
   empty → policy (park, or ask). Provision, serialized on you.

## Phase 1 — delegate, then review

For each repository of each claimed task, spawn one worker. What they get is in
`provisioning`'s last section: the worktree path, the whole spec pasted, that
repository's gates, the hazards that bear on the change, and that every question comes to
you. Keep concurrency at the number Phase 0 settled on or below; queue the rest.

Where a task spans repositories: independent layers working against a contract they have
agreed can go in parallel; a layer that is hard-blocked on a contract that does not exist
yet goes second. Say which you chose and why.

A worker's question comes to you, never to a human directly. Answer it from the code, the
spec or the skills. Genuinely needs a person → policy.

**A harness with no subagents runs this sequentially**, one repository at a time, with you
doing the work in each. Nothing else about the process changes — the review is still an
independent pass, and it is still against `reviewing`.

### The review loop, and its ceiling

1. The worker reports done → run a review against `reviewing`, in the same worktree.
2. Clean → `bd update <id> --status ready_to_merge`. **Do not close.**
3. Findings → back to the worker, who fixes in the same worktree, then review again. That
   is **one pass**.
4. **Count reviewer verdicts, not worker edits. The cap is
   `[defaults].review_passes`** — five by default. Still not clean after the last one →
   policy, with what the reviewer still objects to. Five unresolved rounds is a real
   problem, and it is exactly when a person belongs in the loop.
5. Out-of-scope findings never gate a pass. Run them through the budget above at the
   **end** of the task's review loop, in one go, so the loop is not interleaved with
   filing.

## Phase 2 — merge, strictly one task at a time

Every `ready_to_merge` task enters: Phase R's recovered orphans and this run's survivors
together. Follow `merging` per task, with your mode's policy wherever it says to stop.

1. **Order** — topological over the survivors (`bd dep tree <id>`), tie-broken by
   priority and then by id. A cycle among them → park every task in it.
2. **One task completely before the next.** `merging` says why.
3. A partially merged multi-repository task rolls its merged repositories back before it
   parks — `merging` has the mechanics. The target branch never holds half a task.

When every repository of a task passed: `bd note <id> "merged: <one line>"`, then

- **live check on** — leave it at `ready_to_merge`, keep its worktrees, and add it to
  this batch's merged set. Phase 3 is the only place it closes.
- **live check off** — close it here: `bd close <id>`, then stand its workers down and
  remove its worktrees per `merging`. Then, if it has an epic parent and `bd dep tree
  <epic-id>` shows no other child still open, in progress, waiting to merge or parked →
  `bd close <epic-id>`. A parked child leaves the epic open. Skip Phase 3 entirely.

The lifecycle is `open → in_progress → ready_to_merge → closed`, and it has exactly one
close point.

## Phase 3 — live check, and close

Only when the run has the live check on. Mechanics are in `live-checking`.

**Classify first:**

1. **A child of an epic** closes without its own check — `bd note <id> "epic child — live
   check deferred to <epic-id>"`, close, remove worktrees, then run the epic gate below.
2. **Nothing a person can see** (a refactor, tooling, an internal job — judge from the
   spec) gets an API smoke at most, and nothing when even that would prove nothing:
   `bd note <id> "live check skipped: nothing user-facing"`, close, remove worktrees.
3. **Everything else** gets the full check.

**The verdicts:**

- **PASS** → `bd close <id>`, remove worktrees.
- **FAIL** → fix forward. The target branch is **not** rolled back; the code merged and
  its gates were green, and unpicking it costs more than the defect.
  1. **Generation guard.** A first line `livecheck-origin: <root-id> generation: <N>`
     marks this task as itself a fix. At `N ≥ 2`, file nothing: park this task with the
     whole chain in the note and carry on.
  2. Otherwise file a bug — title `livecheck-fix: <short summary>`, description starting
     `livecheck-origin: <root-id> generation: <N+1>`, then the check's full report. Then
     by the failing task's own spawn depth:
     - **depth 0** — a human-authored task broke where a person would see it. Priority 1,
       left `open`. **This is the one filed task that may enter the ready queue on its own
       authority**: the defect is reproduced, user-visible, and already merged.
     - **depth 1 or more** — priority one step worse than the failing task's, label
       `spawned`, then `bd update <id> --status deferred` as the second call above. A fix
       for a filed task is not worth keeping a run going for.
  3. An epic-gate failure additionally links the fix as a child of the epic, so the gate
     re-arms when it closes.
  4. `bd note <id> "closed with follow-up <fix-id>"`, close, remove worktrees. After an
     epic-gate failure the epic itself stays open.
- **INFRA** → not a verdict about the code. Retry the whole check once. A second INFRA →
  park the task, with what failed to start and the tail of its log.

**The epic gate.** After every close in this phase: if the task had an epic parent and
`bd dep tree <epic-id>` shows no other child still open, in progress, waiting to merge or
parked → run one check for the epic itself, against the epic's own description, as an
end-to-end flow. PASS → close the epic. FAIL → fix forward as above, with the fix as an
epic child, and the epic stays open. **A parked child skips the gate entirely**: note on
the epic that it is waiting on that child, and leave it open.

## The report at the end

- **Merged and closed** — per repository: the new tip of the target branch, the gate
  results, anything that was regenerated rather than merged, and the live check's verdict
  per task (or "live check: off").
- **Parked** — id, the concrete reason, and the worktree path.
- **Filed** — id, title, depth and root, one line each, under "waiting for a person to
  promote (`bd update <id> --status open`)". None → say so.
- **Digested** — the count, and the root ids to read.
- **What is left for a person** — the list `merging` ends with: pushing, reinstalling
  dependencies where they changed, applying anything a regenerated artefact needs
  applied.

## The rules that hold everywhere in this run

- **Never push.** A run merges into a local branch and says so.
- **Never abort the whole batch for one task.** Park it, or ask, and move on.
- **Never promote a `deferred` task, and never claim one.** Only a person does that.
- **Only mechanical conflicts resolve themselves.** Anything needing a judgement about
  what the program should do stops.
- **You own the tracker and the worktrees.** Nobody you spawned runs either, and nobody
  you spawned spawns anything — that is the guard against a run that grows without bound.
