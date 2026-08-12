use std::path::PathBuf;
use std::time::Duration;

use notify::RecommendedWatcher;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, MissedTickBehavior};

use crate::project;

use super::bd::Bd;
use super::model::{Delta, Health, HealthState, Issue, IssuePatch, Snapshot, TrackerError};
use super::store::Store;
use super::watcher::{self, WatchEvent};

/// The expected bd version. Kept in step with BD_VERSION in
/// scripts/fetch-bd.mjs — a mismatch shows up in health.
const EXPECTED_BD_VERSION: &str = "1.1.2";
/// Writes arrive in bursts; we wait for the stream to settle.
const DEBOUNCE: Duration = Duration::from_millis(250);
/// The safety full sweep: it catches deletions and missed events.
const FULL_RESYNC: Duration = Duration::from_secs(60);
/// Slack for updated_at being rounded to the second. A miss costs more than a
/// repeat, and the diff is idempotent.
const OVERLAP_SECONDS: i64 = 5;

pub enum Request {
    Health(oneshot::Sender<Health>),
    Snapshot(oneshot::Sender<Snapshot>),
    /// The folder being watched, **how well it is being read**, and the board
    /// it holds — as one answer.
    ///
    /// Three facts rather than three questions, because the caller is
    /// `attachments`, deciding which of a project's stored pictures nothing
    /// refers to any more, and every pair of them can be made to disagree by
    /// asking separately. A project switch between two calls would have one
    /// project's board deciding what to delete from another project's folder.
    /// And health is here rather than left to `tracker_health` for a sharper
    /// reason: an empty snapshot means "this board holds nothing" or "this
    /// board could not be read", the two are indistinguishable in `Snapshot`,
    /// and reading the second as the first deletes the attachments of every
    /// live task in the project. Health is the only thing that tells them
    /// apart, so it travels in the same message as the emptiness it explains.
    Current(oneshot::Sender<(Option<PathBuf>, Health, Snapshot)>),
    SetProject(Option<PathBuf>, oneshot::Sender<Snapshot>),
    InitTracker(oneshot::Sender<Result<Snapshot, TrackerError>>),
    Resync(oneshot::Sender<Result<Snapshot, TrackerError>>),
    Update(String, IssuePatch, oneshot::Sender<Result<Issue, TrackerError>>),
    Close(String, Option<String>, oneshot::Sender<Result<Issue, TrackerError>>),
    Reopen(String, oneshot::Sender<Result<Issue, TrackerError>>),
    Delete(String, oneshot::Sender<Result<(), TrackerError>>),
}

#[derive(Clone)]
pub struct TrackerHandle(pub mpsc::Sender<Request>);

/// The directory the worker is currently looking at.
struct Project {
    dir: PathBuf,
    /// A folder without a tracker needs bd too: init is done with it.
    bd: Bd,
    /// Watching lives only while the directory has a tracker. The field keeps
    /// the watcher alive — dropping it stops the watching silently.
    _watcher: Option<RecommendedWatcher>,
    /// Whether `.beads` was there when the directory was opened.
    tracked: bool,
}

/// Reading and writing are only possible where a tracker exists.
fn tracked(current: &Option<Project>) -> Option<&Bd> {
    current.as_ref().filter(|p| p.tracked).map(|p| &p.bd)
}

/// The worker's health: the current value (the same one `tracker_health`
/// returns) and the two persistent troubles it is composed of.
///
/// Troubles come in three kinds and must not be confused. A one-off bd failure
/// clears itself: the next successful call is the proof that things work again.
/// "Wrong bd version" is about the binary: it survives both a successful
/// `bd list` and a project switch. "No .beads directory", "the watcher died",
/// "no project selected" are about the open folder: they have to survive a
/// successful call, but die together with the project they belonged to.
struct HealthReporter {
    app: AppHandle,
    current: Health,
    bd: Option<Health>,
    project: Option<Health>,
}

impl HealthReporter {
    fn new(app: AppHandle) -> Self {
        Self { app, current: Health { state: HealthState::Ok, message: None }, bd: None, project: None }
    }

    fn current(&self) -> Health {
        self.current.clone()
    }

    /// A persistent trouble with the binary: it survives everything, including
    /// a project switch.
    fn degrade_bd(&mut self, state: HealthState, message: String) {
        self.bd = Some(Health { state, message: Some(message) });
        self.set(self.baseline());
    }

