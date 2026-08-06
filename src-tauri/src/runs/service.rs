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
use super::model::{Run, RunError, RunSettings, RunState, StopReason};
use super::preflight;
use super::queue::{self, Action, QueueSnapshot};
use crate::agents::Intent;
use crate::terminal::model::Exit;
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

    let mut previous: Option<QueueSnapshot> = None;
    let mut crashes: u32 = 0;
    let mut unreadable: u32 = 0;
    let mut crashed_last = false;

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
        match queue::next_action(&now, previous.as_ref(), iteration, MAX_ITERATIONS, crashed_last) {
            Action::Stop(reason) => {
                run.advance(RunState::Stopped { reason });
                say(&run);
                return;
            }
            Action::Run(_) => {}
        }
        previous = Some(now);

        let session = match spawn_batch(&terminal, &run).await {
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

        let exit = await_exit(&terminal, session).await;
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
            crashed_last = false;
        } else {
            crashes += 1;
            crashed_last = true;
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
async fn spawn_batch(terminal: &TerminalHandle, run: &Run) -> Result<u64, String> {
    let (tx, rx) = oneshot::channel();
    let intent = Intent::Run { settings: run.settings.clone() };
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
