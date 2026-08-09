//! The run worker: one tokio task holding the run, the same shape
//! `tracker/service.rs` and `terminal/service.rs` have and for the same
//! reason. Nothing shares mutable state with it.
//!
//! What it does is a loop somebody else's process does the work in: read the
//! board, decide, start one session for one batch, wait for that session to
//! end, read the board again. The deciding is `queue.rs` and is tested; this
//! file is the part that talks to the other two workers, and like them it
//! carries no tests.
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

#[derive(Clone)]
pub struct RunHandle(pub mpsc::Sender<Request>);

/// The worker's own view of one project's run in flight. `Run` is what leaves
/// the worker; this is what stays.
struct Active {
    /// Which loop task this entry belongs to. The map is keyed by project and a
    /// project's next run may start the moment its last one is out of the map,
    /// so a late report has to be told from the run that replaced it — the same
    /// job `generation` does for the tracker, and the same defect if it is
    /// missing: a finished run's state written over a live one's.
    token: u64,
    run: Run,
    /// Cancels the loop task. Dropping it is what a stop after the final batch
    /// comes down to.
    stop: mpsc::Sender<()>,
}

pub fn start(app: AppHandle, tracker: TrackerHandle, terminal: TerminalHandle) -> RunHandle {
    let (tx, mut rx) = mpsc::channel::<Request>(8);
    let (report_tx, mut report_rx) = mpsc::unbounded_channel::<(u64, Run)>();

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
                // The loop task's own progress. It owns no state the front end
                // reads — it hands a whole `Run` back here, and this task is
                // the only thing that writes one out.
                report = report_rx.recv() => {
                    let Some((token, run)) = report else { break };
                    // Nothing under that key any more — a stop already took the
                    // run out and emitted it — or something newer under it.
                    // Either way this report is the past, and emitting it would
                    // put a finished run back on the screen.
                    let Some(current) = active.get_mut(&run.project) else { continue };
                    if current.token != token {
                        continue;
                    }
                    current.run = run.clone();
                    emit(&app, &run);
                    if run.is_over() {
                        active.remove(&run.project);
                    }
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
    report: &mpsc::UnboundedSender<(u64, Run)>,
    request: Request,
) {
    match request {
        Request::State(project, tx) => {
            let _ = tx.send(current(active, &project));
        }
        Request::Stop(project, tx) => {
            let mut answer = None;
            if let Some(current) = active.get_mut(&project) {
                current.run.request_stop();
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
                if run.is_over() {
                    active.remove(&project);
                }
            }
            let _ = tx.send(answer);
        }
        Request::Start(project, settings, tx) => {
            // This project's own run and nothing else. Another project's is not
            // in the way of anything: it has its own board, its own worktrees
            // and its own target branch.
            if active.get(&project).is_some_and(|a| !a.run.is_over()) {
                let _ = tx.send(Err(RunError::AlreadyRunning));
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
            active.insert(project.clone(), Active { token, run: run.clone(), stop: stop_tx });

            tauri::async_runtime::spawn(drive(
                token,
                run.clone(),
                config.preflight.clone(),
                PathBuf::from(&project),
                tracker.clone(),
                terminal.clone(),
                report.clone(),
                stop_rx,
            ));

            emit(app, &run);
            let _ = tx.send(Ok(run));
        }
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
    report: mpsc::UnboundedSender<(u64, Run)>,
    mut stop: mpsc::Receiver<()>,
) {
    let say = |run: &Run| {
        let _ = report.send((token, run.clone()));
    };

    if let Some(config) = preflight_config {
        if let Err(detail) = bring_up(&root, &config).await {
            run.advance(RunState::Stopped { reason: StopReason::Preflight { detail } });
            say(&run);
            return;
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

/// The declared commands, then the declared health checks. Blocking work goes
/// to a thread: spawning `docker compose` and waiting on it would otherwise
/// hold the whole async runtime.
async fn bring_up(root: &Path, config: &config::Preflight) -> Result<(), String> {
    let root = root.to_path_buf();
    let commands = config.commands.clone();
    let ran = tokio::task::spawn_blocking(move || {
        for command in &commands {
            preflight::run_command(&root, command).map_err(|err| err.to_string())?;
        }
        Ok::<(), String>(())
    })
    .await;
    match ran {
        Ok(Ok(())) => {}
        Ok(Err(detail)) => return Err(detail),
        Err(err) => return Err(format!("a preflight command could not be run: {err}")),
    }

    for check in &config.health {
        if !healthy(check.clone()).await {
            return Err(preflight::PreflightError::Unhealthy { check: preflight::describe(check) }
                .to_string());
        }
    }
    Ok(())
}

async fn healthy(check: config::HealthCheck) -> bool {
    let started = tokio::time::Instant::now();
    loop {
        let probe = check.clone();
        let up = tokio::task::spawn_blocking(move || preflight::is_healthy(&probe))
            .await
            .unwrap_or(false);
        match preflight::poll_step(up, started.elapsed(), preflight::HEALTH_TIMEOUT) {
            preflight::Poll::Healthy => return true,
            preflight::Poll::GiveUp => return false,
            preflight::Poll::Again => tokio::time::sleep(preflight::HEALTH_INTERVAL).await,
        }
    }
}

/// The board, or `None` when the tracker could not be asked.
async fn board(tracker: &TrackerHandle) -> Option<Vec<crate::tracker::model::Issue>> {
    let (tx, rx) = oneshot::channel();
    tracker.0.send(TrackerRequest::Snapshot(tx)).await.ok()?;
    rx.await.ok().map(|snapshot| snapshot.issues)
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
