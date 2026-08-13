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

- `provisioning` — pinning the tracker, claiming, reading a spec, cutting worktrees
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

**Park for a question, and for nothing else.** A park is a real question about what the
program should do — what the spec left open, which of two behaviours was meant, a
finding whose fix is a decision — that you cannot resolve from the code, the spec or the
skills, and that a person has to answer before the work can be right. Wherever any of
the four skills says to stop over one of those:

1. `bd update <id> --status parked`
2. `bd note <id> "parked: <one concrete line>"`
3. Leave that task's worktrees where they are — somebody will want to look.
4. **Carry on with the rest of the batch.** One task parking never ends the run.

**A technical obstacle is not a question either, and it does not park.** The merge
refused because the main checkout is dirty, a lock is held, a remote is ahead — nobody
is being asked anything and nothing about the code is in doubt. Leave the task at
`ready_to_merge`, which is what it honestly is, note the obstacle in one line, and carry
on. That state is picked up on its own: Phase 2 takes every `ready_to_merge` survivor,
so the next run retries the merge the moment the obstacle is gone, with nobody having to
remember. It cannot spin, either — a batch that changes nothing leaves the ready and
unfinished sets identical, and the run stops itself with `NoProgress` (`queue.rs`).

**A check this run could not run is not a question, and must never park finished work.**
Nobody is being asked anything: the code is written, reviewed and merged, and what is
missing is a pair of eyes or a tool that was not there. File the looking as its own card
in the status `human_check` and close the task that produced it — the card is out of
`bd ready`, so no later run takes it, and it stands in a column somebody scans by eye.
Phase 3's INFRA branch is this rule's one named instance, and the card has its own
section there.

The distinction is the cost of getting it wrong in each direction. A parked question
costs one answer and the work waits for it, which is right, because work built on a
guess is worse than work not built. A parked check, or a parked obstacle, costs the whole
task: it is finished and it sits out of the queue with nothing anybody can do to it,
and — the reason this paragraph exists —
**nothing ever unparks a task when its blocker goes away by itself.**
Five tasks sat parked for a day in exactly that way, over a dirty main checkout that was
committed three hours later; `bd ready` never returned them again, and it took a person
noticing that their features had gone missing from the app.

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

Phase 3's human check card is not on this table and is not governed by it. It is not a
finding — nothing was found — but a note that finished work still owes somebody a look, and
it has its own rules where it is filed.

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
   `bd config set status.custom "ready_to_merge,parked,human_check"`.
   All three, every time, including the ones this phase never writes itself: dropping
   `human_check` here would delete the status out from under Phase 3, whose card is then
   filed and left in `open` for the next batch to pick up and try to implement.
   In the same breath, make sure the merge lock exists — look for it, create it only
   when it is not there: `bd list -l smetana-lock --json` answering nothing → create it
   exactly as `merging`'s lock section says. Idempotent the same way the statuses are:
   an existing lock is left alone, held or free — a held one is another lead
   mid-merge, not a problem to fix here.
2. `bd list --status in_progress --json`. Anything carrying the `smetana-lock` label
   never enters recovery: a held lock is another lead mid-merge or a stale claim, both
   `merging`'s lock section's business — and parking it would leave it unclaimable for
   everybody. **An issue on this list is not provably an
   orphan.** The assignee is the evidence — `bd show <id>` carries it, and every run
   claims under its own actor — so a `smetana-run-<id>` that is not this run's own
   (yours is `$BEADS_ACTOR` in your environment) may be a killed run's leftovers, or a
   run still live on the same board, mid-flight in its own worktrees.

   **The app's run registry is what tells those apart.** It is `.smetana/runs.json` in
   the project root — read it there, not from your worktree: the folder is ignored, so
   it never travelled into the tree you were cut. It holds one record per run the app
   has going, each carrying the run's `token`, its `targetBranch`, the `batches` it has
   started — one `actor` apiece, the very `smetana-run-<id>` strings you are matching —
   and a `writer`, the app process that wrote the record. Session ids restart at 1 on
   every launch, so the actor's name proves nothing on its own; the `writer` is what
   does.

   Read a record's liveness off its `writer`, never off a date in the file:

   ```bash
   cat .smetana/runs.json                     # from the project root
   ps -p <writer.pid> -o pid=,comm=           # per record you care about
   ```

   No such process → the app that wrote that record is gone, and every actor under it
   is dead. A process that is there but is plainly not Smetana → the pid has been reused
   since; the record is dead too. Smetana under that pid → that run is live, and its
   claims are not yours to touch. Judge that last one, do not string-compare it: the
   recorded `command` is the kernel's short name for the process, while `ps -o comm=`
   prints the full executable path on macOS and the short name on Linux, so the two are
   equal on one platform and not the other. What you are deciding is whether the process
   under that pid is plausibly the app.

   The app has usually dealt with the *processes* of a dead record already — at its next
   start it hangs up the process groups it recorded, for every project it had open, so a
   project nobody has reopened since may still have them running. It writes to the
   tracker nowhere either way. The claims are yours.

   So: **an actor named under a dead writer is a claim you may recover.** **The
   default everywhere else, in every mode: a claim you cannot show dead is left in
   place** — not recovered, not parked, not noted. That covers an actor under a live
   writer, an actor the file does not name at all, a project with no
   `.smetana/runs.json`, and a file you cannot read. It is another run's work until
   proven otherwise, and interfering with it is worse than skipping recovery. Recover
   what the registry shows dead, plus anything else the caller's policy lets you treat
   as dead, and finish it **in place**, one at a time:
   - Its slug is `<id>-<short-kebab-title>` and its worktrees are already at
     `<repo>/.worktrees/<slug>` — the id in the slug is what proves they are this task's.
     None found → park it ("in_progress with no worktree to resume").
   - Re-read the spec. Empty or vague → park it.
   - Resume Phase 1 in the **existing** worktrees. Brief the worker to **finish** the
     task — the tree may already hold partial work; it is not starting over. Then the
     review loop, same five-pass ceiling. Clean → `ready_to_merge`. Not clean after the
     fifth → park, with what the reviewer still objects to.
