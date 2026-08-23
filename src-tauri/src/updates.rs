//! In-app updates: checking, downloading, holding what came down, installing it
//! and relaunching.
//!
//! # Why this is Rust and not a store in `src/`
//!
//! Everything else the app asks the desktop for is called from a store, so this
//! being a module is a decision. Two reasons, and either alone would be enough.
//!
//! The About tab lives in the **settings window**, which is a second OS window
//! a person closes as soon as they have read the version. A download driven
//! from that window's store dies with it — a hundred megabytes half fetched and
//! nothing to say so. Here the download is a task the app owns, and closing the
//! window it was started from changes nothing about it.
//!
//! And the run gate below **cannot be answered in the front end at all**.
//! `runsState.runs` is filtered to the active project, so it does not know
//! whether a run is live in a neighbouring one; the authority is the run
//! worker's map, and it is reachable only from here.
//!
//! # The state travels whole
//!
//! [`UpdateState`] is one tagged value — `idle`, `checking`, `available`,
//! `downloading`, `ready`, `failed` — and never a set of flags a window has to
//! reassemble. A tag rather than booleans is also what keeps a state this front
//! end has never heard of from silently reading as one it has: an unknown
//! `kind` matches nothing, where a missing boolean is indistinguishable from
//! `false`.
//!
//! Every change is emitted on `updates:state`, and [`updates_state`] answers the
//! same value on demand — so a window opened halfway through a download draws
//! the download, and one that was open all along never had to ask.
//!
//! # Downloading is automatic; installing never is
//!
//! Reaching `ready` is something this module does by itself. Leaving `ready` is
//! only ever [`updates_install`], because the app holds unsaved editor buffers
//! and live terminals, and a relaunch nobody asked for loses them.
//!
//! # One switch, over the timer alone
//!
//! `updates.autoCheck` in `settings.json` decides whether [`schedule`] below
//! reaches the network by itself, and it decides nothing else: the command a
//! person presses on the About tab is accepted whatever it says, and an update
//! already downloaded stays staged and installable. The value is read at each
//! tick rather than once, which is what makes the switch take effect without a
//! restart — see [`schedule`].
//!
//! # The run gate
//!
//! Installing restarts the app, and a restart kills the PTY children a run's
//! sessions started. A run is the app driving itself for hours with nobody
//! watching, so the install is **refused while any run is live anywhere** —
//! including in a project nobody is looking at. The refusal names the projects,
//! because a button that will not act and will not say why sends somebody to
//! guess.
//!
//! What counts as live is every entry in the run worker's map, which is the
//! same count `runs/awake.rs` holds a power assertion for: a run that has
//! stopped and is winding down still has a batch in flight, and that batch is
//! still a process a restart would orphan.
//!
//! # No ACL grant, deliberately
//!
//! `capabilities/default.json` lists nothing for either plugin, and the absence
//! is a decision — the same one `autostart.rs` records. The front end calls the
//! three commands at the bottom of this file; it never calls `plugin:updater|*`
//! or `plugin:process|*`. Those permissions would therefore be required by
//! nothing, and what they would cost is real: `updater:default` publishes
//! `plugin:updater|download_and_install` to the webview, which is the one route
//! by which a page could replace the bundle without passing the run gate above,
//! and `process:allow-restart` is the same shape one step further on. The
//! capability file's habit is a narrow grant for what is actually called
//! (`clipboard-manager:allow-write-text`, not `:default`); here the honest
//! answer is nothing at all.
//!
//! # Not from a development build
//!
//! Under `debug_assertions` no timer runs, a check answers `failed` with a
//! sentence saying so, and an install is refused. This is not caution. On macOS
//! the plugin derives where to install from the running executable's own path
//! (`extract_path_from_executable`), and for `target/debug/app` that is
//! `target/debug` itself — the install would `remove_dir_all` the whole build
//! directory and move an unpacked `.app` into its place. The same guard is why
//! `autostart.rs` will not register a login item from a development build, and
//! for the same reason: a development build must not be able to do to the tree
//! what the shipped one does to `/Applications`.
//!
//! # A lock, where the rest of this tree has workers
//!
//! `tracker/`, `terminal/` and `runs/` each own their state in a tokio task
//! because the calls they serialize take seconds, and a mutex over them would
//! block unrelated callers for unpredictable stretches. Nothing here is slow
//! **behind the lock**: the network is outside it, and what it guards is three
//! fields read in microseconds. What a worker would buy — an order for
//! concurrent requests — is bought instead by [`Machine::check`], which is the
//! one place a second flow is refused, and it is refused rather than queued
//! because two checks running at once is not a thing anybody wants twice of.
//! The lock is never held across an `await`: [`Updates::with`] takes a
//! synchronous closure, which is what makes that structural rather than a rule
//! somebody has to remember.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::UpdaterExt;
use tokio::sync::oneshot;

