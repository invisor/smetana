//! The run worker: one tokio task holding the run, the same shape
//! `tracker/service.rs` and `terminal/service.rs` have and for the same
//! reason. Nothing shares mutable state with it.
//!
//! What it does is a loop somebody else's process does the work in: read the
//! board, decide, start one session for one batch, wait for that session to
//! end, read the board again. The deciding is `queue.rs` and is tested; this
//! file is mostly the part that talks to the other two workers, and that part
//! carries no tests the same as the other workers' do not.
//!
//! The exception is the map at the bottom of this file, and it is an exception
//! because it stopped being plumbing and became a decision: which project is
//! taken, for how long, and whether another batch may go out (smetana-0kb).
//! `absorb`, `permit` and `admit` are that decision with nothing of Tauri in
//! them, and they are tested — the same extraction `tracker/store.rs` and
//! `Session::apply` in `terminal/model.rs` already are, and for the same
//! reason: both ways of getting it wrong are silent. An entry that leaves too
//! early lets a second run start beside a live loop, and one that never leaves
//! makes the project unstartable until the app is restarted.
//!
//! One run per project, keyed by the project's path — a second run in the same
//! project is refused rather than queued, and a run in another project is none
//! of this one's business. Different projects are different folders, stacks,
//! boards and target branches; the only thing they share is a subscription
//! limit, and a run does not reserve one (smetana-tra).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::config::{self, ConfigState};
use super::model::{Asked, Run, RunError, RunSettings, RunState, StopReason};
use super::preflight;
use super::queue::{self, Action, LastBatch, QueueSnapshot};
use super::usage::{self, Decision};
use crate::agents::{Intent, Profile};
use crate::terminal::model::{Exit, SessionState};
use crate::terminal::service::{Request as TerminalRequest, TerminalHandle};
use crate::tracker::service::{Request as TrackerRequest, TrackerHandle};

/// The backstop against a board that churns without finishing anything. It is
/// not a budget: a healthy run ends on an empty queue long before this.
const MAX_ITERATIONS: u32 = 40;
/// How many times in a row a session may exit non-zero before the run gives
/// up. A transient failure of the harness is common; five of them is not.
const MAX_CRASHES: u32 = 5;
const CRASH_BACKOFF_BASE: Duration = Duration::from_secs(5);
const CRASH_BACKOFF_MAX: Duration = Duration::from_secs(60);
/// How often the wait on a batch asks what the session's own state is. Only an
/// unattended run asks at all — see `watch_batch`.
///
/// Slow on purpose: what it is looking for stands until somebody answers it, so
/// nothing is lost by taking two of these to see it, and every tick costs the
/// terminal worker a clone of the project's session list. A dialog therefore
/// ends a run about ten seconds after it is drawn, against the for ever it used
/// to take.
const ASK_POLL: Duration = Duration::from_secs(5);

pub enum Request {
    Start(String, Box<RunSettings>, oneshot::Sender<Result<Run, RunError>>),
    Stop(String, oneshot::Sender<Option<Run>>),
    State(String, oneshot::Sender<Option<Run>>),
}

/// What a loop task says to the worker. Three messages on one channel so they
/// arrive in the order the loop sent them, and none of them is the loop writing
/// state anywhere — the worker is still the only thing that owns the map.
enum Report {
    /// Where the loop has got to.
    State { token: u64, run: Box<Run> },
    /// May another batch go out? The worker's answer *is* the decision — see
    /// `may_spawn`.
    Spawning { token: u64, project: String, allow: oneshot::Sender<bool> },
    /// The loop task is gone, however it went.
    Ended { token: u64, project: String },
}

#[derive(Clone)]
pub struct RunHandle(pub mpsc::Sender<Request>);

/// The worker's own view of one project's run in flight. `Run` is what leaves
/// the worker; this is what stays.
///
/// An entry is in the map for exactly as long as a loop task is alive in that
/// project — no longer, and **no shorter**. A run declared stopped keeps its
/// entry until the loop reports its own ending, because a stopped run whose
/// loop is still winding down is still a loop that would spawn a batch, and
/// letting a second run start beside it is how the strictly sequential merge
/// this limit exists for comes apart (smetana-0kb).
struct Active {
    /// Which loop task this entry belongs to. The map is keyed by project and a
    /// project's next run may start the moment its last one is out of the map,
    /// so a late report has to be told from the run that replaced it — the same
    /// job `generation` does for the tracker, and the same defect if it is
    /// missing: a finished run's state written over a live one's.
    token: u64,
    run: Run,
    /// A batch has been authorized and the loop has not reported it yet: the
    /// window in which `run.session` is still `None` and a batch is on its way
    /// regardless. A stop landing in it has to read as one landing mid-batch,
    /// or it declares the run over while the batch goes out behind it.
    starting: bool,
    /// Cancels the loop task. Dropping it is what a stop after the final batch
    /// comes down to.
    stop: mpsc::Sender<()>,
}

