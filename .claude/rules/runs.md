---
paths:
  - "src-tauri/src/runs/**"
  - "src/stores/runs.js"
  - "src/components/run/**"
---

# Runs: a batch of the board, carried out by sessions

A *run* is the app driving itself — read the board, start an agent session on a batch of it, wait for
that session to end, read the board again — and it is `src-tauri/src/runs/` plus `src/stores/runs.js`
plus `src/components/run/`. It sits on top of the other two workers rather than beside them: it owns
no board and no PTY, and `lib.rs` hands it clones of both handles so it queues behind them like every
other caller.

| file | what it does |
|---|---|
| `model.rs` | `Run`, `RunSettings`, `RunScope`, `RunMode`, `RunState`, `StopReason`, `RunError` — the vocabulary, and the settings rules that are not the dialog's to keep |
| `config.rs` | `.smetana/project.toml`: the shape of the project a run works in |
| `survey.rs` | what a project looks like from outside, before anyone has configured it |
| `gitignore.rs` | keeping `.smetana/` out of the repository |
| `registry.rs` | `.smetana/runs.json`: what a live run leaves on disk, and the rules for reading it — pure, and where those tests are |
| `procs.rs` | the process table and the two signals: the only `unsafe` in `runs/` |
| `recovery.rs` | the disk half of the registry, and the start-up sweep for what an unclean exit left running |
| `preflight.rs` | bringing the project up before the first batch — declared commands, then declared health checks |
| `usage.rs` | what the subscription has left, and whether to run at full size, a smaller one, or not yet |
| `browser.rs` | whether there is anything on this machine to drive a browser with — pure over file contents and directory listings, and where those tests are |
| `queue.rs` | what is left to do and whether to run another batch — pure, and where the tests are |
| `summary.rs` | what the run did, as a diff of the board between its first read and its last — pure, and where those tests are |
| `report.rs` | that summary and the batches' own accounts, rendered into a self-contained HTML document — pure, and where those tests are |
| `awake.rs` | one power assertion for as long as any run is live anywhere — the counting rule, pure, and where those tests are |
| `service.rs` | the worker: the loop, one run per scope per project |
| `commands.rs` | thin `#[tauri::command]`s, shaped exactly like the tracker's |

`service.rs` is the same single-tokio-task shape as the other two workers. The deciding is `queue.rs`
and that is pure; the map's own lifecycle — `absorb`, `permit`, `admit` and the browser-candidate
list — is pure too, and unlike the other workers this file carries a test module of its own for that
part, because both ways of getting the lifecycle wrong are silent. A project holds several runs at
once and the map is keyed by each run's `token` (smetana-5hf): what is refused is a second run over
the **same scope**, since two runs told to take the whole queue are two leads racing for the same
tasks, while a queue run beside a task run, or two runs over different epics, divide the board
between them. Which tasks each may touch is not this worker's question — bd's atomic claim under
per-session actors (smetana-4fh) is the exclusivity. A run in another project is none of this one's
business. The one thing all runs share is a subscription limit, and a run does not reserve one
(smetana-tra). The loop runs on a task of its own so the worker stays answerable while a batch runs
for an hour, and reports whole `Run` values back through a channel — the worker is the only thing
that ever writes one out. The `token` does the job `generation` does for the tracker: a stop names one
run by it, every `run:state` event carries it, and a late report from an ended run finds no entry
rather than the run that started after it.

**What ends a batch is not one question but two, and answering only the first was a defect that hid
for a fortnight.** An unattended batch is told to exit (`agents::is_batch`, `Profile::batch_args`),
so `await_exit` is its whole ending. The other two modes run the harness the way a person does —
that is what Crew and Solo *are* — so the session finishes the work and sits at its prompt for ever,
and a loop watching the process never came round: `finish` is the only thing in the app that ever
makes a `RunSummary` or writes a report, so a Crew run's account arrived when somebody eventually
pressed stop, hours later, or never, because the app was closed first. Evidence in this repository's
own `.smetana`: a batch whose account file landed at 13:48 against a document stamped 17:45, and a
run the day before with an account and no document at all.

