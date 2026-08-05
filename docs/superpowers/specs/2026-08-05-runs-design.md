# Runs: a task queue that works itself, in any project

## Problem

There is a working autonomous pipeline in another repository of this author's — `holiday-curb`.
A Node runner (`scripts/lead-auto-loop.mjs`) relaunches a fresh headless `claude -p "/lead-auto"`
per batch; each batch claims up to three ready `bd` tasks, provisions one git worktree per affected
repository, spawns worker teammates and a reviewer, merges every review-clean task into a local
integration branch in dependency order, verifies the result in a real browser, and closes the task
only on a green check. Everywhere a human would be asked a question, the task is parked instead. It
has run overnight and it works.

Every load-bearing decision in it was paid for with a defect, and those decisions are worth keeping.
Almost every *value* in it is a fact about `holiday-curb` and worth nothing anywhere else: three
subprojects named `backend`/`frontend`/`admin`, `npm run typecheck|lint|test`, a docker compose in
`backend/`, Postgres on 5433, a stand on 4101/3100/5273, Node 24, an integration branch called
`staging`, drizzle migrations, a magic-link login read out of a log file.

Smetana already holds most of the machinery this needs and has no notion of a run:

| already here | what it gives a run |
|---|---|
| `tracker/store.rs` | every issue with its status and dependencies, in memory, kept current by a watcher |
| `terminal/` | agent sessions under real PTYs, one per session, with "this one is waiting on a person" detection |
| `agents/` | `Intent` → prompt, per-harness skill delivery, `Profile` per CLI agent |
| `resources/smetana/` | a bundled plugin the app already hands to a session |
| the board | what a dashboard would have had to draw |

What is missing is the thing in the middle: something that decides there is work, starts a session
against it, waits, looks again, and stops.

## What the user asked for

A `play` button beside the `+` in the Ready column, and the same button on single task cards and on
epic cards. Pressing it opens a dialog: which branch the finished work merges into, how autonomous
the run is, the minimum priority worth taking automatically, whether the live check runs, and
whether findings discovered along the way get filed — filed into a column that cannot feed the queue
back into itself.

And the whole thing has to work the same for a single repository as for a folder holding four, and
must not know what language the project is written in.

## Decisions taken before the design

Four forks were settled deliberately; the reasoning belongs in the record because the alternatives
were real.

**The loop lives in Rust, inside the app.** A new `runs/` module shaped like `tracker/` and
`terminal/`: one tokio task owning the state, a request queue, events out. Not a shipped script — a
second source of truth about what is happening, a dashboard of its own, and Node in the app's
dependencies, all to re-derive what the board already draws.

**All git mechanics stay with the lead agent, not in Rust.** Merging several worktrees of one task
into one branch is not a fixed procedure, and the dangerous conflicts are the ones git does not
flag — two branches generating a migration with the same number off the same base, a lockfile that
merges cleanly and installs something else. Encoding that as Rust would encode a guess. It stays as
prose the lead follows, the way `merge-core.md` already is.

**Parallelism stays inside the batch, through the harness's own subagents.** One batch is one agent
process; the lead spawns workers and a reviewer and talks to them through the harness's mailbox.
The alternative — Smetana spawning one PTY session per task and relaying review findings itself —
would work on every harness including Codex, but it buys harness-independence with a relay the app
has to implement and a set of workers that share no context. Codex therefore degrades to one agent
working sequentially, which is the same shape of degradation Codex already has in this codebase: no
layer B detection, so a person waiting on it shows as waiting without the question.

**The project's shape is a file in the project, written once.** Not derived every run (the same
facts re-derived per batch, and gates that may differ between runs, which makes a green merge mean
nothing), and not kept in `settings.json` (invisible in review, does not travel with the repository).

## Design

### The vocabulary

```
RunScope    ::= Queue | Task(id) | Epic(id)
RunMode     ::= Auto | Supervised | Solo
RunSettings ::= { target_branch, min_priority, live_check, file_findings }
```

A run is a scope, a mode and its settings, plus the state the worker keeps. **One run per project.**
Two runs in one set of repositories break the invariant that makes merging safe — one task fully to
completion before the next — even when they target different branches, because they share the
working copies, the stand, the database and the ports.

