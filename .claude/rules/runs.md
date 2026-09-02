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
| `journal.rs` | every decision the loop made, stamped and written as it is made — the line builders are pure and carry the tests, `Journal` is the write-through |
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
A dependency counts as blocking only when it is bd's `blocks` kind. And `LastBatch` has an answer per
ending rather than "did it crash", because a batch stopped by a spent allowance moved the board no
more than a crashed one did — reading either as a stuck queue would end a run over nothing — while a
harness that keeps falling over needs a person and an exhausted allowance needs only time.

**A clean exit is not the same fact as a batch having happened** (smetana-0t4), and until this was
written the loop asked the dying batch nothing else. Claude Code exits with zero when it did the work
and when it died on its first request to the API, so a night where every session came back after
eight minutes with one `Request timed out` in it read as a run doing its job: `crashes` was reset,
`LastBatch::Completed` was recorded, and two tasks sat `in_progress` under a dead actor until
somebody cleared them by hand in the morning. Two rules answer it, and both are in `queue.rs` where a
test can reach them. `did_nothing` is the cheap signal — no account of itself and a board exactly
where the batch found it — and it feeds `LastBatch::Empty`, counted in a row against
`MAX_EMPTY_BATCHES` beside `MAX_CRASHES` and ending the run as `StopReason::NothingDone`. It is a
second threshold rather than a share of the crash count because an empty batch exits with zero and
never reaches that counter, and it is not `NoProgress`, which needs a *completed* batch and an
unmoved board twice running — a dying batch that got as far as claiming one task defeats it. "The
board moved" is stricter here than progress is in `next_action`, deliberately: this rule ends runs,
so the closed and parked counts count too.

`queue::release` is the other, and it runs behind **every session the run has finished with**
rather than only the batch that parks: `in_progress` goes back to `open` with the claim dropped,
`ready_to_merge` keeps its status and loses only the claim, and the merge lock goes back to `open`
and unclaimed where — and only where — the app has *proved* the batch's own process group gone.
Releasing it behind a batch that may still be alive would be releasing it in the middle of
somebody's merge, which is why every weaker answer refuses; that argument is about a *live* batch and
does not reach a dead one. Returned rather than parked, and that is the whole distinction: parking
is for a task carrying a question to a person, and this one carries none, so parking it would hide
it from every run that comes after. The note is an ordinary one for the same reason — a `parked:`
line puts an answer in the trail that nobody gave — and it names the batch, its actor and, where
`git::task_work` could find them, the branch the work was left on and the commit at its tip. That
last part is the one thing the app can say that nobody else will: work committed on a branch and
never merged is invisible to the board. The lock is the exception to that too and takes no note at
all; the paragraph below says why, and it is the reason the branch lookup is skipped for it.

**Two endings are not in that set whole, and each for its own reason.** A hand-back leaves the
session alive with a person in it — that is what the mode is for — so nothing of that batch is
released: the sentence that stops `release` giving back the lock of a batch it cannot show dead
applies to a task claim word for word, and in Solo the freed task would land back in `ready` and
send a second session out on the work somebody is at that moment still talking about. The unanswered
question is the other, and it is a split rather than a refusal: `park_claims` owns everything the
batch holds `in_progress`, since those are the claims the question is about and a release running
first would leave it nothing to park, while the **reviewed** half is released beside it —
`queue::claimed_by` is `in_progress` under that actor and nothing else, so reviewed work would
otherwise sit claimed under a session the run has just walked away from, which on the `Stop` arm
means until some later run's Phase R. That arm is also
the one place a release goes out behind a session still sitting at its prompt, and it is safe there
for a reason of its own rather than by the rule above: parking is writing to that very session's
claims a moment later and doing strictly more. The two sets cannot overlap, which is what makes the
order safe — `release` writes nothing but the assignee on a reviewed row, and parking re-reads a
fresh board filtered on `in_progress` — and `queue::is_reviewed` is the predicate, so
`ready_to_merge` stays a string that file spells once.