So in attended modes `watch_batch` waits on whichever comes first — the exit, still, or
`handed_back`. The signal is the account the lead was already asked for in every mode, and it is
deliberately **a file that parses** rather than a file that exists: JSON is not written atomically,
and waking on the first byte would send `read_batch` at half a document a moment later, so the
report would say the batch left no account of itself in the one case where it left a good one.
Parsing is that check and there is no second mechanism to keep in step with it. `clear_account` is
the trap under the rule, and it is certain rather than theoretical: `token` counts from zero on
every app start — the very property `write_report` refuses to lean on for its own file names — so a
previous launch's `.smetana/runs/1/batch-1.json` is sitting under this batch's name before it
spawns, and without clearing it the batch would hand back in the instant it started. Clearing also
closes a quieter half for every mode, where a batch that crashed before writing a word would have
had a previous launch's prose put in this run's report.

Two things deliberately do not follow. The session is **left running** — ending the run does not end
the conversation, which is the mode's whole point — and nothing is orphaned by that, because
`registry::forget_run` conditions on the processes rather than on the stop reason and keeps a record
naming one that is still there. And `prompt.rs` says something different to each half: to an
unattended batch the file is "a record, not a gate", true because its ending is the exit; to an
attended one it is how the work is handed back, with a way out that is a sentence in the
conversation rather than a silence, since a lead that shrugged the file off would leave the run
hanging with nothing on screen to say why.

**Stopping is cooperative, and that is a decision with a cost attached.** `request_stop` sets a flag
and the loop reads it between batches; the batch in flight is allowed to finish, because a run
interrupted between a merge and a close is exactly the state the recovery phase exists to clean up. A
run with nothing in flight stops at once, which is what lets the stop button reach a paused one.
`StopReason` keeps `Cancelled` and `SessionRemoved` apart: both are somebody's doing and neither is a
crash, but pressing stop let the batch finish while removing the session killed it where it stood,
and the person reading the bar is deciding whether to go and look at what got left behind.

**A map entry outlives the run it holds, and that is what makes "one run per scope" true**
(smetana-0kb). It leaves in exactly one place — `Report::Ended`, sent by a `Drop` guard when the loop
task is gone however it went — so "there is an entry" and "a loop task is alive" are one fact rather
than two that agree most of the time. Removing the entry the moment a stop declared the run over
looked equivalent and was not: the loop was still between reading the board and spawning, so it put a
batch out that nothing could then stop. The spawn itself is **asked for rather than checked**:
`may_spawn` puts the question on the channel the worker's own `select!` already drains, so the same
single task decides it and handles `Request::Stop`. That is not a FIFO guarantee, but the two can
never interleave, so **both orderings are safe**: stop first and the spawn is refused, spawn first
and the stop that follows finds a batch in flight and waits for it. Yes records that batch as in
flight (`Active.starting`, the fact `Run.session` cannot carry yet).

A stop leaves a gap between the run reading `Stopped` and its entry leaving, and the **refusal in
that gap has its own reason**, `RunError::WindingDown`. Reusing `AlreadyRunning` put two
contradictory things on screen at once — a bar saying the run is stopped and a message saying one is
going — which a person reads as the stop not having taken. The gap is not always brief: the loop may
be inside a board read or a 60s usage probe, and it holds its scope for the whole of it — only its
scope, since the rest of the project's runs were never this one's to hold.

**The machine does not fall asleep by itself while a run is going**, and the count of runs that
decides it is *derived from the size of the worker's map* rather than stored anywhere (`awake.rs`).
A run is the app driving itself for hours, at night, with nobody touching the keyboard, which is
exactly what an idle timer reads as an empty room; the paused run waiting out a spent allowance is
the longest such silence in the subsystem and therefore the night most likely to be lost. The worker
calls `sync(active.len())` at the end of every pass, and that inherits the map's own guarantee
whole: the entry leaves for *every* ending because the `Ending` drop guard sends it, so no stop
reason has to be enumerated here, nothing new has to remember to release, and a panic unwinding
through a loop task releases too. A flag on a `Run` instead would be two halves of one fact, drifting
silently in both directions — a machine held awake for a week by a flag nobody cleared, or a run that
quietly lost its hold and stopped at three in the morning. One assertion for the whole app, taken on
the rise above zero and released on the fall back to it; several projects and several runs per
project share it (smetana-5hf). The system is held and the display is not, so the screen still goes
dark. A failure to take it is a line in the log and never a refusal to run, and it is not retried
until the count returns to zero, or eight hours of log would say the same thing. Nothing about it is
on screen: `RunBar` already says a run is going, and that is the reason the machine is awake.