The worker does exactly four things: decide whether eligible work remains, start one agent session
against it, wait for that session to end, look again. Everything else happens inside the session.

### The queue is computed from the tracker snapshot, not from `bd`

`holiday-curb`'s runner shells out to `bd ready --json` and `bd list --json` every iteration, with a
retry wrapper (`lib/bd.mjs`) because each call costs about two seconds and occasionally fails. None
of that is needed here: `tracker/store.rs` already holds every issue with its status and its
dependency edges, and the watcher keeps it current no matter who changed it.

So "what is ready" becomes a pure function over the snapshot — `status == "open"` and every blocking
dependency closed — filtered by the scope and the minimum priority. It is instant, it cannot fail
transiently, and it is a unit test rather than a fixture directory. `queue.rs` carries it.

The one thing that must go through `bd` is configuring the custom statuses, once per run start:
`ready_to_merge` and `parked` are written as the full set, because a partial value clobbers the
rest. That is a tracker-config write, not an issue write, and it belongs to Rust rather than to the
lead: getting it wrong is silent and costs the other statuses.

### The statuses

| status | where it comes from |
|---|---|
| `open` → ready | the board's Ready column |
| `in_progress` | `bd update --claim`, which is also the lock against two runs claiming one issue |
| `ready_to_merge` | custom; set by the lead after a clean review, cleared by the merge |
| `parked` | custom; a dead end left for a human, with the reason in a note |
| `deferred` | **bd's own built-in**; where findings discovered along the way are filed |
| `closed` → done | the only close point |

`deferred` is hidden from `bd ready` and visible in `bd list`, which is exactly the property
`holiday-curb` invented a custom `triage` status for. Using the built-in costs one custom status
fewer and comes with `bd defer --reason` for free. Nothing in a run ever promotes a `deferred`
issue — that is the rule that stops the queue feeding itself, and it is the whole point of the
column: on 2026-07-24 in `holiday-curb`, filing every out-of-scope finding as ready work turned 13
human-authored tasks into 105, one of them 61 descendants deep.

The three of these that bd does not know as reserved statuses render on the board as generated hash
colours with a two-letter code, which is the documented behaviour for any status outside the
design system's reserved set, not a defect.

### Three modes are one policy, not three processes

`holiday-curb` has `/lead-auto`, `/lead-orchestrate` and `/lead-merge`, which duplicate each other
because the mechanics were factored into shared core files and only the escalation policy differs.
Here that difference is a parameter:

| mode | at a fork | offered for |
|---|---|---|
| `Auto` | park the task, note the reason, carry on | queue, epic, task |
| `Supervised` | ask; the session turns `needs-you` and the loop waits | queue, epic, task |
| `Solo` | the agent does the work itself, no subagents, asks freely | a single non-epic task |

In `Supervised`, a session waiting on an answer is neither a crash nor a lack of progress. The loop
does not start another batch while one is alive, and the existing attention detection is what makes
the waiting visible — the agent's row goes loud on its own, in a tab nobody is looking at.

### `.smetana/project.toml` — the shape of the project

Next to `.beads/` in the project root, and not tracked by git: `runs/gitignore.rs` puts `.smetana/`
into the project's `.gitignore` when a setup session starts, before the agent has written anything,
so the folder is ignored from the moment it appears. In a multi-repository folder the root is
usually not under git at all, and then there is nothing to do — each repository below tracks only
itself and never sees the folder. Nothing about the mechanics differs between the two cases — a
monorepo is `repos = ["."]`.

That the app decides this rather than the setting-up skill is the point. An instruction in prose is
a thing an agent can follow, argue with or quietly skip, and it did all three: reading a
`.gitignore` whose neighbouring lines hide the tracker and the docs, it will as readily conclude the
folder belongs there as conclude the opposite, and the answer then differs from project to project.