    /// A persistent trouble with the open folder: it lives exactly as long as
    /// that folder is open.
    fn degrade_project(&mut self, state: HealthState, message: String) {
        self.project = Some(Health { state, message: Some(message) });
        self.set(self.baseline());
    }

    /// Another folder was opened — the previous one's troubles have nothing to
    /// do with it.
    fn clear_project(&mut self) {
        self.project = None;
        self.set(self.baseline());
    }

    /// A one-off failure of a bd call.
    fn failed(&mut self, e: &TrackerError) {
        self.set(Health { state: HealthState::Error, message: Some(e.to_string()) });
    }

    /// bd worked: clear the one-off failure, but not the persistent trouble.
    fn recovered(&mut self) {
        self.set(self.baseline());
    }

    /// A folder's trouble outranks the binary's: it is more specific, and there
    /// is something a person can do about it right now.
    fn baseline(&self) -> Health {
        self.project
            .clone()
            .or_else(|| self.bd.clone())
            .unwrap_or(Health { state: HealthState::Ok, message: None })
    }

    /// Health is both remembered and broadcast: the event is the fast path for
    /// whoever is already listening, and the stored value is the answer for
    /// whoever did not manage to subscribe in time and asks with the
    /// tracker_health command. The event only fires on a change of value:
    /// health on every successful tick is noise that hides the real trouble.
    fn set(&mut self, next: Health) {
        if self.current == next {
            return;
        }
        self.current = next;
        let _ = self.app.emit("tracker:health", self.current.clone());
    }
}

/// There is nowhere to write in a folder without a tracker. This is not a bd
/// launch failure — we never even tried to call bd — hence the separate error.
fn no_tracker(health: &HealthReporter) -> TrackerError {
    TrackerError::NoTracker(
        health
            .current
            .message
            .clone()
            .unwrap_or_else(|| "the tracker is unavailable".to_string()),
    )
}

fn emit_delta(app: &AppHandle, delta: Delta) {
    if !delta.is_empty() {
        let _ = app.emit("tracker:delta", delta);
    }
}

/// updated_at is rounded to the second, so we ask with slack.
fn since_with_overlap(last_seen: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(last_seen) {
        Ok(t) => (t - chrono::Duration::seconds(OVERLAP_SECONDS))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        Err(_) => "1970-01-01T00:00:00Z".to_string(),
    }
}

/// A failure leaves through both channels: as an event for whoever is
/// listening, and as a value for whoever called tracker_resync and is awaiting
/// an answer. Columns and issues are independent, so we ask for both and hand
/// out the first error. Success is an event too: without it one failed bd call
/// would leave health in `error` for the rest of the process's life.
async fn full_sync(
    app: &AppHandle,
    bd: &Bd,
    store: &mut Store,
    health: &mut HealthReporter,
) -> Result<(), TrackerError> {
    let columns = match bd.columns().await {
        Ok(columns) => {
            if store.set_columns(columns) {
                emit_delta(app, store.columns_delta());
            }
            Ok(())
        }
        Err(e) => Err(e),
    };
    let issues = match bd.list_all().await {
        Ok(issues) => {
            emit_delta(app, store.apply_full(issues));
            Ok(())
        }
        Err(e) => Err(e),
    };

    let result = columns.and(issues);
    match &result {
        Ok(()) => health.recovered(),
        Err(e) => health.failed(e),
    }
    result
}

async fn incremental_sync(app: &AppHandle, bd: &Bd, store: &mut Store, health: &mut HealthReporter) {
    let since = since_with_overlap(store.last_seen());
    match bd.list_updated_after(&since).await {
        Ok(issues) => {
            emit_delta(app, store.apply_incremental(issues));
            health.recovered();
        }
        Err(e) => health.failed(&e),
    }
}