**The lock may be written in exactly one way, and it took two tickets to settle which**
(smetana-dgv, then smetana-rxzd). A batch that stops to ask while it is merging is holding the lock
`in_progress` under its own actor, which is precisely the shape the parking filter passed: the lock
was written `parked`, with the question as its note and the dead actor left on it. Only an `open`
issue is claimable, so a lock in any other status is claimable by nobody at all — not the next batch,
not the next run, not a lead somebody starts by hand — and the radius is every merge in the project
from then on rather than one task. `running-tasks` forbids a lead to park the lock in as many words;
the app was doing it in code. `queue::claimed_by` therefore filters it out of the parking set for
good, through the same `queue::is_lock` the snapshot and `left_behind` use.

`queue::release` is the one path that may move it, and only into `open`, unclaimed, and only behind a
batch the app has **proved dead**. That refusal used to be unconditional too, and the other half of
the same night was the cost: nothing but the lead's own last command ever released the lock, so a
lead killed mid-merge left it held for ever and the next run stood at a claim nobody was going to
give up. The proof is the evidence `registry::forget_run` already trusts — `registry::group_is_dead`,
which is `liveness(group, table(group.pid)) == Liveness::Dead` — and three things about it each cost
a night if they are loosened. **`Unknown` releases nothing**: the asymmetry `registry.rs` documents
between `sweep` and `forget_run` is the same instinct pointed at a third cost here, so
`!= Liveness::Dead` is not rewritten as `== Liveness::Alive` and the reverse is no more allowed.
**A batch with no recorded `group` releases nothing**, since `group_of` answers `None` both when the
session is gone and when the terminal worker could not be asked. And **only the lock this batch's own
actor holds** is touched, which `left_behind`'s filter on the assignee already guarantees — a lock
held by a lead somebody started by hand is reachable by nothing the app does. The question is asked
of the batch's recorded **group leader** and never of the record's `writer`, and it is asked at the
ending rather than reused from the start, where the lead is alive by construction. What it reads is
that one process — `group.pid` and the stamp beside it — and not the group, which on Unix outlives
its leader: a lead that exited while something it delegated is still merging answers gone. That is
the reach of the evidence, named rather than papered over, and the group-wide probe that would close
it (`killpg(pgid, 0)`) is a decision of its own for `procs.rs`. What keeps the ending that *kills* a
lead out of this is order rather than the reading: `remove_session` runs after the release, so on an
unanswered question the lead is still at its dialog and the answer is `Unproven`. The release writes
`status: open` and an empty assignee and **no note at all** — the lock's issue carries claims and
releases and nothing else, because any other write moves `updated_at` and makes a dead claim look
fresh to every lead waiting out `merging`'s staleness hour. The run's own document carries the
sentence instead: `report::LockRelease`, a line under the batch card naming the actor the lock was
taken from and the evidence it was taken on.

`usage.rs` is the piece the runs design deliberately left out and then took back. Reading
`claude -p "/usage"` is a parse of somebody else's prose that can break silently, which is why it was
refused; what did not survive contact was the trade, since a run that exhausts its allowance
overnight spends five sessions and a minute of backoff discovering it and then stops with `Crashed`,
which says the harness kept failing when nothing failed. So the parse is back with its failure mode
named rather than assumed: **an unreadable answer never blocks a run** — it reads as `Normal`, the
batch goes at full size, which is where things were before the module existed. The gate runs *before*
each batch, so the exhausted case costs no session at all. `service.rs` asks the same question again
after a session exits non-zero, and there it is not a gate but a classification: a spent limit told
apart from a harness that fell over.

**The bands are the person's now**, `settings.json`'s `subscription` section (`.claude/rules/settings.md`),
and `decide` takes them as an argument where it used to hold them as two constants.
`PAUSE_THRESHOLD` and `REDUCED_THRESHOLD` are what ships, not what applies. They are read **off the
disk at every gate check** rather than snapshotted when the run starts, which is deliberately the
opposite of what `drive` does with `agent` and `remove_worktrees` — and their argument does not carry
here. Changing the harness mid-run would make a run ask one subscription about an allowance and spend
another's; changing a threshold only moves the moment it waits. Somebody watching a paused run and
lowering the gate wants **that** run to go on, not to stop it and start it again, so the file is
asked inside the poll loop and a threshold moved overnight takes effect within ten minutes.

