---
name: live-checking
description: Use when a merged task has to be verified beyond its tests — bringing the project up from the target branch, exercising what the task claims to deliver, and reporting a verdict
---

# Checking a merged task for real

Gates prove the code compiles and its tests pass. They do not prove the thing a person
asked for exists. This is the step between the two, and it runs **after** the task has
merged into the target branch, against the target branch — not against a worktree.

Read `.smetana/project.toml` first. `[live_check].mode` decides whether this runs at
all:

- **`none`** — this project has no way to check a task beyond its gates. Say so and
  stop; that is an honest outcome and pretending otherwise is not.
- **`command`** — `[live_check].command` is an end-to-end suite. Run it, and the run's
  exit status plus its output is the verdict. Most of this file then does not apply.
- **`browser`** — there is an interface a person would click through. Everything below
  applies.

`[live_check].notes` is prose from somebody who knows this project: how the stand comes
up, how to sign in, what is seeded. Read it. `[preflight]` is the shared infrastructure —
`commands` bring it up, `health` says how to know it is up.

## The stand

The main checkouts are wherever the person left them, and the merged work is only on the
target branch. So the stand runs from its own long-lived worktrees, one per repository,
and they are **detached at the target branch's tip** — never with the target branch
checked out. A checked-out branch cannot be fast-forwarded, and the merge process needs
to fast-forward it.

Bring-up is idempotent and runs at the start of every check:

1. `git -C <repo> worktree prune`, then create `<repo>/.worktrees/live-<target>` detached
   at the target branch if it is not there, and re-detach it at the tip if it is.
2. Copy each path in `[repo.<name>].env_files` from the main checkout — the same files
   provisioning copies, for the same reason.
3. Run `[repo.<name>].setup` when the dependency manifest changed since the last check.
   Dependencies persist inside the stand's worktree between checks; reinstalling every
   time is minutes wasted on every task.
4. Run `[preflight].commands`, then poll `[preflight].health` until every entry answers
   or a budget runs out — around two minutes each, checked every couple of seconds.
5. Start the project's own processes from the stand worktrees, each with its log going
   to a file you can read afterwards.

Keep the scratch state — logs, pid files, screenshots — in one gitignored directory
under the project root, so teardown is a directory and a list of pids rather than a
search.

**Anything that will not come up is `INFRA`, not `FAIL`.** Read the tail of its log,
include it in the report, tear down, and say so. That the stand is broken says nothing
about the task's code, and reporting it as a code failure sends somebody to the wrong
place.

## Teardown

Always — on pass, on fail, and on your own way out of a crash. Kill every pid you
recorded, sweep the ports you bound, and put back anything you stopped in order to run
(a shared background worker consuming the same queue is the usual one — if you stopped
it, start it again). A stand left running holds ports the next check needs and quietly
serves stale code to whoever looks next.

## Signing in

Use the real path a person uses. `[live_check].notes` says what this project's is, and
which seeded accounts exist. Where a flow depends on something arriving out of band — a
link in an email, a code — the stand is configured not to send it and to log it instead:
read it out of the log rather than reaching around the flow. A shortcut that skips the
real path verifies the shortcut.

## What to check

Derive the scenario from the task's own spec, acceptance criteria first. **Check what
the task claims to deliver, not the whole product** — the rest of the product is other
tasks' business, and a check that wanders takes long enough that nobody runs it.

Per scenario:

1. Walk the path a person would: navigate, click, fill, submit.
2. Read the console. Errors related to this feature are a fail. Pre-existing noise
   plainly unrelated to the task is not — say which you saw and why you discounted it.
3. Read the network traffic. A 4xx or 5xx on this feature's endpoints is a fail; record
   the URL, the status and the body.
4. Screenshot the states that matter, into the scratch directory.

A task with no interface at all gets a smoke test against its API instead, or nothing
when even that would prove nothing — and then say that, rather than inventing a check.

**Before reporting a failure, run the failing scenario once more from scratch.** Only a
reproduced failure is a failure. One that does not reproduce is worth a line in the
report; it is not a verdict.

## Driving the browser

Use whatever programmatic browser automation the harness offers, with its own clean
profile. **Never drive the person's own browser.** An unattended run cannot depend on a
window somebody may have closed, and it must not be typing into a session that belongs to
a human.

## The report

Handed back verbatim to whoever asked, who records it against the issue:

```
LIVE CHECK <task-id>: PASS | FAIL | INFRA
stand: <target branch> @ <repo> <sha>, …
scenarios:
  - <name>: PASS | FAIL — <one line>
console: <the errors that mattered, or "clean">
network: <failed requests: url, status, body — or "clean">
screenshots: <paths>
```

`INFRA` is not a verdict about the code, and the report must not read as though it were.