use crate::runs::service::{Request as RunRequest, RunHandle};

/// How long after start the first check waits. The first seconds of a launch
/// are the ones somebody is watching: the board's first `bd list` takes two to
/// three seconds, the file tree and the git status land beside it, and a
/// release feed fetched into the middle of that is a request nobody is waiting
/// for competing with three they are.
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(60);

/// And once a day after that. A ceiling rather than a promise: a machine asleep
/// through the tick simply checks late, and the app is restarted often enough
/// that the delay above is the check most launches get.
const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// How often a download reports itself. The plugin's callback fires per chunk —
/// thousands of times for a bundle this size — and an event per chunk would be
/// a progress bar rendered more often than the screen refreshes. The count
/// itself is exact whatever this is; only the telling is throttled.
const PROGRESS_TICK: Duration = Duration::from_millis(250);

const EVENT: &str = "updates:state";

/// Where the update machine is, handed over whole.
///
/// `kind` is the tag, matching `RunState` and every other state this app puts
/// on the wire. The optional fields inside a variant are serialized as `null`
/// rather than omitted, so one variant always has one shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UpdateState {
    /// Nothing known, nothing in flight. Also where a check that found nothing
    /// lands — "you are up to date" is this, not a state of its own.
    #[default]
    Idle,
    Checking,
    /// A release was found and its download is about to start. Transient by
    /// design, and still worth being a state: it is the only place the notes
    /// and the date travel, and it is what a window shows in the seconds
    /// before the first byte arrives.
    Available { version: String, notes: Option<String>, date: Option<String> },
    /// `total` is `None` until the server says how long the body is; some do
    /// not, and a bar with no end is a truer drawing than one invented.
    Downloading { received: u64, total: Option<u64> },
    /// Downloaded, verified and waiting for somebody to press install.
    Ready { version: String },
    /// A check or a download that did not finish, in words. A check from here
    /// is accepted, which is what makes this recoverable rather than terminal.
    Failed { message: String },
}

/// What a run gate or an install refuses for. `kind`/`detail` is `RunError`'s
/// shape, so the front end reads both the same way.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum UpdateError {
    /// Pressed with nothing downloaded. Not reachable from a window drawing
    /// this module's state, and worth an answer anyway: the state can change
    /// between the press and its arrival here.
    #[error("there is no downloaded update to install")]
    NothingReady,
    /// The gate. `projects` is the list, joined, because a refusal that cannot
    /// say where the run is leaves somebody hunting through their projects for
    /// it.
    #[error("a run is going in {projects}; installing restarts the app and would end it")]
    RunLive { projects: String },
    /// The run worker could not be reached, so nothing here knows whether a run
    /// is live. Refused rather than allowed: silence is not permission when the
    /// cost of being wrong is an agent killed mid-task.
    #[error("{0}")]
    Runs(String),
    #[error("a development build does not replace itself")]
    DevelopmentBuild,
    /// The install itself would not go through — no permission to write where
    /// the bundle lives, an archive that would not unpack. The message is the
    /// plugin's, framed. The state stays `ready`: what was downloaded is still
    /// downloaded, and the press is still there to make again.
    #[error("{0}")]
    Install(String),
}