```toml
[project]
# Repositories a run may provision worktrees in. Order is merge order:
# whatever produces a contract merges before whatever consumes it.
repos = ["backend", "frontend", "admin"]

[defaults]
target_branch = "staging"
min_priority = 2
max_parallel_tasks = 3
review_passes = 5

[repo.backend]
setup = "npm install"                    # after a worktree is created
gates = ["npm run typecheck", "npm run lint", "npm test"]
env_files = [".env"]                     # copied from the main checkout

[repo.core]                              # another project, another language
gates = ["cargo fmt --check", "cargo clippy -- -D warnings", "cargo test"]

[preflight]
commands = ["docker compose up -d"]
health = [{ url = "http://localhost:4001/health" }, { tcp = 5433 }]

[merge]
# Prose, for what does not reduce to a rule but must be checked after EVERY merge.
# Declared before the array of tables below: in TOML a bare key after
# [[merge.regenerate]] would belong to that entry, not to [merge].
hazards = """
backend/src/db/migrations: two branches cut from the same base produce a migration
with the same number; git keeps both files and flags nothing. Check the indices and
the journal after every merge, regenerate on a mismatch, and never silently drop a
hand-written data migration.
"""

# Files that are never merged by hand: take the target branch's copy, regenerate.
[[merge.regenerate]]
paths = ["admin/src/api-types.ts"]
command = "npm run generate:api-types"

[live_check]
mode = "browser"                         # browser | command | none
```

Three properties of this file are deliberate.

**Declarative where the work is mechanical, prose where it needs judgement.** `regenerate` is a rule
anyone can apply without thinking. `hazards` is the part of `merge-core.md` that cannot be reduced
to a list of paths, and it stays as text the lead is required to read. Nothing names a language;
gates are commands.

**A damaged config refuses to start the run.** This is the opposite of `settings.json`, where a
broken section loses itself and the app carries on, and it is opposite for the right reason: there,
the cost of leniency is cosmetic; here, a run that lost half its gates produces green merges that
mean nothing. The dialog names the section it could not parse and the button stays disabled.

**No file, no run.** But the survey that produces it does not wait for a run — see below.

### The survey runs when a project is added, and costs nothing

Finding the git repositories under a folder, reading `package.json` / `Cargo.toml` / `go.mod` /
`pyproject.toml` / `Makefile` for candidate gate commands, noticing a `docker-compose.yml` — all of
that is a deterministic filesystem walk costing milliseconds and no tokens. `survey.rs` does it,
split the way `files/` is: pure logic beside the disk.

Adding a project to the list opens a dialog saying the project needs a survey and that
`.smetana/project.toml` will be created, with ok and cancel. Ok starts an agent session with the
`Setup` intent, the survey pasted in as facts: here are the repositories, here are the scripts
found, this looks like a stand. What is left for the agent is what the survey cannot decide — which
of those commands are really gates, what belongs in `hazards`, whether a live check exists — and it
writes the file. Cancel leaves the project added and unconfigured.

An unconfigured project is an ordinary state, not an error, and it is surfaced where it costs
nothing: a quiet marker on the project's row, which is also the button that starts the setup, and —
once the play buttons exist — their disabled state with the reason. The board is **not** replaced by
a notice the way `no-project` and `not-a-beads-repo` replace it: most projects will be unconfigured,
and taking the app's main screen away to advertise a feature is the wrong trade. That row is also
the path back when a config is deleted or goes stale later.

The app never writes the draft itself. `.smetana/project.toml` is a file a person will read and
edit, and it should appear as the result of somebody's decision rather than on its own.

### Skills, not slash commands

Slash commands are a Claude Code mechanism; Codex has none. The process therefore moves into the
bundled plugin that already exists, and reaches a session through the delivery split already built:
`PluginDir` names skills, `Inline` gives absolute paths to their `SKILL.md` — the same choice
`prompt.rs` already makes for `brainstorming` in the `Auto` position. Bodies are never pasted: there
are seven of them, tens of kilobytes, and both harnesses can read files.

| skill | what it was |
|---|---|
| `filing-a-task` | already shipped |
| `project-setup` | new |
| `running-tasks` | `lead-auto.md` + `lead-orchestrate.md`; the policy arrives as a parameter |
| `provisioning` | `provisioning-core.md` |
| `merging` | `merge-core.md` |
| `live-checking` | `live-check-core.md` |
| `reviewing` | `review-checklist` + `code-reviewer.md`, minus the stack |

