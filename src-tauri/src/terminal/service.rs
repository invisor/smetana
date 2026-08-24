//! The terminal worker: a single owner of mutable state. Commands, output
//! chunks from the reader threads and the detection tick meet in one
//! `select!` — the same reason as in the tracker: operations of
//! unpredictable length must not block one another.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use base64::Engine;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, MissedTickBehavior};

use super::detect::{detect, DetectInput, Quiet};
use super::model::{Exit, Session, SessionId, SessionMark, SessionState, TerminalError};
use super::pty::{Chunk, Pty};
use super::ring::Ring;
use super::screen::Screen;
use crate::agents::{self, Intent};

/// How much raw output every session remembers — this is what xterm.js
/// repaints itself from when it attaches.
const RING_CAP: usize = 1024 * 1024;
/// The geometry of a session that has never been shown yet.
const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 30;
/// Output is coalesced into this tick, so that one event does not go out per
/// chunk.
const FLUSH: Duration = Duration::from_millis(16);
/// Detection runs every Nth flush tick, not every tick, because the two have
/// different needs. A human's eye wants output at ~60 Hz; detection's own
/// thresholds are `SETTLE` (150 ms) and `IDLE_AFTER` (3 s), against which the
/// ~64 ms of latency this adds is nothing. And it is not free: `reassess`
/// dumps the whole screen of every live session into fresh `String`s and
/// clones its question, which at 60 Hz is a lot of burn for an app that is
/// idle most of the time.
///
/// This is also the rate at which the screen is compared with the one before
/// it, and therefore the resolution of layer A's clock. A screen that changed
/// and changed back inside one interval reads as still — with the interval
/// free-running against whatever the agent is drawing, that would have to hold
/// across the forty-odd samples an `IDLE_AFTER` is made of, and the cost of
/// being wrong is one dot drawn dashed instead of spinning. Lengthening this
/// interval also lengthens how late `Idle` can arrive — see the note at the top
/// of `detect.rs`.
const REASSESS_EVERY: u32 = 4;
/// The longest a capture may wait, whatever the caller asked for. A timeout
/// is clamped to it before it becomes a deadline, so an absurd number costs a
/// long wait rather than a capture that never expires at all.
const CAPTURE_CEILING: Duration = Duration::from_secs(3600);
/// How long the exit path waits for the worker to kill its PTYs. The same
/// ceiling the settings store puts on its own flush when the window closes,
/// for the same reason: the app always exits, and a wedged worker costs a
/// cleanup rather than the app.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(2);
/// How long a signalled agent is given to leave on its own, and how often it
/// is asked. Both live well inside `SHUTDOWN_WAIT`: the grace period is the
/// mechanism, that ceiling is the backstop for a worker that never gets here
/// at all, and a mechanism that spends its own backstop leaves nothing to
/// catch it.
const KILL_GRACE: Duration = Duration::from_millis(1200);
const KILL_POLL: Duration = Duration::from_millis(50);

/// The answer to `terminal_attach`: everything the session has said so far,
/// plus the sequence number that output arriving after it will continue from.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attached {
    /// base64: a `Vec<u8>` serialised as a JSON array of numbers would
    /// inflate every chunk about fivefold.
    pub data: String,
    pub seq: u64,
}

pub enum Request {
    List(String, oneshot::Sender<Vec<Session>>),
    /// Every session the worker holds, whatever project it belongs to, reduced
    /// to what the project rail draws. `List` answers about one project because
    /// the agents panel is about one project; the rail is about all of them,
    /// and asking `List` once per open project would be a command per row.
    Marks(oneshot::Sender<Vec<SessionMark>>),
    /// Project, agent id and intent. The agent id is what settings asked for,
    /// not necessarily what runs — see the `Create` arm and `agents::pick`.
    Create(String, String, Intent, oneshot::Sender<Result<Session, TerminalError>>),
    /// The project to open a shell in, and where inside it to start — a shell
    /// takes no agent and no intent, because there is nothing this app is asking
    /// it to do. The second field is a path relative to the project's root, or
    /// `None` for the root itself, which is what every caller but the file
    /// tree's menu means. It is checked with `resolve_within` like every other
    /// path this app takes from the front end; see `shell_cwd`.
    CreateShell(String, Option<String>, oneshot::Sender<Result<Session, TerminalError>>),
    Remove(SessionId, oneshot::Sender<()>),
    Attach(SessionId, oneshot::Sender<Result<Attached, TerminalError>>),
    /// Carries the id it is leaving, and not for symmetry: see the handler.
    Detach(SessionId),
    Resize(SessionId, u16, u16),
    Write(SessionId, String, oneshot::Sender<Result<(), TerminalError>>),
    RunCapture(SessionId, String, u64, u64, oneshot::Sender<Result<Vec<String>, TerminalError>>),
    /// The pid of this session's process group, for the run registry to write
    /// down — `None` for a session the worker does not have, or one whose
    /// process never had a pid to give. Asked for rather than carried on
    /// `Session`, which is serialized to the front end: nothing on screen draws
    /// a pid, and the one caller is the run worker recording what to hang up
    /// after an unclean exit.
    Group(SessionId, oneshot::Sender<Option<u32>>),
    /// Answer once this session's process is gone, with how it ended. See
    /// `ExitWaiter` for why it is a request and not a subscription, and `Exit`
    /// for why the answer is three-valued rather than an exit code.
    AwaitExit(SessionId, oneshot::Sender<Exit>),
    /// The one reply that is not a `oneshot`: it is awaited from the exit
    /// event, on a synchronous thread, and only `std::sync::mpsc` can put a
    /// ceiling on a blocking receive.
    ShutDown(std::sync::mpsc::Sender<()>),
}