/// Opens a directory: clears the snapshot, brings up bd and the watcher, sets
/// health and does the first sweep.
///
/// Deltas go out as usual all the while, starting with the reset itself — the
/// snapshot for whoever called the command is assembled afterwards. For the
/// duration of the switch the front end stops listening to deltas and takes the
/// command's answer wholesale; otherwise the new project's issues would land on
/// top of the old project's.
async fn open(
    app: &AppHandle,
    dir: Option<PathBuf>,
    store: &mut Store,
    health: &mut HealthReporter,
    tx_tick: &mpsc::Sender<WatchEvent>,
) -> Option<Project> {
    // First of all, before anything else: a reset is a delta too, and the gap
    // in numbering it carries has to leave before any other.
    emit_delta(app, store.reset());

    let Some(dir) = dir else {
        health.degrade_project(HealthState::NoProject, "no project selected".to_string());
        return None;
    };

    let bd = Bd::new(app.clone(), dir.clone());

    if !project::has_tracker(&dir) {
        health.degrade_project(
            HealthState::NotABeadsRepo,
            format!("no .beads directory in {}", dir.display()),
        );
        // The folder stays open anyway: bd init is done in it.
        return Some(Project { dir, bd, _watcher: None, tracked: false });
    }

    health.clear_project();

    // Watching is not a condition of working: without it the once-a-minute
    // sweep remains, and more than the log should know about that.
    let watcher = match watcher::spawn(dir.join(".beads"), tx_tick.clone()) {
        Ok(w) => Some(w),
        Err(e) => {
            health.degrade_project(
                HealthState::Error,
                format!("could not watch .beads: {e}; only the periodic sweep remains"),
            );
            None
        }
    };

    let project = Project { dir, bd, _watcher: watcher, tracked: true };
    // Nobody awaits the first sweep: health has already told the story of a failure.
    let _ = full_sync(app, &project.bd, store, health).await;
    Some(project)
}

