# Stage 3 — the run worker

Spec: `docs/superpowers/specs/2026-08-05-runs-design.md`. Stages 1 and 2 are on
`feature/runs-project-config`; this continues in the same branch.

What this stage delivers: a run can be started and stopped over IPC, it works the board
until there is nothing left to do, and the app can say what it is doing. **No buttons** —
the interface is stage 4. Verified through the mock IPC transport and the Rust suite; the
one thing that cannot be verified without stage 4 is a person clicking start.

## Global constraints

- Comments, test names, `expect`/`panic` strings and log lines in **English**. Commit
  messages in Russian.
- No new dependency without saying why. Nothing here needs one.
- The worker follows the shape `tracker/service.rs` and `terminal/service.rs` already
  have: one tokio task owning all mutable state, a request queue, a `select!`. Nothing
  shares state with it.
- Pure logic carries the tests: `model.rs`, `queue.rs`, `preflight.rs`. `service.rs` gets
  no test — it is I/O and orchestration, exactly like the two workers before it.
- One run per project, and one run at a time in the window. Concurrency is deliberately
  out of scope (smetana-tra).

## File structure

| file | new? | what it does |
|---|---|---|
| `runs/model.rs` | new | `RunScope`, `RunMode`, `RunSettings`, `Run`, `RunState`, `StopReason`, `RunError`, and the pure transitions |
| `runs/queue.rs` | new | the snapshot of the board under a scope and a floor, and `next_action` |
| `runs/preflight.rs` | new | the declared commands and the health poll, clock injected |
| `runs/service.rs` | new | the worker: a session per batch, awaits its exit, re-reads, decides, emits |
| `runs/commands.rs` | edit | `run_start`, `run_stop`, `run_state` beside `project_config` |
| `terminal/service.rs` | edit | `Request::AwaitExit` — the only new thing the run worker needs from it |
| `agents/mod.rs` | edit | `Intent::Run`, and `Profile::autonomy` |
| `agents/prompt.rs` | edit | the `Run` arm |
| `agents/claude.rs`, `codex.rs` | edit | what each harness needs to run unattended |
| `lib.rs` | edit | the worker's setup and the three commands |
| `stores/runs.js` | edit | `startRun`, `stopRun`, the run state, the event subscription |
| `stores/mockBackend.js` | edit | answers for the three commands in a browser |
| `tests/stores/runs.test.js` | edit | the store's new surface |

## Task 1 — `runs/model.rs`: the vocabulary

The types the front end and the worker share. Nothing here does I/O.

```rust
pub enum RunScope { Queue, Task(String), Epic(String) }
pub enum RunMode { Auto, Supervised, Solo }
pub struct RunSettings {
    pub scope: RunScope,
    pub mode: RunMode,
    pub target_branch: String,
    pub min_priority: u8,
    pub live_check: bool,
    pub file_findings: bool,
}
pub enum RunState { Preflight, Working { iteration: u32 }, Waiting, Stopped { reason: StopReason } }
pub enum StopReason { QueueEmpty, NoProgress, MaxIterations, Crashed(u32), Unreadable, Cancelled, Preflight(String) }
```

`Run` carries the settings, the project, the current state, the iteration count and the
id of the session running right now (`None` between batches). It is what
`run_state` returns and what every event carries, so the front end never has to
reconstruct it.

**`Solo` is only valid on `RunScope::Task`.** The rule lives here, as
`RunSettings::validate`, not in the dialog: a mode that quietly means something else on
an epic is the kind of thing that survives a refactor of the dialog.

Serialization is snake_case, matching `ConfigState` — the comment there records why.

Tests: `Solo` on a queue or an epic is refused and on a task is accepted; a stop reason
round-trips through serde with the shape the front end reads; `RunState` transitions
refuse to leave `Stopped` (the same rule `Session::apply` has, for the same reason).

## Task 2 — `runs/queue.rs`: what is left to do

The port of `loop-state.mjs`, reading the tracker's in-memory issues rather than shelling
out to bd. Pure: `&[Issue]` in, numbers out.

```rust
pub struct QueueSnapshot {
    pub ready: Vec<String>,
    pub unfinished: Vec<String>,   // in_progress + ready_to_merge
    pub closed: usize,
    pub parked: usize,
}
pub fn snapshot(issues: &[Issue], scope: &RunScope, min_priority: u8) -> QueueSnapshot
pub fn next_action(now: &QueueSnapshot, prev: Option<&QueueSnapshot>, iteration: u32,
                   max_iterations: u32, last_batch_crashed: bool) -> Action
```

Decisions carried over from the source, each with its own test:

- **`ready` means bd's `open` and nothing else**, minus anything worse than the floor,
  minus anything with an unsatisfied dependency. `deferred`, `parked` and
  `ready_to_merge` are excluded — that exclusion is what stops a run feeding itself.
- **`unfinished` is tracked separately** because `bd ready` hides it. A run keeps going
  while *either* set is non-empty, or orphans left by a killed batch are never picked up.
- **Progress is either set changing**, not the closed or parked counts. A batch that only
  advances tasks to `in_progress`, or merges an orphan, or parks a stuck one, has made
  progress.
- **A crashed batch suppresses the no-progress stop.** An unchanged queue after a crash
  means the batch never ran, not that the queue is stuck.
- Scope narrows the eligible set: `Task(id)` is that issue alone, `Epic(id)` is the
  issues whose `parent` is that id.

Tests: an empty board stops with `QueueEmpty`; identical sets across two passes stop with
`NoProgress`; the same, after a crash, runs again; the floor drops a P3 and keeps a P2;
an epic scope ignores an unrelated ready issue; a ready issue blocked by an open
dependency is not ready; the iteration cap stops.

