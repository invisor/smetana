use std::path::{Path, PathBuf};
use std::time::Duration;

use notify::RecommendedWatcher;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, MissedTickBehavior};

use crate::project;

use super::access;
use super::backup;
use super::bd::Bd;
use super::model::{
    Delta, Failure, Health, HealthState, Issue, IssuePatch, Repair, Snapshot, TrackerError,
};
use super::store::Store;
use super::watcher::{self, WatchEvent};

/// The expected bd version. Kept in step with BD_VERSION in
/// scripts/fetch-bd.mjs — a mismatch shows up in health.
///
/// `pub` because the briefing a repair session is started with names it, and
/// naming it from one constant is what stops the agent being told about a bd
/// this build does not ship.
pub const EXPECTED_BD_VERSION: &str = "1.1.2";
/// Writes arrive in bursts; we wait for the stream to settle.
const DEBOUNCE: Duration = Duration::from_millis(250);
/// The safety full sweep: it catches deletions and missed events.
const FULL_RESYNC: Duration = Duration::from_secs(60);
/// Slack for updated_at being rounded to the second. A miss costs more than a
/// repeat, and the diff is idempotent.
const OVERLAP_SECONDS: i64 = 5;

/// Which half of [`Request::BoardAt`] answered — the live store, or a bd call
/// made in the folder that was asked about.
///
/// Carried back rather than kept quiet because it is exactly the fact a journal
/// reader wants weeks later: `Direct` says the folder this board was read for
/// was **not** the one the app window had open at that moment, which is the
/// condition that used to end a run on a stranger's empty queue with nothing on
/// the record to say so.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoardSource {
    Cache,
    Direct,
}

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
    /// Take a copy of `.beads` and run bd's own two migrations over it, then
    /// reopen the folder. Offered for **any** tracker failure rather than for a
    /// diagnosis, because there is no diagnosis to be had — `Bd::repair`
    /// records the measurements.
    Repair(oneshot::Sender<Result<Repair, TrackerError>>),
    /// The whole of the last tracker failure, for the agent that is being asked
    /// to look at it. A read: nothing is called and nothing is written.
    Failure(oneshot::Sender<Failure>),
    Resync(oneshot::Sender<Result<Snapshot, TrackerError>>),
    /// The board of a **named folder**, as issues — the request a run makes.
    ///
    /// Every other read here answers about the project the worker currently
    /// holds, which is whatever the app window is showing. A run is not the app
    /// window: it was started against one folder and lives for hours, and a
    /// person switching project mid-run had its next read answered from a
    /// stranger's board — an empty queue, and a night that stopped a batch
    /// early with two unblocked tasks left on the board (smetana-ynyc).
    ///
    /// Where `dir` is the folder the worker holds, this is the same live store
    /// every other reader gets, and `fresh` is the resync [`Request::Resync`]
    /// performs. Where it is any other folder, the answer is a one-off
    /// `bd list` in that folder: no watcher, no cache, no store — there is
    /// nothing on screen for it to update — and `fresh` is already true of it
    /// by construction.
    BoardAt {
        dir: PathBuf,
        fresh: bool,
        reply: oneshot::Sender<Result<(Vec<Issue>, BoardSource), TrackerError>>,
    },
    Update(String, IssuePatch, oneshot::Sender<Result<Issue, TrackerError>>),
    /// One `bd update` in a **named folder** — the write half of
    /// [`Request::BoardAt`], and it exists for the same reason.
    ///
    /// A run parks its stuck batch's claims and gives a dead batch's work back,
    /// and both were addressed at whatever project the app window was showing.
    /// A park that lands on another project's board is a claim left
    /// `in_progress` under a dead actor on this one, and a stray write on that
    /// one.
    ///
    /// Where `dir` is the folder the worker holds, this is [`Request::Update`]
    /// exactly: the same `Bd`, and the store takes the delta so the board on
    /// screen redraws. Where it is any other folder, the write is made and the
    /// store is left alone — nothing on screen is showing that board.
    UpdateAt {
        dir: PathBuf,
        id: String,
        patch: IssuePatch,
        reply: oneshot::Sender<Result<Issue, TrackerError>>,
    },
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