The worker role files (`.claude/agents/backend.md` and its siblings) do not survive as files — they
were descriptions of `holiday-curb`'s stack. What replaces them is a section of the brief the lead
fills in: the repository, the worktree path, that repository's gates from the config, and the
pasted spec.

Out of `running-tasks` go all the values that were facts about `holiday-curb`. What stays is
everything that was a conclusion drawn from a defect: the spawn budget with its lineage marker and
depth cap, the five-pass review ceiling, strictly sequential merging, the atomic rollback of a
half-merged multi-repository task, standing workers down before removing a worktree, and "only
mechanical conflicts resolve themselves".

### The worker

`src-tauri/src/runs/`, shaped like `tracker/` and `terminal/`:

| file | what it does |
|---|---|
| `model.rs` | `Run`, `RunScope`, `RunMode`, `RunSettings`, `RunState`, `StopReason`, `RunError` — the vocabulary and the pure state transitions |
| `queue.rs` | what is eligible under a scope and a priority floor, what is unfinished, `next_action`, the stop conditions — the port of `loop-state.mjs`, and where the tests are |
| `config.rs` | reading and parsing `.smetana/project.toml`; a damaged file is a refusal with a named section |
| `survey.rs` | the project scan |
| `preflight.rs` | the declared commands and the health poll — the port of `preflight.mjs` |
| `service.rs` | the worker: one tokio task; starts a session per batch, awaits its exit, re-reads the snapshot, decides, emits events |
| `commands.rs` | thin `#[tauri::command]`s: `run_start`, `run_stop`, `run_state`, `project_survey` |

The loop: snapshot → `next_action` → one session with `Intent::Run { scope, mode, settings }` →
await exit → snapshot again. It stops when no eligible work remains; when a whole batch that ran to
completion changed neither the eligible set nor the unfinished set (something is stuck); at the
iteration ceiling; or after five consecutive session failures, with exponential backoff between
attempts. Progress means the *identity* of those sets changing, not a count of closed issues — a
batch that only advanced tasks to `ready_to_merge` made progress.

Unfinished work (`in_progress` / `ready_to_merge` left by a killed run) is tracked separately and
keeps the loop alive, because it is invisible to the eligible set and nothing else would ever pick
it up. Recovering it is the first phase of every batch, in place, in the worktrees that already
exist — the ID inside the worktree slug is what guarantees a found tree belongs to that task.

Sessions do not survive a restart, so neither does a run. That is not a gap to close: a run row with
dead processes behind it would be worse than none, and the recovery phase is exactly how the next
run picks the work back up.

The worker learns a session ended from the terminal worker, which already owns that fact —
`Session::finish` records the exit code and `SessionState::Exited` is terminal. Exposing it to
another in-process subscriber is the one addition `terminal/service.rs` needs.

Three things from `lead-auto-loop.mjs` are deliberately not ported. The dashboard: the board and the
agents panel already are one. The retrying `bd` wrapper: the snapshot is in memory. And usage
gating, which reads `claude -p "/usage"` and regexes two lines out of its plain-text output — a
parse of somebody else's prose that breaks silently. An overnight run that exhausts a limit will
instead hit five consecutive failures and stop cleanly with a reason a person can read. If that
turns out to hurt, a pause-on-limit can come back later.

### Autonomy is a property of the profile

`Auto` must not stop on a permission prompt. That is not the loop's business: `agents/claude.rs`
passes `--permission-mode bypassPermissions`, `agents/codex.rs` passes its own equivalent, and the
`Launch` carries whether this session is autonomous. Everything harness-specific stays in the file
named after the harness, as it already does.

### The interface

**The column header.** `ColumnHeader` already has an `actions` slot; `play` goes to the left of the
`+`, behind a `runnable` prop and a `run` emit. `play` and `pause` are registered in
`core/icons.js` already; only `square` needs adding. Ready is the one column carrying both buttons:
`+` files a task, `play` works the queue.

**The card.** In `TaskCard`'s top row, on the right, beside where `ASK` and `new` live. It is
**always in the DOM** and only its opacity changes on hover and selection — a button that appears on
hover would change the card's height under the pointer, and in this system interaction is a step of
surface, never a shift. An epic card carries the same button with a different scope: `type ===
"epic"` means the children (`parent`), anything else means one task.