/// The pure half: which state the module is in, and which transitions are
/// legal from it. No Tauri, no network and no clock — every input arrives as a
/// call, which is what makes the whole of it reachable from `cargo test`.
///
/// Every transition below is **guarded by the state it comes from**, and one
/// that does not fit is ignored rather than applied. That is what makes a late
/// callback harmless: a download's progress arriving after the flow it belongs
/// to has already failed finds a machine that is no longer downloading, and
/// changes nothing.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Machine {
    state: UpdateState,
    /// The version the current check found. `Downloading` does not carry it and
    /// `Ready` must, so it is remembered here rather than threaded back through
    /// the download.
    version: Option<String>,
}

impl Machine {
    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    /// Whether a check may start, and the start of it if so.
    ///
    /// **Two of the six states accept, and each of the other four is a flow
    /// already in hand.** `checking` and `downloading` are the obvious two.
    /// `ready` is refused because a check from there would find the same
    /// release and fetch it again, throwing away the one the person is being
    /// offered. `available` is refused for the sharpest reason of the four: it
    /// is the state a check sits in for the two statements between finding a
    /// release and asking for its first byte, and a second check accepted in
    /// that window would spawn a second flow whose guards then swallow every
    /// transition the first one makes — while the first download runs to
    /// completion anyway and overwrites what the second one staged. Nothing
    /// ever rests in `available`, so refusing from it costs nothing and is what
    /// makes "only one flow is ever in flight" structural rather than a matter
    /// of timing.
    ///
    /// `failed` accepts, which is the whole of "a later check can still
    /// succeed from there".
    pub fn check(&mut self) -> bool {
        match self.state {
            UpdateState::Idle | UpdateState::Failed { .. } => {
                self.state = UpdateState::Checking;
                self.version = None;
                true
            }
            UpdateState::Checking
            | UpdateState::Available { .. }
            | UpdateState::Downloading { .. }
            | UpdateState::Ready { .. } => false,
        }
    }

    /// The check answered, and there is nothing newer.
    pub fn nothing_new(&mut self) {
        if matches!(self.state, UpdateState::Checking) {
            self.state = UpdateState::Idle;
        }
    }

    /// The check answered with a release.
    pub fn found(&mut self, version: String, notes: Option<String>, date: Option<String>) {
        if matches!(self.state, UpdateState::Checking) {
            self.version = Some(version.clone());
            self.state = UpdateState::Available { version, notes, date };
        }
    }

    /// The first byte is about to be asked for.
    pub fn downloading(&mut self) {
        if matches!(self.state, UpdateState::Available { .. }) {
            self.state = UpdateState::Downloading { received: 0, total: None };
        }
    }

    /// One chunk. `total` is repeated by the plugin on every call and is taken
    /// each time rather than once, since it is `None` until the response
    /// headers have been read.
    pub fn received(&mut self, chunk: usize, size: Option<u64>) {
        if let UpdateState::Downloading { received, total } = &mut self.state {
            *received += chunk as u64;
            if size.is_some() {
                *total = size;
            }
        }
    }

    /// Downloaded and verified. The version is the one the check found, so a
    /// `ready` can never announce a different release from the `available`
    /// that preceded it.
    pub fn ready(&mut self) {
        if matches!(self.state, UpdateState::Downloading { .. }) {
            let version = self.version.clone().unwrap_or_default();
            self.state = UpdateState::Ready { version };
        }
    }

    /// Unguarded, unlike every transition above, and deliberately: there is
    /// only ever one flow in flight, so a failure always belongs to the state
    /// the machine is in — and a failure nobody is told about is the one thing
    /// worse than a failure.
    pub fn failed(&mut self, message: String) {
        self.state = UpdateState::Failed { message };
    }

    /// Whether there is something to install. The staged bytes are the other
    /// half of the answer and live outside this struct; this is the half a
    /// window draws its button from.
    pub fn installable(&self) -> bool {
        matches!(self.state, UpdateState::Ready { .. })
    }
}

/// The gate, pure. `live` is every project the run worker holds an entry for.
///
/// It knows nothing about which project is active, and that is the point: a run
/// in a project nobody is looking at refuses exactly as loudly as one on
/// screen, because a restart ends both.
fn gate(live: &[String]) -> Result<(), UpdateError> {
    if live.is_empty() {
        return Ok(());
    }
    Err(UpdateError::RunLive { projects: live.join(", ") })
}