#[derive(Clone)]
pub struct TerminalHandle(pub mpsc::Sender<Request>);

struct Live {
    session: Session,
    /// Which agent this session runs — layer B's own dialog reader comes
    /// from here, not from a name hardcoded in `detect.rs`. `None` for a
    /// session that runs no agent, which is the person's own shell: it has no
    /// profile to be read by, deliberately (see `Request::CreateShell`).
    profile: Option<&'static dyn agents::Profile>,
    pty: Pty,
    ring: Ring,
    screen: Screen,
    /// Set for a batch whose harness prints a machine format — see
    /// `terminal::transcript`. `None` for every session a person is sitting in
    /// front of, whose harness is already drawing for them.
    transcript: Option<crate::terminal::transcript::Transcript>,
    /// A bell rang and has not been cleared yet. It is cleared by a write
    /// into the session — a human answering, from the keyboard or with a
    /// button — by a view attaching, which is a human looking at it, and by
    /// the process exiting. Detection only reads it.
    bell_pending: bool,
    /// Layer A's clock: how long the screen has looked the way it looks now.
    /// Deliberately not the same thing as `last_output` — an agent waiting on
    /// a person can keep repainting the very screen it is waiting on, and used
    /// to read as busy for it. See the note at the top of `detect.rs`.
    quiet: Quiet,
    /// When bytes last arrived, whether or not they changed anything. Only
    /// `close_captures` reads it: a capture is waiting for its own write to
    /// come back, and the stream is what tells it the write was heard at all.
    last_output: Instant,
    /// Accumulated since the last flush — leaves as a single event on the
    /// tick.
    pending: Vec<u8>,
    seq: u64,
}

/// Somebody waiting for a session to end — in practice the run worker, which
/// starts one session per batch and has nothing to do until it is over.
///
/// A request rather than a subscription, and that is the race it exists to
/// settle: the run worker creates a session and asks in the next breath, and
/// the process may already be gone by then. A subscriber would have missed the
/// event and waited forever; a request looks at the state that is here.
///
/// `grace` is set the first time the session is seen `Exited` **without** an
/// exit code. End of stream can arrive before the child has been reaped, and
/// `reassess` re-polls for the code every detection tick until it comes — so
/// answering the instant the state flips would report a clean batch as a crash.
/// A code that never arrives at all (a signal) answers `Exit::NoCode` once the
/// grace is spent, and that is a crash, which is what being signalled is.
struct ExitWaiter {
    id: SessionId,
    tx: oneshot::Sender<Exit>,
    grace: Option<Instant>,
}

/// How long to keep re-polling for an exit code before answering without one.
const EXIT_CODE_GRACE: Duration = Duration::from_secs(5);

/// An unclosed capture: wait until this session's output has stopped for
/// `settle`, then hand back its screen. It lives in the worker's loop rather
/// than in a task of its own, because the screen is the worker's state and is
/// never handed out.
///
/// The stream and not the picture, which is the opposite of what layer A
/// measures now, and deliberately: a capture has just written into the session
/// and is waiting for an answer to arrive at all, and a screen that happens to
/// look unchanged for a moment mid-answer is not one. Reading a half-finished
/// reply as a settled one would be handing a caller the wrong text with nothing
/// to say so.
struct Capture {
    id: SessionId,
    settle: Duration,
    deadline: Instant,
    tx: oneshot::Sender<Result<Vec<String>, TerminalError>>,
}

