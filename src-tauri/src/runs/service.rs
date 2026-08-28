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
//! A project holds as many runs as it has scopes to give them, and the map is
//! keyed by each run's own `token` — a second run over the **same** scope is
//! refused rather than queued (smetana-5hf: two runs both told to take the
//! whole queue is not parallelism, it is two leads racing for the same tasks),
//! while a queue run beside a task run, or two runs over different epics,
//! divide the board between them. Which tasks each one may touch is not this
//! worker's to police: bd's atomic claim under per-session actors is the
//! exclusivity (smetana-4fh), and a second mechanism here could only disagree
//! with it. A run in another project is none of this one's business.
//! Different projects are different folders, stacks, boards and target
//! branches; the only thing they share is a subscription limit, and a run does
//! not reserve one (smetana-tra).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::awake;
use super::config::{self, ConfigState};
use super::model::{
    Asked, OnQuestion, RepeatedQuestion, Run, RunError, RunScope, RunSettings, RunState, StopReason,
};
use super::preflight;
use super::queue::{self, Action, LastBatch, QueueSnapshot};
use super::recovery;
use super::registry::Proc;
use super::report::{self, BatchLine, BatchOutcome};
use super::summary::{self, Baseline, RunSummary};
use super::usage::{self, Decision};
use crate::agents::{Intent, Profile};
use crate::terminal::model::{Exit, SessionState};
use crate::terminal::service::{Request as TerminalRequest, TerminalHandle};
use crate::tracker::model::IssuePatch;
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
/// costs its batch about ten seconds after it is drawn, against the for ever it
/// used to take.
const ASK_POLL: Duration = Duration::from_secs(5);
/// How many runs of one project may end in the same second before a report has
/// nowhere to go. A ceiling rather than an endless walk, since every step is a
/// syscall; a project would need twenty live runs stopping together to reach it.
const MAX_REPORTS_PER_SECOND: u32 = 20;

pub enum Request {
    Start(String, Box<RunSettings>, oneshot::Sender<Result<Run, RunError>>),
    /// Stop one run, named by its token. A project holds several runs now, so
    /// a stop aimed at a project would be ambiguous in exactly the case this
    /// task exists for; `None` back means no such run — it ended and left the
    /// map before the stop arrived, which is a stop with nothing left to do.
    Stop(u64, oneshot::Sender<Option<Run>>),
    /// Every run in this project still in the map — live, stopping, or stopped
    /// and winding down — oldest first.
    State(String, oneshot::Sender<Vec<Run>>),
    /// Which projects have a live run that will be driving a browser. The run
    /// dialog asks before it opens, so it can say that the tool exists and
    /// something else is holding it — a different sentence from the tool not
    /// being there at all.
    ///
    /// Counted per run rather than per project, and the asking project is not
    /// excluded any more: a live-check run in this very project is exactly the
    /// thing that holds Playwright's one profile against a second one beside
    /// it, now that `admit` no longer refuses that second run for being in the
    /// same project. What comes back is a candidate list rather than an answer
    /// — the worker knows a run asked for a live check and not what kind of
    /// check that project declares, and `browser_tools` reads the config to
    /// settle it.
    BrowserBusy(oneshot::Sender<Vec<String>>),
    /// Which projects hold a run at all — the whole map, not this project's
    /// share of it. The updater's install gate asks before it replaces the
    /// bundle and relaunches (`updates.rs`), and that question has no answer in
    /// the front end: `runs.js` is filtered to the active project, so a run in
    /// a neighbouring one is invisible there.
    ///
    /// One request rather than a widening of `State`, which is about one
    /// project and answers with whole `Run` values: what the gate needs is the
    /// projects and nothing else, and a caller handed every run in the app
    /// would be a caller able to act on one.
    LiveProjects(oneshot::Sender<Vec<String>>),
}

/// What a loop task says to the worker. Three messages on one channel so they
/// arrive in the order the loop sent them, and none of them is the loop writing
/// state anywhere — the worker is still the only thing that owns the map.
enum Report {
    /// Where the loop has got to.
    State { token: u64, run: Box<Run> },
    /// A batch has started: the name it claims under in bd's audit trail, and
    /// the process group it can be found in. Only the registry reads it — it is
    /// what a next start after a `kill -9` has to hang up, and what Phase R
    /// matches an `in_progress` claim against.
    ///
    /// Its own report rather than a field on `State`, because the group is
    /// asked of the terminal worker and may not answer: a batch whose pid could
    /// not be read still has an actor worth writing down.
    Started { token: u64, session: u64, group: Option<Proc> },
    /// May another batch go out? The worker's answer *is* the decision — see
    /// `may_spawn`.
    Spawning { token: u64, allow: oneshot::Sender<bool> },
    /// The loop task is gone, however it went.
    Ended { token: u64 },
}

#[derive(Clone)]
pub struct RunHandle(pub mpsc::Sender<Request>);

/// The worker's own view of one run in flight. `Run` is what leaves the
/// worker; this is what stays.
///
/// An entry is in the map for exactly as long as its loop task is alive — no
/// longer, and **no shorter**. A run declared stopped keeps its entry until
/// the loop reports its own ending, because a stopped run whose loop is still
/// winding down is still a loop that would spawn a batch, and letting a second
/// run of the same scope start beside it is how the exclusivity this map keeps
/// comes apart (smetana-0kb).
///
/// The map is keyed by the run's own `token` — issued once, never reused — so
/// a late report from an ended run finds no entry rather than somebody else's:
/// the same job `generation` does for the tracker, and the same defect if it
/// were missing, a finished run's state written over a live one's.
struct Active {
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
    report: mpsc::UnboundedSender<Report>,
}

impl Drop for Ending {
    fn drop(&mut self) {
        let _ = self.report.send(Report::Ended { token: self.token });
    }
}

/// `known` is the projects to sweep for what an unclean exit left behind: the
/// settings file's open list plus whatever the app is opening with. Handed in
/// rather than read here, because `lib.rs` has already loaded that file to
/// decide which project to open.
pub fn start(
    app: AppHandle,
    tracker: TrackerHandle,
    terminal: TerminalHandle,
    known: Vec<PathBuf>,
) -> RunHandle {
    let (tx, mut rx) = mpsc::channel::<Request>(8);
    let (report_tx, mut report_rx) = mpsc::unbounded_channel::<Report>();

    tauri::async_runtime::spawn(async move {
        // Before the first request is served, and on this task rather than a
        // task of its own: this worker is the only thing that writes the
        // registry, which is what makes its read-modify-write safe, and a run
        // started beside a sweep that is about to hang up a leftover agent
        // would be two agents in one worktree. Requests queue meanwhile, which
        // is a fraction of a second unless something really is still running.
        recovery::recover(&known).await;

        // Keyed by each run's token: several runs share a project now, and the
        // token is the one name that is never two runs'. Which project an
        // entry belongs to is `run.project`, the same path the tracker, the
        // settings and the front end name a project by.
        let mut active: HashMap<u64, Active> = HashMap::new();
        let mut next_token: u64 = 1;
        // The machine is not to fall asleep under a live run. Owned here and
        // nowhere else: the count it works from is the size of the map below,
        // which is why no ending has to remember to release — see `awake.rs`.
        let mut keeper = awake::system();

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
            // The end of the pass, whichever arm was taken: a start has just
            // put an entry in the map, or a `Report::Ended` has just taken one
            // out. Derived from the map rather than from either event, so that
            // every ending — an empty queue, the stop button, a crash, a panic
            // unwinding through the loop task — releases without being
            // enumerated here.
            keeper.sync(active.len());
        }
        // Both arms break when their channel closes, which is the app on its
        // way out; the hold goes with the task.
    });

    RunHandle(tx)
}

fn emit(app: &AppHandle, run: &Run) {
    let _ = app.emit("run:state", run);
}