The promise is worth stating precisely, because the gap between the two sentences is the shape of the
bug report that arrives otherwise. It is **"the machine does not fall asleep by itself"**, not "the
machine cannot sleep": on macOS a closed laptop lid suspends the machine whatever assertions are held
— only mains power with an external display changes that — and sleep chosen from a menu, or forced by
a critically low battery, goes straight through.

Every declared command and every health probe the preflight starts is given the **login shell's**
`PATH`, from the same `shell_env` the terminal uses and for the same reason: a bundled app inherits
launchd's, which holds nothing a person installed, so `docker compose up -d` exited 127 against
infrastructure that was up and answering — and the one phase whose whole job is to name the missing
piece named the wrong one. `shell_env::path` falls back to the inherited value, so this is never a
narrowing.

**The preflight is the one phase where a stop is not cooperative** (smetana-16w). `bring_up` read the
stop channel nowhere at all, so a stop pressed during it waited out every declared command at 600s
apiece and every health check at 120s — on this project the first declared command is `npm install`.
It now watches that channel: the command in flight is killed where it stands, and a check is given up
between looks rather than during one, since a look is bounded by seconds of its own where a command
has nothing bounding it but the ceiling. Killing is safe here for the reason it is refused between
batches: a declared command brings infrastructure up and is run again from the top next time. The
signal goes to the process group, because the child is a shell and the work is what it started. The
ending is unchanged. Two smaller rules hold that up, both found by driving the race rather than by
reading it: `may_start_batch` refuses a run that is merely `stopping`, not only one already over,
since "the batch in flight finishes" has always meant that one and no more; and a report from the
loop is **adopted, not assigned** (`Run::adopt`), because stop is asked for on the worker's side and
never travels to the loop task, so taking the loop's copy wholesale unasked the stop a moment before
the check that reads it.

`queue.rs` is a port of `holiday-curb`'s `loop-state.mjs` with one substitution that changes its cost
and not its logic: the source shelled out to `bd ready` and `bd list` between every batch, about four
seconds each, while this reads the snapshot the tracker worker already keeps current. It tracks
`unfinished` — `in_progress` and `ready_to_merge` — separately from `ready`, because `bd ready` hides
both and a run watching only the ready set would leave a killed batch's orphans on the board forever.
A dependency counts as blocking only when it is bd's `blocks` kind. And `LastBatch` has three answers
rather than "did it crash", because a batch stopped by a spent allowance moved the board no more than
a crashed one did — reading either as a stuck queue would end a run over nothing — while a harness
that keeps falling over needs a person and an exhausted allowance needs only time.

`usage.rs` is the piece the runs design deliberately left out and then took back. Reading
`claude -p "/usage"` is a parse of somebody else's prose that can break silently, which is why it was
refused; what did not survive contact was the trade, since a run that exhausts its allowance
overnight spends five sessions and a minute of backoff discovering it and then stops with `Crashed`,
which says the harness kept failing when nothing failed. So the parse is back with its failure mode
named rather than assumed: **an unreadable answer never blocks a run** — it reads as `Normal`, the
batch goes at full size, which is where things were before the module existed. The gate runs *before*
each batch, so the exhausted case costs no session at all. `service.rs` asks the same question again
after a session exits non-zero, and there it is not a gate but a classification: a spent limit told
apart from a harness that fell over, from the one source of truth.