/// Sends `Report::Ended` when the loop task ends, whichever way it ends. That
/// is what makes "there is an entry in the map" and "a loop task is alive" the
/// same fact rather than two that agree most of the time; the map's own comment
/// leans on it.
///
/// A guard rather than a send at the bottom of `drive`, for two reasons and not
/// one. The everyday reason is that `drive` leaves from a dozen places — every
/// `return` in the loop, and more arrive with every stop condition anyone adds
/// — and a guard covers all of them without any of them having to remember. The
/// rarer reason is a panic unwinding through the task, which no send at the
/// bottom would survive, and whose cost is a project nobody can run in again
/// until the app is restarted.
struct Ending {
    token: u64,
    project: String,
    report: mpsc::UnboundedSender<Report>,
}

impl Drop for Ending {
    fn drop(&mut self) {
        let project = std::mem::take(&mut self.project);
        let _ = self.report.send(Report::Ended { token: self.token, project });
    }
}

pub fn start(app: AppHandle, tracker: TrackerHandle, terminal: TerminalHandle) -> RunHandle {
    let (tx, mut rx) = mpsc::channel::<Request>(8);
    let (report_tx, mut report_rx) = mpsc::unbounded_channel::<Report>();

    tauri::async_runtime::spawn(async move {
        // Keyed by the project's path: that key is what makes a run in one
        // project invisible to another, and it is the same path the tracker,
        // the settings and the front end name a project by.
        let mut active: HashMap<String, Active> = HashMap::new();
        let mut next_token: u64 = 1;

        loop {
            tokio::select! {
                request = rx.recv() => {
                    let Some(request) = request else { break };
                    handle(&app, &mut active, &mut next_token, &tracker, &terminal, &report_tx, request);
                }
                // The loop task's own progress, its one question, and its
                // ending. It owns no state the front end reads — it hands a
                // whole `Run` back here, and this task is the only thing that
                // writes one out.
                report = report_rx.recv() => {
                    let Some(report) = report else { break };
                    handle_report(&app, &mut active, report);
                }
            }
        }
    });

    RunHandle(tx)
}

fn emit(app: &AppHandle, run: &Run) {
    let _ = app.emit("run:state", run);
}

fn handle(
    app: &AppHandle,
    active: &mut HashMap<String, Active>,
    next_token: &mut u64,
    tracker: &TrackerHandle,
    terminal: &TerminalHandle,
    report: &mpsc::UnboundedSender<Report>,
    request: Request,
) {
    match request {
        Request::State(project, tx) => {
            let _ = tx.send(current(active, &project));
        }
        Request::Stop(project, tx) => {
            let mut answer = None;
            if let Some(current) = active.get_mut(&project) {
                // `starting` is the fact `run.session` cannot carry: a batch
                // authorized moments ago and not yet reported. Without it a
                // stop in that window reads as a run with nothing in flight
                // and ends it on the spot, while the batch it did not know
                // about runs to completion and merges (smetana-0kb).
                current.run.request_stop(current.starting);
                // A closed channel is the signal; the loop reads it between
                // batches, which is what makes stopping cooperative. It is
                // never killed mid-batch: a run interrupted between a merge
                // and a close is exactly the state the recovery phase
                // exists to clean up, and doing that deliberately is not a
                // feature.
                let _ = current.stop.try_send(());
                answer = Some(current.run.clone());
            }
            if let Some(run) = &answer {
                emit(app, run);
            }
            // The entry stays whether or not the run is over. It leaves in one
            // place only — `Report::Ended`, when the loop task is actually gone.
            let _ = tx.send(answer);
        }
        Request::Start(project, settings, tx) => {
            // This project's own run and nothing else. Another project's is not
            // in the way of anything: it has its own board, its own worktrees
            // and its own target branch.
            if let Err(err) = admit(active, &project) {
                let _ = tx.send(Err(err));
                return;
            }
            let settings = *settings;
            if let Err(err) = settings.validate() {
                let _ = tx.send(Err(err));
                return;
            }
            let config = match config::load(Path::new(&project)) {
                ConfigState::Missing => {
                    let _ = tx.send(Err(RunError::NotConfigured));
                    return;
                }
                ConfigState::Broken { message } => {
                    // The first and only place a damaged config is ever shown
                    // to anybody. Everything else in the app treats it as "no
                    // configuration", which is right for a marker on a row and
                    // wrong here: a run against missing gates produces green
                    // merges that proved nothing.
                    let _ = tx.send(Err(RunError::BrokenConfig(message)));
                    return;
                }
                ConfigState::Ok { config } => *config,
            };

            let run = Run::new(project.clone(), settings);
            let (stop_tx, stop_rx) = mpsc::channel::<()>(1);
            let token = *next_token;
            *next_token += 1;
            active.insert(
                project.clone(),
                Active { token, run: run.clone(), starting: false, stop: stop_tx },
            );

            let ending = Ending { token, project: project.clone(), report: report.clone() };
            let driving = drive(
                token,
                run.clone(),
                config.preflight.clone(),
                PathBuf::from(&project),
                tracker.clone(),
                terminal.clone(),
                report.clone(),
                stop_rx,
            );
            tauri::async_runtime::spawn(async move {
                // Bound rather than dropped: it has to outlive the loop, since
                // its whole job is to fire when the loop is over.
                let _ending = ending;
                driving.await;
            });

            emit(app, &run);
            let _ = tx.send(Ok(run));
        }
    }
}