fn handle(
    app: &AppHandle,
    active: &mut HashMap<u64, Active>,
    next_token: &mut u64,
    tracker: &TrackerHandle,
    terminal: &TerminalHandle,
    report: &mpsc::UnboundedSender<Report>,
    request: Request,
) {
    match request {
        Request::State(project, tx) => {
            let _ = tx.send(runs_in(active, &project));
        }
        Request::BrowserBusy(tx) => {
            let _ = tx.send(browser_candidates(active));
        }
        Request::LiveProjects(tx) => {
            let _ = tx.send(live_projects(active));
        }
        Request::Stop(token, tx) => {
            let mut answer = None;
            if let Some(current) = active.get_mut(&token) {
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
            // This project's own run over this very scope and nothing else.
            // Another project's is not in the way of anything, and neither is
            // another scope's in this one: a queue run beside a task run
            // divide the board, and which tasks each may touch is bd's atomic
            // claim to keep, not this map's.
            if let Err(err) = admit(active, &project, &settings.scope) {
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

            // Which agent this run works with, read from the settings file here
            // and carried for the whole of the run — the same choice
            // `Intent::Run` makes about the settings themselves, and for the
            // same reason: a run outlives an edit to that file, and one that
            // silently changed harness between batches would have asked the
            // allowance of a subscription it then stopped spending
            // (smetana-3fi). Re-reading it per batch would buy nothing but that.
            let agent = crate::settings::agent(app);
            // Beside it and read the same way, for the same reason: a run that
            // silently changed its mind about worktrees between batches would
            // leave half a night's checkouts on the disk and sweep the other
            // half away, with nothing in either report saying which.
            let remove_worktrees = crate::settings::git_remove_worktrees(app);

            let token = *next_token;
            *next_token += 1;
            let run = Run::new(token, project.clone(), settings);
            let (stop_tx, stop_rx) = mpsc::channel::<()>(1);
            active.insert(token, Active { run: run.clone(), starting: false, stop: stop_tx });
            // On disk from here until the run ends, so that an app killed
            // mid-run leaves something behind that says what was going and
            // under whose name — the map above is memory and dies with the
            // process. Written here rather than by the loop task for the reason
            // the sweep runs here: one writer.
            recovery::note_run(Path::new(&project), token, &run.settings.target_branch);

            let ending = Ending { token, report: report.clone() };
            let driving = drive(
                token,
                run.clone(),
                config.preflight.clone(),
                PathBuf::from(&project),
                agent,
                remove_worktrees,
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

fn handle_report(app: &AppHandle, active: &mut HashMap<u64, Active>, report: Report) {
    // The disk half first, because an ending takes its entry out of the map and
    // the project this run belongs to is on that entry. Kept out of `absorb`,
    // which stays pure.
    record(active, &report);
    if let Some(run) = absorb(active, report) {
        emit(app, &run);
    }
}

/// What a report changes in `.smetana/runs.json`: a batch is added to its run's
/// record, and an ending — any ending, since `Report::Ended` comes from a `Drop`
/// guard — takes the record away, unless it still names a process that is
/// running. That last condition is `registry::forget_run`'s, and the ending it
/// exists for is `NeedsAnswer`, which deliberately leaves its session alive.
///
/// Everything else leaves the file alone: a run's state is not evidence of
/// anything after the process is gone, and rewriting the file on every progress
/// report would be a write per board read for nothing.
fn record(active: &HashMap<u64, Active>, report: &Report) {
    let (token, project) = match report {
        Report::Started { token, .. } | Report::Ended { token } => match active.get(token) {
            Some(entry) => (*token, entry.run.project.clone()),
            // An ending whose entry has already gone, or a batch reported by a
            // loop whose run is over: there is nothing on disk to attach it to.
            None => return,
        },
        _ => return,
    };
    let root = Path::new(&project);
    match report {
        Report::Started { session, group, .. } => {
            recovery::note_batch(
                root,
                token,
                crate::terminal::model::run_actor(*session),
                group.clone(),
            );
        }
        Report::Ended { .. } => recovery::forget_run(root, token),
        _ => {}
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
fn absorb(active: &mut HashMap<u64, Active>, report: Report) -> Option<Run> {
    match report {
        Report::State { token, run } => {
            let run = *run;
            // Nothing under that token any more: the run ended and its entry
            // left. The report is the past, and emitting it would put a
            // finished run back on the screen. A token is issued once and
            // never reused, so — unlike the project-keyed map this grew out of
            // — there is no "something newer under the key" case to guard.
            let current = active.get_mut(&token)?;
            // The loop has said where it is and `run.session` carries that now,
            // so the stand-in for it has done its job. Cleared on every report,
            // which is what stops a run that has finished a batch from looking
            // busy to the next stop.
            current.starting = false;
            // A run the worker has already declared stopped is not revived by a
            // batch that was on its way out — the rule `Run::advance` keeps one
            // level down, needed here because the loop's own copy knows nothing
            // of that stop until it next looks at the channel.
            //
            // One thing does cross that line, and only one: the run's account
            // of itself. `request_stop` ends a run with nothing in flight at
            // once, so this side reaches `Stopped` while the loop is still
            // inside a board read — and the loop is the only thing that runs
            // `finish`, which is the only thing that writes a summary. Dropping
            // its report wholesale left the document on disk and the `Run` on
            // the wire saying there was none, for every stop landing between
            // batches, on a paused run, or during the preflight. `take_summary_from`
            // takes that field and refuses the rest, ending included.
            if current.run.is_over() {
                return current.run.take_summary_from(run).then(|| current.run.clone());
            }
            // Adopted rather than assigned: `stopping` is this side's field and
            // the loop's copy always says false — see `Run::adopt`.
            current.run.adopt(run);
            Some(current.run.clone())
        }
        Report::Spawning { token, allow } => {
            let _ = allow.send(permit(active, token));
            None
        }
        // The registry's business alone, and `record` has already dealt with
        // it: nothing about the map or the front end changes because a batch's
        // pid was written down.
        Report::Started { .. } => None,
        Report::Ended { token } => {
            // The one place an entry leaves the map. By its own token, so an
            // ending can only ever take its own entry out — the guard that
            // used to need a token comparison is the key itself now.
            active.remove(&token);
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
fn permit(active: &mut HashMap<u64, Active>, token: u64) -> bool {
    match active.get_mut(&token) {
        Some(current) if current.run.may_start_batch() => {
            current.starting = true;
            true
        }
        _ => false,
    }
}

/// Whether this project can take a new run over this scope. Presence of a
/// same-project, same-scope entry is the test, not the state of the run in it
/// — an entry is there for exactly as long as a loop task is alive — but the
/// two cases are told apart in the answer, because they are different things
/// to be told. A live run is a reason to leave it alone, and the refusal names
/// the scope it holds; a run that stopped a second ago and is still winding
/// down is a reason to try again shortly, and calling that one "already going"
/// contradicts the bar, which says stopped at the same moment.
///
/// "Same scope" is `RunScope`'s own equality — Queue against Queue, or the
/// same id under the same kind. Everything else runs beside this project's
/// other runs: which tasks each may touch is bd's claim to arbitrate, and a
/// second exclusion here could only disagree with it.
fn admit(active: &HashMap<u64, Active>, project: &str, scope: &RunScope) -> Result<(), RunError> {
    let mut winding_down = false;
    for entry in active.values() {
        if entry.run.project != project || entry.run.settings.scope != *scope {
            continue;
        }
        if entry.run.is_over() {
            winding_down = true;
        } else {
            return Err(RunError::AlreadyRunning { scope: scope.describe() });
        }
    }
    if winding_down {
        Err(RunError::WindingDown)
    } else {
        Ok(())
    }
}

/// Every run this project holds, oldest first. Tokens only ever grow, so
/// sorting by them is starting order — the map itself has no order to offer.
fn runs_in(active: &HashMap<u64, Active>, project: &str) -> Vec<Run> {
    let mut runs: Vec<Run> = active
        .values()
        .filter(|a| a.run.project == project)
        .map(|a| a.run.clone())
        .collect();
    runs.sort_by_key(|run| run.token);
    runs
}

/// The projects whose live runs asked for a live check — the candidate list
/// `Request::BrowserBusy` answers with. Per run and deduplicated to projects
/// only because that is what the caller reads configs by; the asking project
/// is deliberately in it (see the request's own comment). `is_over` and not
/// `stopping`: a run winding down still has its batch in flight, and that
/// batch still has the browser.
fn browser_candidates(active: &HashMap<u64, Active>) -> Vec<String> {
    let mut projects: Vec<String> = active
        .values()
        .filter(|a| !a.run.is_over() && a.run.settings.live_check)
        .map(|a| a.run.project.clone())
        .collect();
    projects.sort();
    projects.dedup();
    projects
}

/// Every project the map holds an entry for, sorted and deduplicated — the
/// updater's install gate reads it, and nothing else does.
///
/// **Every entry counts, whatever state its run is in**, which is the one place
/// this differs from `browser_candidates` above. It is the same count
/// `keeper.sync(active.len())` works from, and for the same reason: a run that
/// has stopped and is winding down still has a batch in flight, so it is still
/// agent processes a relaunch would orphan. Filtering by `is_over` here would
/// let an install through in exactly the seconds a stop is being carried out.
fn live_projects(active: &HashMap<u64, Active>) -> Vec<String> {
    let mut projects: Vec<String> =
        active.values().map(|entry| entry.run.project.clone()).collect();
    projects.sort();
    projects.dedup();
    projects
}

/// Everything an ending needs to write the run's document, gathered as the loop
/// goes: the run's own clock, the board as it first stood, what each batch said
/// about itself, and the directory those accounts are read from.
///
/// A struct rather than four more parameters repeated at a dozen call sites —
/// which is the very shape `finish` exists to collapse.
struct Account {
    started: Instant,
    /// `None` until the first board read lands, which is exactly the case a run
    /// that died in its preflight is in: there is no baseline, so there is no
    /// diff, and the document says so rather than counting zero.
    baseline: Option<Baseline>,
    batches: Vec<BatchLine>,
    reports: PathBuf,
}

/// The loop itself, on a task of its own so the worker above stays answerable
/// while a batch runs for an hour.
#[allow(clippy::too_many_arguments)]
async fn drive(
    token: u64,
    mut run: Run,
    preflight_config: Option<config::Preflight>,
    root: PathBuf,
    agent: String,
    // `settings.json`'s `git.removeWorktrees`, read once when the run started
    // and carried for the whole of it — the same snapshot `agent` above is,
    // and it reaches the lead as a line of `Intent::Run`'s prompt.
    remove_worktrees: bool,
    tracker: TrackerHandle,
    terminal: TerminalHandle,
    report: mpsc::UnboundedSender<Report>,
    mut stop: mpsc::Receiver<()>,
) {
    let say = |run: &Run| {
        let _ = report.send(Report::State { token, run: Box::new(run.clone()) });
    };

    // The clock starts before anything the run will be held to account for: the
    // preflight, the pauses on a spent allowance and the crash backoff are all
    // part of how long the night took.
    let mut account = Account {
        started: Instant::now(),
        baseline: None,
        batches: Vec::new(),
        // Keyed by the token, which is unique within this app instance — the
        // document itself is timestamped instead, because a token counts from
        // zero on every start and would collide across restarts.
        reports: root.join(".smetana").join("runs").join(token.to_string()),
    };
    if let Err(err) = std::fs::create_dir_all(&account.reports) {
        // Never a reason to hold a run up: a batch whose account cannot be
        // written is a document saying that batch left none, which is the same
        // outcome as a batch killed before it could write one.
        log::warn!("could not make {}: {err}", account.reports.display());
    }

    if let Some(config) = preflight_config {
        match bring_up(&root, &config, &mut stop).await {
            BringUp::Ready => {}
            BringUp::Cancelled => {
                finish(&mut run, StopReason::Cancelled, &say, &account, &root, &tracker).await;
                return;
            }
            BringUp::Failed(detail) => {
                let reason = StopReason::Preflight { detail };
                finish(&mut run, reason, &say, &account, &root, &tracker).await;
                return;
            }
        }
    }

    // Which agent's allowance to ask about: the one this run was started with,
    // resolved the same way `terminal/service.rs` resolves the one it spawns —
    // the same id through the same `pick` over the same `PATH` (`shell_env`
    // answers once and remembers), so short of an agent being installed or
    // removed mid-run the gate and the batch land on one harness. That is the
    // whole of smetana-3fi's second consequence: a readable answer about
    // somebody else's subscription pauses a run whose own limit is intact, or
    // sends a full-size batch into one that is already spent.
    //
    // Nothing installed is not an error here — the gate simply cannot ask, and
    // `spawn_batch` is where that failure belongs and is already reported.
    let profile = crate::agents::pick(&agent, crate::shell_env::path());

    let mut previous: Option<QueueSnapshot> = None;
    let mut crashes: u32 = 0;
    let mut unreadable: u32 = 0;
    let mut last_batch = LastBatch::Completed;
    // Batches ended by an unanswered question, counted in a row: the first
    // costs its batch, the same question again costs the run. Loop state like
    // `last_batch`, because the loop is the only thing that sees every ending.
    let mut questions = RepeatedQuestion::default();

    for iteration in 0.. {
        if stop.try_recv().is_ok() || run.stopping {
            finish(&mut run, StopReason::Cancelled, &say, &account, &root, &tracker).await;
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
                finish(&mut run, StopReason::Unreadable, &say, &account, &root, &tracker).await;
                return;
            }
            continue;
        };
        unreadable = 0;
        // The first board read inside the loop, after the preflight, is the
        // baseline the whole report is a diff against — so a task already
        // `closed` when the run started is not credited to it.
        if account.baseline.is_none() {
            account.baseline = Some(Baseline::of(&issues, &run.settings.scope));
        }

        let mut now = queue::snapshot(&issues, &run.settings.scope, run.settings.min_priority);
        // Narrower than the mode on purpose: what the decision cares about is
        // whether this run may take a second batch, not who answers a question
        // — see `RunMode::one_batch`.
        let once = run.settings.mode.one_batch();
        let mut action =
            queue::next_action(&now, previous.as_ref(), iteration, MAX_ITERATIONS, last_batch, once);
        // An empty board is the one ending worth paying a resync for. The
        // snapshot above is the tracker worker's cache, and it learns of a
        // batch's writes through the watcher — so a board read microseconds
        // after the batch's process exited can still be missing the closes that
        // released the next task. Every other ending survives being two seconds
        // late; this one ends the night saying there was nothing left to take.
        // `finish` already resyncs for the same reason, one step later and too
        // late to change the decision.
        if matches!(action, Action::Stop(StopReason::QueueEmpty)) {
            if let Some(fresh) = fresh_board(&tracker).await {
                now = queue::snapshot(&fresh, &run.settings.scope, run.settings.min_priority);
                action = queue::next_action(
                    &now,
                    previous.as_ref(),
                    iteration,
                    MAX_ITERATIONS,
                    last_batch,
                    once,
                );
            }
        }
        match action {
            Action::Stop(reason) => {
                finish(&mut run, reason, &say, &account, &root, &tracker).await;
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
            finish(&mut run, StopReason::Cancelled, &say, &account, &root, &tracker).await;
            return;
        };

        // Asked rather than checked, and that difference is the fix. Reading
        // the stop channel once more here would narrow the window and leave it
        // open: the stop and the spawn are two events in two tasks, and nothing
        // orders the answer against the microseconds that follow it. The worker
        // can order them, because it is the single task that handles both.
        if !may_spawn(&report, token).await {
            finish(&mut run, StopReason::Cancelled, &say, &account, &root, &tracker).await;
            return;
        }

        // The batch's own number and its own clock. The number is what names
        // the file the lead writes its account into, so the app can match an
        // account to the batch it timed rather than trusting a count kept twice.
        let batch_no = run.batches + 1;
        let batch_started = Instant::now();

        // Before the batch that writes it, never after: this batch's account is
        // what says it handed the work back, and `token` counts from zero on
        // every app start, so a previous launch's file can be sitting under this
        // very name already. See `clear_account`.
        clear_account(&account.reports, batch_no);

        let session = match spawn_batch(
            &terminal,
            &run,
            tasks,
            &agent,
            &account.reports,
            batch_no,
            remove_worktrees,
        )
        .await
        {
            Ok(id) => id,
            Err(err) => {
                let reason = StopReason::Preflight { detail: err };
                finish(&mut run, reason, &say, &account, &root, &tracker).await;
                return;
            }
        };

        // Before anything waits on the batch: from here on the app may be
        // killed at any moment, and what the registry does not know about by
        // then is an agent nobody will ever signal.
        let group = group_of(&terminal, session).await;
        let _ = report.send(Report::Started { token, session, group });

        run.working_in(session);
        run.batches += 1;
        run.advance(RunState::Working { iteration });
        say(&run);

        let outcome = watch_batch(&terminal, &run, session, &account.reports, batch_no).await;
        // Read here rather than at the ending, so that a batch's account is
        // taken while it is the freshest thing on disk and every way out of the
        // match below is covered by one read instead of three.
        //
        // Both halves of the record are taken in the same breath, and that is
        // the shape smetana-pmj asked for: the agent's file, which a killed
        // agent never wrote, and the ending this loop is holding in `outcome`,
        // which it knows in every case. Before the match rather than inside its
        // arms, because every arm ends the batch and three copies of this is how
        // one of them would come to be missing it.
        let mut record = read_batch(
            &account.reports,
            batch_no,
            batch_started.elapsed().as_secs(),
            outcome_of(&outcome),
        );
        // Only for a batch that said nothing: an account is a lead telling
        // somebody where it left the board, and a second board read behind a
        // lead that already answered would cost every run a resync for a line
        // nobody needed.
        if !record.reported {
            record.left_behind = held_by(&tracker, session).await;
        }
        account.batches.push(record);

        let exit = match outcome {
            // The work is done and the session is still alive with a person in
            // it. There is no exit code to read and none to wait for, so this
            // arm never reaches the classification below: a hand-back is a
            // batch that completed, and the crash counter starts over exactly
            // as a clean exit makes it.
            //
            // The session is deliberately left running. Ending the run does not
            // end the conversation — that is what the mode is for — and the
            // registry keeps a record naming a process that is still there, by
            // `forget_run`'s own condition on the processes rather than on the
            // reason, so nothing is orphaned by this.
            Batch::HandedBack => {
                crashes = 0;
                last_batch = LastBatch::Completed;
                continue;
            }
            Batch::Ended(exit) => {
                // Ended some other way, so the questions-in-a-row count starts
                // over — see `RepeatedQuestion` for why "in a row" is literal.
                questions.cleared();
                exit
            }
            // The batch has stopped to ask, and this run has nobody in it to
            // answer. Waiting on the process would be waiting for ever, and
            // ending the run here cost the whole night for one question
            // (smetana-8pe) — the lead's own layer parks what it cannot settle
            // and carries on, and a lead stuck at a harness dialog just cannot
            // do that for itself. So the question costs one batch: the session
            // is killed, whatever it claimed is parked with the question as the
            // note, and the loop goes round for the next batch. Only the same
            // question ending two batches in a row ends the run — a machine
            // that cannot start needs a person, not more batches.
            Batch::Unanswered { question } => match questions.ended_by(&question) {
                OnQuestion::Park => {
                    // The kill first, stated honestly: `Remove` is
                    // `Pty::kill()`, which reaches the direct child alone —
                    // the lead dies at its dialog, while anything it
                    // delegated is orphaned rather than signalled (`pty.rs`
                    // records exactly this about `kill`; the group-wide
                    // SIGHUP is `hangup`, the shutdown path's). So parking
                    // can still race a surviving sub-agent's last bd writes;
                    // the ~2s resync in `fresh_board` absorbs most of them,
                    // and a claim that lands after the parking stays
                    // `in_progress`, which the next batch reads as unfinished
                    // work to recover rather than losing.
                    remove_session(&terminal, session).await;
                    park_claims(&tracker, session, &question).await;
                    last_batch = LastBatch::Asked;
                    continue;
                }
                OnQuestion::Stop => {
                    // The claims are parked here too — nothing may be left
                    // `in_progress` by this path — but the session is left
                    // alive and still at its prompt: the terminal is where a
                    // person answers it, which is exactly what the bar tells
                    // them. The race the Park arm kills for is tolerated
                    // here, and the trade is deliberate: a lead stopped by
                    // the same startup dialog twice has typically delegated
                    // nothing yet, so there is usually nobody left to race —
                    // and killing would take away the very terminal the
                    // person is being sent to.
                    park_claims(&tracker, session, &question).await;
                    let reason = StopReason::NeedsAnswer { question };
                    finish(&mut run, reason, &say, &account, &root, &tracker).await;
                    return;
                }
            },
        };
        // A session somebody removed from the agents panel is not a harness
        // that fell over: nothing is going to go better on the next try, and
        // retrying it would answer "take this away" with another one just like
        // it. So the run ends here, with its own reason — the crash backstop
        // below is for processes that failed on their own.
        if exit == Exit::Removed {
            finish(&mut run, StopReason::SessionRemoved, &say, &account, &root, &tracker).await;
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
            let reason = StopReason::Crashed { attempts: crashes };
            finish(&mut run, reason, &say, &account, &root, &tracker).await;
            return;
        }
        let backoff = CRASH_BACKOFF_MAX.min(CRASH_BACKOFF_BASE * 2u32.pow(crashes - 1));
        // Interruptible: two minutes of backoff is long enough that a
        // person who pressed stop would otherwise think it did nothing.
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = stop.recv() => {
                finish(&mut run, StopReason::Cancelled, &say, &account, &root, &tracker).await;
                return;
            }
        }
    }
}

/// The single way the loop task ends a run.
///
/// Every ending *this task* reaches goes through here, so that the next one
/// somebody adds cannot quietly arrive with no report behind it — there were a
/// dozen exits into `Stopped` in `drive` before this, and a dozen call sites is
/// exactly how that happens. This is also the only place a `RunSummary` is ever
/// made.
///
/// **It is not the only way a run reaches `Stopped`.** `Run::request_stop` ends
/// one with nothing in flight at once, on the worker's own copy, which is what
/// makes the stop button immediate and what lets it reach a paused run — so for
/// a stop landing between batches, on a paused run or during the preflight, that
/// copy is already stopped by the time this runs and `absorb` will not adopt the
/// report below. `Run::take_summary_from` is the other half, and the account
/// reaches the front end through it rather than through here.
///
/// The board is read once more through `fresh_board`, which already carries the
/// ~2 s resync the run's own last writes need: the agent's `bd` writes reach
/// this process through the watcher, and a batch that closed a task moments
/// before it exited may not have landed in the cached snapshot yet. The named
/// cost is one extra tracker round trip per run ending, at a moment when
/// nothing at all is waiting on it.
///
/// A baseline that was never taken, or a final read that fails, both produce
/// `tasks: None` — never an empty diff. An unreadable board and an empty board
/// are opposite facts, and the whole of `RunSummary::tasks` being an `Option`
/// is that they must not be written down as the same one.
async fn finish(
    run: &mut Run,
    reason: StopReason,
    say: &impl Fn(&Run),
    account: &Account,
    root: &Path,
    tracker: &TrackerHandle,
) {
    let seconds = account.started.elapsed().as_secs();
    let tasks = match (account.baseline.as_ref(), fresh_board(tracker).await) {
        (Some(base), Some(issues)) => {
            Some(summary::diff(base, &issues, &run.settings.scope))
        }
        _ => None,
    };
    let report = write_report(root, run, seconds, tasks.as_ref(), &account.batches);
    run.summary = Some(RunSummary { seconds, tasks, report });
    run.advance(RunState::Stopped { reason });
    say(run);
}

/// One batch's record: the account the lead left, and the ending this loop saw.
///
/// A missing file and a damaged one are the same ordinary outcome — a batch
/// that was killed, crashed or cancelled leaves nothing — and neither is an
/// error: the batch's tasks still appear in the document from the board, and
/// the document says that batch left no account of itself.
///
/// `outcome` is passed in rather than looked up, because it is not on disk and
/// never could be: an agent that was killed writes nothing by definition, which
/// is exactly why a document resting on the file alone went silent in the one
/// case somebody opens it for (smetana-pmj). `left_behind` starts empty and is
/// filled by the caller, which is the only side that can ask the board.
fn read_batch(dir: &Path, n: u32, seconds: u64, outcome: BatchOutcome) -> BatchLine {
    let parsed = match std::fs::read_to_string(dir.join(format!("batch-{n}.json"))) {
        Ok(text) => report::parse_batch(&text),
        Err(_) => report::ParsedBatch { tasks: vec![], notes: None, reported_ok: false },
    };
    BatchLine {
        n,
        seconds,
        tasks: parsed.tasks,
        notes: parsed.notes,
        reported: parsed.reported_ok,
        outcome,
        left_behind: vec![],
    }
}

/// The loop's own ending for a batch, in the vocabulary the document draws.
///
/// The one place `Batch` and `Exit` are translated, and it adds nothing to
/// either: the only judgement in it is that a zero code is a clean exit and any
/// other number is not, which is the same reading the loop makes a few lines
/// below when it decides whether to count a crash. Kept beside that decision
/// rather than in `report.rs`, so the renderer stays pure over its own types and
/// knows nothing of the terminal.
fn outcome_of(batch: &Batch) -> BatchOutcome {
    match batch {
        Batch::Ended(Exit::Code(0)) => BatchOutcome::Exited,
        Batch::Ended(Exit::Code(code)) => BatchOutcome::Failed { code: *code },
        Batch::Ended(Exit::NoCode) => BatchOutcome::NoCode,
        Batch::Ended(Exit::Removed) => BatchOutcome::Removed,
        Batch::HandedBack => BatchOutcome::HandedBack,
        Batch::Unanswered { question } => BatchOutcome::Unanswered { question: question.clone() },
    }
}

/// What this batch's actor was still holding when the batch ended: the merge
/// lock, work left `in_progress`, work left `ready_to_merge`.
///
/// Read through `fresh_board` for the reason `park_claims` reads through it —
/// the claims being looked for are the *agent's* own bd writes, which reach this
/// process through the watcher, and a claim made moments before the session died
/// may not have landed in the cached snapshot. The cost is one resync per
/// accountless batch, at a moment when the batch is already over.
///
/// **Read only.** Nothing here releases a lock, parks a claim or writes a note:
/// the app writes to the tracker nowhere as part of recovery (`recovery.rs`),
/// and `running-tasks` Phase R is what clears this up with the worktrees in
/// front of it. Naming what was read in the run's own document does not cross
/// that line — and it is the difference between somebody learning about an
/// abandoned lock now and the next run learning about it by failing.
///
/// An unreadable board answers with nothing rather than an error: the batch's
/// record is still worth writing, and a report is not the place a tracker outage
/// is reported.
async fn held_by(tracker: &TrackerHandle, session: u64) -> Vec<queue::Leftover> {
    let actor = crate::terminal::model::run_actor(session);
    let Some(issues) = fresh_board(tracker).await else { return vec![] };
    queue::left_behind(&issues, &actor)
}

/// Has this batch handed its work back — the question that ends a batch in a
/// mode with a person in it.
///
/// An unattended batch is told to *exit*, so its ending is the process dying
/// and nothing here is asked. The other two modes run the harness the way a
/// person does, so the session finishes the work and sits at its prompt for
/// ever: waiting on the exit meant the run never came round at all, and the
/// account it had already written sat on disk with nobody reading it.
///
/// **A file that parses, never a file that exists.** The lead writes this JSON
/// with an ordinary write and nothing makes it atomic, so waking on the first
/// byte would send `read_batch` at half a document a moment later and the report
/// would say the batch left no account of itself in the one case where it left a
/// good one. Parsing is that check; there is no second mechanism to keep in step
/// with this one.
///
/// A batch that cannot write its file at all never hands back this way, and the
/// run waits on the process exactly as it does today — which is why the prompt
/// asks the lead to say so in the conversation instead. That is a person
/// pressing stop, not a silence somebody has to guess at.
/// Take away whatever a previous app process left under this batch's name,
/// before the batch that will write it starts.
///
/// `token` counts from zero on every app start — the property `write_report`
/// already refuses to lean on for its file names — so this run's directory is
/// `.smetana/runs/1` and so was one two launches ago. Without this a batch in an
/// attended mode would hand back in the instant it spawned, on somebody else's
/// account, and `read_batch` would put a previous launch's prose in this run's
/// report for a batch that crashed before writing a word.
///
/// A leftover belongs to a run that is over and whose document was written long
/// ago, so nothing is lost by it. A file that is not there is the ordinary case
/// and not an error; anything else is logged and left, since a batch that then
/// writes over it lands where it was going anyway.
fn clear_account(dir: &Path, n: u32) {
    let path = dir.join(format!("batch-{n}.json"));
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => log::warn!("could not clear {}: {err}", path.display()),
    }
}

fn handed_back(dir: &Path, n: u32) -> bool {
    std::fs::read_to_string(dir.join(format!("batch-{n}.json")))
        .map(|text| report::parse_batch(&text).reported_ok)
        .unwrap_or(false)
}

/// The document on disk, and the absolute path of it — `None` when it could not
/// be written, because a card offering a report that is not there is worse than
/// one without a button.
///
/// Timestamped rather than keyed by the run's token: the token counts from zero
/// on every app start, so two nights' reports would collide across a restart.
/// Nothing ever deletes one — they are small text, and deciding when a record of
/// a night's work stops mattering is not this app's call.
fn write_report(
    root: &Path,
    run: &Run,
    seconds: u64,
    tasks: Option<&summary::Tasks>,
    batches: &[BatchLine],
) -> Option<String> {
    let now = chrono::Local::now();
    let dir = root.join(".smetana").join("reports");
    if let Err(err) = std::fs::create_dir_all(&dir) {
        log::warn!("could not make {}: {err}", dir.display());
        return None;
    }
    let scope = run.settings.scope.describe();
    let finished = now.format("%Y-%m-%d %H:%M").to_string();
    let html = report::render(&report::RunReport {
        title: run.settings.mode.report_title(),
        project: &run.project,
        scope: &scope,
        finished: &finished,
        seconds,
        tasks,
        batches,
    });
    let (path, mut file) = claim_report(&dir, &now.format("%Y-%m-%d-%H%M%S").to_string())?;
    match std::io::Write::write_all(&mut file, html.as_bytes()) {
        Ok(()) => Some(path.to_string_lossy().into_owned()),
        Err(err) => {
            log::warn!("could not write {}: {err}", path.display());
            None
        }
    }
}

/// A file nothing else has, made rather than found.
///
/// The timestamp alone is not unique: a project holds several runs at once
/// (smetana-5hf), so two stops in the same second — one press each, or a queue
/// run and a task run both reaching `QueueEmpty` together — used to leave one
/// document, silently, and losing a night's record without a word is worse than
/// an ugly file name.
///
/// `create_new` rather than a check for the path followed by a write: the two
/// runs are on two loop tasks, so an `exists()` test would be a race with itself
/// — and it is the *creation* that has to be the exclusive step. The token is
/// deliberately not the disambiguator: it counts from zero on every app start,
/// which is the very property the timestamp was chosen to avoid leaning on, so
/// two runs numbered 1 in two app instances would collide exactly as before. A
/// suffix leans on nothing.
fn claim_report(dir: &Path, stem: &str) -> Option<(PathBuf, std::fs::File)> {
    for nth in 1..=MAX_REPORTS_PER_SECOND {
        let path = dir.join(match nth {
            1 => format!("{stem}.html"),
            n => format!("{stem}-{n}.html"),
        });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => return Some((path, file)),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                log::warn!("could not make {}: {err}", path.display());
                return None;
            }
        }
    }
    log::warn!("no free report name in {} for {stem}", dir.display());
    None
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
/// `None` means a stop arrived while it waited. The ending itself is not made
/// here, deliberately: the caller ends the run through `finish`, which is the
/// single place a run reaches `Stopped` and therefore the single place one gets
/// a report written for it.
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
            return None;
        }
        match ask(profile).await {
            Decision::Pause { pct, resets } => {
                run.advance(RunState::Paused { pct, resets });
                say(run);
                tokio::select! {
                    _ = tokio::time::sleep(usage::POLL) => {}
                    _ = stop.recv() => return None,
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

/// The board read fresh, for parking: a full resync first, because the claims
/// being looked for are writes the *agent* made with its own bd, which the
/// snapshot only learns of through the watcher — a claim written moments before
/// the session was killed may not have landed yet, and a missed claim here is a
/// task left `in_progress` under a dead actor. The cached snapshot is the
/// fallback when the resync fails, being better than parking nothing; `None`
/// when the tracker cannot be asked at all.
async fn fresh_board(tracker: &TrackerHandle) -> Option<Vec<crate::tracker::model::Issue>> {
    let (tx, rx) = oneshot::channel();
    tracker.0.send(TrackerRequest::Resync(tx)).await.ok()?;
    match rx.await.ok()? {
        Ok(snapshot) => Some(snapshot.issues),
        Err(_) => board(tracker).await,
    }
}

/// Park what a stuck batch's session claimed: everything `in_progress` under
/// the session's own bd actor (smetana-4fh is what makes that set exact) goes
/// to `parked` with the question as its note — one `bd update` apiece through
/// the tracker worker, whose snapshot learns of each write the way it learns of
/// every write, as a delta out of `finish`.
///
/// Parking the whole batch is coarser than the lead's own parking, and
/// deliberately so: this path only runs when the lead itself is stuck, and a
/// lead at a harness dialog has not told anybody which of its tasks it was
/// thinking about. A park that fails is left alone rather than retried — the
/// task stays `in_progress`, which the queue reads as unfinished work for the
/// next batch to recover, so the failure costs a recovery rather than a task.
async fn park_claims(tracker: &TrackerHandle, session: u64, question: &str) {
    let actor = crate::terminal::model::run_actor(session);
    let Some(issues) = fresh_board(tracker).await else { return };
    for id in queue::claimed_by(&issues, &actor) {
        let patch = IssuePatch {
            status: Some(queue::PARKED.to_string()),
            append_notes: Some(queue::parking_note(question)),
            ..Default::default()
        };
        let (tx, rx) = oneshot::channel();
        if tracker.0.send(TrackerRequest::Update(id, patch, tx)).await.is_ok() {
            let _ = rx.await;
        }
    }
}

/// End a stuck batch's session the way the remove button in the agents panel
/// does. Awaited rather than fired off, so the parking that follows starts
/// after the kill has gone in rather than beside it — what that kill does and
/// does not reach is recorded at the call site.
async fn remove_session(terminal: &TerminalHandle, session: u64) {
    let (tx, rx) = oneshot::channel();
    if terminal.0.send(TerminalRequest::Remove(session, tx)).await.is_ok() {
        let _ = rx.await;
    }
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
async fn may_spawn(report: &mpsc::UnboundedSender<Report>, token: u64) -> bool {
    let (tx, rx) = oneshot::channel();
    let asked = report.send(Report::Spawning { token, allow: tx });
    if asked.is_err() {
        return false;
    }
    rx.await.unwrap_or(false)
}

/// One batch. `agent` is the id from `settings.json`, read when the run started;
/// what actually spawns is still `terminal_create`'s answer, the same as for
/// every other session, so the fallback to whatever is installed lives in one
/// place and the row in the agents panel is where a substitution becomes
/// visible.
///
/// `tasks` is what *this* batch may take, which is not always what the person
/// chose: a low allowance caps it (`usage::cap`). The run's own settings are
/// left alone, so the bar and the report keep naming the choice rather than the
/// condition of the moment — the same split `views/panelWidths.js` makes.
#[allow(clippy::too_many_arguments)]
async fn spawn_batch(
    terminal: &TerminalHandle,
    run: &Run,
    tasks: Option<u8>,
    agent: &str,
    reports: &Path,
    batch: u32,
    remove_worktrees: bool,
) -> Result<u64, String> {
    let (tx, rx) = oneshot::channel();
    let settings = RunSettings { max_parallel_tasks: tasks, ..run.settings.clone() };
    // The directory and this batch's number ride with the settings for the same
    // reason those do: the session outlives everything about the run except what
    // it was handed, and a batch working out its own file name is a batch the
    // app cannot then match to the one it timed.
    // `remove_worktrees` rides beside them rather than inside `settings`, and
    // the field's own doc on `Intent::Run` says why: `RunSettings` has a
    // per-project mirror in `settings.json`, and this answer is global.
    let intent =
        Intent::Run { settings, reports: reports.to_path_buf(), batch, remove_worktrees };
    terminal
        .0
        .send(TerminalRequest::Create(run.project.clone(), agent.to_string(), intent, tx))
        .await
        .map_err(|_| "the terminal worker is not running".to_string())?;
    match rx.await {
        Ok(Ok(session)) => Ok(session.id),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => Err("the terminal worker did not answer".to_string()),
    }
}

/// The process group this batch's session runs in, with the evidence that says
/// which process that pid is. `None` when the terminal worker cannot answer or
/// the platform cannot read a start time — the batch is still recorded by its
/// actor, which is what the tracker half matches on.
async fn group_of(terminal: &TerminalHandle, session: u64) -> Option<Proc> {
    let (tx, rx) = oneshot::channel();
    terminal.0.send(TerminalRequest::Group(session, tx)).await.ok()?;
    recovery::group(rx.await.ok()??)
}

/// What ended the wait on a batch.
enum Batch {
    /// The session's process is gone; `Exit` says how.
    Ended(Exit),
    /// The batch has written its account and handed the work back, in a mode
    /// whose session stays alive afterwards because a person is sitting in it.
    /// The work is over; the conversation is not.
    HandedBack,
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
/// that the run notices. What the noticing costs is the caller's decision, and
/// it is one batch rather than the night (smetana-8pe): the loop parks that
/// batch's claims and carries on, and only the same question twice in a row
/// ends the run.
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
async fn watch_batch(
    terminal: &TerminalHandle,
    run: &Run,
    session: u64,
    reports: &Path,
    batch: u32,
) -> Batch {
    let mut ended = std::pin::pin!(await_exit(terminal, session));
    // A supervised or solo run has somebody who can answer in the terminal, and
    // that is the mode's whole point — ending their run at the first question
    // would be taking it away from them. See `RunMode::unattended`.
    //
    // It is also why the process is no signal here at all: the harness runs the
    // way it does for a person, so it finishes the work and sits at its prompt
    // for ever. Waiting on the exit meant the run did not end when the work did
    // — it ended when somebody eventually pressed stop, hours later, or never,
    // if the app was closed first — and `finish` is the only thing in the app
    // that ever writes a report. So the account the lead was asked to leave is
    // read as what it plainly is: handing the work back. See `handed_back` for
    // why the signal is a file that parses rather than one that exists.
    //
    // Whichever comes first wins, and the exit still counts: an agent that does
    // exit — because the person typed `/exit`, or the harness ended on its own —
    // ends the batch exactly as it always did.
    if !run.settings.mode.unattended() {
        loop {
            tokio::select! {
                exit = &mut ended => return Batch::Ended(exit),
                _ = tokio::time::sleep(ASK_POLL) => {
                    if handed_back(reports, batch) {
                        return Batch::HandedBack;
                    }
                }
            }
        }
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
/// the tree reaches — plus the scope rule that turned "one run per project"
/// into "one run per scope" (smetana-5hf). Everything here is `absorb`,
/// `permit`, `admit`, `runs_in` and `browser_candidates` over a plain
/// `HashMap` — no worker, no runtime, no `AppHandle`.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::model::{RunMode, RunScope};

    fn settings(scope: RunScope) -> RunSettings {
        RunSettings {
            scope,
            mode: RunMode::Auto,
            target_branch: "main".into(),
            create_target: false,
            min_priority: None,
            max_parallel_tasks: Some(2),
            live_check: true,
            file_findings: true,
        }
    }

    fn task(id: &str) -> RunScope {
        RunScope::Task { id: id.into() }
    }

    fn epic(id: &str) -> RunScope {
        RunScope::Epic { id: id.into() }
    }

    /// One entry, the way `Request::Start` leaves things.
    fn insert(active: &mut HashMap<u64, Active>, token: u64, project: &str, scope: RunScope) {
        let (stop, _rx) = mpsc::channel::<()>(1);
        active.insert(
            token,
            Active { run: Run::new(token, project.into(), settings(scope)), starting: false, stop },
        );
    }

    /// One queue run in `/p`, the smallest map most tests want.
    fn map(token: u64) -> HashMap<u64, Active> {
        let mut active = HashMap::new();
        insert(&mut active, token, "/p", RunScope::Queue);
        active
    }

    fn state(token: u64, run: &Run) -> Report {
        Report::State { token, run: Box::new(run.clone()) }
    }

    #[test]
    fn a_batch_is_cleared_for_a_live_run_and_remembered_as_starting() {
        let mut active = map(1);
        assert!(permit(&mut active, 1));
        assert!(
            active[&1].starting,
            "the window in which the batch is on its way and `session` is still None"
        );
    }

    #[test]
    fn a_batch_is_refused_for_a_run_that_was_asked_to_stop() {
        // The half that took a driven race to find: refusing only a run already
        // over lets a stop landing just after the loop's own check start a
        // whole further round, board read and all.
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.session = Some(9);
        active.get_mut(&1).expect("the entry").run.request_stop(false);
        assert!(!active[&1].run.is_over(), "the batch in flight is still finishing");

        assert!(!permit(&mut active, 1), "that batch and no more");
        assert!(!active[&1].starting, "a refusal records nothing");
    }

    #[test]
    fn a_batch_is_refused_for_a_token_that_is_not_in_the_map() {
        // An older loop asking after its run has ended and left, or a token
        // that never existed: either way there is nothing to authorize.
        let mut active = map(7);
        assert!(!permit(&mut active, 6));
        assert!(!active[&7].starting);
    }

    #[test]
    fn an_ending_removes_its_own_entry_and_nobody_elses() {
        // The identity guard in its new form: the token is the key, so an
        // ending can only take out the entry it belongs to — the comparison
        // the project-keyed map needed is the lookup itself now.
        let mut active = map(7);
        absorb(&mut active, Report::Ended { token: 6 });
        assert!(active.contains_key(&7), "that ending belongs to a run that is already gone");

        absorb(&mut active, Report::Ended { token: 7 });
        assert!(!active.contains_key(&7), "and the one that owns it does end it");
    }

    #[test]
    fn a_stopped_run_holds_its_scope_until_its_loop_reports_the_ending() {
        // The stop is immediate on screen, the scope stays taken while the
        // loop winds down, and it is the ending — not the stop — that frees
        // it. The rest of the project is not held with it: another scope is
        // admitted straight through the winding down.
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.request_stop(false);
        assert!(active[&1].run.is_over(), "nothing was in flight, so the stop is immediate");

        assert_eq!(admit(&active, "/p", &RunScope::Queue), Err(RunError::WindingDown));
        assert_eq!(admit(&active, "/p", &task("a-1")), Ok(()), "another scope is not held by it");
        assert!(!permit(&mut active, 1), "and no batch goes out of a stopped run");

        absorb(&mut active, Report::Ended { token: 1 });
        assert_eq!(
            admit(&active, "/p", &RunScope::Queue),
            Ok(()),
            "the loop is gone, so the scope is free"
        );
    }

    #[test]
    fn a_second_run_of_the_same_scope_is_refused_and_the_refusal_names_it() {
        // The two refusals are different sentences on purpose: this one means
        // leave it alone, `WindingDown` means try again in a moment. And it
        // names what is in the way, because with several runs in a project "a
        // run is already going" no longer says which.
        let mut active = map(1);
        insert(&mut active, 2, "/p", task("a-1"));
        insert(&mut active, 3, "/p", epic("a-2"));

        assert_eq!(
            admit(&active, "/p", &RunScope::Queue),
            Err(RunError::AlreadyRunning { scope: "the queue".into() })
        );
        assert_eq!(
            admit(&active, "/p", &task("a-1")),
            Err(RunError::AlreadyRunning { scope: "task a-1".into() })
        );
        assert_eq!(
            admit(&active, "/p", &epic("a-2")),
            Err(RunError::AlreadyRunning { scope: "epic a-2".into() })
        );
    }

    #[test]
    fn a_different_scope_runs_beside_this_projects_other_runs() {
        // A queue run beside a task run, and runs over different ids, divide
        // the board rather than fight over it — which tasks each may touch is
        // bd's atomic claim to arbitrate, not this map's.
        let mut active = map(1);
        insert(&mut active, 2, "/p", task("a-1"));

        assert_eq!(admit(&active, "/p", &task("a-2")), Ok(()));
        assert_eq!(admit(&active, "/p", &epic("a-9")), Ok(()));
        assert_eq!(admit(&active, "/elsewhere", &RunScope::Queue), Ok(()), "another project is nobody's business");
    }

    #[test]
    fn state_answers_every_run_the_project_holds_oldest_first() {
        let mut active = map(4);
        insert(&mut active, 2, "/p", task("a-1"));
        insert(&mut active, 3, "/elsewhere", RunScope::Queue);

        let runs = runs_in(&active, "/p");
        assert_eq!(runs.iter().map(|r| r.token).collect::<Vec<_>>(), vec![2, 4]);
        assert!(runs.iter().all(|r| r.project == "/p"), "another project's run is not in it");
    }

    /// The updater's gate reads this, and the case it exists for is a run in a
    /// project the person is not looking at — the one the front end cannot see,
    /// since `runs.js` is filtered to the active project.
    #[test]
    fn the_live_projects_are_every_project_in_the_map_and_not_one_of_them() {
        assert!(live_projects(&HashMap::new()).is_empty(), "an idle app installs freely");

        let mut active = map(1);
        insert(&mut active, 2, "/p", task("a-1"));
        insert(&mut active, 3, "/elsewhere", RunScope::Queue);

        assert_eq!(
            live_projects(&active),
            vec!["/elsewhere".to_string(), "/p".to_string()],
            "one entry per project, however many runs it holds"
        );
    }

    /// A stop is not an ending: the batch it asked to finish is still a process,
    /// so the project stays in the list until the loop task reports itself gone.
    /// The same rule the power assertion is held under.
    #[test]
    fn a_run_winding_down_is_still_a_live_project() {
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.session = Some(9);
        active.get_mut(&1).expect("the entry").run.request_stop(false);
        assert_eq!(live_projects(&active), vec!["/p".to_string()]);

        absorb(&mut active, Report::Ended { token: 1 });
        assert!(live_projects(&active).is_empty(), "and the ending is what frees it");
    }

    #[test]
    fn stopping_one_run_leaves_the_projects_other_run_going() {
        // The acceptance criterion at map level: the stop reaches exactly the
        // run it names, and the neighbour's next batch is still authorized.
        let mut active = map(1);
        insert(&mut active, 2, "/p", task("a-1"));
        active.get_mut(&1).expect("the entry").run.session = Some(9);
        active.get_mut(&1).expect("the entry").run.request_stop(false);

        assert!(!permit(&mut active, 1), "the stopped run takes no further batch");
        assert!(permit(&mut active, 2), "and the other run never hears about it");
    }

    #[test]
    fn a_report_clears_the_starting_flag_so_the_next_stop_is_not_held_by_it() {
        // `starting` stands in for a session id that does not exist yet. Left
        // set after the loop has reported, it would make a stop between batches
        // wait for a batch that is not there.
        let mut active = map(1);
        assert!(permit(&mut active, 1));

        let mut reported = Run::new(1, "/p".into(), settings(RunScope::Queue));
        reported.advance(RunState::Deciding);
        absorb(&mut active, state(1, &reported));
        assert!(!active[&1].starting);
    }

    #[test]
    fn a_report_from_an_ended_run_changes_nothing_and_is_not_emitted() {
        // The token guard in its new form: the ended run's entry is gone, so
        // its late report finds nothing — and the run that started after it in
        // the same project is not written over (the map used to key by project,
        // where exactly that could happen).
        let mut active = map(7);
        let mut reported = Run::new(6, "/p".into(), settings(RunScope::Queue));
        reported.advance(RunState::Working { iteration: 3 });

        assert!(absorb(&mut active, state(6, &reported)).is_none(), "nothing to put on the wire");
        assert_eq!(active[&7].run.state, RunState::Preflight, "and nothing written either");
    }

    #[test]
    fn a_report_does_not_revive_a_run_the_worker_has_already_stopped() {
        // The loop's copy knows nothing of a stop until it next looks at the
        // channel, so it keeps reporting progress for a moment afterwards.
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.request_stop(false);

        let mut reported = Run::new(1, "/p".into(), settings(RunScope::Queue));
        reported.advance(RunState::Working { iteration: 1 });
        assert!(absorb(&mut active, state(1, &reported)).is_none(), "nothing goes on the wire");
        assert!(active[&1].run.is_over(), "and a finished run does not come back on screen");
    }

    /// A loop task's ending, as it comes back from `finish`: stopped, and
    /// carrying the account only that helper ever makes.
    fn ended(token: u64, reason: StopReason, report: &str) -> Run {
        let mut run = Run::new(token, "/p".into(), settings(RunScope::Queue));
        run.summary = Some(RunSummary {
            seconds: 42,
            tasks: Some(summary::Tasks::default()),
            report: Some(report.into()),
        });
        run.advance(RunState::Stopped { reason });
        run
    }

    #[test]
    fn a_run_stopped_with_nothing_in_flight_still_gets_its_account() {
        // The immediate branch of `request_stop` — stop while deciding, while
        // paused overnight on a spent allowance, or during the preflight — ends
        // the run on this side before the loop has looked at the channel. Only
        // the loop runs `finish`, so its report is the one and only place the
        // document's path ever comes from: dropped, the file sits on disk
        // correctly written while the run on the wire says there is no report.
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.request_stop(false);
        assert!(active[&1].run.is_over(), "nothing was in flight, so the stop was immediate");
        assert!(active[&1].run.summary.is_none(), "and this side never made an account");

        let reported = ended(1, StopReason::Cancelled, "/p/.smetana/reports/x.html");
        let emitted = absorb(&mut active, state(1, &reported))
            .expect("the account has to reach the front end or the document is unreachable");

        assert_eq!(
            emitted.summary.as_ref().and_then(|s| s.report.as_deref()),
            Some("/p/.smetana/reports/x.html")
        );
        assert_eq!(emitted.summary.as_ref().expect("a summary").seconds, 42);
    }

    #[test]
    fn a_late_account_cannot_change_how_a_stopped_run_ended() {
        // The summary and nothing else. Somebody pressed stop and was told
        // Cancelled; the loop may have reached the board a moment later and
        // found the queue empty, and rewriting the ending under them would put
        // a different run's story on the bar.
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.request_stop(false);

        let mut reported = ended(1, StopReason::QueueEmpty, "/p/.smetana/reports/x.html");
        reported.batches = 5;
        let emitted = absorb(&mut active, state(1, &reported)).expect("the account still crosses");

        assert_eq!(emitted.state, RunState::Stopped { reason: StopReason::Cancelled });
        assert_eq!(emitted.batches, 0, "nothing but the account crosses");
        assert!(emitted.summary.is_some());
    }

    #[test]
    fn an_account_is_taken_once_and_a_later_report_changes_nothing() {
        // The guard is on the field rather than on a flag: once the run holds
        // an account, another report is the past again and is not emitted, so
        // the front end is not sent the same finished run twice.
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.request_stop(false);
        let first = ended(1, StopReason::Cancelled, "/p/.smetana/reports/x.html");
        assert!(absorb(&mut active, state(1, &first)).is_some());

        let second = ended(1, StopReason::Cancelled, "/p/.smetana/reports/other.html");
        assert!(absorb(&mut active, state(1, &second)).is_none(), "nothing to put on the wire");
        assert_eq!(
            active[&1].run.summary.as_ref().and_then(|s| s.report.as_deref()),
            Some("/p/.smetana/reports/x.html"),
            "and the first account stands"
        );
    }

    #[test]
    fn a_report_is_emitted_with_the_stop_this_side_asked_for_still_on_it() {
        // `adopt` through the path that actually runs: a cooperative stop, then
        // the loop's next report, then the check that guards the next batch.
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.session = Some(4);
        active.get_mut(&1).expect("the entry").run.request_stop(false);

        let mut reported = Run::new(1, "/p".into(), settings(RunScope::Queue));
        reported.advance(RunState::Deciding);
        let emitted = absorb(&mut active, state(1, &reported)).expect("a run to put on the wire");

        assert_eq!(emitted.state, RunState::Deciding, "where the loop is, is the loop's to say");
        assert!(emitted.stopping, "and whether it was asked to stop is not");
        assert!(!permit(&mut active, 1), "so the next batch is still refused");
    }

    #[test]
    fn two_runs_ending_in_the_same_second_keep_two_documents() {
        // A project holds several runs at once, so one timestamp is not one
        // run. Losing a night's record without a word is worse than an ugly
        // file name, and the creation itself is the exclusive step because the
        // two runs are on two loop tasks.
        let dir = std::env::temp_dir()
            .join(format!("smetana-reports-{}-same-second", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory to write into");

        let (first, _) = claim_report(&dir, "2026-08-12-143155").expect("the first document");
        let (second, _) = claim_report(&dir, "2026-08-12-143155").expect("the second document");

        assert_ne!(first, second, "the second run must not overwrite the first");
        assert_eq!(first.file_name().expect("a name"), "2026-08-12-143155.html");
        assert_eq!(second.file_name().expect("a name"), "2026-08-12-143155-2.html");
        assert!(first.exists() && second.exists(), "both are claimed on disk, not just named");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_batch_hands_back_by_leaving_an_account_that_parses() {
        // What ends a batch in an attended mode. The process does not exit
        // there — a person is sitting in front of it and that is the whole
        // point of the mode — so the signal is the account the lead was asked
        // to write, and the signal is deliberately **a file that parses**
        // rather than a file that exists.
        //
        // JSON is not written atomically. Waking on the first byte would hand
        // `read_batch` half a document a moment later, and the report would say
        // the batch left no account of itself in exactly the case where it left
        // a good one. Parsing is that check, and there is no second mechanism
        // to keep in step with it.
        let dir =
            std::env::temp_dir().join(format!("smetana-handback-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory to write into");

        assert!(!handed_back(&dir, 1), "nothing written yet is not a hand-back");

        let path = dir.join("batch-1.json");
        std::fs::write(&path, "{\"tasks\": [{\"id\": \"a-1\", \"di").expect("a partial write");
        assert!(!handed_back(&dir, 1), "a document caught mid-write is not a hand-back");

        std::fs::write(&path, "{\"tasks\": [{\"id\": \"a-1\", \"did\": \"closed it\"}]}")
            .expect("the whole account");
        assert!(handed_back(&dir, 1), "the account is complete, so the work is handed back");

        assert!(!handed_back(&dir, 2), "and it says nothing about any other batch");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_batch_starts_with_no_account_left_by_a_previous_app_process() {
        // The trap under the rule above, and it is certain rather than
        // theoretical: `token` counts from zero on every app start, so this
        // run's directory is `.smetana/runs/1` and so was the one two launches
        // ago. Without this, a batch would hand back in the same instant it
        // spawned — on a file written by somebody else's night.
        //
        // Clearing rather than remembering the file's age: a leftover account
        // belongs to a run that is over and whose document was written long
        // ago, so there is nothing there for anybody to read. It also fixes the
        // quieter half for every mode — `read_batch` would otherwise put a
        // previous launch's prose into this run's report for a batch that
        // crashed before writing anything.
        let dir = std::env::temp_dir().join(format!("smetana-stale-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory to write into");
        std::fs::write(dir.join("batch-1.json"), "{\"tasks\": []}").expect("a previous account");
        assert!(handed_back(&dir, 1), "the leftover is a document that parses");

        clear_account(&dir, 1);

        assert!(!handed_back(&dir, 1), "so this batch has handed nothing back yet");
        clear_account(&dir, 1); // nothing there is an ordinary outcome, not an error

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_batchs_record_carries_the_ending_the_loop_saw_even_with_no_file_to_read() {
        // smetana-pmj. The account is the agent's and a killed agent writes
        // none; the ending is the loop's and it always has one. Reading them
        // together is what stops the document going silent in the one case
        // somebody opens it for.
        let dir = std::env::temp_dir().join(format!("smetana-outcome-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory to write into");

        let killed = read_batch(&dir, 1, 90, BatchOutcome::NoCode);
        assert!(!killed.reported, "nothing on disk is still no account");
        assert_eq!(killed.outcome, BatchOutcome::NoCode, "and the run's own half is there anyway");
        assert!(killed.left_behind.is_empty(), "the board is the caller's to ask about");

        std::fs::write(dir.join("batch-2.json"), "{\"tasks\": [], \"notes\": \"fine\"}")
            .expect("an account");
        let spoke = read_batch(&dir, 2, 90, BatchOutcome::Exited);
        assert!(spoke.reported, "a file that parses is an account");
        assert_eq!(spoke.outcome, BatchOutcome::Exited, "and the two halves stand together");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn every_ending_the_loop_can_reach_has_a_word_of_its_own() {
        // The one translation between what the loop holds and what the document
        // draws, and the only judgement in it is the split between a zero code
        // and any other number — the same reading the loop makes when it decides
        // whether to count a crash. `Exit::Removed` is a person's doing and
        // `NoCode` is a signal, and the document has to be able to tell somebody
        // which of the two took their night.
        assert_eq!(outcome_of(&Batch::Ended(Exit::Code(0))), BatchOutcome::Exited);
        assert_eq!(
            outcome_of(&Batch::Ended(Exit::Code(137))),
            BatchOutcome::Failed { code: 137 },
            "the number travels: a 137 and a 1 send somebody to different places"
        );
        assert_eq!(outcome_of(&Batch::Ended(Exit::NoCode)), BatchOutcome::NoCode);
        assert_eq!(outcome_of(&Batch::Ended(Exit::Removed)), BatchOutcome::Removed);
        assert_eq!(outcome_of(&Batch::HandedBack), BatchOutcome::HandedBack);
        assert_eq!(
            outcome_of(&Batch::Unanswered { question: "Trust this folder?".into() }),
            BatchOutcome::Unanswered { question: "Trust this folder?".into() },
            "the question is the whole of what the run knows about why"
        );
    }

    #[test]
    fn browser_busy_counts_runs_and_the_asking_project_is_among_them() {
        // The other half of what this task changed about the browser question:
        // a live-check run in this very project is exactly what holds
        // Playwright's one profile against a second run beside it, so the
        // asking project is no longer filtered out — and a project appears
        // once however many of its runs want the browser.
        let mut active = map(1);
        insert(&mut active, 2, "/p", task("a-1"));
        insert(&mut active, 3, "/elsewhere", RunScope::Queue);

        assert_eq!(browser_candidates(&active), vec!["/elsewhere".to_string(), "/p".to_string()]);
    }

    #[test]
    fn a_run_that_is_over_or_never_wanted_the_browser_is_not_a_candidate() {
        // `is_over` and not `stopping`: a run winding down still has its batch
        // in flight, and that batch still has the browser.
        let mut active = map(1);
        active.get_mut(&1).expect("the entry").run.request_stop(false);
        insert(&mut active, 2, "/p", task("a-1"));
        active.get_mut(&2).expect("the entry").run.settings.live_check = false;

        assert_eq!(browser_candidates(&active), Vec::<String>::new());
    }
}