/// Somebody else's error, framed so the first half of the sentence is ours and
/// the second is whatever the machine said. The borrowed half is lowercased at
/// its first letter, because these arrive capitalized ("Could not fetch a valid
/// release JSON…") and this app writes sentence case.
fn because(doing: &str, err: impl std::fmt::Display) -> String {
    let said = err.to_string();
    let mut chars = said.chars();
    let said = match chars.next() {
        Some(first) => first.to_lowercase().collect::<String>() + chars.as_str(),
        None => said,
    };
    format!("{doing}: {said}")
}

/// A downloaded update, held until somebody presses install. The bytes are
/// verified against the release signature by the plugin before `download`
/// returns, so what is kept here is already trusted.
struct Staged {
    update: tauri_plugin_updater::Update,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct Held {
    machine: Machine,
    staged: Option<Staged>,
}

/// What the app manages: the machine and whatever has been downloaded. Cloning
/// shares it — the timer task, the download task and every command hold the
/// same one.
#[derive(Clone, Default)]
pub struct Updates(Arc<Mutex<Held>>);

impl Updates {
    /// The only way in. A synchronous closure, so the guard cannot be held
    /// across an `await` — see the note at the top of this file.
    ///
    /// A poisoned lock is taken anyway: what it guards is three fields with no
    /// invariant between them that a panic could break, and refusing to answer
    /// the version row for the rest of the session would be the larger fault.
    fn with<T>(&self, f: impl FnOnce(&mut Held) -> T) -> T {
        let mut held = self.0.lock().unwrap_or_else(|err| err.into_inner());
        f(&mut held)
    }