fn handle_report(app: &AppHandle, active: &mut HashMap<String, Active>, report: Report) {
    if let Some(run) = absorb(active, report) {
        emit(app, &run);
    }
}

/// Everything a report does to the map, with none of Tauri in it: the answer is
/// what to put on the wire, if anything.
///
/// Split out from `handle_report` because the map's lifecycle became a decision
/// with this change, and the file's own rule is that a decision belongs
/// somewhere a test can reach — the same move `tracker/store.rs` and
/// `Session::apply` already are. The two ways to get it wrong are silent and
/// they are not symmetrical: an entry that leaves too early lets a second run
/// start beside a live loop, and one that never leaves makes the project
/// unstartable until the app is restarted. Neither shows up anywhere else in
/// the tree.
fn absorb(active: &mut HashMap<String, Active>, report: Report) -> Option<Run> {
    match report {
        Report::State { token, run } => {
            let run = *run;
            // Nothing under that key any more, or something newer under it.
            // Either way this report is the past, and emitting it would put a
            // finished run back on the screen.
            let current = active.get_mut(&run.project)?;
            if current.token != token {
                return None;
            }
            // The loop has said where it is and `run.session` carries that now,
            // so the stand-in for it has done its job. Cleared on every report,
            // which is what stops a run that has finished a batch from looking
            // busy to the next stop.
            current.starting = false;
            // A run the worker has already declared stopped is not revived by a
            // batch that was on its way out — the rule `Run::advance` keeps one
            // level down, needed here because the loop's own copy knows nothing
            // of that stop until it next looks at the channel.
            if current.run.is_over() {
                return None;
            }
            // Adopted rather than assigned: `stopping` is this side's field and
            // the loop's copy always says false — see `Run::adopt`.
            current.run.adopt(run);
            Some(current.run.clone())
        }
        Report::Spawning { token, project, allow } => {
            let _ = allow.send(permit(active, token, &project));
            None
        }
        Report::Ended { token, project } => {
            // The one place an entry leaves the map.
            if active.get(&project).is_some_and(|a| a.token == token) {
                active.remove(&project);
            }
            None
        }
    }
}

/// May this loop start another batch, and if so, remember that it is starting.
///
/// The authorization, and the whole of the fix's first half. It runs in the
/// worker's own task, which is also where `Request::Stop` is handled, so the
/// two cannot interleave however the `select!` happens to pick between its arms
/// — both orderings are safe and each has its own outcome. Either the stop was
/// handled first and the answer here is no, or this was and the stop that
/// follows finds a batch in flight and waits for it, which is what stopping has
/// always meant.
///
/// What counts as "the stop got here first" is `may_start_batch`, and it is
/// wider than "already over" for a reason recorded there.
fn permit(active: &mut HashMap<String, Active>, token: u64, project: &str) -> bool {
    match active.get_mut(project) {
        Some(current) if current.token == token && current.run.may_start_batch() => {
            current.starting = true;
            true
        }
        _ => false,
    }
}

/// Whether this project can take a new run. Presence in the map is the test,
/// not the state of the run in it — an entry is there for exactly as long as a
/// loop task is alive — but the two cases are told apart in the answer, because
/// they are different things to be told. A live run is a reason to leave it
/// alone; a run that stopped a second ago and is still winding down is a reason
/// to try again shortly, and calling that one "already going" contradicts the
/// bar, which says stopped at the same moment.
fn admit(active: &HashMap<String, Active>, project: &str) -> Result<(), RunError> {
    match active.get(project) {
        None => Ok(()),
        Some(entry) if entry.run.is_over() => Err(RunError::WindingDown),
        Some(_) => Err(RunError::AlreadyRunning),
    }
}

fn current(active: &HashMap<String, Active>, project: &str) -> Option<Run> {
    active.get(project).map(|a| a.run.clone())
}