## Task 3 — `runs/preflight.rs`: bringing the project up

The port of `preflight.mjs`, minus everything about holiday-curb's Node and containers.

```rust
pub async fn wait_for_healthy<F, Fut>(check: F, timeout: Duration, interval: Duration,
                                      clock: &impl Clock) -> bool
pub fn parse_health(check: &HealthCheck) -> …   // url → GET, tcp → connect
```

The clock is injected so the tests do not sleep — that is the whole reason the source
file separated them. Running `[preflight].commands` is a sequence of `std::process`
spawns with their output captured; a non-zero exit is `StopReason::Preflight(name)` and
the run never starts, because a run against an infrastructure that did not come up
produces failures nobody caused.

Tests: a check that is true immediately returns without sleeping; one that never becomes
true returns false after the budget and not before; one that becomes true on the third
poll returns true; the injected clock proves how many intervals elapsed.

## Task 4 — `terminal/service.rs`: awaiting a session's exit

The one thing the run worker needs that the terminal worker does not offer:

```rust
Request::AwaitExit(SessionId, oneshot::Sender<Option<i32>>)
```

The waiter is stored beside the session and fired from the same place `Session::finish`
is called. A session that has already exited answers immediately rather than hanging —
that is the race, and it is why this is a request rather than a subscription: the run
worker creates the session and asks in the next breath, and the process may be gone by
then.

No test — this is the worker, like everything else in that file.

## Task 5 — `agents/`: the `Run` intent and running unattended

- `Intent::Run { settings: RunSettings, config_path: String }`. `prompt.rs` grows an arm
  naming `smetana:running-tasks` (PluginDir) or the absolute path (Inline), and stating
  the run's settings as facts: scope, mode, target branch, priority floor, whether the
  live check runs, whether findings may be filed. The skill reads the rest from
  `.smetana/project.toml` itself.
- `Profile::autonomy(&self, mode: RunMode) -> Vec<String>` — the arguments a harness
  needs to work without a person. For Claude Code that is
  `--permission-mode bypassPermissions` in `Auto`, and nothing in `Supervised` and
  `Solo`, where a person is there to answer. Codex gets its equivalent, and where a
  harness has none the run still works — it simply stops on every permission prompt,
  which `Supervised` already handles and which is a fact about that harness, not a bug
  here.
- `CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0` goes into the environment for a `Run`
  session. Without it the CLI kills still-working subagents at its 600-second default,
  and that was a real defect in the source: a batch losing its workers mid-task with
  nothing in the output to say why.

Tests in `prompt.rs`: the `Run` prompt names the skill in both deliveries; every setting
appears in it; `Solo` says the agent does the work itself and `Auto` says it parks
instead of asking. Tests in `claude.rs`/`codex.rs`: `Auto` carries the bypass argument
and `Supervised` does not.

## Task 6 — `runs/service.rs`: the worker

One tokio task, `select!` over a request queue and the current batch's exit.

The loop, per iteration:

1. Ask the tracker for its snapshot (`TrackerHandle`, `Request::Snapshot`). Two failures
   in a row → `StopReason::Unreadable`.
2. `queue::snapshot`, then `queue::next_action`. Stop → emit and finish.
3. Create a session through `TerminalHandle` with `Intent::Run`, remember its id, emit
   the state.
4. `Request::AwaitExit`. A non-zero exit is a crash: bound consecutive crashes at five
   with exponential backoff from 5s to 60s, and stop with `Crashed(n)` at the ceiling.
5. Round again.

`run_stop` is cooperative: it sets a flag, and the loop finishes the batch in flight
rather than killing a session mid-merge. **A run interrupted between a merge and a close
is the state Phase R exists to recover**, and killing one mid-merge is how you get there
on purpose. The front end says "stopping after this batch" — that is stage 4's line, and
this stage emits the state it renders.

Events: `run://state` carrying the whole `Run`, on every transition. The front end
resyncs by calling `run_state`, the same shape `tracker_health` has, and for the same
reason — the event fires before the webview can subscribe.

Preflight runs once, before the first iteration, not per batch.

## Task 7 — `commands.rs` and `lib.rs`

`run_start(project, settings)` → `Result<Run, RunError>`; `run_stop(project)` → `Run`;
`run_state(project)` → `Option<Run>`. Thin, exactly like the tracker's: put a request on
the queue, await the reply. `RunError` covers a run already going in this project, a
project with no config (`Missing`) or a broken one (`Broken`) — **this is where the
broken config finally becomes visible**, which is half of smetana-8av.

## Task 8 — `stores/runs.js` and the mock

`runsState` grows `run` (the `Run` or null). `startRun(settings)`, `stopRun()`, and a
`run://state` subscription set up in `initRuns`, with the same stale-response guard on
`runsState.project` that `loadConfig` already has — a run state arriving for the project
that was just switched away from must not be shown under the new project's name.

`mockBackend.js` answers `run_state` with null, and rejects `run_start` and `run_stop`
loudly, like every other write. A browser has no worker and a run that looked like it
started would be worse than none.

Tests: `startRun` puts the returned run in the store; a `run://state` event for another
project is ignored; `stopRun` on no run is a no-op rather than a throw.

## Order

1 → 2 → 3 in any order (all pure, no dependencies). 4 and 5 before 6. 7 after 6. 8 last.

## Out of scope for this stage

Buttons, the dialog, the scope-bar segment (stage 4). Pausing on subscription limits
(smetana-bvn). More than one run at once (smetana-tra). Creating a branch that does not
exist (smetana-cfm). Pushing (smetana-5sc).