**`RunModal`** — a new `components/run/` group, following every rule the others do: computed style
objects of token references, exported from `index.js`, present in `Gallery.vue`.

| field | control | behaviour |
|---|---|---|
| — | a line at the top | "Ready queue · 7 tasks" / "smetana-42 · title" / "Epic smetana-12 · 5 children" |
| Branch | `Select` | the intersection of local branch names across the configured repositories; defaults to `defaults.target_branch` |
| Mode | `Select` | Auto / Supervised / Solo; Solo only for a single task |
| Minimum priority | `Select` | P0…P4; hidden for a single task |
| Live check | `Switch` | disabled, with the reason, when the config says `mode = "none"` or has no section |
| File findings | `Switch` | captioned: new bugs go to deferred |

Off, that last switch does not silence findings — it removes the only path that creates an issue
from one. What a reviewer or a checker surfaces is still recorded, as a note on the root task, which
is the trail a person reads when deciding what deserves promoting. On, the spawn budget still
applies: only a blocking finding at depth 1–2 becomes an issue, and it lands in `deferred`.

The last values used are remembered in `settings.json` beside the rest of the project's state
(`projects.<path>.run`), the same as open tabs and the selected task.

The branch list comes from one `git branch --format=…` when the dialog opens, rather than from
reading `.git` directly. That is a deliberate divergence from `git.rs`, which reads `.git/HEAD` to
avoid spawning a process for one line: HEAD is one line in a known file, while the branch list is
spread across `refs/heads/` and `packed-refs`, and reimplementing packed-refs parsing to save one
spawn on a dialog opening is the wrong trade.

**While a run is going**, every `play` is disabled with a "run in progress" tooltip — there is one
run per project. The run itself lives in the scope bar above everything: a dot, the scope, the
iteration and a stop button. There is no progress screen and there should not be one; the progress
*is* the board — cards move into in_progress and ready_to_merge, agents appear as rows, and the one
that hits a question goes loud by itself, through machinery that already works.

## Tests

Following the split the codebase already has. Pure Rust logic carries the tests: `queue.rs`
(eligibility under each scope, the priority floor, progress and every stop condition), `config.rs`
(parsing, defaults, and that a damaged section refuses rather than degrades), `survey.rs` (what a
folder of repositories resolves to), `preflight.rs` (the poll, injected clock and sleep, as in
`lib/preflight.test.mjs`), and `prompt.rs` for the new intents — including that the JSON the front
end actually sends deserializes, which is the one place either suite crosses the IPC boundary.

The front end gets store tests for `runs.js` through the mocked IPC transport, including the
stale-response guard `terminals.js` and `git.js` already carry: a run state arriving for the project
that was just switched away from must not be shown under the new project's name.

Not covered, deliberately, as everywhere else: `.vue` files. `RunModal` and the buttons are checked
by eye through `?view=gallery` in all four theme × density combinations.

## Implementation order

Each step leaves the app working and is verifiable on its own.

1. **Config and survey.** `config.rs`, `survey.rs`, the `project_survey` command, the add-project
   dialog, the `Setup` intent and the `project-setup` skill. Visible result: a project gets a
   `.smetana/project.toml` a person can read. Nothing else in the app changes.
2. **The skills.** `provisioning`, `merging`, `live-checking`, `reviewing`, `running-tasks`, ported
   and stripped of the stack. Verifiable by starting an agent by hand against the skills, with no
   run engine at all.
3. **The worker.** `runs/`, the commands, the events, the session-exit hook in `terminal/service.rs`,
   the `Run` intent in `prompt.rs`, autonomy in the profiles.
4. **The interface.** The buttons, `RunModal`, the run segment in the scope bar, `runs.js`, the
   remembered settings.

## Deliberately out of scope for v1

- Pausing a run on subscription limits (see above).
- More than one run at a time, and a queue of runs behind the active one.
- Creating a target branch that does not exist yet — the dialog offers what is there.
- Pushing anything. A run merges into a local branch and says so, as `holiday-curb` does.
- Reserved design-system colours for `ready_to_merge` / `parked` / `deferred`; the generated palette
  handles them correctly today.