Either threshold may be **off**, and off means *do not pre-empt* rather than *do not notice*. That
distinction is the whole of why there are three functions here rather than one, and it is the easiest
thing in the module to break by simplifying. `decide` is the person's bands, used by the gate and by
`report`, which is where the sentence under the percentages on the Agents tab comes from — so what
that sentence says and what a run actually does cannot disagree. `spent` is a fixed rule at
`SPENT = 90`, a second constant rather than a reuse of `PAUSE_THRESHOLD` though it ships with the
same number: that one is a default somebody may move, this one is the app's own reading of "the
harness will refuse the next session", which is not theirs. **The classification after a non-zero
exit asks `spent` and nothing else** — had it followed the person's own threshold, turning that
threshold off would have made every exhausted allowance read as a crash and stopped the run as
`Crashed` after `MAX_CRASHES`, which is the exact failure this module exists to prevent, arriving
through the settings window. And `gate` is what the loop calls: `decide`, except that a run whose
previous batch died on a spent allowance (`LastBatch::Limited`) is held in `Paused` while `spent` is
true, whatever the thresholds say. Without that half, a run with the gate off would spend a session
finding the wall, be told `Limited`, come straight back, be told to go, and do it again for as long
as the queue lasted. `spent(None)` is false, like everything else here: an unreadable probe never
holds a run up.

**The sentence about the limit is one per footer, not one per run.** The subscription is one per
machine, so two runs paused seconds apart wrote the same sentence twice, differing only in the minute
the harness happened to name in each — "resets …" is the harness's own words and the two asked it at
different seconds. `components/run/limitVoice.js` picks which segment speaks: the first paused run in
`runsState.runs`, which is the oldest, so the words sit leftmost and do not move as later runs come
and go. The other paused segments keep the pause glyph and their own Stop button and say nothing —
dropping them altogether was refused, because that button belongs to that run and would go with it.
The rule is a pure module rather than a computed in `RunBar.vue` for this file's usual reason: a
`.vue` is unreachable by every test in the tree.

Beside that one sentence stands **"Run anyway"**, an `IconButton` with `play` — the direct pair of
the `pause` glyph at the head of the segment. Five things about it, each of which was the other way
round at some point:

- **It releases every run alive at that moment, not its own.** One reading of one subscription stood
  them all up, so letting them go one at a time would be work for its own sake. Hence one button, at
  the one sentence, and `run_release` takes no token where `run_stop` takes one.
- **The release lasts until each run ends.** A released run stops looking at `pauseAt` for the rest
  of its life. Lifting it for one batch was refused: the reading stays above the threshold for hours,
  and somebody would be pressing the button all evening.
- **Nothing is written to `settings.json`.** It is a flag on the run in the worker — a
  `watch::Sender<bool>` per `Active`, which both carries the value and wakes the run out of its
  ten-minute poll, since a release nobody notices for ten minutes reads as a press that did nothing.
  Making the button a shortcut to `pauseAt = 0` was refused: one press for one evening would have
  changed the policy for good, and silently.
- **A run started after the press pauses as usual.** The release belongs to the runs that were alive,
  which is exactly why it is a flag on each of them rather than state on the app: an app-wide flag
  would have no moment at which to be cleared.
- **It does not override a hold on a spent allowance.** `usage::held` is that distinction —
  `after_limited && spent` — and it rides out on `RunState::Paused { spent }`, because the two pauses
  are otherwise identical from the front end: both carry a percentage and a reset. Where it is true
  the button is not drawn at all, since pressing it would let a session through that dies the moment
  it starts, which is the churn the gate exists to prevent. `gate` reaches the hold whatever
  `pause_at` says, which is what makes the release structurally unable to override it — the released
  run has its `pause_at` moved to `OFF` and nothing else.

A released run says nothing special about itself: the bar goes back to the ordinary "Batch N". A
detail on the model of the reduced batch ("past the limit, 92% used") was refused — that the run is
going is the whole answer, and the line is spoken for.

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