/// The loop itself, on a task of its own so the worker above stays answerable
/// while a batch runs for an hour.
#[allow(clippy::too_many_arguments)]
async fn drive(
    token: u64,
    mut run: Run,
    preflight_config: Option<config::Preflight>,
    root: PathBuf,
    tracker: TrackerHandle,
    terminal: TerminalHandle,
    report: mpsc::UnboundedSender<Report>,
    mut stop: mpsc::Receiver<()>,
) {
    let say = |run: &Run| {
        let _ = report.send(Report::State { token, run: Box::new(run.clone()) });
    };

    if let Some(config) = preflight_config {
        match bring_up(&root, &config, &mut stop).await {
            BringUp::Ready => {}
            BringUp::Cancelled => {
                run.advance(RunState::Stopped { reason: StopReason::Cancelled });
                say(&run);
                return;
            }
            BringUp::Failed(detail) => {
                run.advance(RunState::Stopped { reason: StopReason::Preflight { detail } });
                say(&run);
                return;
            }
        }
    }

    // Which agent's allowance to ask about. Resolved once, the same way
    // `terminal/service.rs` resolves the one it spawns, so the run asks the
    // harness that will actually run rather than the one settings named.
    // Nothing installed is not an error here — the gate simply cannot ask, and
    // `spawn_batch` is where that failure belongs and is already reported.
    let profile = crate::agents::pick(&agent_id(), crate::shell_env::path());

    let mut previous: Option<QueueSnapshot> = None;
    let mut crashes: u32 = 0;
    let mut unreadable: u32 = 0;
    let mut last_batch = LastBatch::Completed;

    for iteration in 0.. {
        if stop.try_recv().is_ok() || run.stopping {
            run.advance(RunState::Stopped { reason: StopReason::Cancelled });
            say(&run);
            return;
        }

        run.advance(RunState::Deciding);
        run.session = None;
        say(&run);

        let Some(issues) = board(&tracker).await else {
            unreadable += 1;
            // Once is a slow bd call or a watcher restart; twice running is a
            // tracker that is not there, and a run that cannot read the board
            // is deciding from nothing.
            if unreadable >= 2 {
                run.advance(RunState::Stopped { reason: StopReason::Unreadable });
                say(&run);
                return;
            }
            continue;
        };
        unreadable = 0;

        let now = queue::snapshot(&issues, &run.settings.scope, run.settings.min_priority);
        match queue::next_action(&now, previous.as_ref(), iteration, MAX_ITERATIONS, last_batch) {
            Action::Stop(reason) => {
                run.advance(RunState::Stopped { reason });
                say(&run);
                return;
            }
            Action::Run(_) => {}
        }
        previous = Some(now);

        // Before spending the allowance, find out what is left of it. This is
        // the whole reason the gate is worth having: an exhausted limit costs
        // no session at all, where discovering it by failing costs one every
        // time round.
        let Some(tasks) = headroom(&mut run, &say, profile, &mut stop).await else {
            return;
        };

        // Asked rather than checked, and that difference is the fix. Reading
        // the stop channel once more here would narrow the window and leave it
        // open: the stop and the spawn are two events in two tasks, and nothing
        // orders the answer against the microseconds that follow it. The worker
        // can order them, because it is the single task that handles both.
        if !may_spawn(&report, token, &run.project).await {
            run.advance(RunState::Stopped { reason: StopReason::Cancelled });
            say(&run);
            return;
        }

        let session = match spawn_batch(&terminal, &run, tasks).await {
            Ok(id) => id,
            Err(err) => {
                run.advance(RunState::Stopped { reason: StopReason::Preflight { detail: err } });
                say(&run);
                return;
            }
        };

        run.session = Some(session);
        run.batches += 1;
        run.advance(RunState::Working { iteration });
        say(&run);

        let exit = match watch_batch(&terminal, &run, session).await {
            Batch::Ended(exit) => exit,
            // The batch has stopped to ask, and this run has nobody in it to
            // answer. Waiting on the process would be waiting for ever, so the
            // run ends here and says what it is stuck on. The session is left
            // alive and still at its prompt: a person who comes back can answer
            // it in the terminal, which is exactly what the bar tells them.
            Batch::Unanswered { question } => {
                run.advance(RunState::Stopped { reason: StopReason::NeedsAnswer { question } });
                say(&run);
                return;
            }
        };
        // A session somebody removed from the agents panel is not a harness
        // that fell over: nothing is going to go better on the next try, and
        // retrying it would answer "take this away" with another one just like
        // it. So the run ends here, with its own reason — the crash backstop
        // below is for processes that failed on their own.
        if exit == Exit::Removed {
            run.advance(RunState::Stopped { reason: StopReason::SessionRemoved });
            say(&run);
            return;
        }
        // `NoCode` is a session that was signalled, which did not finish the
        // batch either — the same reading `terminal/service.rs` records beside
        // the waiter.
        if exit == Exit::Code(0) {
            crashes = 0;
            last_batch = LastBatch::Completed;
            continue;
        }

        // An allowance that ran out mid-batch and a harness that fell over are
        // the same absence to anyone reading an exit code, and they need
        // opposite responses: one is retried, the other is waited out. So the
        // gate's own question is asked a second time, here as a classification
        // rather than as a gate — the source of the answer is the same one, and
        // there is no second mechanism to keep in step with the first.
        if matches!(ask(profile).await, Decision::Pause { .. }) {
            // Not a crash: the counter is untouched, and `Limited` is what
            // keeps the next round from reading an unmoved board as stuck.
            // Nothing pauses here — the gate at the top of the loop is where
            // waiting lives, and it is about to ask again anyway.
            last_batch = LastBatch::Limited;
            continue;
        }

        crashes += 1;
        last_batch = LastBatch::Crashed;
        if crashes >= MAX_CRASHES {
            run.advance(RunState::Stopped { reason: StopReason::Crashed { attempts: crashes } });
            say(&run);
            return;
        }
        let backoff = CRASH_BACKOFF_MAX.min(CRASH_BACKOFF_BASE * 2u32.pow(crashes - 1));
        // Interruptible: two minutes of backoff is long enough that a
        // person who pressed stop would otherwise think it did nothing.
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = stop.recv() => {
                run.advance(RunState::Stopped { reason: StopReason::Cancelled });
                say(&run);
                return;
            }
        }
    }
}