`browser.rs` answers the question the config could not: `[live_check].mode = "browser"` says what the
*project* wants and nothing about the machine the run rides on, so a run with the live check on
started happily where there was nothing to drive a browser with and found out inside the check, as
INFRA (smetana-29s). Either tool is enough — Playwright, which is two facts and not one (an MCP entry
in `~/.claude.json`, the project's `.mcp.json` or `~/.codex/config.toml`, **and** the browsers
actually downloaded under `ms-playwright`), or the Claude in Chrome extension, found by its id in a
Chrome profile. Every path and id in it is fragile by nature, and that is accepted rather than
hidden: an extension writes itself into no agent's configuration, so the unpacked directory is the
only evidence there is. Hence the rule the whole file is built on — **anything unobservable reads as
"no", loudly**: the toggle goes off and the tooltip names what was not found, rather than staying
live on a guess. Matching an MCP entry goes the other way on purpose (its name *or* what it runs,
either alone), because a false "present" leaves things where they were before the module existed
while a false "absent" takes a working feature away under a tooltip claiming a tool is missing that
is sitting right there.

Busy-ness is the second reason and deliberately only half a question. `Request::BrowserBusy` answers
which projects have a live run that asked for a live check, counted per run and including the asking
project, since a live-check run in this very project is what holds Playwright's one profile against a
second run beside it; `browser_tools` then reads each candidate's config, because the worker knows a
run wanted a check and not whether that project's check opens a browser, and naming a `command` check
as the reason would be an invention. **The extension's busy-ness is out of reach entirely, and so is
a browser a person is driving themselves.** So busy-ness may block **only where Playwright is the
tool that would be used**, which means the extension is absent — letting the branch fire whenever
*either* tool was present disabled the toggle on an extension-only machine over a tool nobody had
shown to be held. The sentence a person reads is composed on the front end
(`components/run/browserTools.js`, pure and tested, one of the `branchChoice.js` family), since it is
UI copy; the scope is `browser` and nothing else.

A pause is a `RunState`, not a `sleep` inside the loop, and that is load-bearing twice over: a run
that had simply gone quiet for three hours is indistinguishable from one that hung, and the bar is
where somebody looks to tell those apart — and being a state is what lets the stop button reach it,
since a paused run has no session in flight. `resets` is the harness's own sentence about when the
allowance clears ("Aug 11 at 5:59pm (Europe/Moscow)"), passed through untouched and never turned into
a moment in time: that would be a second parse of the same prose, and its failure would be a run that
woke at the wrong hour.

`config.rs` refuses to load a damaged file, the **opposite** of `settings/model.rs` and opposite for
the right reason. There, a broken section loses itself and the cost is a forgotten panel width; here
it would be a run whose gates quietly went missing and whose green merges therefore proved nothing —
hence `deny_unknown_fields` throughout, since a typo has to be louder than a silence. `runs::service`
is the first and only place a damaged config is shown to anybody; everywhere else in the app it reads
as "no configuration", which is right for a marker on a row and wrong for starting a run. The file is
declarative where the work is mechanical and prose where it needs judgement — `hazards` stays as text
the lead reads, because two branches emitting the same migration number off one base is not a
pattern, it is a thing to look for.

`gitignore.rs` keeps `.smetana/` out of the repository, and it is code rather than a line in the
setup skill on purpose: an instruction in prose can be followed, argued with or quietly skipped, and
this one was all three — an agent reading a `.gitignore` whose neighbouring lines hide the tracker
and the docs can reasonably conclude either way, and the answer then differs from project to project.
The app decides once, in code. `amend` is pure and carries the tests; it treats `.smetana`,
`.smetana/`, `/.smetana` and even the negation `!.smetana` as already covered, that last one because
it can only have been typed on purpose.

## The run's own account of itself

A run used to end saying one word — `Queue empty`, `Crashed`, `Cancelled` — and that was the whole of
what the app had to say about however many hours it just spent. `summary.rs` and `report.rs` are the
other half: the app keeping its own record and writing it out as an HTML document under
`.smetana/reports/YYYY-MM-DD-HHMMSS.html`. Timestamped rather than keyed by the run's `token`,
because that counts from zero on every app start and would collide across restarts, and nothing ever
deletes one — they are small text, and deciding when a record of a night's work stops mattering is
not this app's call. One second is not one run, though, since a project holds several at once
(smetana-5hf), so `claim_report` *makes* the file with `create_new` and walks a `-2`, `-3` suffix
rather than checking whether the path exists: two runs are two loop tasks, so the creation itself has
to be the exclusive step.