    pub fn state(&self) -> UpdateState {
        self.with(|held| held.machine.state().clone())
    }
}

/// Whether this build may replace itself at all. See the module header for what
/// the answer costs when it is wrong.
fn development() -> bool {
    cfg!(debug_assertions)
}

fn announce(app: &AppHandle, state: &UpdateState) {
    let _ = app.emit(EVENT, state);
}

/// Start a check unless one is already going, and answer with the state the
/// machine is in afterwards. The work itself is a task: this returns as soon as
/// the machine has been claimed, so the press does not wait for a network round
/// trip and a hundred megabytes behind it.
fn request_check(app: &AppHandle, updates: &Updates) -> UpdateState {
    if development() {
        let state = updates.with(|held| {
            held.machine
                .failed("this is a development build; it does not update itself".into());
            held.machine.state().clone()
        });
        announce(app, &state);
        return state;
    }
    let accepted = updates.with(|held| held.machine.check());
    let state = updates.state();
    if !accepted {
        return state;
    }
    announce(app, &state);
    let app = app.clone();
    let updates = updates.clone();
    tauri::async_runtime::spawn(async move { pursue(app, updates).await });
    state
}

/// The whole of a check: ask, and if there is something, fetch it. One task, so
/// the two halves cannot interleave with a second flow — nothing else can be in
/// flight, since the machine was claimed before this was spawned.
async fn pursue(app: AppHandle, updates: Updates) {
    let updater = match app.updater() {
        Ok(updater) => updater,
        Err(err) => return fail(&app, &updates, because("could not check for updates", err)),
    };
    let found = match updater.check().await {
        Ok(found) => found,
        Err(err) => return fail(&app, &updates, because("could not check for updates", err)),
    };
    let Some(update) = found else {
        let state = updates.with(|held| {
            held.machine.nothing_new();
            held.machine.state().clone()
        });
        return announce(&app, &state);
    };

    // `Date`'s own display is ISO 8601, which is what the About row wants and
    // what `time` would otherwise be a dependency for.
    let date = update.date.map(|stamp| stamp.date().to_string());
    let state = updates.with(|held| {
        held.machine.found(update.version.clone(), update.body.clone(), date);
        held.machine.state().clone()
    });
    announce(&app, &state);

    let state = updates.with(|held| {
        held.machine.downloading();
        held.machine.state().clone()
    });
    announce(&app, &state);

    let mut last = Instant::now();
    let downloaded = update
        .download(
            |chunk, total| {
                let state = updates.with(|held| {
                    held.machine.received(chunk, total);
                    held.machine.state().clone()
                });
                if last.elapsed() >= PROGRESS_TICK {
                    last = Instant::now();
                    announce(&app, &state);
                }
            },
            || {},
        )
        .await;
    let bytes = match downloaded {
        Ok(bytes) => bytes,
        Err(err) => return fail(&app, &updates, because("could not download the update", err)),
    };

    let state = updates.with(|held| {
        held.staged = Some(Staged { update, bytes });
        held.machine.ready();
        held.machine.state().clone()
    });
    announce(&app, &state);
}

fn fail(app: &AppHandle, updates: &Updates, message: String) {
    log::warn!("{message}");
    let state = updates.with(|held| {
        held.machine.failed(message);
        held.machine.state().clone()
    });
    announce(app, &state);
}

/// Which projects hold a run right now, asked of the one thing that knows.
async fn live_runs(runs: &RunHandle) -> Result<Vec<String>, UpdateError> {
    let (tx, rx) = oneshot::channel();
    runs.0
        .send(RunRequest::LiveProjects(tx))
        .await
        .map_err(|_| UpdateError::Runs("the run worker is not running".into()))?;
    rx.await.map_err(|_| UpdateError::Runs("the run worker did not answer".into()))
}

/// The first check after start, and one a day after that.
///
/// A task rather than a call in `setup`: the delay is the whole point, and
/// `setup` is on the path to the first frame. Nothing runs at all in a
/// development build.
///
/// `updates.autoCheck` is asked **at every tick** and never read once into a
/// variable here, which is the whole of "switching it off stops the scheduled
/// check and switching it on restores it, both without a restart". The timer
/// itself keeps ticking either way: a tick with the switch off skips the check
/// and sleeps again, so there is nothing to start up when somebody turns it
/// back on. Reading a file once a day costs nothing, and a channel from the
/// front end would be a second route to a value that already has one.
///
/// The switch reaches this and nothing else. [`updates_check`] — the press on
/// the About tab — goes on working with it off, because a press is not the app
/// acting on its own; and anything already downloaded stays staged and
/// installable, since nothing here touches the machine or the bytes.
pub fn schedule(app: AppHandle) {
    if development() {
        log::info!("a development build does not check for updates");
        return;
    }
    let updates = app.state::<Updates>().inner().clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(FIRST_CHECK_DELAY).await;
        loop {
            if crate::settings::updates_auto_check(&app) {
                request_check(&app, &updates);
            }
            tokio::time::sleep(CHECK_INTERVAL).await;
        }
    });
}

/// What a window draws from when it opens. The event carries every change
/// afterwards; this is the one that costs nothing to ask and is the only way a
/// window opened mid-download knows there is one.
#[tauri::command]
pub fn updates_state(updates: State<'_, Updates>) -> UpdateState {
    updates.state()
}

/// The press on About's check button, and what the timer calls. Never fails:
/// a check that cannot start is answered with the state that stopped it, and a
/// check that starts and then fails says so through the event.
#[tauri::command]
pub fn updates_check(app: AppHandle, updates: State<'_, Updates>) -> UpdateState {
    request_check(&app, &updates)
}