/// What the harness says is left of the allowance. Blocking work goes to a
/// thread, the same as the preflight's commands: waiting on somebody else's CLI
/// would otherwise hold the whole async runtime.
async fn ask(profile: Option<&'static dyn Profile>) -> Decision {
    let Some(profile) = profile else { return Decision::Normal };
    let read = tokio::task::spawn_blocking(move || usage::read(profile)).await.unwrap_or(None);
    usage::decide(read.as_ref())
}

/// Wait until there is allowance enough to run a batch, and answer with how
/// many tasks that batch may take.
///
/// `None` means the run ended while it waited — the caller returns.
///
/// The wait is a state rather than a sleep, which is what puts it in the scope
/// bar and what lets the stop button reach it: a run with no session in flight
/// stops the moment it is asked, and a paused one has none. The poll is
/// interruptible for the same reason the crash backoff is, only more so — ten
/// minutes of silence after pressing stop would read as the button having done
/// nothing at all.
async fn headroom(
    run: &mut Run,
    say: &impl Fn(&Run),
    profile: Option<&'static dyn Profile>,
    stop: &mut mpsc::Receiver<()>,
) -> Option<Option<u8>> {
    loop {
        // The channel and not `run.stopping`: this task holds its own `Run`,
        // and `Request::Stop` sets that flag on the worker's copy. Nothing
        // carries it back here, so a check on the field would look like a guard
        // and be one only by accident.
        if stop.try_recv().is_ok() {
            run.advance(RunState::Stopped { reason: StopReason::Cancelled });
            say(run);
            return None;
        }
        match ask(profile).await {
            Decision::Pause { pct, resets } => {
                run.advance(RunState::Paused { pct, resets });
                say(run);
                tokio::select! {
                    _ = tokio::time::sleep(usage::POLL) => {}
                    _ = stop.recv() => {
                        run.advance(RunState::Stopped { reason: StopReason::Cancelled });
                        say(run);
                        return None;
                    }
                }
            }
            decision => {
                // Cleared as well as set: an allowance that came back up must
                // not leave the bar claiming a reduction that is over.
                run.reduced = match decision {
                    Decision::Reduced { pct } => Some(pct),
                    _ => None,
                };
                return Some(usage::cap(run.settings.max_parallel_tasks, &decision));
            }
        }
    }
}

/// How the preflight ended.
///
/// `Cancelled` is kept apart from `Failed` because they are opposite things to
/// read in the bar: one is somebody's own stop, the other is a project that
/// would not come up. Folding the first into the second would accuse the
/// project of a failure the person caused on purpose.
enum BringUp {
    Ready,
    Cancelled,
    Failed(String),
}

/// The declared commands, then the declared health checks. Blocking work goes
/// to a thread: spawning `docker compose` and waiting on it would otherwise
/// hold the whole async runtime.
///
/// This is the one phase of a run that is **not** stopped cooperatively
/// (smetana-16w). The stop between batches waits for the batch in flight
/// because a session interrupted between a merge and a close leaves work to
/// recover; a declared command leaves nothing of the kind — it brings
/// infrastructure up and is run again from the top next time — while its
/// ceilings are 600s apiece against a health check's 120s, and the first one on
/// this project is `npm install`. Waiting those out is a stop that visibly does
/// nothing for minutes, and the project stays unstartable for all of them,
/// because the entry only leaves the map when this task is gone.
async fn bring_up(
    root: &Path,
    config: &config::Preflight,
    stop: &mut mpsc::Receiver<()>,
) -> BringUp {
    let cancel = preflight::Cancel::default();
    let owned = root.to_path_buf();
    let commands = config.commands.clone();
    let asked = cancel.clone();
    let mut running = tokio::task::spawn_blocking(move || {
        for command in &commands {
            match preflight::run_command(&owned, command, &asked) {
                Ok(preflight::Ran::Done) => {}
                Ok(preflight::Ran::Cancelled) => return Ok(preflight::Ran::Cancelled),
                Err(err) => return Err(err.to_string()),
            }
        }
        Ok(preflight::Ran::Done)
    });

    let ran = tokio::select! {
        joined = &mut running => joined,
        _ = stop.recv() => {
            // The thread owns the child, so killing it is that thread's own
            // last act and this waits for it — a command left running behind a
            // run that has ended is the orphan `terminate` exists to prevent.
            // Bounded by one poll of `run_command`, not by the command.
            cancel.ask();
            let _ = running.await;
            // Whatever the join then said, the answer is the stop: a command
            // that happened to finish in that same instant does not un-press
            // the button, and the token is off the channel now — no later look
            // at it, here or in `drive`, would ever find it again.
            return BringUp::Cancelled;
        }
    };
    match ran {
        Ok(Ok(preflight::Ran::Done)) => {}
        Ok(Ok(preflight::Ran::Cancelled)) => return BringUp::Cancelled,
        Ok(Err(detail)) => return BringUp::Failed(detail),
        Err(err) => return BringUp::Failed(format!("a preflight command could not be run: {err}")),
    }

    for check in &config.health {
        match healthy(check.clone(), stop).await {
            Probe::Up => {}
            Probe::Cancelled => return BringUp::Cancelled,
            Probe::Down => {
                return BringUp::Failed(
                    preflight::PreflightError::Unhealthy { check: preflight::describe(check) }
                        .to_string(),
                )
            }
        }
    }
    BringUp::Ready
}