**One document serves all three modes, and it names which of them it is.** A run is exactly one task
in Solo, exactly one batch in Crew and a night of batches in Autopilot, so the difference is what to
call the thing and not what to build — `RunMode::report_title` owns those three words, beside
`one_batch`, which is the rule that makes two of the sentences true, and `report.rs` places them in
the heading and in the tab's own title while owning none of them.

**Three facts about what the app can know decide the whole shape.** It can see the board and its own
clock, so *which* tasks moved and *how long* the run took are its to work out. It cannot see what was
*done* — nothing comes back from a session but an exit code, the same missing channel `claimedBy`
reconstructs around and `SessionWork::Run` refuses to invent — so the lead is asked for it: one JSON
file per batch at `.smetana/runs/<token>/batch-<n>.json`, named in the `Run` prompt and in
`running-tasks`. What comes back is small and sometimes not even a code, but the *ending* is always
there and it is the app's own, so it is written down beside the lead's account rather than thrown
away — see the two halves of a batch below. And it cannot see per-task time: a batch may hold
several tasks with no signal at either end of one of them, so a task gets a duration of its own
**only when its batch held exactly one**, where the two are the same number and nothing is inferred.

Attribution is a **board diff**, not an actor match: a task is this run's when it is `closed` now and
was not `closed` at the baseline, the first board read inside the loop, after the preflight.
`queue::claimed_by` misses two real cases — an orphan Phase R recovered from a *previous* killed run
carries that dead run's actor, and an epic closed in Phase 3 was never claimed by anybody — so the
diff's own cost is taken instead: a task a person closes by hand in another window during the run is
credited to it. The report's scope is deliberately wider than `queue::in_scope`: an epic run reports
the epic itself, since Phase 3 closes it, and the priority floor is not applied, since it decides
what may be *taken*. The merge lock is excluded through `queue::is_lock` rather than a second copy of
the label.

`RunSummary.tasks` is an `Option`, and that is the point of the type: `None` means the diff could not
be computed — the run died in the preflight so there is no baseline, or the final board read failed —
and it is **never** rendered as "0 closed, 0 parked", the same rule `projectBytes` and
`cleanup::refusal` keep. A batch that left no file, or an unparseable one, is likewise named in the
document as having left no account of itself rather than drawn as an empty row, while its tasks still
appear from the board.

**A batch in the document carries two halves, and only one of them is the agent's** (smetana-pmj).
The other is `report::BatchOutcome`: what the loop saw end the batch, drawn under every batch card
whether or not a file was written. It has to be, because the two fail together — an agent killed
mid-merge writes nothing by definition, so a document resting on the file alone goes silent in
exactly the case somebody opens it for. That is not hypothetical: a batch died at 22:01 holding the
merge lock and its whole record was the one sentence about the missing account, and the minute was
reconstructed afterwards from `log show`, a transcript under `~/.claude/projects/` and a file in
`/private/tmp`, none of which the app can see and none of which survives a reboot. The phrase itself
stays and stays about the *account* — the agent really did write nothing — it simply stops being the
whole entry.

The vocabulary is deliberately not a new one: `service::outcome_of` reads out `Batch` and `Exit`,
which the loop is already holding, so a clean exit, a code, a signal with no code, a session somebody
removed, work handed back and a batch the run ended at an unanswered question are six words the app
already had and had never said out loud. The split a person wants first is between falling over and
being ended by the run, and nothing in the document drew it before.

**And the report names what a silent batch left on the board**, through `queue::left_behind` over a
`fresh_board` read: the merge lock if its actor still holds it, and anything left `in_progress` or
`ready_to_merge` under that actor, with ids. Only for a batch that left no account — a lead that
answered has already said where it left things, and a second resync per batch is not worth a line
nobody needed. It is wider than `claimed_by` on purpose, since this is a record rather than a parking
list. **Named, never acted on**: the recovery boundary below holds, so nothing here releases a lock
or rewrites a status, and the line exists precisely because the alternative is the *next* run
discovering the lock by failing to take it. That boundary is about *recovery* and is not a claim
that the loop never writes to bd: `park_claims` does, on the unanswered path, seconds after this
very reading — one batch's `in_progress` claims to `parked` with the question as the note, and the
only bd write `drive` makes.