pub fn start(app: AppHandle) -> TerminalHandle {
    let (tx, mut rx) = mpsc::channel::<Request>(32);
    let (chunks_tx, mut chunks_rx) = mpsc::unbounded_channel::<Chunk>();

    tauri::async_runtime::spawn(async move {
        let mut sessions: HashMap<SessionId, Live> = HashMap::new();
        let mut captures: Vec<Capture> = Vec::new();
        let mut exit_waiters: Vec<ExitWaiter> = Vec::new();
        let mut active: Option<SessionId> = None;
        let mut next_id: SessionId = 1;
        let mut tick = tokio::time::interval(FLUSH);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let mut ticks: u32 = 0;

        loop {
            tokio::select! {
                request = rx.recv() => {
                    // The senders are gone — there is nobody left to work for.
                    let Some(request) = request else { break };
                    // Shutting down is the one request that cannot be served
                    // inside `handle`: killing gracefully means waiting, and
                    // waiting means `.await`. The reply goes back only once
                    // the PTYs are gone, because that is what the caller is
                    // holding the exit for. Then `return`, not `break`: the
                    // sweep below is for the other ways out of this loop, and
                    // running it here would be a second grace period spent on
                    // sessions this one has already dealt with.
                    if let Request::ShutDown(tx) = request {
                        kill_all(&mut sessions).await;
                        let _ = tx.send(());
                        return;
                    }
                    handle(&app, &mut sessions, &mut captures, &mut exit_waiters, &mut active, &mut next_id, &chunks_tx, request);
                }
                chunk = chunks_rx.recv() => {
                    // Cannot happen while this task owns the sender it hands
                    // to every reader thread — and if that ownership ever
                    // moves, breaking is a stopped worker, whereas
                    // continuing is a branch that is instantly ready
                    // forever, i.e. a spinning core.
                    let Some(chunk) = chunk else { break };
                    absorb(&app, &mut sessions, chunk);
                }
                _ = tick.tick() => {
                    flush(&app, &mut sessions, active);
                    ticks = ticks.wrapping_add(1);
                    if ticks % REASSESS_EVERY == 0 {
                        reassess(&app, &mut sessions);
                    }
                    // Captures are closed on every tick, unlike detection:
                    // `settle` comes from the caller and may well be shorter
                    // than the detection interval.
                    close_captures(&mut captures, &sessions);
                    close_exit_waiters(&mut exit_waiters, &sessions);
                }
            }
        }

        // The worker lost its senders or its chunk channel rather than being
        // asked to stop — nobody is waiting on this, but an agent left alive
        // is an orphan in the process list all the same. The requested exit
        // returns above and does not come through here.
        kill_all(&mut sessions).await;
    });

    TerminalHandle(tx)
}

/// Signal, wait, then kill — not because killing is unreliable, but because
/// killing first is a lie about what happened. An agent that gets SIGKILL
/// flushes nothing, and this runs every single time the app closes; a hangup
/// is what a person closing a terminal window sends, and every CLI is built
/// for it. The wait is bounded because the window is already going: an agent
/// that will not leave in a second is not going to be talked out of it, and
/// the app must not hang on the argument. What is above this — `SHUTDOWN_WAIT`
/// in `shutdown` — is not a longer version of the same wait: it guards against
/// a worker that never reaches this function at all, which is why the grace
/// period here has to stay well under it.
async fn kill_all(sessions: &mut HashMap<SessionId, Live>) {
    // Only the living are signalled. A session that has already exited sits
    // in the map until `Request::Remove` takes it out, and its pid can by
    // then belong to somebody else — signalling a process group that is not
    // ours is not a thing to do on the way out, however unlikely.
    let mut signalled = false;
    for live in sessions.values_mut() {
        if live.pty.exit_code().is_none() {
            signalled |= live.pty.hangup();
        }
    }
    // Nothing was asked to leave — on a platform with no such signal, or
    // because everything here is already gone — so there is nothing to wait
    // for, and waiting would only make closing the window slower.
    if signalled {
        let deadline = Instant::now() + KILL_GRACE;
        while Instant::now() < deadline {
            // `exit_code` is the same non-blocking `try_wait` the rest of the
            // worker uses, so this also reaps whoever has already left.
            if sessions.values_mut().all(|live| live.pty.exit_code().is_some()) {
                return;
            }
            tokio::time::sleep(KILL_POLL).await;
        }
    }
    for live in sessions.values_mut() {
        if live.pty.exit_code().is_none() {
            live.pty.kill();
        }
    }
}

/// The exit path. Called from `RunEvent::Exit` — the event loop is already
/// leaving and nothing can prevent it — on the main thread, outside async;
/// hence the blocking send and the timed receive rather than `.await`.
///
/// The wait is worth having because an agent left alive is an orphan in the
/// process list. It is worth exactly two seconds because the window has to
/// close either way — and it is a ceiling on a worker that never answers, not
/// the grace period itself: that one is `kill_all`'s, and deliberately
/// shorter.
///
/// The blocking send cannot hang: the queue is only full while the worker is
/// not draining it, the one place the worker awaits inside its loop is the
/// grace period this very request starts, and the global async runtime is a
/// process-lifetime static that outlives this callback. If the worker has
/// already stopped, its receiver is gone and the send returns an error at once
/// instead of blocking.
pub fn shutdown(app: &AppHandle) {
    let Some(handle) = app.try_state::<TerminalHandle>() else { return };
    let (tx, rx) = std::sync::mpsc::channel();
    if handle.0.blocking_send(Request::ShutDown(tx)).is_err() {
        return;
    }
    let _ = rx.recv_timeout(SHUTDOWN_WAIT);
}