/// What one health check settled on.
enum Probe {
    Up,
    Down,
    Cancelled,
}

/// Wait for one check to come good, giving up on a stop as well as on the
/// budget.
///
/// The stop is read between looks rather than during one, which is the opposite
/// of what `bring_up` does to a declared command and for a plain reason: a look
/// is bounded by seconds of its own — `curl --max-time 5`, a two-second connect
/// — where a command has nothing bounding it but `COMMAND_TIMEOUT`. So the most
/// a stop waits here is one look, and there is nothing to kill to shorten it.
async fn healthy(check: config::HealthCheck, stop: &mut mpsc::Receiver<()>) -> Probe {
    let started = tokio::time::Instant::now();
    loop {
        // The channel and not `run.stopping`, for the reason `headroom` records:
        // this task holds its own `Run` and the flag is set on the worker's copy.
        if stop.try_recv().is_ok() {
            return Probe::Cancelled;
        }
        let probe = check.clone();
        let up = tokio::task::spawn_blocking(move || preflight::is_healthy(&probe))
            .await
            .unwrap_or(false);
        match preflight::poll_step(up, started.elapsed(), preflight::HEALTH_TIMEOUT) {
            preflight::Poll::Healthy => return Probe::Up,
            preflight::Poll::GiveUp => return Probe::Down,
            preflight::Poll::Again => {
                tokio::select! {
                    _ = tokio::time::sleep(preflight::HEALTH_INTERVAL) => {}
                    _ = stop.recv() => return Probe::Cancelled,
                }
            }
        }
    }
}

/// The board, or `None` when the tracker could not be asked.
async fn board(tracker: &TrackerHandle) -> Option<Vec<crate::tracker::model::Issue>> {
    let (tx, rx) = oneshot::channel();
    tracker.0.send(TrackerRequest::Snapshot(tx)).await.ok()?;
    rx.await.ok().map(|snapshot| snapshot.issues)
}

/// May another batch go out? The worker answers, and the answer is the decision
/// itself: yes records the batch as in flight on the worker's own copy of the
/// run, so a stop arriving after it takes the cooperative path — set `stopping`,
/// let the batch finish, end at the top of the next round — instead of
/// declaring the run over while the batch goes out behind it.
///
/// A worker that cannot answer is a no. There would be nothing left to report a
/// batch to, and of the two ways to be wrong here only one of them merges
/// something nobody asked for.
async fn may_spawn(report: &mpsc::UnboundedSender<Report>, token: u64, project: &str) -> bool {
    let (tx, rx) = oneshot::channel();
    let asked = report.send(Report::Spawning { token, project: project.to_string(), allow: tx });
    if asked.is_err() {
        return false;
    }
    rx.await.unwrap_or(false)
}

/// One batch. The agent to run is not this worker's choice — it is whatever
/// `terminal_create` resolves from settings, the same as every other session,
/// so a run uses the agent the person configured and the substitution rules
/// stay in one place.
///
/// `tasks` is what *this* batch may take, which is not always what the person
/// chose: a low allowance caps it (`usage::cap`). The run's own settings are
/// left alone, so the bar and the report keep naming the choice rather than the
/// condition of the moment — the same split `views/panelWidths.js` makes.
async fn spawn_batch(
    terminal: &TerminalHandle,
    run: &Run,
    tasks: Option<u8>,
) -> Result<u64, String> {
    let (tx, rx) = oneshot::channel();
    let settings = RunSettings { max_parallel_tasks: tasks, ..run.settings.clone() };
    let intent = Intent::Run { settings };
    terminal
        .0
        .send(TerminalRequest::Create(run.project.clone(), agent_id(), intent, tx))
        .await
        .map_err(|_| "the terminal worker is not running".to_string())?;
    match rx.await {
        Ok(Ok(session)) => Ok(session.id),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => Err("the terminal worker did not answer".to_string()),
    }
}

/// Which agent a batch runs. The settings file holds the answer and the
/// terminal worker already reads it for every other session; until this worker
/// is given a way to ask, it names the default and lets `agents::pick` do what
/// it does everywhere else — fall back to whatever is installed, and say so in
/// the session's own row.
fn agent_id() -> String {
    crate::agents::IDS[0].to_string()
}

/// What ended the wait on a batch.
enum Batch {
    /// The session's process is gone; `Exit` says how.
    Ended(Exit),
    /// The session stopped to ask a person something, in a run that has no
    /// person in it.
    Unanswered { question: String },
}