/// The bd version is a property of the binary, not of a folder: we ask once per
/// launch and from any working directory — `bd --version` does not read the tracker.
async fn check_version(app: &AppHandle, health: &mut HealthReporter) {
    let probe = Bd::new(app.clone(), std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    match probe.version().await {
        Ok(Some(version)) if version == EXPECTED_BD_VERSION => {}
        Ok(other) => health.degrade_bd(
            HealthState::BdVersionMismatch,
            format!("expected bd version {EXPECTED_BD_VERSION}, got {other:?}"),
        ),
        Err(e) => health.failed(&e),
    }
}

/// The only place with mutable state — and it is single-threaded. A bd call
/// costs about two seconds, so a request queue gives a comprehensible order
/// instead of unpredictable blocking on a mutex.
pub fn start(app: AppHandle, initial: Option<PathBuf>) -> TrackerHandle {
    let (tx_req, mut rx_req) = mpsc::channel::<Request>(32);
    let (tx_tick, mut rx_tick) = mpsc::channel::<WatchEvent>(16);

    tauri::async_runtime::spawn(async move {
        let mut store = Store::default();
        let mut health = HealthReporter::new(app.clone());

        check_version(&app, &mut health).await;
        let mut current = open(&app, initial, &mut store, &mut health, &tx_tick).await;

        let mut ticker = tokio::time::interval(FULL_RESYNC);
        // By default missed ticks (the machine slept, a bd call dragged on) fire
        // back to back — several two-second full sweeps one after another right
        // after a stall. Delay shifts the schedule instead.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await; // the first one fires instantly

        // The deadline for the deferred catch-up is loop state, not an await
        // inside a branch: while the debounce runs, the worker must keep
        // answering commands.
        let mut due: Option<Instant> = None;

        loop {
            tokio::select! {
                request = rx_req.recv() => {
                    // The senders are gone — there is no front end left, and
                    // nobody to work for.
                    let Some(request) = request else { break };
                    let switched = handle(&app, &mut current, &mut store, &mut health, &tx_tick, request).await;
                    if switched {
                        // The folder changed: the previous project's deadline is
                        // void, and the accumulated watcher ticks belong partly
                        // to a folder that is already gone and partly to the new
                        // one, which has already had its full sweep inside
                        // open(). We drain them here rather than in handle:
                        // there is no access to rx_tick there, it is taken by a
                        // neighbouring branch of this select!. The cost: a change
                        // that happened in the folder during the ~2 seconds the
                        // opening took may go with them — the sixty-second safety
                        // sweep will pick it up.
                        due = None;
                        while rx_tick.try_recv().is_ok() {}
                    }
                }
                Some(event) = rx_tick.recv() => match event {
                    // The first event sets the deadline and the rest stick to it:
                    // a burst of writes collapses into one catch-up.
                    WatchEvent::Changed => { due.get_or_insert_with(|| Instant::now() + DEBOUNCE); }
                    WatchEvent::Failed(message) => health.degrade_project(
                        HealthState::Error,
                        format!(
                            "watching .beads stopped: {message}; \
                             only the periodic sweep remains"
                        ),
                    ),
                },
                // The default value is never used: with no deadline the branch
                // is disabled, and select! evaluates the condition before the
                // expression.
                _ = tokio::time::sleep_until(due.unwrap_or_else(Instant::now)), if due.is_some() => {
                    due = None;
                    if let Some(bd) = tracked(&current) {
                        incremental_sync(&app, bd, &mut store, &mut health).await;
                    }
                }
                _ = ticker.tick() => {
                    // Nobody awaits the periodic sweep either.
                    if let Some(bd) = tracked(&current) {
                        let _ = full_sync(&app, bd, &mut store, &mut health).await;
                    }
                }
            }
        }
    });

    TrackerHandle(tx_req)
}

/// There are two reasons a tracker may be missing — no project is selected, or
/// the folder has no `.beads`. Either way the state can be reported; there is
/// nothing to change.
///
/// Returns `true` if the folder changed: the calling loop then has to forget the
/// deferred catch-up deadline and drain the accumulated watcher ticks — both
/// belong to a folder that may already be gone.
async fn handle(
    app: &AppHandle,
    current: &mut Option<Project>,
    store: &mut Store,
    health: &mut HealthReporter,
    tx_tick: &mpsc::Sender<WatchEvent>,
    request: Request,
) -> bool {
    match request {
        Request::Health(reply) => {
            let _ = reply.send(health.current());
            false
        }
        Request::Snapshot(reply) => {
            let _ = reply.send(store.snapshot());
            false
        }
        Request::Current(reply) => {
            // The directory whatever its state — a folder with no `.beads` in
            // it is still the project somebody has open, and its stored
            // pictures are still its own. What that folder's health says about
            // the board is sent beside it precisely because the two differ
            // there: the pictures belong to the project, and with no tracker
            // nothing can vouch for a single one of them.
            let dir = current.as_ref().map(|p| p.dir.clone());
            let _ = reply.send((dir, health.current(), store.snapshot()));
            false
        }
        Request::SetProject(dir, reply) => {
            // Drop the previous project before opening the new one: otherwise
            // its watcher (inside Project) stays alive for the whole ~2 seconds
            // open() takes, and an event from the old folder would land in the
            // new one's health.
            *current = None;
            *current = open(app, dir, store, health, tx_tick).await;
            let _ = reply.send(store.snapshot());
            true
        }
        Request::InitTracker(reply) => {
            let result = match current.as_ref() {
                Some(p) if !p.tracked => p.bd.init().await,
                Some(_) => Err(TrackerError::NoTracker("this folder already has a tracker".into())),
                None => Err(TrackerError::NoTracker("no project selected".into())),
            };
            match result {
                Ok(()) => {
                    let dir = current.as_ref().map(|p| p.dir.clone());
                    // The same order as in SetProject: the old Project (with no
                    // tracker, so there was no watcher either) is dropped before open().
                    *current = None;
                    *current = open(app, dir, store, health, tx_tick).await;
                    let _ = reply.send(Ok(store.snapshot()));
                    true
                }
                // Health is deliberately left alone: "there is no tracker here"
                // is still true, and the board's place should keep the button
                // rather than "bd is broken". The command's answer tells the
                // story of the failure. The folder was not reopened — there is
                // no switch to signal.
                Err(e) => {
                    let _ = reply.send(Err(e));
                    false
                }
            }
        }
        Request::Resync(reply) => {
            let result = match tracked(current) {
                Some(bd) => full_sync(app, bd, store, health).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(result.map(|()| store.snapshot()));
            false
        }
        Request::Update(id, patch, reply) => {
            let result = match tracked(current) {
                Some(bd) => bd.update(&id, &patch).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
            false
        }
        Request::Close(id, reason, reply) => {
            let result = match tracked(current) {
                Some(bd) => bd.close(&id, reason.as_deref()).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
            false
        }
        Request::Reopen(id, reply) => {
            let result = match tracked(current) {
                Some(bd) => bd.reopen(&id).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
            false
        }
        Request::Delete(id, reply) => {
            let result = match tracked(current) {
                Some(bd) => bd.delete(&id).await,
                None => Err(no_tracker(health)),
            };
            if result.is_ok() {
                emit_delta(app, store.remove_one(&id));
            }
            let _ = reply.send(result);
            false
        }
    }
}

/// The result of our own write goes into the snapshot at once, without waiting
/// for the watcher: the tick that follows will produce an empty diff.
fn finish(
    app: &AppHandle,
    store: &mut Store,
    result: Result<Issue, TrackerError>,
) -> Result<Issue, TrackerError> {
    if let Ok(issue) = &result {
        emit_delta(app, store.upsert_one(issue.clone()));
    }
    result
}