fn emit_state(app: &AppHandle, session: &Session) {
    let _ = app.emit("terminal:state", session);
}

/// Output chunks: into the ring for the human, into the screen for the app,
/// into the send queue for the active session.
fn absorb(app: &AppHandle, sessions: &mut HashMap<SessionId, Live>, chunk: Chunk) {
    match chunk {
        Chunk::Data(id, bytes) => {
            let Some(live) = sessions.get_mut(&id) else { return };
            // Before the ring, the screen and the queue, so that all three go on
            // seeing one identical stream — the invariant this subsystem is
            // built on. A session with no transcript is every session a person
            // started, and its bytes are its own.
            let bytes = match live.transcript.as_mut() {
                Some(transcript) => transcript.feed(&bytes),
                None => bytes,
            };
            live.ring.push(&bytes);
            if live.screen.feed(&bytes) {
                live.bell_pending = true;
            }
            live.pending.extend_from_slice(&bytes);
            live.last_output = Instant::now();
            // Leaving `Starting` is the tick's business, not this function's:
            // `reassess` reaches a new session before its first byte does.
        }
        Chunk::Gone(id) => {
            let Some(live) = sessions.get_mut(&id) else { return };
            // The chunk carries no exit code — end of stream arrives before
            // the child has necessarily been reaped, so the code is asked for
            // separately, here, where the wait lives.
            let code = live.pty.exit_code();
            live.session.finish(code);
            // The process is gone: there is nobody left to call for.
            live.bell_pending = false;
            emit_state(app, &live.session);
        }
    }
}

/// The active session's accumulated output — as one event. Background
/// sessions do not leak to the front end: their screen is for the app, not
/// for the human, and it is already here.
fn flush(app: &AppHandle, sessions: &mut HashMap<SessionId, Live>, active: Option<SessionId>) {
    for (id, live) in sessions.iter_mut() {
        if live.pending.is_empty() {
            continue;
        }
        if Some(*id) != active {
            live.pending.clear();
            continue;
        }
        // The resync anchor: the front end throws away what it has and
        // re-attaches the moment it sees a gap in this number.
        live.seq += 1;
        let data = base64::engine::general_purpose::STANDARD.encode(&live.pending);
        live.pending.clear();
        let _ = app.emit(
            "terminal:output",
            serde_json::json!({ "id": id, "seq": live.seq, "data": data }),
        );
    }
}

/// Recomputing state from the screen and the timings. The event goes out only
/// on a change — a row that repaints sixty times a second is of no use to
/// anyone.
///
/// This is also where a session stops being `Starting`: detection knows only
/// `Running`, `Idle` and `NeedsYou`, so the first tick after creation moves
/// it on. `Starting` is therefore brief, which is right — it means "created,
/// nothing seen yet", and a session that failed to spawn never gets here to
/// be called running.
fn reassess(app: &AppHandle, sessions: &mut HashMap<SessionId, Live>) {
    for live in sessions.values_mut() {
        if live.session.state == SessionState::Exited {
            // The second poll for an exit code, and the only one there is.
            // End of stream can arrive before the child has been reaped, and
            // `exit_code` is a non-blocking `try_wait` that answers `None`
            // until it has been — so the poll in `absorb` loses that race
            // often enough to matter. Without asking again the session would
            // keep `exitCode: null` for good, and the front end, which tells
            // done from failed by exactly that number, could tell neither.
            if live.session.exit_code.is_none() {
                if let Some(code) = live.pty.exit_code() {
                    live.session.finish(Some(code));
                    emit_state(app, &live.session);
                }
            }
            continue;
        }
        let lines = live.screen.lines();
        // The screen against the one this session showed last tick — the
        // clock is restarted by a change to the picture, not by the arrival of
        // bytes. `into_std` because `Quiet` keeps no clock of its own and is
        // driven from here, which is what makes it testable without sleeping.
        let still_for = live.quiet.still_for(&lines, Instant::now().into_std());
        let out = detect(DetectInput {
            bell_pending: live.bell_pending,
            still_for,
            screen: &lines,
            // The one fact layer A needs about where this picture comes from:
            // a rendered transcript holds still between tool calls without the
            // agent having stopped. See `DetectInput::transcript`.
            transcript: live.transcript.is_some(),
            profile: live.profile,
            // The state as it stands *before* this tick's `apply`, which is
            // the whole of what layer B's threshold is asymmetric about: a
            // session already waiting on a person keeps that reading while
            // they type an answer into it. See `detect`.
            was: live.session.state,
        });
        let before = (live.session.state, live.session.question.clone());
        live.session.apply(out.state, out.question);
        if before != (live.session.state, live.session.question.clone()) {
            emit_state(app, &live.session);
        }
    }
}