**The app writes exactly four keys of that file and never any others** (smetana-2cfl). `[defaults]` —
`target_branch`, `min_priority`, `max_parallel_tasks`, `review_passes` — is edited from a dialog off
the project tile's right-click menu, and everything else stays the setup agent's. The split is by
what a person reaches for between runs: those four are scalars somebody turns while watching a board,
where the rest is a description of the machinery — commands, gate lists, paths, and the prose above —
that is discovered by looking at the folder rather than chosen in a form. "Set up again" is unchanged
and is still the answer to "this project grew a fourth repository"; it costs a session and takes no
instruction, which is the wrong price for "run three at a time, not five".

The write is `config::with_defaults`, **surgical through `toml_edit` and never a serde round trip**.
`toml::to_string` keeps no comment and returns the keys in the struct's order, so somebody who
changed one number would find the file reordered and a colleague's note gone — and `[merge].hazards`
is by design a paragraph a lead reads. Four rules hold it up, each with a test of its own:
`target_branch` with no value is the key **removed**, never `target_branch = ""`, because the field
is an `Option<String>` and `None` is what makes the run dialog fall back to the branch the project is
on; a missing `[defaults]` table is created rather than refused, since most files start without one;
the produced text is re-parsed by `config::parse` before it reaches the disk, because a save that
leaves behind a file the app then refuses to load turns a wrong number into a broken project; and the
write is atomic in `settings/file.rs`'s shape — a uniquely named temporary beside the target, then a
rename, since a rename within one directory is the only thing the filesystem promises.

Four details of that pass are worth knowing before touching it, because every one of them is silent
when got wrong.

A value is **mutated in place with its decor carried across**, never assigned as a whole `Item`: a
`Value`'s decor is the space before it and anything after it on the same line, so the obvious
`table["min_priority"] = value(n)` turns `min_priority = 1  # the floor here` into `min_priority = 3`.
The key's own decor survives that, which is why a comment on the line *above* is unharmed either way
and the loss is easy to miss. A key **already saying what it should is left alone** rather than
written again, because a replacement is rendered canonically — `Formatted::new` carries no `repr` —
so rewriting an unchanged key would turn `target_branch = 'staging'` into `"staging"` and
`max_parallel_tasks = 0x10` into `16` on a save that touched neither. The comparison behind that is
of the value and not of how it was written: `Value`'s own `PartialEq` is derived over the repr and
the decor too, and would answer "different" for exactly those two inputs. It is also what makes
`with_defaults` idempotent structurally rather than by luck.