/// The worker's own `Bd`, but only if `dir` is the folder it is holding and
/// that folder can be read.
///
/// The one place the folder-addressed requests decide which half of themselves
/// answers, so `BoardAt` and `UpdateAt` cannot come to disagree about what
/// counts as "the folder we already have". A clone rather than a borrow because
/// both arms go on to touch the store and health, which the borrow would hold;
/// `Bd` is a folder and an app handle, so the copy costs nothing.
///
/// `tracked` is folded in for the same reason it gates every other call here:
/// a folder with no `.beads`, or one the system is refusing, has no board to
/// serve from a store that was never filled. Those fall to the direct call,
/// which fails in the caller's own folder and in bd's own words rather than in
/// this worker's.
fn own_bd(current: &Option<Project>, dir: &Path) -> Option<Bd> {
    current.as_ref().filter(|p| p.tracked && access::same_dir(dir, &p.dir)).map(|p| p.bd.clone())
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
    /// The last bd call that came back non-zero, as its argument list and its
    /// stderr. Kept beside health rather than folded into it, because it is
    /// remembered for a different reader: health is one sentence for the
    /// screen, and this is the briefing an agent is handed when the tracker
    /// itself is what is broken and cannot be asked again.
    ///
    /// It is cleared the moment bd works again, and that matters more than
    /// keeping it: a call that failed at ten o'clock and succeeded at five past
    /// describes nothing about the watcher dying at half past, and a briefing
    /// naming that stale command would send an agent at the wrong thing while
    /// the actual trouble appeared nowhere in it. `Request::Failure` puts the
    /// current health line in the briefing regardless, so nothing is lost by
    /// this being empty.
    last_command_failure: Option<(String, String)>,
}