/// On every tick, check each unclosed capture: no output for `settle` — hand
/// the screen over; the deadline has passed — hand back Timeout. The answer
/// goes out exactly once.
fn close_captures(captures: &mut Vec<Capture>, sessions: &HashMap<SessionId, Live>) {
    let now = Instant::now();
    let mut keep = Vec::new();
    for capture in captures.drain(..) {
        match sessions.get(&capture.id) {
            None => {
                let _ = capture.tx.send(Err(TerminalError::NoSession(capture.id)));
            }
            Some(live) if live.last_output.elapsed() >= capture.settle => {
                let _ = capture.tx.send(Ok(live.screen.lines()));
            }
            Some(_) if now >= capture.deadline => {
                let _ = capture.tx.send(Err(TerminalError::Timeout));
            }
            Some(_) => keep.push(capture),
        }
    }
    *captures = keep;
}

/// Answer whoever is waiting for a session to end. Run on the same tick as the
/// captures, and deliberately after `reassess`, so a code that arrived on this
/// tick is handed over on this tick rather than the next.
fn close_exit_waiters(waiters: &mut Vec<ExitWaiter>, sessions: &HashMap<SessionId, Live>) {
    let now = Instant::now();
    let mut keep = Vec::new();
    for mut waiter in waiters.drain(..) {
        match sessions.get(&waiter.id) {
            // Gone from the map, and `Request::Remove` is the only thing that
            // takes a session out of it — so this is a person removing the row,
            // never a process falling over. Said as its own answer rather than
            // as a missing exit code: the waiter cannot tell the two apart from
            // here, and the responses they deserve are opposite ones.
            None => {
                let _ = waiter.tx.send(Exit::Removed);
            }
            Some(live) if live.session.state != SessionState::Exited => keep.push(waiter),
            Some(live) => match live.session.exit_code {
                Some(code) => {
                    let _ = waiter.tx.send(Exit::Code(code));
                }
                None => {
                    let deadline = *waiter.grace.get_or_insert(now + EXIT_CODE_GRACE);
                    if now >= deadline {
                        let _ = waiter.tx.send(Exit::NoCode);
                    } else {
                        keep.push(waiter);
                    }
                }
            },
        }
    }
    *waiters = keep;
}

/// Where a shell is to start: the project's own root when nobody said, and
/// otherwise the folder named by a path relative to it.
///
/// The check is `files::fs::resolve_within`, the same one every other command
/// taking a path from the front end goes through — it refuses `..` and an
/// absolute path outright, and canonicalizes, so a symlink inside the project
/// pointing outward cannot open a shell on the rest of the disk. The extra
/// `is_dir` is not redundant with it: `resolve_within` answers about a path, and
/// a PTY handed a file for a working directory fails inside the child, where the
/// only thing left to show a person is a spawn error about a shell.
///
/// The root itself is deliberately not resolved when there is nothing to check
/// against it. Every other caller passes `None`, and canonicalizing on their
/// behalf would silently change what `Session::cwd` reads as — the project's
/// path as the front end knows it — for the sake of a check they do not need.
fn shell_cwd(root: &Path, rel: Option<&str>) -> Result<PathBuf, TerminalError> {
    let Some(rel) = rel else { return Ok(root.to_path_buf()) };
    let full = crate::files::fs::resolve_within(root, rel)
        .map_err(|err| TerminalError::BadCwd(err.to_string()))?;
    if !full.is_dir() {
        return Err(TerminalError::BadCwd(rel.to_owned()));
    }
    Ok(full)
}