**Every ending the loop task reaches goes through one `finish(...)` in `service.rs`, and that
consolidation is the feature.** A dozen exits into `RunState::Stopped` is how the next ending
somebody adds quietly arrives with no report behind it — so `finish` is the only thing that ever
makes a `RunSummary`, and `advance` clears the field on every transition that is not `Stopped`, which
makes "`None` in every state but `Stopped`" a property of the type rather than a habit. `finish`
reads the board once more through `fresh_board`, for the ~2 s resync the run's own last writes need.
`did` is agent-written text going into a document a person opens and is HTML-escaped without
exception.

**The loop is not the only thing that reaches `Stopped`, though.** `request_stop` ends a run with
nothing in flight *at once* — which is what makes the button immediate and lets it reach a paused one
— so for a stop landing between batches, on a run waiting out a spent allowance overnight, or during
the preflight, the worker's copy is already `Stopped { Cancelled }` by the time the loop looks at the
channel. The loop then runs `finish`, writes the document correctly, and reports a run `absorb`
refuses, because nothing revives a stopped run; left there, the file sat on disk while the `Run` on
the wire said there was none. So `Run::take_summary_from` is `adopt`'s narrow opposite number: from a
report about a run this side has already ended it takes **the summary and nothing else**, once, and
emits the result. The ending deliberately does not travel with it — somebody pressed stop and was
told `Cancelled`, while the loop may have got as far as finding the queue empty a moment later, and
rewriting the reason under them would put a different run's story on the bar. Neither property the
map rests on moves: the stop is still immediate, and an entry still leaves in exactly one place.

## What an unclean exit leaves, and who clears it

Everything a run knows lives in memory, and sessions are deliberately kept out of `settings.json`
because a session row with a dead process behind it is worse than an empty list. The orderly ending
is `RunEvent::Exit`. A crash, a force quit, a `kill -9` and — in development — every Rust rebuild
reach none of it, and what they strand is tasks claimed by a run that no longer exists and agent
processes nobody will signal. This is the same shape as the window-geometry defect `window.rs` was
written for, where the only write happened at `Exit`.

**The app writes a registry and deals with processes; the tracker half stays with
`smetana:running-tasks` Phase R.** The split follows what each half can see: the app can see the
process table and the tracker cannot, and Phase R already recovers claimed tasks correctly with the
worktrees in front of it. So the app never rewrites `in_progress`, never parks anything, and writes
to bd nowhere as part of recovery — doing both would be a second mechanism doing Phase R's job, and
two mechanisms on one fact drift. The registry is `.smetana/runs.json` in the project folder, beside
`project.toml` and outside the repository, so Phase R reads it with an ordinary file read, needing
nothing from the app and no path passed through a prompt — which a file in `app_config_dir()` could
not be, being platform-dependent and findable by a skill only if the app told it.

**A record proves its own liveness, and an actor id alone cannot.** `BEADS_ACTOR` is
`smetana-run-<session-id>` and session ids restart at 1 on every launch, so after a restart a fresh
session takes a dead run's name. Every record therefore carries a `writer` — the app process that
wrote it — as a pid *plus* that process's start time, which is what survives pid reuse; each batch
carries its actor and its process group. Nothing in the file is read as a date to decide liveness:
the one timestamp says when the run began and ages a record out after a week. The stamp is read per
platform in `procs.rs` (macOS `proc_pidinfo`, Linux `/proc/<pid>/stat` against `btime`) rather than
through a crate, since `libc` is already here for `killpg`; a platform that cannot answer keeps no
registry at all, because a record nobody could ever show stale is worse than none.

At start-up the run worker sweeps every project the settings file lists as open, before it serves its
first request — one writer, so the read-modify-write is safe, and no batch can go out beside a sweep
about to hang up a leftover agent in the same worktree. For a record whose writer is provably dead it
signals the recorded process groups exactly as a clean exit does. **Anything the registry does not
name is never touched** — the app cannot show it started it — and neither is a group whose pid has
since been reused, nor anything under a writer that is alive or unreadable. The sweep is silent: the
app is finishing its own interrupted shutdown rather than taking a new decision, and a modal about
housekeeping after every rebuild would be the loudness budget spent on the opposite of a card needing
a human. What was killed goes to the log.