**One comment genuinely does not survive**, and it is every comment belonging to a key that is being
removed: choosing no target branch takes the key out, and both the line above it (the key's prefix)
and anything after its value (the value's suffix) travel inside the `Item` that goes. That is the
right answer rather than a gap — the only other home for either line is whichever key follows, where
it would describe something it was never written about — and a test of that name pins it, so the
guarantee is read with its exception. Last, the section is reached through `as_table_like_mut`, so
the two other spellings TOML allows for it, `defaults = { … }` and the dotted
`defaults.min_priority = 1`, are edited in their own shape rather than answered with "defaults is not
a table", which reads as nonsense to whoever typed one.

The dialog is `'project-settings'` in `dialogRegistry.js`, a window of its own on `ground:
['project']` like the setup window and for its reason: the file belongs to the project, so a window
left standing over a project somebody clicked away from would save four numbers into the wrong
repository. `components/run/projectDefaults.js` is the pure half — the fall-backs, which are
`Defaults::default()`'s own (no branch, 2, 3, 5), the bounds, the branch option list, "has
anything changed" and the sentence that stands in for the fields when there is no file to fill them
from — and `ProjectSettingsModal.vue` draws it. The bounds are narrower than the `u8` the file holds and that is
their purpose: the type stops 300, the bound stops the typo that spawns two hundred agents overnight.
A stored branch the list no longer holds stays in the list as an option of its own, because opening a
settings screen must never be a way to change a value silently.

**This window writes two files, and the two halves save differently.** `[defaults]` is the
repository's and goes on an explicit Save; above it is the caveman level this machine talks to agents
in while this project is open, which is `project.caveman` in `settings.json`
(`.claude/rules/settings.md`) and is written the moment it is picked. Neither half is a mistake to
tidy into the other. A file that is committed and travels to everybody in the repository is one where
a keystroke has to be a decision; a preference on one machine is not, and this one has a mechanical
reason besides — **Save is offered only over a parsed file, and this window now opens without one**,
so a level behind that button would be a control nobody could reach in the very states it was moved
here to serve. The row says both things in its own description, and the ghost button reads Close
rather than Cancel where there are no fields, since the only thing on screen has already saved
itself. The level rides back as a `caveman` result of its own rather than as a second shape of
`save` — `EMITS` in `views/DialogWindow.vue`, then `applyPatch` in the app window — because one
handler working out which file it is being asked to write is how the two get confused.

The menu item is **live on the active project whatever state its file is in**, and refuses on one
fact only: another project's row, under `projectMenu.js`'s existing "Switch to this project first".
Two more captions stood there — "Set this project up first" and "This project's configuration will
not parse" — and both went with the caveman row's arrival, because a project with no file, or with a
damaged one, still has a level to set and used to be able to set it on the settings window's Agents
tab. The reason a form cannot help with a damaged file has not changed, so it is said **inside the
window** instead, where there is room for a sentence: no fields, no Save, and one line naming which
of the two states it is (`configNotice`). Greying each field in turn was the other answer and is more
code for the same meaning, with four dead controls saying nothing about why.

The `invoke` is `stores/runs.js`'s `saveDefaults` and not the view's, since the stores are the only
files in `src/` that know Tauri exists, and it **re-reads through `loadConfig`** on success: without
that the run dialog would go on offering the old defaults and the menu would keep its old
`configured` flag, which is the app having written a value it does not itself believe. The branch
list is `stores/git.js`'s `loadBranches`, the run dialog's own source, rather than a second read of
the same thing. `mockBackend.js` needs nothing: a command it does not handle falls through to its
loud refusal, which is what every write but `settings_save` already gets, and a browser has no
project to write to.

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
`ready_to_merge` under that actor, with ids. Drawn only for a batch that left no account — a lead
that answered has already said where it left things, and the line would say it again. The read
itself is every batch's now, since `release_claims` and `did_nothing` want it too; what is
conditional is the line in the document. It is wider than `claimed_by` on purpose, since this is a
record rather than a parking list, and the lock is one of the two directions it is wider in. The
**lock** is named whatever is then done about it, and the line exists precisely because the
alternative is the *next* run discovering the lock by failing to take it; it says what the board
held at the moment the batch ended, and the release that usually follows is the next sentence rather
than a reason to stop saying it. That reading is acted on, by `release_claims` off the same
`fresh_board` call, and the recovery boundary is untouched by that: it is about what a *previous*
app left, which Phase R clears with the worktrees in front of it, while here the run is holding the
actor, the session and the moment the batch ended, and nobody else will ever know as much. So the
loop makes two kinds of bd write and no others — `park_claims` on the unanswered path, one batch's
`in_progress` claims to `parked` with the question as the note and the merge lock never among them,
and `release_claims` behind every session the run has finished with, the reviewed half of that same
unanswered batch included and the lock among them only where the app has proved that batch's process
group gone. That last set is not "every session that is gone": on the `Stop` arm the session
is left alive at its prompt, and the release is safe there not because nothing is running but
because parking is already writing to that very session's claims and doing strictly more — which is
also why the lock cannot travel on that arm, its group being alive by construction.

A release is **named in the document too**, and it has to be: nothing is written to the lock's own
issue, so `report::LockRelease` is the only record there will ever be that the app took a claim off
somebody. The line sits under the batch card, in `.outcome`'s secondary text beside the `.held`
reading above it, and carries the two things a person doubting the release would ask for — the actor
it was taken from, and the evidence it was taken on.

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

## The journal: what the loop decided, while it was deciding it

The report above is about the **work**, and there was nothing at all about the
**mechanics** (smetana-7di). `.smetana/runs.json` is a registry of the live and
is rewritten in place, so it holds no history by definition; the report names
batch durations and the agent's own prose and never a batch's exit code, the
reason another batch followed, or the reason the run stopped. The night of 29
August is the measurement: six batches in two hours, four of which did nothing,
and a day later it could not be settled off the disk whether those four were
counted as `LastBatch::Completed` or as `Crashed`, nor which `StopReason` ended
the run — both readings fit everything that survived. `journal.rs` closes
exactly that question, and its list of lines is **closed at nine**: the run's
own settings, every preflight command and health check with its outcome, every
board read with the ids in it, every answer from the spend gate with the
percentages behind it, every `next_action` with the `LastBatch` it came out of,
every batch's start and ending, the two counters after each batch, and the
ending with the document it was written into.

Closed means whole, in both directions. A run makes **four** board reads and all
four are on the record, each marked and each written down when it fails as well
as when it answers: the one a decision is made from, the resync that settles an
empty queue, the one after a batch, and the run's last. The post-batch read is
the load-bearing one — `queue::did_nothing` turns it into `LastBatch::Empty` or
`LastBatch::Completed`, which is exactly the discrimination 29 August could not
make, and a read that *failed* falls to the arm counting the batch as completed.
The final read is the other that matters: its failure is why `RunSummary::tasks`
is an `Option`, and the line is what says which of the two the document's dashes
came from. A record whose gaps are invisible is worse than a shorter one that is
honest about its scope.

**Two destinations, one text.** Every line goes to the app log with a `runs:`
prefix, so somebody who has only that file open sees the whole of a run, and to
`.smetana/runs/<token>/journal-<start time>.log`. Neither alone does it: the app
log splices two nights and every other subsystem together and gives the report
nothing to name, while a file alone is invisible to anybody debugging the app as
a whole. `Journal::say` is the single call site for both, which is what stops
them becoming two texts that disagree. The name carries the run's **start** time
because the directory does not carry the run — `token` counts from zero on every
app start, so `.smetana/runs/1/` is reused by a run two launches later.

**It is a write-through and not a buffer**: opened at the run's start, one line
written and flushed per event. The run this exists for is the one that died, and
a journal assembled at the end is empty in precisely the case somebody goes
looking for it. **A journal that cannot be opened never stops a run** — it keeps
the `log::info!` half and no call site learns the difference, the same choice
`lib.rs` makes about the app log itself. Nothing cleans these files up, and that
is said out loud rather than left for somebody to look for.

Three things follow in other files. `Batch` and `Probe` in `service.rs` are
`pub(super)` for this one reader, because the acceptance is that an ending is
named **literally** — `Code(0)` told apart from `NoCode` and from `Removed` — and
every prose rendering of those three loses one of the distinctions; `report.rs`
still says the same endings in sentences, for a different reader. `ask` hands
back the `Usage` reading beside the `Decision`, since `Normal` is the answer both
to a fresh week and to a probe nobody could read. And the report's footer carries
the journal's absolute path, as plain text and never a link — this document
reaches nowhere, which is what makes it safe in a sandboxed frame. Nothing about
what the loop *decides* changed: the journal only writes down what was already
happening.

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
worktrees in front of it. So the app writes to bd nowhere as part of *recovery* — doing that would be
a second mechanism doing Phase R's job, and two mechanisms on one fact drift. What its own live run
left claimed a second ago is not recovery and is the run's own to give back (`release_claims`,
smetana-0t4); a previous app's leavings are Phase R's, whole. The registry is `.smetana/runs.json` in
the project folder, beside `project.toml` and outside the repository, so Phase R reads it with an
ordinary file read, needing
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

A batch's `group.command` is **waited for rather than read once**, and that is smetana-6nr0 rather
than caution. The terminal worker answers with the pid the moment the fork is done, and between the
fork and the exec the kernel names the child after the process that forked it — so every
`group.command` this app had ever written said `app`, the app's own name, against a pid `ps` showed
as `claude` with a start time matching to the microsecond. That is the inversion the hazard entry
describes as hypothetical: the lock rule compares the recorded name with what stands under the pid
now and reads a difference as a reused pid, so a live batch read as dead and Phase R would break the
lock off a lead mid-merge. `procs::spawned` therefore refuses any name it cannot tell from this
process's own and `recovery::group` asks again for up to a second — the window measured 160–675 µs —
and **writes nothing at all if the name never becomes the process's own**. That direction is the
whole point: both skills read a missing `group` as "leave the lock alone" and a wrong one as "break
it", so an absence costs a recovery and a lie costs two runs on one branch.

At start-up the run worker sweeps every project the settings file lists as open, before it serves its
first request — one writer, so the read-modify-write is safe, and no batch can go out beside a sweep
about to hang up a leftover agent in the same worktree. For a record whose writer is provably dead it
signals the recorded process groups exactly as a clean exit does. **Anything the registry does not
name is never touched** — the app cannot show it started it — and neither is a group whose pid has
since been reused, nor anything under a writer that is alive or unreadable. The sweep is silent: the
app is finishing its own interrupted shutdown rather than taking a new decision, and a modal about
housekeeping after every rebuild would be the loudness budget spent on the opposite of a card needing
a human. What was killed goes to the log, and the log is a file:
`~/Library/Logs/com.invisor.smetana/smetana.log` on macOS, and on Linux
`$XDG_DATA_HOME/com.invisor.smetana/logs/smetana.log` — which on most desktops means
`~/.local/share/com.invisor.smetana/logs/smetana.log`, because that variable is usually unset and the
spec's own default is what fills in. It is written by every build since smetana-2tf and not only a
debug one, and it rolls at 2 MiB and eleven files, so what a night wrote is still there in the
morning; `lib.rs` holds that arithmetic, and the stamps in it are UTC where a run report's are local.

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
under a person's own name rather than a `smetana-run-<n>`, which this file was never going to carry —
so for any name but that one shape an absence from it says nothing. What the registry does add, since
smetana-0u7 and smetana-fa4u, is **two further grounds for breaking the lock beside the hour**. A
holder this file shows dead is broken at once, because an hour spent waiting on a process that does
not exist buys nobody anything, and that one is a single field read for it and for nothing else. A
holder named `smetana-run-<n>` that a file read whole, parsed and of `version` 1 names in no record's
`batches[].actor` is broken at once as well, and that one reads no field of a record at all but the
absence of every record naming that actor. Which puts `forget_run`'s condition under a reader it was
not written for: a record goes only when nothing it names is still running, so its absence is what a
lead outside this app reads a dead holder by, and loosening it without moving `smetana:merging` and
`smetana:running-tasks` with it would have a lead break the lock of a run that is still merging.
**A batch's liveness is its own `group`, not its record's `writer`.** A
batch killed mid-merge under an app that is still up leaves a lock its holder will never come back
for, and the writer being alive says only that the app is; the `writer` stays the signal for a task
claim, where the question is whether the run still exists to finish what it took. The same reading
decides the app's own release (smetana-rxzd), which is not a coincidence but the one fact stated
twice for two readers. Both readings here are the skills' to make and not this side's: the registry
is a file Phase R and `merging` read, and nothing in it writes to bd. The app's own two bd writes are
`park_claims` on the unanswered path, which refuses the lock outright through `queue::claimed_by`, and
`release_claims` behind every session the run has finished with, where `queue::release` gives the lock
back to `open` on exactly the reading in bold above — the batch's own `group`, read as `Liveness::Dead`
(smetana-rxzd). So a lock left behind by this app's own killed batch is now usually released by the
app, and everything else — a lead somebody started by hand, a batch whose group cannot be read — is
still `smetana:running-tasks` Phase R's or a person's. Both halves were wrong once and each cost its
own night: until smetana-dgv the parking did not refuse the lock, so a batch killed at a question
mid-merge left it `parked`, which is worse than released — an unclaimable lock is every later merge
in the project waiting on a person — and until smetana-rxzd nothing released it at all, so a batch
killed mid-merge left it held for ever and the next run stood at a dead claim.

On the front end, `runs.js` is deliberately small — a file read with no worker behind it, freshness
from switching projects, from window focus, and from any of the project's sessions starting or
stopping work. It keeps the back end's `config` and `Run` objects **whole** rather than unpacking
them into flags, the same instinct `tracker.js` follows with statuses: a state this front end has not
heard of must not silently read as one it has. The runs ride as a set keyed by `token`, so a late
word about one run can never write over another. It is guarded against its own stale response exactly
as `git.js` and `terminals.js` are, and the `run:state` listener carries that guard in its other form
— an event is not a response to anything, so a batch ending just as somebody moves project would
otherwise post its run under the new project's name. `RunBar` draws one segment per run in the status
footer, each stop button naming its own token, and keeps a stopped run there until the project changes
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