3. Leave anything already at `ready_to_merge` alone. Phase 2 takes it.
4. Only when nothing you may recover is left at `in_progress`, go on to Phase 0. Phase 2 then merges the
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
   holds it is skipped, not retried. The merge lock sits `open`, so `bd ready` hands it
   over like a task: **anything carrying the `smetana-lock` label is never taken** —
   `provisioning` says how each claim form skips it. Then load the spec through
   `provisioning`. Vague or empty → policy (park, or ask). Provision, serialized on
   you.

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

**The whole phase runs under the merge lock.** Claim it per `merging`'s lock section
before the first task, and release it after the last — on every ending of this phase,
a batch that parked everything included. Two runs merging into one branch serialize on
that claim; waiting for it, and breaking a stale one, are report lines, never silence.

1. **Order** — topological over the survivors (`bd dep tree <id>`), tie-broken by
   priority and then by id. A cycle among them → park every task in it.
2. **One task completely before the next.** `merging` says why.
3. A partially merged multi-repository task rolls its merged repositories back before it
   stops — `merging` has the mechanics, and which stop it is follows the rule above: a
   question parks, an obstacle stays `ready_to_merge`. The target branch never holds
   half a task.

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
  **close the task and file a human check card** (below). It used to park here, and that
  was the wrong end to stop at: Phase 2 has already put the work on the target branch and
  its gates were green, so holding it out of the run's account because there was nothing
  on this machine to look at it with buries finished work under a fault that has nothing
  to do with it. `bd note <id> "live check INFRA: <what failed to start, and the tail of
  its log>"`, then close and remove worktrees exactly as a PASS does.

  A run's browser tooling is the ordinary way to get here: the machine may have none
  (`browser.rs` decides that before the run starts), and Playwright's one shared profile
  may be held by a browser this process cannot see — a person's own Chrome, or another
  project's run. All of it is INFRA and none of it is about the task.

**The epic gate.** After every close in this phase: if the task had an epic parent and
`bd dep tree <epic-id>` shows no other child still open, in progress, waiting to merge or
parked → run one check for the epic itself, against the epic's own description, as an
end-to-end flow. PASS → close the epic. FAIL → fix forward as above, with the fix as an
epic child, and the epic stays open. INFRA → retried once and then treated exactly as a
task's second INFRA: close the epic and file its human check card. The gate is the only
place an epic can reach a verdict at all — its children never get checks of their own — so
without this clause the one case the card was written for has no outcome at the epic level,
and a whole epic's worth of merged work would sit open over a stand that would not start.
**A parked child skips the gate entirely**: note on the epic that it is waiting on that
child, and leave it open.

### The card that says a person still has to look

Some work is finished, merged and green and still cannot be counted until somebody goes and
uses it. That is not a question about the spec and not work that has stalled, so it is
neither `parked` nor `deferred`: file a **separate check task** in the status `human_check`,
and leave the task it is about to close exactly as it otherwise would. The checked task's
own status is never changed to make room for this.

**Only in this phase, and only when the live check is on.** A run started with the live
check off files none of these at all — somebody who switched it off has said they do not
want to be sent looking tonight.

**The findings switch does not reach this card, and the live-check switch is the one that
does.** That is not an exemption granted to it; the two switches answer different
questions. Findings are things a review or a check *noticed* that might be worth working
on later, and the switch turns off filing them because a queue that feeds itself is worse
than a lost observation. This card is neither noticed nor work: it is the run stating what
it could not verify about a task it has just merged and closed, which is a fact about that
task and not a candidate for anybody's queue. So a run with findings off and the live check
on files these exactly as it would otherwise, and a run with the live check off files none
whatever the findings switch says.

File one in exactly three cases:

1. **A second INFRA.** There was nothing here to check with: the stand never came up, the
   browser was held by somebody else's profile. This is the case the whole card exists for.