/// Wait for the batch to end — and, where nobody is watching, for the session
/// to stop and ask instead.
///
/// `await_exit` alone waits on the **process**, and a session sitting at a
/// dialog never exits. Codex draws "Do you trust the contents of this
/// directory?" the first time it runs anywhere new and waits there, and
/// `--dangerously-bypass-approvals-and-sandbox` does not skip it (smetana-wnl):
/// an unattended run in an unfamiliar folder hung on the very first batch,
/// silently and for ever. The other half of that ticket — writing `trust_level`
/// into `~/.codex/config.toml` — is refused for the reason `agents/codex.rs`
/// already refuses to touch a person's home directory, so what is owed here is
/// that the run ends and says why.
///
/// What it watches is the state the terminal worker already keeps for every
/// session, active or not, asked for over the channel every other caller uses.
/// Nothing about detection changes and nothing is added to that worker.
///
/// **Only a question a profile actually read counts**, never `needs-you` on its
/// own. Layer A raises that state from a bell alone, a CLI rings one on
/// finishing a task as readily as on asking something, and a run's session is
/// nobody's on screen — so its bell is never acknowledged and would stand for
/// the rest of the session's life. Ending a run on that would end it on the
/// agent having finished a task well. A question is also the only form of this
/// a person can be told anything useful about.
async fn watch_batch(terminal: &TerminalHandle, run: &Run, session: u64) -> Batch {
    let mut ended = std::pin::pin!(await_exit(terminal, session));
    // A supervised or solo run has somebody who can answer in the terminal, and
    // that is the mode's whole point — ending their run at the first question
    // would be taking it away from them. See `RunMode::unattended`.
    if !run.settings.mode.unattended() {
        return Batch::Ended(ended.await);
    }
    let mut asked = Asked::default();
    loop {
        tokio::select! {
            exit = &mut ended => return Batch::Ended(exit),
            _ = tokio::time::sleep(ASK_POLL) => {
                let seen = asking(terminal, run, session).await;
                if let Some(question) = asked.confirm(seen.as_deref()) {
                    return Batch::Unanswered { question };
                }
            }
        }
    }
}

/// The question this batch's session has stopped on, as the terminal worker
/// sees it right now.
///
/// `None` for a session that is working, that has gone from the worker's map,
/// or that is loud with nothing readable behind it — the last of those is the
/// bell case, and it is deliberately not evidence of anything here.
async fn asking(terminal: &TerminalHandle, run: &Run, session: u64) -> Option<String> {
    let (tx, rx) = oneshot::channel();
    terminal.0.send(TerminalRequest::List(run.project.clone(), tx)).await.ok()?;
    rx.await
        .ok()?
        .into_iter()
        .find(|s| s.id == session)
        .filter(|s| s.state == SessionState::NeedsYou)
        .and_then(|s| s.question)
        .map(|question| question.text)
}

/// A terminal worker that is not there to answer counts as a batch that ended
/// without a code, never as a session somebody removed: `Removed` stops the run
/// outright, and a worker that has gone away is not a person's decision.
async fn await_exit(terminal: &TerminalHandle, session: u64) -> Exit {
    let (tx, rx) = oneshot::channel();
    if terminal.0.send(TerminalRequest::AwaitExit(session, tx)).await.is_err() {
        return Exit::NoCode;
    }
    rx.await.unwrap_or(Exit::NoCode)
}