/// Replace the bundle and come back on the new one.
///
/// Every way this can decline is a refusal with a reason rather than a press
/// that did nothing — a development build, nothing downloaded, a run going
/// somewhere, a run worker that will not answer, an install that would not go
/// through. A button that will not act and will not say why sends somebody to
/// guess.
///
/// On success this does not return in any useful sense: the app is on its way
/// out. `request_restart` and not `restart` — it goes through the event loop
/// whatever thread it is called from, so `RunEvent::Exit` in `lib.rs` fires and
/// the PTY children are killed exactly as they are on an ordinary quit. The
/// direct `restart` skips both when it happens to be called on the main thread,
/// which would leave every open terminal's process orphaned.
#[tauri::command]
pub async fn updates_install(
    app: AppHandle,
    updates: State<'_, Updates>,
    runs: State<'_, RunHandle>,
) -> Result<(), UpdateError> {
    if development() {
        return Err(UpdateError::DevelopmentBuild);
    }
    if !updates.with(|held| held.machine.installable() && held.staged.is_some()) {
        return Err(UpdateError::NothingReady);
    }
    gate(&live_runs(&runs).await?)?;

    // Taken out for the blocking call and put back if it fails, and the state
    // is deliberately left at `ready` either way. An install that did not
    // happen has not stopped the update being downloaded and waiting — the
    // failure is this call's answer, not a new state — and moving to `failed`
    // would strand the bytes where nothing can press them, since the button is
    // drawn from `ready`. `failed` is for a check or a download, which are the
    // two things nobody pressed.
    let Some(staged) = updates.with(|held| held.staged.take()) else {
        return Err(UpdateError::NothingReady);
    };
    // Blocking rather than inline: unpacking an archive over a directory is
    // file IO, and on macOS the path where the bundle cannot be written asks
    // for a password through AppleScript and waits for the answer.
    let outcome = tauri::async_runtime::spawn_blocking(move || {
        let outcome = staged.update.install(&staged.bytes);
        (staged, outcome)
    })
    .await;
    let (staged, outcome) = match outcome {
        Ok(pair) => pair,
        Err(err) => return Err(refused_install(&app, &updates, None, err)),
    };
    if let Err(err) = outcome {
        return Err(refused_install(&app, &updates, Some(staged), err));
    }
    app.request_restart();
    Ok(())
}