impl HealthReporter {
    fn new(app: AppHandle) -> Self {
        Self {
            app,
            current: Health { state: HealthState::Ok, message: None },
            bd: None,
            project: None,
            last_command_failure: None,
        }
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
    ///
    /// `dir` is the folder the call was made in, and it is what tells a broken
    /// tracker apart from a folder the operating system is refusing to let this
    /// app open — see `access::health_for_failure`, which is where the decision
    /// is, and `HealthState::FolderRefused` for why the two must not arrive as
    /// one state. `None` is for the one caller that has no folder yet:
    /// `check_version` runs before anything is open.
    ///
    /// The remembered command is kept either way. It is an account of what bd
    /// was asked and what it said, and that stays true of a refused folder —
    /// bd failed there too, and for the same reason.
    fn failed(&mut self, e: &TrackerError, dir: Option<&std::path::Path>) {
        if let TrackerError::Command { command, stderr, .. } = e {
            self.last_command_failure = Some((command.clone(), stderr.clone()));
        }
        self.set(access::health_for_failure(dir, e));
    }

    /// bd worked: clear the one-off failure, but not the persistent trouble.
    ///
    /// The remembered command goes with it. It is an account of a failure, and
    /// a successful call is the proof that the account is out of date — the
    /// same reasoning that makes a one-off failure clear itself here.
    fn recovered(&mut self) {
        self.last_command_failure = None;
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
        Err(e) => health.failed(e, Some(bd.cwd())),
    }
    result
}

/// Close what somebody merged by hand.
///
/// **Why the app does this at all.** A task merged through the app is closed by
/// the agent that merged it, as the last step of `merging`. A person who merges
/// the branch themselves — in a terminal, in another tool, on a fast-forward
/// that leaves no merge commit at all — closes nothing, and the task sits in
/// `ready_to_merge` on the board for ever while the work is in the target
/// branch and, in the case this was written for, already on a staging
/// deployment. bd cannot see it either: `bd orphans` looks for the id in commit
/// messages, and a fast-forward puts it in none.
///
/// **`ready_to_merge` and no other status** (`store::ids_with_status`). A task
/// in `open` or `in_progress` may well have a branch carrying its slug — half
/// merged, or cut for a different attempt at the same work — and closing that
/// one would throw away work nobody finished. A missed closure costs a minute;
/// a false one costs the work.
///
/// **The whole of the git side is `vcs::merged`**, which is where the ancestry
/// question and the multi-repository rule live and where their tests are.
/// `git.rs` finds the branch and spawns nothing, as its own header requires.
///
/// **On the blocking pool**, for the reason `vcs::commands::off_the_runtime`
/// gives: this worker is the one task answering every tracker command in the
/// app, and a git call parked on somebody's disk would hold the board, the task
/// panel and every write behind it. What comes back is a handful of ids, and
/// the closing itself is bd's own two seconds, which this worker already spends
/// on every write it makes.
///
/// **A failure is a log line and nothing else.** Health is deliberately left
/// alone: nobody asked for this write, so a failed one must not put a red line
/// under a board that is otherwise being read perfectly well — and a bd that is
/// genuinely broken is already saying so through the sweep above. The task
/// simply stays where it is and the next tick tries again, which is what makes
/// this idempotent rather than merely repeated.
async fn close_merged(app: &AppHandle, current: &Option<Project>, store: &mut Store) {
    let Some(project) = current.as_ref().filter(|p| p.tracked) else { return };
    let ids = store.ids_with_status(crate::runs::queue::READY_TO_MERGE);
    if ids.is_empty() {
        return;
    }
    let Some(target) = target_branch(app, &project.dir) else { return };

    let dir = project.dir.clone();
    let aimed_at = target.clone();
    let found =
        tokio::task::spawn_blocking(move || crate::vcs::merged::merged_tasks(&dir, &aimed_at, &ids))
            .await;
    let found = match found {
        Ok(found) => found,
        // A panicked blocking task, or a runtime shutting down under it. The
        // empty answer would be a quiet lie — "nothing has been merged" — so it
        // is named where a developer will find it.
        Err(err) => {
            log::warn!("[tracker] the merged-branch sweep did not finish: {err}");
            return;
        }
    };

    for task in found {
        let reason = crate::vcs::merged::reason(&task, &target);
        match project.bd.close(&task.id, Some(&reason)).await {
            Ok(issue) => {
                log::info!("[tracker] closed {}: {reason}", task.id);
                emit_delta(app, store.upsert_one(issue));
            }
            Err(e) => log::warn!("[tracker] could not close {}: {e}", task.id),
        }
    }
}

/// Which branch this project's work is aimed at: what the run dialog was left
/// on here, then the project's own `[defaults] target_branch`.
///
/// The same two terms `components/run/branchChoice.js` takes, in the same
/// order, so the board and the run dialog cannot come to disagree about where
/// this project merges. **Its third term is deliberately not here.** The dialog
/// falls back to the branch most recently worked on, which is a fair guess to
/// put in a field somebody is looking at and about to press a button on; behind
/// this sweep it would be a guess nobody sees, closing tasks against whichever
/// branch happened to be touched last. A project that has named no target
/// branch anywhere gets no automatic closures, which is the honest answer.
///
/// Two small file reads, on the worker rather than on the blocking pool: they
/// are what `settings::agent` and `runs::config::load` already cost every other
/// caller, and reading them at each tick is what lets a branch chosen a minute
/// ago take effect without a restart.
fn target_branch(app: &AppHandle, dir: &Path) -> Option<String> {
    let project = dir.to_string_lossy();
    if let Some(branch) = crate::settings::target_branch(app, &project) {
        return Some(branch);
    }
    match crate::runs::config::load(dir) {
        crate::runs::config::ConfigState::Ok { config } => config.defaults.target_branch,
        _ => None,
    }
}

async fn incremental_sync(app: &AppHandle, bd: &Bd, store: &mut Store, health: &mut HealthReporter) {
    let since = since_with_overlap(store.last_seen());
    match bd.list_updated_after(&since).await {
        Ok(issues) => {
            emit_delta(app, store.apply_incremental(issues));
            health.recovered();
        }
        Err(e) => health.failed(&e, Some(bd.cwd())),
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

    // Before asking whether there is a tracker in here at all, because a folder
    // the operating system is refusing answers "no" to every question about
    // itself and the answer means nothing. `has_tracker` is an `is_dir`, and
    // macOS lets a `stat` through while refusing the `read_dir` underneath — so
    // without this the notice would offer "Initialize bd" over a folder that
    // already has a `.beads` nobody is allowed to open, and pressing it would
    // put a second tracker inside the first.
    //
    // `tracked: false`, for the reason it is false in a folder with no tracker:
    // nothing here can be read and nothing may be written. So the sixty-second
    // sweep leaves this folder alone until it is opened again — which is what
    // the repair arranges, by restarting the app once macOS has forgotten the
    // refusal.
    if let Some(message) = access::refusal(&dir) {
        health.degrade_project(HealthState::FolderRefused, message);
        return Some(Project { dir, bd, _watcher: None, tracked: false });
    }

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
        // No folder is open yet: `bd --version` reads no tracker and is made
        // from whatever directory the process started in, so there is nothing
        // here for a refusal to be about.
        Err(e) => health.failed(&e, None),
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
                    // And after it, on the same tick rather than on a timer of
                    // its own: what this asks about is the board the sweep has
                    // just made current, so a second timer could only ask the
                    // same question of a staler answer.
                    close_merged(&app, &current, &mut store).await;
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
        Request::Repair(reply) => {
            let Some((dir, bd)) = current.as_ref().map(|p| (p.dir.clone(), p.bd.clone())) else {
                let _ = reply.send(Err(TrackerError::NoTracker("no project selected".into())));
                return false;
            };
            // Before the copy exists, not after: the entry that keeps it out of
            // the person's repository has to be in place first, or an 84 MB
            // Dolt copy is an untracked row in the Git panel for however long
            // the two calls are apart. Kept in `runs::gitignore` rather than
            // written here, because that file is where this app decides once,
            // in code, what of its own it hides — and the terminal worker
            // already calls the same function for `.smetana/`.
            //
            // A failure is logged and the repair goes on, the same way the
            // terminal worker treats it: a `.gitignore` that could not be
            // written is not a reason to leave a tracker broken.
            if let Err(err) = crate::runs::gitignore::ensure(&dir) {
                log::warn!("[tracker] could not add .beads.backup-*/ to .gitignore: {err}");
            }
            // The copy next, and a failure to take it ends the whole thing
            // here: it is the only reason the button in front of this has no
            // confirmation dialog, so a repair that could not take one has no
            // right to migrate. `TrackerError::Backup` says as much in its own
            // sentence. A folder with no `.beads` is refused by this very call
            // rather than by a check above it — an empty copy would look like
            // a taken one.
            //
            // On a blocking thread, because copying a database directory is
            // however much of somebody's disk it is, and the worker is the one
            // task answering every other command meanwhile.
            let taken = {
                let dir = dir.clone();
                tokio::task::spawn_blocking(move || backup::copy_beads(&dir, chrono::Utc::now()))
                    .await
            };
            let backup = match taken {
                Ok(Ok(path)) => path,
                // Through health, exactly as a failed migration goes: the state
                // does not move — `error` before, `error` after — but the
                // message becomes this refusal, so the line under the board
                // says why the copy could not be taken instead of quoting a
                // `bd list` failure from before anybody pressed anything. This
                // is the *likelier* of the two failure paths, and it is the one
                // that was console-only: a full disk, a permission problem or a
                // read-only volume is ordinary, while a failing `bd migrate` is
                // not. `last_command_failure` is untouched, because
                // `TrackerError::Backup` is not a `Command` and there is no bd
                // call behind it to name — an agent asked about this hears the
                // refusal through the health line the briefing always carries.
                Ok(Err(e)) => {
                    health.failed(&e, Some(dir.as_path()));
                    let _ = reply.send(Err(e));
                    return false;
                }
                Err(e) => {
                    let failed =
                        TrackerError::Backup(format!("the copy did not finish: {e}"));
                    health.failed(&failed, Some(dir.as_path()));
                    let _ = reply.send(Err(failed));
                    return false;
                }
            };

            let output = match bd.repair().await {
                Ok(output) => output,
                // **The migration's failure becomes the tracker's health, and
                // that is a decision.** The state does not move — it was
                // `error` and stays `error`, so the board's place keeps the
                // notice and both buttons — but the message does, to the words
                // of the call a person just asked for. Two things follow, and
                // both are why this is not left alone the way `InitTracker`
                // leaves it (there the state itself would have been wrong).
                // The line under the notice stops quoting an older `bd list`
                // failure while the migration's own words went only to the
                // console, which is a small copy of the very defect this task
                // closes. And `Request::Failure` then briefs the agent on the
                // migration rather than on whatever failed before it, which is
                // the more informative of the two and the reason the second
                // button is being pressed at all.
                //
                // The next sixty-second sweep may overwrite it with a fresh
                // `bd list` failure. That is fine: it is then what is true.
                //
                // The copy stays where it is — nothing in this app removes a
                // person's data quietly.
                Err(e) => {
                    health.failed(&e, Some(dir.as_path()));
                    let _ = reply.send(Err(e));
                    return false;
                }
            };

            // Reopening is what makes the board come back without the front end
            // asking for anything: a full sweep runs inside `open`, and health
            // clears itself the moment bd reads the tracker again. The same
            // drop-then-open order SetProject uses, for the same reason — the
            // old watcher must not outlive the folder's state.
            *current = None;
            *current = open(app, Some(dir), store, health, tx_tick).await;
            let _ = reply.send(Ok(Repair {
                backup: backup.to_string_lossy().into_owned(),
                output,
                snapshot: store.snapshot(),
            }));
            true
        }
        Request::Failure(reply) => {
            let (command, stderr) = health
                .last_command_failure
                .clone()
                .unwrap_or_else(|| (String::new(), String::new()));
            // What the tracker is saying **now** comes first and comes always,
            // and the remembered stderr is appended only when it adds something
            // the health line does not already carry. Two reasons, and the
            // second is the one that bites. A failure whose health line is a
            // `TrackerError::Command` already quotes bd's stderr inside it, so
            // appending it again would hand the agent the same paragraph twice.
            // And a trouble that never came from a bd command at all — the
            // watcher dying, a folder with no `.beads` — has no stderr of its
            // own, and would otherwise be briefed on as an older command's
            // failure with the actual trouble named nowhere.
            let now_says = health.current.message.clone().unwrap_or_default();
            let mut said = now_says.clone();
            if !stderr.trim().is_empty() && !now_says.contains(stderr.trim()) {
                if !said.is_empty() {
                    said.push_str("\n\n");
                }
                said.push_str(&stderr);
            }
            let _ = reply.send(Failure {
                dir: current
                    .as_ref()
                    .map(|p| p.dir.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                bd_version: EXPECTED_BD_VERSION.to_string(),
                command,
                stderr: said,
            });
            false
        }
        Request::Resync(reply) => {
            let result = match tracked(current) {
                Some(bd) => full_sync(app, bd, store, health).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(result.map(|()| store.snapshot()));
            false
        }
        Request::BoardAt { dir, fresh, reply } => {
            let result = match own_bd(current, &dir) {
                Some(bd) if fresh => full_sync(app, &bd, store, health)
                    .await
                    .map(|()| (store.snapshot().issues, BoardSource::Cache)),
                Some(_) => Ok((store.snapshot().issues, BoardSource::Cache)),
                // Health is deliberately untouched here, and so is the store.
                // Both belong to the project on screen; a run reading its own
                // board in the background has no business putting a red line
                // under somebody else's, and there is nothing drawn from this
                // answer to redraw.
                None => Bd::new(app.clone(), dir)
                    .list_all()
                    .await
                    .map(|issues| (issues, BoardSource::Direct)),
            };
            let _ = reply.send(result);
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
        Request::UpdateAt { dir, id, patch, reply } => {
            let result = match own_bd(current, &dir) {
                // `finish` and not a bare write: this is the board on screen,
                // so the delta has to go out exactly as it does for a write
                // somebody made in the window.
                Some(bd) => finish(app, store, bd.update(&id, &patch).await),
                None => Bd::new(app.clone(), dir).update(&id, &patch).await,
            };
            let _ = reply.send(result);
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