/// The map's lifecycle, which is the half of smetana-0kb that no other test in
/// the tree reaches. Everything here is `absorb`, `permit` and `admit` over a
/// plain `HashMap` — no worker, no runtime, no `AppHandle`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::model::{RunMode, RunScope};

    fn settings() -> RunSettings {
        RunSettings {
            scope: RunScope::Queue,
            mode: RunMode::Auto,
            target_branch: "main".into(),
            create_target: false,
            min_priority: None,
            max_parallel_tasks: Some(2),
            live_check: true,
            file_findings: true,
        }
    }

    /// One project, one entry, the way `Request::Start` leaves things.
    fn map(token: u64) -> HashMap<String, Active> {
        let (stop, _rx) = mpsc::channel::<()>(1);
        let mut active = HashMap::new();
        active.insert(
            "/p".to_string(),
            Active {
                token,
                run: Run::new("/p".into(), settings()),
                starting: false,
                stop,
            },
        );
        active
    }

    fn state(token: u64, run: &Run) -> Report {
        Report::State { token, run: Box::new(run.clone()) }
    }

    #[test]
    fn a_batch_is_cleared_for_a_live_run_and_remembered_as_starting() {
        let mut active = map(1);
        assert!(permit(&mut active, 1, "/p"));
        assert!(
            active["/p"].starting,
            "the window in which the batch is on its way and `session` is still None"
        );
    }

    #[test]
    fn a_batch_is_refused_for_a_run_that_was_asked_to_stop() {
        // The half that took a driven race to find: refusing only a run already
        // over lets a stop landing just after the loop's own check start a
        // whole further round, board read and all.
        let mut active = map(1);
        active.get_mut("/p").expect("the entry").run.session = Some(9);
        active.get_mut("/p").expect("the entry").run.request_stop(false);
        assert!(!active["/p"].run.is_over(), "the batch in flight is still finishing");

        assert!(!permit(&mut active, 1, "/p"), "that batch and no more");
        assert!(!active["/p"].starting, "a refusal records nothing");
    }

    #[test]
    fn a_batch_is_refused_for_a_stale_token_and_for_a_project_with_no_entry() {
        let mut active = map(7);
        assert!(!permit(&mut active, 6, "/p"), "an older loop asking after its run was replaced");
        assert!(!permit(&mut active, 7, "/other"), "no entry, so nothing to authorize");
        assert!(!active["/p"].starting);
    }

    #[test]
    fn only_the_loop_that_owns_the_entry_ends_it() {
        // The stale-token guard on the removal. Without it a report from the
        // run that was replaced takes the live run's entry out, and the project
        // then accepts a second run beside a loop that is still going.
        let mut active = map(7);
        absorb(&mut active, Report::Ended { token: 6, project: "/p".into() });
        assert!(active.contains_key("/p"), "that ending belongs to a run that is already gone");

        absorb(&mut active, Report::Ended { token: 7, project: "/p".into() });
        assert!(!active.contains_key("/p"), "and the one that owns it does end it");
    }

    #[test]
    fn a_stopped_run_holds_its_project_until_its_loop_reports_the_ending() {
        // Both halves of the acceptance criterion in one place: the stop is
        // immediate on screen, the project stays taken while the loop winds
        // down, and it is the ending — not the stop — that frees it.
        let mut active = map(1);
        active.get_mut("/p").expect("the entry").run.request_stop(false);
        assert!(active["/p"].run.is_over(), "nothing was in flight, so the stop is immediate");

        assert_eq!(admit(&active, "/p"), Err(RunError::WindingDown));
        assert!(!permit(&mut active, 1, "/p"), "and no batch goes out of a stopped run");

        absorb(&mut active, Report::Ended { token: 1, project: "/p".into() });
        assert_eq!(admit(&active, "/p"), Ok(()), "the loop is gone, so the project is free");
    }

    #[test]
    fn a_project_taken_by_a_live_run_is_refused_as_already_running() {
        // The two refusals are different sentences on purpose: this one means
        // leave it alone, `WindingDown` means try again in a moment. Saying
        // "already going" of a run the bar has just called stopped reads as the
        // stop not having taken.
        let active = map(1);
        assert_eq!(admit(&active, "/p"), Err(RunError::AlreadyRunning));
        assert_eq!(admit(&active, "/elsewhere"), Ok(()), "another project is nobody's business");
    }

    #[test]
    fn a_report_clears_the_starting_flag_so_the_next_stop_is_not_held_by_it() {
        // `starting` stands in for a session id that does not exist yet. Left
        // set after the loop has reported, it would make a stop between batches
        // wait for a batch that is not there.
        let mut active = map(1);
        assert!(permit(&mut active, 1, "/p"));

        let mut reported = Run::new("/p".into(), settings());
        reported.advance(RunState::Deciding);
        absorb(&mut active, state(1, &reported));
        assert!(!active["/p"].starting);
    }

    #[test]
    fn a_report_from_a_replaced_run_changes_nothing_and_is_not_emitted() {
        let mut active = map(7);
        let mut reported = Run::new("/p".into(), settings());
        reported.advance(RunState::Working { iteration: 3 });

        assert!(absorb(&mut active, state(6, &reported)).is_none(), "nothing to put on the wire");
        assert_eq!(active["/p"].run.state, RunState::Preflight, "and nothing written either");
    }

    #[test]
    fn a_report_does_not_revive_a_run_the_worker_has_already_stopped() {
        // The loop's copy knows nothing of a stop until it next looks at the
        // channel, so it keeps reporting progress for a moment afterwards.
        let mut active = map(1);
        active.get_mut("/p").expect("the entry").run.request_stop(false);

        let mut reported = Run::new("/p".into(), settings());
        reported.advance(RunState::Working { iteration: 1 });
        assert!(absorb(&mut active, state(1, &reported)).is_none(), "nothing goes on the wire");
        assert!(active["/p"].run.is_over(), "and a finished run does not come back on screen");
    }

    #[test]
    fn a_report_is_emitted_with_the_stop_this_side_asked_for_still_on_it() {
        // `adopt` through the path that actually runs: a cooperative stop, then
        // the loop's next report, then the check that guards the next batch.
        let mut active = map(1);
        active.get_mut("/p").expect("the entry").run.session = Some(4);
        active.get_mut("/p").expect("the entry").run.request_stop(false);

        let mut reported = Run::new("/p".into(), settings());
        reported.advance(RunState::Deciding);
        let emitted = absorb(&mut active, state(1, &reported)).expect("a run to put on the wire");

        assert_eq!(emitted.state, RunState::Deciding, "where the loop is, is the loop's to say");
        assert!(emitted.stopping, "and whether it was asked to stop is not");
        assert!(!permit(&mut active, 1, "/p"), "so the next batch is still refused");
    }
}