2. **An acceptance criterion no agent can tick.** "In both themes and both densities",
   "nothing is clipped", "it looks right". Those stay unticked under a PASS on every
   scenario, and a PASS that quietly leaves them behind claims more than it checked.
3. **You could not run it where a person runs it, and you are left with doubts.** For a
   desktop application that is most of the surface — a browser, a component gallery and a
   dev server are reachable from here and the application's own windows are not.

File none for anything else. Not when the check was skipped as nothing a person can see —
a refactor, tooling, internal work. Not on a clean PASS with no doubt left. And **FAIL does
not change**: that is a bug and a fix forward, exactly as above.

**One card per task, and for an epic one card for the epic as a whole.** An epic's children
get none of their own — this phase already defers their check to the epic — so the epic's
card covers the lot.

Link it to what it is about, and never so that it blocks:

```bash
check=$(bd create --title "<what is being checked>" --type task --priority <N> \
          --validate --silent --body-file - <<'EOF'
<the walkthrough, then ## Acceptance Criteria>
EOF
)
bd update "$check" --status human_check
bd dep add "$check" <task-id> --type related
```

The priority is **the checked task's own**, copied across. Nothing ever queues this card, so
the number decides no order of work and buys nothing by being reasoned about; what it does
is carry the weight of the thing being checked into a column somebody scans by eye. Invent
one and every lead invents a different one, and the column sorts by an accident.

`human_check` has to exist as a status before that `bd update`, and Phase R's very first
step is what makes it exist — the full set, all three names. A refusal there is bd declining
a status it has never been told about, not a bd fault: go and look at that config line
before anything else.

`related`, never `blocks`: only a `blocks` dependency holds a queue back, and a card waiting
on a person must never hold work up. `human_check` keeps the card out of a run by itself — a
run takes `open` and recovers `in_progress` and `ready_to_merge`, and this is none of the
three — so it neither joins a batch nor stops the queue from reading as empty.

**It is not a finding, so the depth budget does not reach it and it carries no `spawned`
label.** That budget is written against a queue that feeds itself: a finding became work,
and the work produced another finding. This card cannot do that, because nothing will ever
pick it up. Where it came from is said by the dependency rather than by a `spawned-from:`
line.

**Write it for a person, in plain words: where to click, what to look at, what should
happen.** No file names, no symbols, no paths, no identifiers — whoever reads it is sitting
in front of the running application, not in front of the editor, and the words a task is
written in for an agent are the wrong words here. The title names the thing being checked,
not the task it came out of. The rest of `filing-a-task` still holds — `--validate` and a
real `## Acceptance Criteria`, which here is simply the list somebody ticks off as they walk
through it.

## The report at the end

- **Merged and closed** — per repository: the new tip of the target branch, the gate
  results, anything that was regenerated rather than merged, and the live check's verdict
  per task (or "live check: off").
- **Parked** — id, the concrete reason, and the worktree path. The reason is a question,
  or it does not belong in this list.
- **Filed** — id, title, depth and root, one line each, under "waiting for a person to
  promote (`bd update <id> --status open`)". None → say so.
- **Waiting on a person's eye** — every human check card you filed: its id, its title and
  the task it is about, one line each. None → say so.
- **Digested** — the count, and the root ids to read.
- **What is left for a person** — the list `merging` ends with: pushing, reinstalling
  dependencies where they changed, applying anything a regenerated artefact needs
  applied.

## The batch's own file, beside that report

Smetana keeps its own account of the run and writes it out as an HTML document when the run
ends. It can see the board and its own clock and nothing else — what comes back from your
session is an exit code and nothing more — so which tasks moved and how long the night took
are its to work out, and **what was actually done is yours to say.**

So, after the report above and before you hand back, write the file the prompt names —
`.smetana/runs/<run>/batch-<n>.json`, whose directory already exists:

```json
{
  "tasks": [
    { "id": "smetana-t9o", "did": "one or two sentences on what you actually did" }
  ],
  "notes": "anything about the batch as a whole, or leave it out"
}
```

- **It is in addition to the report and never replaces any part of it.** The report is prose
  for the person reading this terminal; this file is for one program that renders it into a
  document. Neither is a summary of the other.
- **One line per task you touched**, the ones you parked included, saying what stopped them.
- **Nothing is timed here.** A number you reported could not be checked against anything;
  the app times its own batches and says so only where a batch held one task.
- **It is a record, not a gate.** If it cannot be written, carry on — a batch that leaves
  no file is named in the document as having left no account of itself, and its tasks still
  appear there from the board.

## The rules that hold everywhere in this run

- **Never push.** A run merges into a local branch and says so.
- **Never abort the whole batch for one task.** Park it, or ask, and move on.
- **Never promote a `deferred` task, and never claim one.** Only a person does that.
- **Only mechanical conflicts resolve themselves.** Anything needing a judgement about
  what the program should do stops.
- **You own the tracker and the worktrees.** Nobody you spawned runs either, and nobody
  you spawned spawns anything — that is the guard against a run that grows without bound.