The record itself **outlives the processes on purpose**, for up to `ABANDONED_DAYS`: its actors are
the evidence Phase R reads, and deleting it the moment the processes were dealt with would send that
half of the recovery back to leaving every claim in place.

A record is removed when its run's **loop task ends — however it ended**: `Report::Ended` comes from
the same `Drop` guard the worker's map leans on, so a cancellation, a crash and a failed preflight
all take the record with them with none of them enumerated anywhere. The one condition is the
processes rather than the reason. `runs::service` ends a run with `NeedsAnswer` **without killing the
session** — the person is being sent to that terminal to answer — so `registry::forget_run` keeps a
record that still names something running, trimmed to the batches actually still there; deleting it
would leave a live agent, still claiming under its actor, named nowhere, and a `kill -9` a minute
later orphans exactly the process this file exists to reclaim. Conditioning on the stop reason
instead would have been a `match` somebody has to remember to extend. `smetana:merging`'s 60-minute
lock staleness rule cannot be replaced by the registry and is not: the file names runs this app
started on this machine, while the lock can be held by a lead somebody started by hand in a terminal,
whose actor appears nowhere here. What the registry does add, since smetana-0u7, is a **second ground
for breaking the lock beside the hour** — a holder this file shows dead is broken at once, because an
hour spent waiting on a process that does not exist buys nobody anything — and one field read for
that and for nothing else. **A batch's liveness is its own `group`, not its record's `writer`.** A
batch killed mid-merge under an app that is still up leaves a lock no one will ever release, and the
writer being alive says only that the app is; the `writer` stays the signal for a task claim, where
the question is whether the run still exists to finish what it took. Both readings are the skills' to
make and not this side's: the app never releases the lock — the one bd write the loop
makes is `park_claims` on the unanswered path, which parks a claim rather than freeing one — so the
lock is released by `smetana:running-tasks` Phase R or by nobody.

On the front end, `runs.js` is deliberately small — a file read with no worker behind it, freshness
from switching projects, from window focus, and from any of the project's sessions starting or
stopping work. It keeps the back end's `config` and `Run` objects **whole** rather than unpacking
them into flags, the same instinct `tracker.js` follows with statuses: a state this front end has not
heard of must not silently read as one it has. The runs ride as a set keyed by `token`, so a late
word about one run can never write over another. It is guarded against its own stale response exactly
as `git.js` and `terminals.js` are, and the `run:state` listener carries that guard in its other form
— an event is not a response to anything, so a batch ending just as somebody moves project would
otherwise post its run under the new project's name. `RunBar` draws one segment per run in the scope
bar, each stop button naming its own token, and keeps a stopped run there until the project changes
or a run of the same scope replaces it: the reason it stopped is what somebody came back to read, an
unknown reason is an ordinary outcome rather than a crash, and the endings differ by glyph as well as
by colour. The scope rule itself is `components/run/runScopes.js`, one of the `branchChoice.js`
family and shared with the worker's `admit` by vocabulary rather than by code.

That third freshness channel is `components/run/configFreshness.js`, another of that family, and the
only one that fires while somebody sits and watches a setup agent write `.smetana/project.toml` —
they never leave the window, so focus never returns and no project switch happens. `workingKey` is a
value over the set of the project's sessions that are still `starting` or `running`, and a `watch` on
it re-reads the file on **both** edges, so a session going idle, picking up again and then exiting
costs two reads rather than one — the frequency to weigh before touching this channel. The mark
clears on a read that came back `ok`, never on the optimism that a session ended. What it replaces
was a watcher created inside `startSetup` over a single session id, which tore itself down for good
on its first callback for another project or for a session already gone, so a window that never
switched project and never lost focus kept the "Not set up for runs" triangle over a configuration
that existed, and kept the board's play buttons hidden behind the same `configured` (smetana-0ag).
The width is the fix: a key over a set cannot be lost, and it is scoped to one project. That the key
is a **string** is what keeps the two wholesale reassignments of `terminalState.sessions` quiet — an
unchanged set of working sessions produces an unchanged key and no read at all.
