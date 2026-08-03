//! The terminal worker: a single owner of mutable state. Commands, output
//! chunks from the reader threads and the detection tick meet in one
//! `select!` — the same reason as in the tracker: operations of
//! unpredictable length must not block one another.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, MissedTickBehavior};

use super::detect::{detect, DetectInput};
use super::model::{Session, SessionId, SessionState, TerminalError};
use super::pty::{Chunk, Pty};
use super::ring::Ring;
use super::screen::Screen;

/// How much raw output every session remembers — this is what xterm.js
/// repaints itself from when it attaches.
const RING_CAP: usize = 1024 * 1024;
/// The geometry of a session that has never been shown yet.
const DEFAULT_COLS: u16 = 120;
const DEFAULT_ROWS: u16 = 30;
/// Output is coalesced into this tick, so that one event does not go out per
/// chunk.
const FLUSH: Duration = Duration::from_millis(16);

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
    Create(String, oneshot::Sender<Result<Session, TerminalError>>),
    Remove(SessionId, oneshot::Sender<()>),
    Attach(SessionId, oneshot::Sender<Result<Attached, TerminalError>>),
    Detach,
    Resize(SessionId, u16, u16),
    Write(SessionId, String, oneshot::Sender<Result<(), TerminalError>>),
    RunCapture(SessionId, String, u64, u64, oneshot::Sender<Result<Vec<String>, TerminalError>>),
    ShutDown(oneshot::Sender<()>),
}

#[derive(Clone)]
pub struct TerminalHandle(pub mpsc::Sender<Request>);

struct Live {
    session: Session,
    pty: Pty,
    ring: Ring,
    screen: Screen,
    /// A bell rang and has not been cleared yet. It is cleared by a write
    /// into the session — a human answering, from the keyboard or with a
    /// button — and by the process exiting. Detection only reads it.
    bell_pending: bool,
    last_output: Instant,
    /// Accumulated since the last flush — leaves as a single event on the
    /// tick.
    pending: Vec<u8>,
    seq: u64,
}

/// An unclosed capture: wait until this session's screen has held still for
/// `settle`, then hand back its lines. It lives in the worker's loop rather
/// than in a task of its own, because the screen is the worker's state and is
/// never handed out.
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
        let mut active: Option<SessionId> = None;
        let mut next_id: SessionId = 1;
        let mut tick = tokio::time::interval(FLUSH);
        tick.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                request = rx.recv() => {
                    // The senders are gone — there is nobody left to work for.
                    let Some(request) = request else { break };
                    if handle(&app, &mut sessions, &mut captures, &mut active, &mut next_id, &chunks_tx, request) {
                        break;
                    }
                }
                chunk = chunks_rx.recv() => {
                    let Some(chunk) = chunk else { continue };
                    absorb(&app, &mut sessions, chunk);
                }
                _ = tick.tick() => {
                    flush(&app, &mut sessions, active);
                    reassess(&app, &mut sessions);
                    close_captures(&mut captures, &sessions);
                }
            }
        }

        // Closing the worker: an agent left alive is an orphan in the process
        // list.
        kill_all(&mut sessions);
    });

    TerminalHandle(tx)
}

fn kill_all(sessions: &mut HashMap<SessionId, Live>) {
    for live in sessions.values_mut() {
        live.pty.kill();
    }
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
            live.ring.push(&bytes);
            if live.screen.feed(&bytes) {
                live.bell_pending = true;
            }
            live.pending.extend_from_slice(&bytes);
            live.last_output = Instant::now();
            if live.session.state == SessionState::Starting {
                live.session.apply(SessionState::Running, None);
                emit_state(app, &live.session);
            }
        }
        Chunk::Gone(id, _) => {
            let Some(live) = sessions.get_mut(&id) else { return };
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
fn reassess(app: &AppHandle, sessions: &mut HashMap<SessionId, Live>) {
    for live in sessions.values_mut() {
        if live.session.state == SessionState::Exited {
            continue;
        }
        let lines = live.screen.lines();
        let out = detect(DetectInput {
            bell_pending: live.bell_pending,
            quiet_for: live.last_output.elapsed(),
            screen: &lines,
            // An exited session never gets here — it was skipped above.
            alive: true,
        });
        let before = (live.session.state, live.session.question.clone());
        live.session.apply(out.state, out.question);
        if before != (live.session.state, live.session.question.clone()) {
            emit_state(app, &live.session);
        }
    }
}

/// On every tick, check each unclosed capture: the screen has held still for
/// `settle` — hand it over; the deadline has passed — hand back Timeout. The
/// answer goes out exactly once.
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

/// Returns true when it is time for the worker to stop.
fn handle(
    app: &AppHandle,
    sessions: &mut HashMap<SessionId, Live>,
    captures: &mut Vec<Capture>,
    active: &mut Option<SessionId>,
    next_id: &mut SessionId,
    chunks: &mpsc::UnboundedSender<Chunk>,
    request: Request,
) -> bool {
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
        Request::Create(project, tx) => {
            let id = *next_id;
            *next_id += 1;
            let dir = PathBuf::from(&project);
            let spawned = Pty::spawn(id, "claude", &dir, DEFAULT_COLS, DEFAULT_ROWS, chunks.clone());
            let _ = tx.send(match spawned {
                Ok(pty) => {
                    let session = Session::new(id, "claude", &project, &project);
                    let live = Live {
                        session: session.clone(),
                        pty,
                        ring: Ring::new(RING_CAP),
                        screen: Screen::new(DEFAULT_COLS, DEFAULT_ROWS),
                        bell_pending: false,
                        last_output: Instant::now(),
                        pending: Vec::new(),
                        seq: 0,
                    };
                    sessions.insert(id, live);
                    emit_state(app, &session);
                    Ok(session)
                }
                // The agent is not on PATH — a legitimate outcome, not a
                // panic: the row appears already dead and with a reason.
                Err(err) => Err(err),
            });
        }
        Request::Remove(id, tx) => {
            if let Some(mut live) = sessions.remove(&id) {
                live.pty.kill();
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
        Request::Detach => *active = None,
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
                // read.
                Some(live) if live.session.state == SessionState::NeedsYou => {
                    let _ = tx.send(Err(TerminalError::Busy));
                }
                Some(live) => {
                    live.pty.write(input.as_bytes());
                    // The count of silence starts over: a screen that settled
                    // before our write is not an answer to it.
                    live.last_output = Instant::now();
                    // Waiting inline would stop the worker serving every
                    // other session for as long as the wait; the tick closes
                    // this instead.
                    captures.push(Capture {
                        id,
                        settle: Duration::from_millis(settle_ms),
                        deadline: Instant::now() + Duration::from_millis(timeout_ms),
                        tx,
                    });
                }
            }
        }
        Request::ShutDown(tx) => {
            kill_all(sessions);
            let _ = tx.send(());
            return true;
        }
    }
    false
}