/// Everything the worker does synchronously. `ShutDown` is handled by the
/// loop instead — see there.
fn handle(
    app: &AppHandle,
    sessions: &mut HashMap<SessionId, Live>,
    captures: &mut Vec<Capture>,
    exit_waiters: &mut Vec<ExitWaiter>,
    active: &mut Option<SessionId>,
    next_id: &mut SessionId,
    chunks: &mpsc::UnboundedSender<Chunk>,
    request: Request,
) {
    match request {
        Request::List(project, tx) => {
            let mut list: Vec<Session> = sessions
                .values()
                .filter(|l| l.session.project == project)
                .map(|l| l.session.clone())
                .collect();
            list.sort_by_key(|s| s.id);
            let _ = tx.send(list);
        }
        Request::Marks(tx) => {
            let mut list: Vec<SessionMark> = sessions
                .values()
                .map(|l| SessionMark {
                    id: l.session.id,
                    project: l.session.project.clone(),
                    state: l.session.state,
                    // The variant of the work and none of its payload: this is
                    // every session of every project, and a filing agent's
                    // `work` holds the whole of a person's draft prose.
                    kind: l.session.work.kind(),
                })
                .collect();
            list.sort_by_key(|m| m.id);
            let _ = tx.send(list);
        }
        Request::Create(project, agent, intent, tx) => {
            // The login shell's PATH, not this process's: a bundled app started
            // from Finder inherits launchd's, where nothing a person installed
            // is reachable and every agent would look uninstalled.
            let Some(profile) = agents::pick(&agent, crate::shell_env::path()) else {
                let _ = tx.send(Err(TerminalError::NoAgent(agents::IDS.join(", "))));
                return;
            };
            let id = *next_id;
            *next_id += 1;
            // Only a Setup session pays for the walk, and it happens here
            // rather than in the front end so that what the agent is told is
            // what the disk says at the moment the session starts.
            let facts = matches!(intent, agents::Intent::Setup).then(|| {
                // Before the agent writes anything, so the folder it is about
                // to create is already ignored when it appears rather than
                // after somebody has staged it. Failing costs a line in a
                // .gitignore; refusing to start the session over it would cost
                // the whole feature, so this is logged and stepped over.
                if let Err(err) = crate::runs::gitignore::ensure(Path::new(&project)) {
                    // Not ".smetana/": `ensure` writes whatever of its own
                    // list the file is missing, and that list has grown.
                    eprintln!("[runs] could not amend .gitignore: {err}");
                }
                crate::runs::survey::render(&crate::runs::survey::run(Path::new(&project)))
            });
            // Taken before the intent is handed to the launch, which consumes
            // it. Nothing else is kept from it: the rest is the agent's
            // briefing, not the panel's caption.
            let work = intent.work();
            let launch = agents::Launch {
                profile,
                cwd: PathBuf::from(&project),
                intent,
                skills: agents::library::resolve(app),
                facts,
                // Read from the file here, and here only: this is the one place
                // every session in the app is built, so a manual session and a
                // run's batch get the same answer by construction rather than
                // because two call sites were kept in step.
                //
                // Two costs, both accepted rather than discovered later. The
                // front end owns the truth of the settings and writes them on a
                // 400 ms debounce, so a session started in the same fraction of
                // a second as a language change reads the previous language —
                // one session, and the same lag `settings::agent(app)` already
                // lives with over in `runs::service`. (The `agent` parameter
                // above is not that: it comes down from the front end's live
                // store and has no lag at all.) And a run reads these per
                // batch, where it snapshots its agent and its whole
                // `RunSettings` once and carries them: a language changed at
                // 2am reaches the next batch, so one run's issues can end up
                // written in two languages. That is accepted and not an
                // oversight — snapshotting them onto `Intent::Run` would put
                // the languages back on a second road into a session, which is
                // exactly what reading them here exists to prevent.
                languages: crate::settings::languages(app),
            };
            let spawned = Pty::spawn(id, &launch, DEFAULT_COLS, DEFAULT_ROWS, chunks.clone());
            let _ = tx.send(match spawned {
                Ok(pty) => {
                    // The name of what actually started, which is not always
                    // the name that was asked for: `pick` falls back to an
                    // installed agent, and the row in the panel is where that
                    // becomes visible.
                    let session = Session::new(id, profile.id(), &project, &project, work);
                    let live = Live {
                        session: session.clone(),
                        profile: Some(profile),
                        pty,
                        ring: Ring::new(RING_CAP),
                        screen: Screen::new(DEFAULT_COLS, DEFAULT_ROWS),
                        transcript: agents::is_batch(&launch.intent)
                            .then(|| profile.transcript())
                            .flatten()
                            .map(crate::terminal::transcript::Transcript::new),
                        bell_pending: false,
                        quiet: Quiet::new(),
                        last_output: Instant::now(),
                        pending: Vec::new(),
                        seq: 0,
                    };
                    sessions.insert(id, live);
                    emit_state(app, &session);
                    Ok(session)
                }
                // The agent is not on PATH — a legitimate outcome, not a
                // panic. Nothing is inserted and the failure goes back as the
                // command's error: an id was spent, but no half-built session
                // is left behind for the list to carry.
                Err(err) => Err(err),
            });
        }
        Request::CreateShell(project, cwd, tx) => {
            // Everything the agent branch above does about *which* agent — the
            // pick, the profile, the skills, the languages, the facts — has no
            // counterpart here, and that absence is the whole of what a shell
            // is. What the two do share is the environment, and they share it as
            // one piece of code rather than as two copies: see
            // `pty::apply_environment`.
            let root = PathBuf::from(&project);
            let dir = shell_cwd(&root, cwd.as_deref());
            let id = *next_id;
            *next_id += 1;
            // The id is spent whether or not the directory was usable, exactly
            // as it is for a shell that would not start: ids are a counter, not
            // a resource, and reusing one is how two sessions come to share it.
            let spawned = dir.and_then(|dir| {
                Pty::spawn_shell(id, &dir, DEFAULT_COLS, DEFAULT_ROWS, chunks.clone())
                    .map(|pty| (pty, dir))
            });
            let _ = tx.send(match spawned {
                Ok((pty, dir)) => {
                    // `agent` is the field a session's row would have been
                    // captioned by before rows were captioned by their work, and
                    // nothing draws it now. The shell's own program is the honest
                    // answer for anything reading a session dump.
                    // `cwd` and `project` differ here for the first time: a
                    // shell opened from a folder in the tree runs in that folder
                    // and still belongs to the project it was opened from, which
                    // is what the tab row, the rail and `List` all read.
                    let session = Session::new(
                        id,
                        &crate::shell_env::shell(),
                        &dir.to_string_lossy(),
                        &project,
                        crate::terminal::model::SessionWork::Shell,
                    );
                    let live = Live {
                        session: session.clone(),
                        // No agent, so no layer B — see `DetectInput::profile`.
                        profile: None,
                        pty,
                        ring: Ring::new(RING_CAP),
                        screen: Screen::new(DEFAULT_COLS, DEFAULT_ROWS),
                        // A shell prints for the person in front of it and for
                        // nobody else; there is no machine format to translate.
                        transcript: None,
                        bell_pending: false,
                        quiet: Quiet::new(),
                        last_output: Instant::now(),
                        pending: Vec::new(),
                        seq: 0,
                    };
                    sessions.insert(id, live);
                    emit_state(app, &session);
                    Ok(session)
                }
                // A shell that would not start is the same kind of outcome an
                // agent that is not installed is: an id was spent and nothing
                // is left in the map.
                Err(err) => Err(err),
            });
        }
        Request::Remove(id, tx) => {
            if let Some(mut live) = sessions.remove(&id) {
                live.pty.kill();
                // Announced, because not every removal is the front end's own
                // doing any more: a run kills the session of a batch that
                // stopped on a question (`runs/service.rs`), and without an
                // event the panel would keep drawing that session's last
                // state — `needs-you`, with a question nobody can answer
                // behind a process that is gone — until the next project
                // switch. The front end's own remove has already dropped the
                // row by the time this arrives, and filtering an absent id is
                // a no-op, which is what makes one event serve both callers.
                let _ = app.emit("terminal:removed", serde_json::json!({ "id": id }));
            }
            if *active == Some(id) {
                *active = None;
            }
            let _ = tx.send(());
        }
        Request::Attach(id, tx) => {
            let _ = tx.send(match sessions.get_mut(&id) {
                None => Err(TerminalError::NoSession(id)),
                Some(live) => {
                    *active = Some(id);
                    // A human now has this session on screen, and that is a
                    // reasonable acknowledgement of a bell. It has to be
                    // acknowledged by something: a CLI agent rings on
                    // finishing a task just as readily as on asking a
                    // question, and with only a write and an exit clearing
                    // it, one completion bell would hold the row at
                    // `needs-you` for the rest of the session's life and
                    // refuse every `run_capture` with `busy` alongside it —
                    // an afternoon of that and every row is loud, which is
                    // the design failing quietly.
                    //
                    // Clearing it here is safe because the two detection
                    // layers are independent: if a permission dialog really
                    // is on the screen, the profile raises `needs-you` again
                    // from that screen on the next tick. A real question
                    // survives the acknowledgement; only a bell whose reason
                    // has already been read is lost.
                    live.bell_pending = false;
                    // Everything accumulated before attaching is already in
                    // the ring — sending it as a second event would mean
                    // showing it twice.
                    live.pending.clear();
                    Ok(Attached {
                        data: base64::engine::general_purpose::STANDARD.encode(live.ring.snapshot()),
                        seq: live.seq,
                    })
                }
            });
        }
        // Only when `active` still points at that session. Switching tabs is
        // two calls — detach the old, attach the new — with no ordering
        // guarantee by the time they reach here, and a detach that landed
        // after the attach would clear it: the front end would believe it was
        // attached while `flush` threw away that session's output every tick
        // and `seq` never moved. Nothing would report it — the terminal would
        // simply be frozen.
        Request::Detach(id) => {
            if *active == Some(id) {
                *active = None;
            }
        }
        Request::Resize(id, cols, rows) => {
            if let Some(live) = sessions.get_mut(&id) {
                live.pty.resize(cols, rows);
                // The same size goes into the parsing: the app is obliged to
                // read the screen the human sees.
                live.screen.resize(cols, rows);
            }
        }
        Request::Write(id, data, tx) => {
            let _ = tx.send(match sessions.get_mut(&id) {
                None => Err(TerminalError::NoSession(id)),
                Some(live) => {
                    live.pty.write(data.as_bytes());
                    // The human has answered — there is nobody left to call.
                    live.bell_pending = false;
                    Ok(())
                }
            });
        }
        Request::RunCapture(id, input, settle_ms, timeout_ms, tx) => {
            match sessions.get_mut(&id) {
                None => {
                    let _ = tx.send(Err(TerminalError::NoSession(id)));
                }
                // Writing into an open permission dialog would mean
                // answering, on a human's behalf, a question the app never
                // read. The state alone is not a tight enough guard: it lags
                // by up to `SETTLE` plus a tick, and a dialog that appeared
                // inside that window is just as open. An unrung-out bell is
                // the same fact arriving sooner — it is already on the `Live`,
                // and only a human clears it: by answering, or by putting the
                // session on screen and reading what it rang about.
                //
                // What this does not catch, and cannot: a dialog whose agent
                // rang no bell and whose profile failed to read it. Layer A now
                // calls that session `Idle` rather than `Running`, which is the
                // truth but not a refusal — an idle session is exactly what a
                // capture expects to write into, so `Idle` can never be part of
                // this guard. Only the profile can tell the two apart.
                Some(live) if live.session.state == SessionState::NeedsYou || live.bell_pending => {
                    let _ = tx.send(Err(TerminalError::Busy));
                }
                Some(live) => {
                    live.pty.write(input.as_bytes());
                    // The count of silence starts over: a screen that settled
                    // before our write is not an answer to it. Both clocks —
                    // the capture is waiting on the stream, detection on the
                    // picture, and neither may credit this session for having
                    // been quiet before we spoke to it.
                    let now = Instant::now();
                    live.last_output = now;
                    live.quiet.restart(now.into_std());
                    // Waiting inline would stop the worker serving every
                    // other session for as long as the wait; the tick closes
                    // this instead.
                    captures.push(Capture {
                        id,
                        settle: Duration::from_millis(settle_ms),
                        // Clamped first, so a caller's absurd number costs an
                        // hour rather than a capture that outlives the app.
                        // And `checked_add` rather than plain addition
                        // because that panics on an unrepresentable instant,
                        // and a panic here unwinds the whole loop body: past
                        // `kill_all`, so every PTY is orphaned, and out of
                        // the worker, so every later command reports it gone.
                        // With the clamp in front of it that fallback is
                        // unreachable in practice; closing on the next tick
                        // is simply the harmless way to be wrong.
                        deadline: now
                            .checked_add(Duration::from_millis(timeout_ms).min(CAPTURE_CEILING))
                            .unwrap_or(now),
                        tx,
                    });
                }
            }
        }
        Request::Group(id, tx) => {
            let _ = tx.send(sessions.get(&id).and_then(|live| live.pty.pid()));
        }
        Request::AwaitExit(id, tx) => {
            // No answer here even for a session that is already gone: the tick
            // is the only place that knows whether an exit code is still on its
            // way, and duplicating that judgement here is how the two would
            // drift apart. A session that ended long ago is answered on the
            // next tick, sixteen milliseconds later.
            exit_waiters.push(ExitWaiter { id, tx, grace: None });
        }
        // Handled by the loop, which is the only place that can await the
        // grace period; it never reaches here.
        Request::ShutDown(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory of its own per test, with the pid in the name so parallel
    /// runs do not collide — the same trick `files/fs.rs` uses, canonical for
    /// the same reason: on macOS /var is a symlink to /private/var.
    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("smetana-shell-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the temp directory");
        dir.canonicalize().expect("canonicalize the temp directory")
    }

    #[test]
    fn no_cwd_is_the_project_root() {
        let root = scratch("root");
        assert_eq!(shell_cwd(&root, None).unwrap(), root);
    }

    #[test]
    fn a_folder_in_the_project_is_where_the_shell_starts() {
        let root = scratch("folder");
        std::fs::create_dir_all(root.join("src/components")).unwrap();
        assert_eq!(shell_cwd(&root, Some("src/components")).unwrap(), root.join("src/components"));
    }

    #[test]
    fn a_path_outside_the_project_is_refused() {
        let root = scratch("outside");
        std::fs::create_dir_all(root.join("src")).unwrap();
        for rel in ["../..", "..", "src/../..", "/etc"] {
            assert!(
                matches!(shell_cwd(&root, Some(rel)), Err(TerminalError::BadCwd(_))),
                "{rel} should not be a working directory"
            );
        }
    }

    #[test]
    fn a_file_is_refused_rather_than_handed_to_the_pty() {
        let root = scratch("file");
        std::fs::write(root.join("app.js"), "x").unwrap();
        assert!(matches!(shell_cwd(&root, Some("app.js")), Err(TerminalError::BadCwd(_))));
    }

    #[test]
    fn a_folder_that_is_not_there_is_refused() {
        let root = scratch("missing");
        assert!(matches!(shell_cwd(&root, Some("nowhere")), Err(TerminalError::BadCwd(_))));
    }

    /// The empty string is the tree's own name for the root, and it arrives as
    /// a `Some` from a menu opened on the root row rather than on empty space.
    #[test]
    fn the_empty_path_is_the_root() {
        let root = scratch("empty");
        assert_eq!(shell_cwd(&root, Some("")).unwrap(), root);
    }
}