/// An install that did not happen: put the bytes back, say so in the log, and
/// hand the sentence to the caller. The machine stays where it was — see the
/// note above.
///
/// `None` is the one case that does move it, and it is the reason this takes an
/// `Option` at all: the blocking task itself failed, so the bytes went with it
/// and there is nothing left to install. Left at `ready` that would be a button
/// offering something that no longer exists, with no way out — a check is
/// refused from `ready`, so nothing could ever fetch it again. `failed` says
/// what happened and accepts the next check.
fn refused_install(
    app: &AppHandle,
    updates: &Updates,
    staged: Option<Staged>,
    err: impl std::fmt::Display,
) -> UpdateError {
    let message = because("could not install the update", err);
    match staged {
        Some(staged) => {
            log::warn!("{message}");
            updates.with(|held| held.staged = Some(staged));
        }
        None => fail(app, updates, message.clone()),
    }
    UpdateError::Install(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checking() -> Machine {
        let mut machine = Machine::default();
        assert!(machine.check(), "an idle machine takes a check");
        machine
    }

    /// The whole happy path, in the order it happens, because the interesting
    /// part is that each step only applies from the one before it.
    #[test]
    fn a_check_becomes_a_download_and_then_something_to_install() {
        let mut machine = Machine::default();
        assert_eq!(*machine.state(), UpdateState::Idle);
        assert!(machine.check());
        assert_eq!(*machine.state(), UpdateState::Checking);
        machine.found("0.2.0".into(), Some("notes".into()), Some("2026-08-23".into()));
        assert_eq!(
            *machine.state(),
            UpdateState::Available {
                version: "0.2.0".into(),
                notes: Some("notes".into()),
                date: Some("2026-08-23".into()),
            }
        );
        machine.downloading();
        assert_eq!(*machine.state(), UpdateState::Downloading { received: 0, total: None });
        machine.received(400, Some(1000));
        machine.received(600, Some(1000));
        assert_eq!(
            *machine.state(),
            UpdateState::Downloading { received: 1000, total: Some(1000) }
        );
        machine.ready();
        assert_eq!(*machine.state(), UpdateState::Ready { version: "0.2.0".into() });
        assert!(machine.installable());
    }

    /// The version announced at the end is the one found at the start, which is
    /// the reason the machine remembers it at all: `Downloading` does not carry
    /// it, so without that field `ready` would have nothing to say.
    #[test]
    fn the_version_that_was_found_is_the_version_that_is_ready() {
        let mut machine = checking();
        machine.found("1.4.1".into(), None, None);
        machine.downloading();
        machine.ready();
        assert_eq!(*machine.state(), UpdateState::Ready { version: "1.4.1".into() });
    }

    #[test]
    fn a_check_that_finds_nothing_goes_back_to_idle() {
        let mut machine = checking();
        machine.nothing_new();
        assert_eq!(*machine.state(), UpdateState::Idle);
        assert!(!machine.installable());
    }

    /// The one place a second flow is refused. Without it two timers, or a
    /// press landing on a tick, would run two downloads into one machine.
    #[test]
    fn a_second_check_is_refused_while_one_is_going() {
        let mut machine = checking();
        assert!(!machine.check(), "a check is already going");
        assert_eq!(*machine.state(), UpdateState::Checking);

        machine.found("0.2.0".into(), None, None);
        machine.downloading();
        assert!(!machine.check(), "and a download is going");
        assert_eq!(*machine.state(), UpdateState::Downloading { received: 0, total: None });
    }

    /// The narrowest window of the four refusals, and the reason it is a
    /// refusal at all: a check accepted in the two statements between finding a
    /// release and asking for its first byte would put a second flow beside a
    /// first one that goes on downloading regardless.
    #[test]
    fn a_check_is_refused_in_the_moment_between_finding_and_downloading() {
        let mut machine = checking();
        machine.found("0.2.0".into(), Some("notes".into()), None);
        assert!(!machine.check(), "the release found a moment ago is already being fetched");
        assert_eq!(
            *machine.state(),
            UpdateState::Available {
                version: "0.2.0".into(),
                notes: Some("notes".into()),
                date: None,
            },
            "and the refusal leaves the flow exactly where it was"
        );
        machine.downloading();
        assert_eq!(*machine.state(), UpdateState::Downloading { received: 0, total: None });
    }

    /// A check with an update already waiting would find the same release and
    /// fetch it again, throwing away the one being offered.
    #[test]
    fn a_check_is_refused_with_an_update_waiting_to_be_installed() {
        let mut machine = checking();
        machine.found("0.2.0".into(), None, None);
        machine.downloading();
        machine.ready();
        assert!(!machine.check());
        assert_eq!(*machine.state(), UpdateState::Ready { version: "0.2.0".into() });
    }

    /// A failure is not the end of the story: the next check runs from there.
    #[test]
    fn a_failed_check_can_be_checked_again() {
        let mut machine = checking();
        machine.failed("could not check for updates: the network is down".into());
        assert_eq!(
            *machine.state(),
            UpdateState::Failed {
                message: "could not check for updates: the network is down".into()
            }
        );
        assert!(machine.check(), "and a later check is accepted from there");
        assert_eq!(*machine.state(), UpdateState::Checking);
    }

    /// A download that failed and then reported one more chunk. Every
    /// transition is guarded by the state it comes from, which is what makes
    /// that arrive as nothing at all.
    #[test]
    fn a_late_report_from_a_flow_that_is_over_changes_nothing() {
        let mut machine = checking();
        machine.found("0.2.0".into(), None, None);
        machine.downloading();
        machine.received(10, Some(100));
        machine.failed("could not download the update: connection reset".into());

        let failed = machine.state().clone();
        machine.received(10, Some(100));
        machine.ready();
        machine.nothing_new();
        machine.downloading();
        assert_eq!(*machine.state(), failed, "none of it applies to a machine that failed");
        assert!(!machine.installable());
    }

    /// `total` arrives only once the response headers have been read, so it is
    /// taken on every chunk and a `None` after it never unsets it.
    #[test]
    fn a_download_of_unknown_length_counts_up_without_an_end() {
        let mut machine = checking();
        machine.found("0.2.0".into(), None, None);
        machine.downloading();
        machine.received(64, None);
        assert_eq!(*machine.state(), UpdateState::Downloading { received: 64, total: None });
        machine.received(64, Some(512));
        machine.received(64, None);
        assert_eq!(
            *machine.state(),
            UpdateState::Downloading { received: 192, total: Some(512) },
            "a length once known is not forgotten by a callback that omits it"
        );
    }

    #[test]
    fn nothing_is_installable_before_something_is_downloaded() {
        let mut machine = Machine::default();
        assert!(!machine.installable());
        machine.check();
        assert!(!machine.installable());
        machine.found("0.2.0".into(), None, None);
        assert!(!machine.installable(), "found is not downloaded");
        machine.downloading();
        assert!(!machine.installable());
    }

    #[test]
    fn no_run_anywhere_lets_the_install_through() {
        assert_eq!(gate(&[]), Ok(()));
    }

    /// The case the gate exists for: the run is in a project the person is not
    /// looking at, which is exactly what the front end cannot see.
    #[test]
    fn a_run_in_another_project_refuses_the_install_and_names_it() {
        let refusal = gate(&["/Users/x/other".to_string()]);
        assert_eq!(refusal, Err(UpdateError::RunLive { projects: "/Users/x/other".into() }));
        let said = refusal.expect_err("the gate must refuse").to_string();
        assert!(said.contains("/Users/x/other"), "the refusal names where the run is: {said}");
        assert!(said.contains("restarts the app"), "and why it refuses: {said}");
    }

    #[test]
    fn every_project_holding_a_run_is_named() {
        assert_eq!(
            gate(&["/one".to_string(), "/two".to_string()]),
            Err(UpdateError::RunLive { projects: "/one, /two".into() })
        );
    }

    /// The refusals cross the IPC boundary, so their serialization is the
    /// contract rather than the enum.
    #[test]
    fn a_refusal_travels_as_a_kind_and_a_detail() {
        let json = serde_json::to_string(&UpdateError::RunLive { projects: "/one".into() })
            .expect("the refusal must serialize");
        assert_eq!(json, r#"{"kind":"run_live","detail":{"projects":"/one"}}"#);
        let json = serde_json::to_string(&UpdateError::NothingReady)
            .expect("the refusal must serialize");
        assert_eq!(json, r#"{"kind":"nothing_ready"}"#);
    }

    /// Both windows read the state by its tag, so the wire shape is the
    /// contract. An unknown `kind` matches nothing on the other side, which is
    /// the whole reason this is tagged rather than a set of flags.
    #[test]
    fn the_state_travels_tagged_and_whole() {
        let shapes = [
            (UpdateState::Idle, r#"{"kind":"idle"}"#),
            (UpdateState::Checking, r#"{"kind":"checking"}"#),
            (
                UpdateState::Available {
                    version: "0.2.0".into(),
                    notes: None,
                    date: Some("2026-08-23".into()),
                },
                r#"{"kind":"available","version":"0.2.0","notes":null,"date":"2026-08-23"}"#,
            ),
            (
                UpdateState::Downloading { received: 8, total: Some(16) },
                r#"{"kind":"downloading","received":8,"total":16}"#,
            ),
            (
                UpdateState::Ready { version: "0.2.0".into() },
                r#"{"kind":"ready","version":"0.2.0"}"#,
            ),
            (
                UpdateState::Failed { message: "no".into() },
                r#"{"kind":"failed","message":"no"}"#,
            ),
        ];
        for (state, expected) in shapes {
            let json = serde_json::to_string(&state).expect("the state must serialize");
            assert_eq!(json, expected);
        }
    }

    /// What somebody actually reads when a check fails. The plugin's own
    /// sentences start with a capital; this app's do not.
    #[test]
    fn a_failure_reads_as_one_sentence_in_this_app_s_voice() {
        assert_eq!(
            because("could not check for updates", "Could not fetch a valid release JSON"),
            "could not check for updates: could not fetch a valid release JSON"
        );
        assert_eq!(because("could not install the update", ""), "could not install the update: ");
    }

    /// The two halves of "after start, and no more than once a day".
    #[test]
    fn the_first_check_waits_and_the_next_one_is_a_day_later() {
        assert!(
            FIRST_CHECK_DELAY >= Duration::from_secs(30),
            "the first check waits for the launch to settle"
        );
        assert_eq!(CHECK_INTERVAL, Duration::from_secs(60 * 60 * 24));
    }
}
