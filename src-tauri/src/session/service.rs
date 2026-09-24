//! The driven-session worker: one tokio task that owns every mutable thing
//! about a conversation — the child, its driver, its journal — and serialises
//! everything that touches them.
//!
//! The same shape as `tracker::service` and `terminal::service`, for the same
//! reason: a command, a chunk off a child's stdout and a question from the
//! permission listener all arrive from different places, and letting them share
//! state behind a lock would mean one of them waiting on another for an
//! unpredictable stretch. Here they meet in one `select!` and are handled one
//! at a time.
//!
//! What is deliberately unlike the terminal: the child runs over **pipes**, not
//! a PTY. There is nothing to emulate — no screen, no geometry, no
//! `terminal_resize` — because what comes back is a protocol rather than a
//! picture. And events go out for **every** session rather than the attached
//! one alone: an event is a bounded object, not a repaint, so a background
//! session's turn result is worth having by the time its tab is opened. There
//! is no equivalent of the terminal's `flush()` dropping a background session's
//! bytes.
//!
//! No unit test lives here, and one is deliberately not added: this is I/O and
//! orchestration, the standing the three existing workers already have. The
//! rules that can be checked without a process are in `model.rs` and
//! `journal.rs`, and each carries its own tests.
//!
//! **One thing here is written to disk, and it is the same file the terminal
//! worker writes**: `.smetana/agents.json`, through `terminal::restore`. A
//! driven session's *process* still dies with the app, exactly as a PTY
//! session's does; what survives is the record that offers the conversation
//! back, and it is deliberately not marked with which of the two roads made it.
//! An offline row is taken up by whichever road the project's agent can drive
//! *now* — `conversation_for` in `terminal::service` is the one decision about
//! whether a session is recorded and under what name, asked from here rather
//! than restated.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use portable_pty::CommandBuilder;
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Child;
use tokio::sync::{mpsc, oneshot};

use super::driver::{Driver, Input, LineBuffer};
use super::crew::{CrewNode, CrewPackage};
use super::journal::Journal;
use super::model::{
    is_open_question, state_of, Decision, Event, EventKind, SessionError, SessionId, SessionState,
    StateChange,
};
use super::permission::{Asked, PermissionServer};
use crate::agents::claude_driver::ClaudeDriver;
use crate::agents::codex_driver::CodexDriver;
use crate::agents::{self, Intent, Launch, Profile};

/// How much of a child's stdout is taken in one read. A turn's output arrives
/// as JSONL and is cut into lines by the driver's own `LineBuffer`, so this
/// number decides nothing but how often the worker is woken.
const READ_CHUNK: usize = 16 * 1024;

/// The longest line of a child's stderr that reaches the log. It is the
/// harness's own diagnostics — a stack trace, a deprecation notice — and a
/// harness that decides to print a megabyte of it must not put a megabyte in
/// somebody's log file.
const MAX_STDERR_LINE: usize = 4 * 1024;

/// What a person is told when their words did not reach the agent. Two
/// sentences for two different facts, and each is said twice — once in the
/// conversation, where the answer would have been, and once as the command's
/// own error, because the reply is the only thing the caller sees at the moment
/// it happens.
const ENDED: &str = "This session has ended, so the message was not delivered.";
const UNREACHABLE: &str =
    "This session's agent could not be reached, so the message was not delivered.";

/// How long the exit path waits for the worker to kill its children. The same
/// ceiling `terminal::service::shutdown` puts on its own wait, for the same
/// reason: the app always exits, and a wedged worker costs a cleanup rather
/// than the app.
const SHUTDOWN_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

/// The answer to `session_attach`: the whole conversation so far, the number
/// events arriving after it continue from, and where the session stands.
///
/// The journal is handed over whole rather than from a cursor, and that is the
/// point of it living in Rust: a window opened for the second time gets the
/// same conversation as the first, and a window that was never opened has
/// missed nothing.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attached {
    pub events: Vec<Event>,
    pub seq: u64,
    pub state: SessionState,
    /// The same value `session:state` carries, and here for the window between
    /// the two. `session_start` answers with the worker's own session number
    /// and nothing else, and the next thing a store does is attach — while the
    /// first state *change* may be a whole turn away, since `session:state`
    /// goes out on a change and never on a repeat. Without this the row a
    /// person has just pressed a button for would be drawn under a key that
    /// changes under it the moment the agent first speaks.
    pub conversation: Option<String>,
    /// `Live::cwd` — the directory this session actually runs in, the project
    /// root for every intent but a resumed worktree session. `Markdown.vue`
    /// reads it as `base`, the directory a relative illustration in the
    /// agent's own prose resolves from; the task inspector has no session and
    /// falls back to the project root the same way this field would if the
    /// worker ever answered one empty.
    pub cwd: String,
}

/// A snapshot of exactly one native Crew node. Node ids stay scoped by `root`
/// on every command/event, so a stable Smetana id is never mistaken for an
/// ordinary session id or a provider thread id.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CrewAttached {
    pub events: Vec<Event>,
    pub seq: u64,
    pub state: super::crew::CrewState,
    pub cwd: String,
}

pub enum Request {
    Start(String, Intent, oneshot::Sender<Result<SessionId, SessionError>>),
    /// Start a driven lead and register its Smetana-owned root in one worker
    /// pass. It intentionally bypasses `Request::Start`'s ordinary Run
    /// refusal: a Crew Run is not a normal conversation, but its Codex
    /// app-server transport is still a structured protocol, never a terminal.
    CrewStart(
        String,
        Intent,
        String,
        oneshot::Sender<Result<(u64, SessionId), SessionError>>,
    ),
    CrewTree(u64, oneshot::Sender<Option<Vec<CrewNode>>>),
    CrewAttach(u64, u64, oneshot::Sender<Result<CrewAttached, SessionError>>),
    CrewSend(u64, u64, String, oneshot::Sender<Result<(), SessionError>>),
    /// End an entire native package from the Crew composer. Native child
    /// agents do not expose a safe interrupt verb, so the front end only
    /// offers this on the root and it follows the same cleanup path as close.
    CrewStop(u64, oneshot::Sender<Result<(), SessionError>>),
    /// Wait for a driven Crew root's provider lead to end. This is the run
    /// worker's completion channel; it is distinct from removing the tree.
    CrewAwaitExit(u64, oneshot::Sender<crate::terminal::model::Exit>),
    CrewClear(u64),
    Attach(SessionId, oneshot::Sender<Result<Attached, SessionError>>),
    /// Everything after `seq`, or `None` when the journal no longer holds it —
    /// which is the front end's cue to take a fresh snapshot rather than draw a
    /// conversation with a hole in it.
    Since(SessionId, u64, oneshot::Sender<Result<Option<Vec<Event>>, SessionError>>),
    Send(SessionId, String, Vec<String>, oneshot::Sender<Result<(), SessionError>>),
    /// A person's answer. The last field is `AskUserQuestion`'s own —
    /// `Some` only when a question was answered with chosen or typed text
    /// rather than a plain allow/deny — and travels through unchanged to both
    /// the journal (`EventKind::PermissionAnswered`) and the permission
    /// listener, which is what builds `updatedInput` from it.
    Answer(SessionId, String, Decision, Option<BTreeMap<String, String>>, oneshot::Sender<Result<(), SessionError>>),
    /// End the turn in flight and nothing more. Claude Code keeps its child
    /// alive after its stdin control request; Codex queues an app-server
    /// interrupt until `turn/start` returns its turn id. A harness with no
    /// protocol interrupt still falls to `start_kill()`.
    Stop(SessionId, oneshot::Sender<Result<(), SessionError>>),
    /// End the session outright: the cross on a driven agent row, not the
    /// composer's Stop (smetana-y7mv). Always kills the child regardless of
    /// what a driver's `interrupt` answers, so the cleanup `Chunk::Eof` already
    /// does — forgetting the permission token, dropping the `.smetana/
    /// agents.json` record, deleting the `--mcp-config` file — still runs. A
    /// harness whose `interrupt` now leaves the turn open and the child alive
    /// needs a real end distinct from `Stop`, which this is.
    Close(SessionId, oneshot::Sender<Result<(), SessionError>>),
    /// The one reply that is not a `oneshot`: it is awaited from the exit
    /// event, on a synchronous thread, and only `std::sync::mpsc` can put a
    /// ceiling on a blocking receive. The same shape the terminal's has, for
    /// the same reason.
    ShutDown(std::sync::mpsc::Sender<()>),
}

#[derive(Clone)]
pub struct SessionHandle(pub mpsc::Sender<Request>);

/// What a reader task has to say about its child.
enum Chunk {
    Data(SessionId, Vec<u8>),
    /// The child's stdout ended, which for a harness reading a turn off stdin
    /// is the process leaving. The exit status is reaped by a task of its own —
    /// see `Chunk::Eof`'s arm.
    Eof(SessionId),
}

/// The two halves of talking to a child, held together because they are wanted
/// together and released together.
struct Talking {
    /// The codec. It also holds the `--mcp-config` file naming this session's
    /// permission channel, so dropping it is what deletes that file — and the
    /// file carries the session's bearer token.
    driver: Box<dyn Driver>,
    /// Bytes for the child's stdin, by way of a task of its own. The worker
    /// never awaits a pipe write: a child that has stopped reading would
    /// otherwise wedge every other session's commands behind it.
    stdin: mpsc::UnboundedSender<Vec<u8>>,
}

struct Live {
    /// Startup failed after spawning: retain only until stdout EOF reaps it.
    discard_on_eof: bool,
    /// `None` once the child's stdout has ended. Nothing is left to decode and
    /// nothing can be written, so holding either half would only keep a token
    /// file in `/tmp` and a task parked on a dead process's stdin for the life
    /// of the app. The journal outlives it: a session that has ended is still
    /// one somebody opens a tab on to read.
    talking: Option<Talking>,
    journal: Journal,
    /// The child, until its stdout ends. Taken out then and handed to the task
    /// that reaps it: a process that has closed its output is one this app has
    /// no further use for a handle on, and leaving it here would leave a zombie
    /// for the life of the app.
    child: Option<Child>,
    child_alive: bool,
    /// The last state emitted for this session. `session:state` goes out on a
    /// change and never on a repeat.
    state: SessionState,
    /// The conversation this session is recorded under, or `None` for one with
    /// no record — a fork, and a machine that would not give the random bytes.
    /// It is on the wire with every state change and it is what the record in
    /// `.smetana/agents.json` is keyed by, which is also why the `Eof` arm can
    /// take that record away without holding a second copy of the id.
    conversation: Option<String>,
    /// The project folder, which is where that registry lives. The worker knows
    /// a session by a number, so this is the only thing left here that can find
    /// the file again once the child has gone.
    project: String,
    /// Where this session actually runs — `session_cwd`'s answer, the project
    /// root for every intent but `ResumeSession`, whose own `cwd` names a
    /// worktree. Carried on `Attached` so the front end can resolve a relative
    /// illustration in the agent's prose against the directory the agent is
    /// actually sitting in, `Markdown.vue`'s own `base` — the project root
    /// alone is the wrong answer for exactly the session this field exists to
    /// name.
    cwd: String,
    /// The three fields a `Restorable` record needs beside the conversation
    /// id and the fields above, kept here rather than only at the spawn: a
    /// harness that picks its own id (`Driver::discovered_id`, Codex's
    /// app-server) has none to record at the spawn at all, so the record has
    /// to be built again once `note_conversation` learns one — and it needs
    /// the same three values `spawn_session`'s own first attempt used, not a
    /// second guess at them. `terminal::service`'s `Live` carries these for
    /// the identical reason, as part of the `Session` it never lets go of.
    agent: String,
    work: crate::terminal::model::SessionWork,
    started_at: String,
}

/// A Codex addressed turn remains pending until its JSON-RPC response confirms
/// the target thread accepted it. The provider id scopes the receipt; it is
/// never exposed to the UI and cannot be mistaken for a lead session id.
struct CrewSendWaiter {
    node: u64,
    text: String,
    reply: oneshot::Sender<Result<(), SessionError>>,
}

pub fn start(app: AppHandle) -> SessionHandle {
    let (tx, mut rx) = mpsc::channel::<Request>(32);
    let (chunks_tx, mut chunks_rx) = mpsc::unbounded_channel::<Chunk>();

    tauri::async_runtime::spawn(async move {
        let (asked_tx, mut asked_rx) = mpsc::channel::<Asked>(16);
        // One listener for the whole app, bound once here: `register` is what
        // gives each session its own URL and its own token. A machine that will
        // not give up a loopback port leaves every session without a permission
        // channel, which is a harness that declines what it would have asked
        // about rather than one that does something nobody agreed to.
        let permission = match PermissionServer::start(asked_tx).await {
            Ok(server) => Some(server),
            Err(error) => {
                log::error!("[session] the permission listener could not bind: {error}");
                None
            }
        };
        // With no listener there is no sender either, so the branch below would
        // be ready forever with nothing in it — a spinning core. The flag is
        // what keeps it out of the `select!`.
        let mut asked_open = permission.is_some();

        let mut sessions: HashMap<SessionId, Live> = HashMap::new();
        let mut crews: HashMap<u64, CrewPackage> = HashMap::new();
        let mut crew_ptys: HashMap<u64, crate::terminal::pty::Pty> = HashMap::new();
        let mut claude_crews: HashSet<u64> = HashSet::new();
        let mut claude_teams: HashMap<u64, PathBuf> = HashMap::new();
        let mut claude_team_baselines: HashMap<u64, HashSet<PathBuf>> = HashMap::new();
        let mut claude_tails: HashMap<(u64, PathBuf), crate::agents::claude_crew::TranscriptTail> = HashMap::new();
        let mut claude_lead_tails: HashMap<u64, crate::agents::claude_crew::TranscriptTail> = HashMap::new();
        let mut claude_transcript_nodes: HashMap<(u64, PathBuf), u64> = HashMap::new();
        let mut crew_tick = tokio::time::interval(std::time::Duration::from_millis(300));
        // Session id -> public Crew root for the special Codex lead whose
        // normal app-server stream also carries thread lifecycle records.
        let mut crew_leads: HashMap<SessionId, u64> = HashMap::new();
        // A child receives one `thread/read(includeTurns:true)` hydration when
        // it is first discovered. Its later live records stay separate.
        let mut codex_hydrated: HashSet<(u64, String)> = HashSet::new();
        let mut codex_reconciled: HashSet<u64> = HashSet::new();
        let mut crew_waiters: HashMap<u64, Vec<oneshot::Sender<crate::terminal::model::Exit>>> =
            HashMap::new();
        let mut crew_send_waiters: HashMap<(u64, String), CrewSendWaiter> = HashMap::new();
        let mut starting: HashMap<SessionId, oneshot::Sender<Result<SessionId, SessionError>>> = HashMap::new();
        // Crew admission is withheld until the structured provider transport
        // has created its root. A run must not claim work behind a failed
        // app-server initialize/thread handshake.
        let mut crew_starting: HashMap<SessionId, (u64, oneshot::Sender<Result<(u64, SessionId), SessionError>>)> = HashMap::new();
        let mut next_id: SessionId = 1;

        loop {
            tokio::select! {
                request = rx.recv() => {
                    // The senders are gone — there is nobody left to work for.
                    let Some(request) = request else { break };
                    if let Request::ShutDown(tx) = request {
                        kill_all(&mut sessions);
                        for pty in crew_ptys.values_mut() {
                            pty.kill();
                        }
                        let _ = tx.send(());
                        return;
                    }
                    handle(
                        &app,
                        &mut sessions,
                        &mut crews,
                        &mut crew_ptys,
                        &mut claude_crews,
                        &mut claude_teams,
                        &mut claude_team_baselines,
                        &mut claude_tails,
                        &mut claude_lead_tails,
                        &mut claude_transcript_nodes,
                        &mut crew_leads,
                        &mut crew_waiters,
                        &mut crew_send_waiters,
                        &mut next_id,
                        &mut starting,
                        &mut crew_starting,
                        permission.as_ref(),
                        &chunks_tx,
                        request,
                    );
                }
                chunk = chunks_rx.recv() => {
                    // Cannot happen while this task owns the sender it clones
                    // into every reader — and if that ownership ever moves,
                    // breaking is a stopped worker, whereas continuing is a
                    // branch that is instantly ready forever.
                    let Some(chunk) = chunk else { break };
                    absorb(&app, &mut sessions, &mut crews, &mut crew_leads, &mut crew_waiters, &mut crew_send_waiters, &mut codex_hydrated, &mut codex_reconciled, &mut starting, &mut crew_starting, permission.as_ref(), chunk);
                }
                asked = asked_rx.recv(), if asked_open => {
                    let Some(asked) = asked else {
                        // The listener is gone. Its sessions go on talking;
                        // they simply cannot be asked anything any more.
                        log::error!("[session] the permission listener stopped sending");
                        asked_open = false;
                        continue;
                    };
                    question(&app, &mut sessions, asked);
                }
                _ = crew_tick.tick(), if !claude_crews.is_empty() => {
                    refresh_claude_crews(&app, &mut crews, &claude_crews, &mut claude_teams, &claude_team_baselines, &mut claude_tails, &mut claude_lead_tails, &mut claude_transcript_nodes);
                    // An interactive Claude lead has no pipe reader to emit a
                    // `Chunk::Eof`. Poll its real child here so an exited root
                    // cannot retain its team tailers or a stale Crew row.
                    let exited: Vec<_> = crew_ptys
                        .iter_mut()
                        .filter_map(|(root, pty)| pty.exit_code().map(|code| (*root, code)))
                        .collect();
                    for (root, code) in exited {
                        clear_crew(
                            &app,
                            &mut sessions,
                            &mut crews,
                            &mut crew_ptys,
                            &mut claude_crews,
                            &mut claude_teams,
                            &mut claude_team_baselines,
                            &mut claude_tails,
                            &mut claude_lead_tails,
                            &mut claude_transcript_nodes,
                            &mut crew_leads,
                            &mut crew_waiters,
                            &mut crew_send_waiters,
                            root,
                            crate::terminal::model::Exit::Code(code),
                        );
                    }
                }
            }
        }

        // The worker lost its queue rather than being asked to stop. Nobody is
        // waiting on this, but a child left alive is an orphan in the process
        // list all the same.
        kill_all(&mut sessions);
    });

    SessionHandle(tx)
}

/// The exit path, called from `RunEvent::Exit` beside the terminal's.
///
/// A driven child is not a person's terminal: nothing of it is on a screen to
/// be flushed, and the whole conversation is already in a journal that dies
/// with the app anyway. So there is no grace period here — the app is leaving,
/// and what this is for is not leaving a `claude` process behind.
pub fn shutdown(app: &AppHandle) {
    let Some(handle) = app.try_state::<SessionHandle>() else { return };
    let (tx, rx) = std::sync::mpsc::channel();
    if handle.0.blocking_send(Request::ShutDown(tx)).is_err() {
        return;
    }
    let _ = rx.recv_timeout(SHUTDOWN_WAIT);
}

fn kill_all(sessions: &mut HashMap<SessionId, Live>) {
    for live in sessions.values_mut() {
        if let Some(child) = live.child.as_mut() {
            let _ = child.start_kill();
        }
    }
}

/// The profile built the line; this only moves it onto a type that can be
/// spawned over pipes. Nothing about a command line is decided here.
///
/// `None` only for a builder with no program in it, which no driver produces:
/// `CommandBuilder::new` puts the program at `argv[0]` and the only constructor
/// that does not is `new_default_prog`, which is a shell's and not an agent's.
fn spawnable(builder: &CommandBuilder) -> Option<tokio::process::Command> {
    let argv = builder.get_argv();
    let program = argv.first()?;
    let mut cmd = tokio::process::Command::new(program);
    cmd.args(&argv[1..]);
    if let Some(cwd) = builder.get_cwd() {
        cmd.current_dir(cwd);
    }
    for (key, value) in builder.iter_extra_env_as_str() {
        cmd.env(key, value);
    }
    cmd.stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    Some(cmd)
}

/// The codec for a harness, or `None` for one this app cannot drive.
///
/// Only the explicitly supported interactive harnesses have one. A profile without a codec is refused rather than
/// spawned: the child would run, say everything it had to say in a protocol
/// nothing here can read, and the conversation on screen would stay empty with
/// no error anywhere to explain it.
///
/// Both harnesses drive every intent this road ever sees — `_intent` is
/// unread — because `drivable` above already refuses `Intent::Run` before
/// this is ever called, the one intent no harness drives, and this function
/// answers a question about the *harness* rather than about the intent.
/// Codex used to be narrower, driving only `Bare` and `NewTask` while its
/// other manual intents kept the PTY fallback in `.claude/rules/terminal.md`;
/// that gate was here, on this arm, until every intent a person talks to
/// gained a Codex codec of its own (`CodexDriver`'s own `reopen`, for the one
/// intent — `ResumeSession` — that has nothing of a person's own to open on
/// but still has history worth showing before anybody types a word).
fn driver_for(
    profile: &'static dyn Profile,
    _intent: &Intent,
    ticket: Option<super::permission::PermissionTicket>,
) -> Option<Box<dyn Driver>> {
    match profile.id() {
        "claude" => Some(Box::new(ClaudeDriver::new(ticket))),
        "codex" => Some(Box::new(CodexDriver::new(ticket))),
        _ => None,
    }
}

/// Where a session started under this intent actually runs.
///
/// The project root for everything but a resume, and for a resume the directory
/// its transcript recorded: `claude --resume` resolves an id against the
/// directory it is run in, so the same id at the project root is a session
/// Claude Code has never heard of — and a worktree session reopened there would
/// be an agent reading a tree its own conversation never mentions.
///
/// **The rule is `sessions::model::resume_cwd`**, asked rather than restated:
/// the two roads refuse the same worktree, and a person who switched their
/// agent between one attempt and the next must not be told two different things
/// about one missing folder.
///
/// The refusal is `SessionError::BadCwd` and never a `Spawn`. A `Spawn`'s text
/// reaches a person unchanged, and this one's would be an internal sentence
/// with an absolute path in it; the tag is what lets
/// `stores/conversation.js` answer in the same words `stores/terminals.js`
/// answers the terminal's `BadCwd` with.
fn session_cwd(project: &str, intent: &Intent) -> Result<PathBuf, SessionError> {
    let root = PathBuf::from(project);
    let Intent::ResumeSession { cwd, .. } = intent else { return Ok(root) };
    crate::sessions::model::resume_cwd(&root, cwd)
        .ok_or_else(|| SessionError::BadCwd(cwd.to_owned()))
}

/// Start Claude's native team runtime on a PTY without ever consuming the PTY
/// output. Agent Teams is interactive by contract; topology and journals are
/// discovered from its structured files by the Crew watcher, so routing this
/// through `ClaudeDriver` would be a protocol lie.
fn spawn_claude_crew(
    app: &AppHandle,
    project: &str,
    intent: Intent,
    pinned_agent: &str,
) -> Result<crate::terminal::pty::Pty, SessionError> {
    let (_, model) = crate::settings::role_model(app, Some(project), &intent, None);
    let Some((profile, model)) = agents::pick_with_model(pinned_agent, model, crate::shell_env::path()) else {
        return Err(SessionError::Spawn(format!(
            "{} is not installed",
            crate::agents::claude::Claude.binary()
        )));
    };
    if profile.id() != "claude" {
        return Err(SessionError::NotDriven(
            "the configured Crew provider is not Claude Code".into(),
        ));
    }
    let cwd = session_cwd(project, &intent)?;
    let facts = crate::runs::setup_facts::for_intent(Path::new(project), &intent);
    let launch = Launch {
        profile,
        cwd: cwd.clone(),
        intent,
        skills: agents::library::resolve(app),
        languages: crate::settings::languages(app),
        agent_prompt: crate::settings::agent_prompt(app),
        facts,
        session_id: None,
        model,
        worker_model: None,
    };
    let mut command = crate::agents::claude_crew::interactive_lead_command(&launch);
    command.cwd(&cwd);
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }
    crate::terminal::pty::Pty::spawn_structured(command, profile.binary())
        .map_err(|error| SessionError::Spawn(error.to_string()))
}

/// Start a child for this session, or say why not.
fn spawn_session(
    app: &AppHandle,
    id: SessionId,
    project: &str,
    intent: Intent,
    permission: Option<&PermissionServer>,
    chunks: &mpsc::UnboundedSender<Chunk>,
    pinned_agent: Option<&str>,
) -> Result<Live, SessionError> {
    // The one resolver, read here for the reason `terminal::service` reads it
    // in its own `Create` arm: this is where a driven session is built, so what
    // a person configured is what starts, rather than a harness this file
    // picked for itself.
    let (configured_agent, model) = crate::settings::role_model(app, Some(project), &intent, None);
    let agent = pinned_agent.unwrap_or(&configured_agent);
    // The login shell's `PATH`, not this process's: a bundled app started from
    // Finder inherits launchd's, where nothing a person installed is reachable
    // and every agent would look uninstalled.
    let picked = agents::pick_with_model(agent, model, crate::shell_env::path());
    let Some((profile, model)) = picked else {
        return Err(SessionError::Spawn(format!(
            "none of these is installed: {}",
            agents::IDS.join(", ")
        )));
    };
    // The ticket is minted before the codec is asked for, and a refusal below
    // leaves it registered for the moment it takes `Request::Start` to hear
    // the error — that arm forgets the id on every failure, which is the one
    // place a session that never existed is swept up.
    let ticket = permission.map(|server| server.register(id));
    let Some(driver) = driver_for(profile, &intent, ticket) else {
        // Not an attempt that failed — the front door was wrong about a
        // capability, and `startAgent`'s PTY fallback exists exactly for
        // this tag (`SessionError::NotDriven`'s own header).
        return Err(SessionError::NotDriven(format!(
            "this action is not supported in the conversation panel for {}",
            profile.label()
        )));
    };

    // Refused before anything is spawned, and before the id below is spent:
    // a worktree removed once its task merged is the ordinary case rather
    // than an exotic one, and the transcript outlives it.
    let cwd = session_cwd(project, &intent)?;
    // The id this app writes the conversation under, and the one decision about
    // it in the app — `terminal::service::conversation_for`, asked here rather
    // than restated. A bare session gets a fresh one, a resume carries the one
    // it reopened, a fork gets none at all because `--fork-session` has the
    // harness invent an id this app never learns.
    let conversation = crate::terminal::service::conversation_for(profile, &intent);
    // A resume's id is already on its command line behind `--resume`, and the
    // profile refuses a second one beside it. This is the half that says the
    // worker never asks for it — the same pair of guards `terminal::service`
    // keeps, about two different things.
    let session_id = match &intent {
        Intent::ResumeSession { .. } => None,
        _ => conversation.clone(),
    };
    // The conversation so far, read before the spawn so that the journal a
    // window attaches to already holds it. Under `--input-format stream-json`
    // the harness replays nothing, so this is the whole of what an interactive
    // `--resume` would have painted. One file, read once, and a failure is an
    // empty history rather than a refusal.
    //
    // **Gated on the profile, not tried unconditionally.** `history::read` is
    // Claude Code's own transcript reader, its own file's header says so in
    // its first line; a Codex thread's id finds no such file under
    // `~/.claude/projects` and this call is a harmless wasted lookup for one
    // today, but which mechanism fills the journal has to be a fact about
    // the harness actually driving the session rather than an accident of
    // what a lookup off its id happens to find. Codex's own history arrives
    // over its own protocol instead, translated inside `CodexDriver` once
    // its `thread/read` answers — see `Driver::reopen`, called further down
    // this function, and `.claude/rules/conversation-panel.md`'s account of
    // the two roads.
    let past = match &intent {
        Intent::ResumeSession { id, .. } if profile.id() == "claude" => super::history::read(&cwd, id),
        _ => Vec::new(),
    };
    // Taken before the `Launch` moves the intent, and spent after the spawn has
    // succeeded: it is what captions the row, exactly as it does one worker
    // over, so a resumed conversation is not drawn as a bare agent.
    let work = intent.work();
    // The same walk the PTY worker makes for a setup session, through the one
    // function both call.
    let facts = crate::runs::setup_facts::for_intent(Path::new(project), &intent);
    // The person's own share of the brief, for the journal's opening turn.
    // Taken before the `Launch` consumes the intent, like `work` above.
    let (opening_text, opening_attachments) = intent.opening_words();

    let launch = Launch {
        profile,
        cwd: cwd.clone(),
        intent,
        skills: agents::library::resolve(app),
        // Read from the file here for the reason `terminal::service` reads them
        // there: a session built anywhere else in the app gets the same answers
        // by construction rather than because two call sites were kept in step.
        // They live with the front end's 400 ms debounce, which costs a session
        // started in the same fraction of a second as an edit the previous
        // value.
        languages: crate::settings::languages(app),
        agent_prompt: crate::settings::agent_prompt(app),
        facts,
        session_id,
        model,
        // A run's subagents, and this stage starts no run.
        worker_model: None,
    };

    // The command line is the driver's, and the working directory and the
    // environment are every agent's alike — the same division `terminal::pty`
    // keeps between a profile and the spawn around it.
    let mut builder = driver.start(&launch);
    builder.cwd(&launch.cwd);
    // The same `PATH` every non-PTY spawn in this tree runs with
    // (`agents::oneshot`, `vcs::run`, `runs::preflight`): the login shell's,
    // because a bundled app inherits launchd's. It is deliberately not the
    // PTY's, which also puts the bundled `bd` in front — that lives behind a
    // private function in `terminal::pty`, and this stage may not touch that
    // module. The cost is that an agent in a driven session reaches whatever
    // `bd` the machine has rather than this app's own.
    if let Some(path) = crate::shell_env::path() {
        builder.env("PATH", path);
    }
    let Some(mut command) = spawnable(&builder) else {
        return Err(SessionError::Spawn("the driver produced no command line".into()));
    };

    let mut child = command.spawn().map_err(|error| {
        SessionError::Spawn(format!("{} could not be started: {error}", profile.binary()))
    })?;
    let stdin = child.stdin.take().expect("the child was spawned with a piped stdin");
    let stdout = child.stdout.take().expect("the child was spawned with a piped stdout");
    let stderr = child.stderr.take().expect("the child was spawned with a piped stderr");

    let (stdin_tx, stdin_rx) = mpsc::unbounded_channel::<Vec<u8>>();
    write_stdin(id, stdin, stdin_rx);
    read_stdout(id, stdout, chunks.clone());
    read_stderr(id, stderr);

    // Taken once and kept on `Live` as well as spent here: a harness that
    // picks its own conversation id (`Driver::discovered_id`) has none to
    // record yet, and `note_conversation` needs the identical stamp when it
    // writes the record this spawn could not — not a second `now()` call
    // that would claim the session started later than it did.
    let started_at = chrono::Utc::now().to_rfc3339();
    // After the spawn and not before it: a record for a session that never
    // started would be a row offering a conversation the harness never opened.
    // The same file, the same key and the same rules the terminal worker's
    // records are written under — a row offered back after a restart cannot
    // tell which road made it, and must not have to.
    if let Some(session_id) = conversation.clone() {
        crate::terminal::restore::record(
            Path::new(project),
            crate::terminal::restore::Restorable {
                session_id,
                agent: profile.id().to_owned(),
                cwd: cwd.to_string_lossy().into_owned(),
                project: project.to_owned(),
                work: work.clone(),
                started_at: started_at.clone(),
            },
        );
    }

    // The past, before anything the child says. Appended rather than emitted:
    // nothing has attached to this session yet — it is not even in the worker's
    // map — and `session_attach` is what hands the whole journal over.
    let mut journal = Journal::new();
    for (kind, at) in past {
        journal.append(kind, at);
    }
    let mut live = Live {
        discard_on_eof: false,
        // `Starting` for a session with nothing behind it, which is what
        // `state_of` calls an empty journal; a resumed one opens on a
        // conversation and so opens `Ready`. Asked of the journal rather than
        // written down, so the two cases cannot come apart. Reassessed below
        // once the opening turn, if there is one, has gone in — not emitted
        // either way: the front end reads it out of `session_attach`, which
        // is the next thing it does.
        state: state_of(journal.events(), true),
        talking: Some(Talking { driver, stdin: stdin_tx }),
        journal,
        child: Some(child),
        child_alive: true,
        conversation,
        project: project.to_owned(),
        cwd: cwd.to_string_lossy().into_owned(),
        agent: profile.id().to_owned(),
        work,
        started_at,
    };

    // The brief, as the session's first turn. Into the journal before the
    // bytes go to stdin, for the reason `Request::Send` does the same: the
    // harness never echoes a turn back. Appended rather than emitted — nothing
    // has attached to this session yet, it is not even in the worker's map,
    // and `session_attach` is what hands the whole journal over — which is
    // also why `state` is asked again afterwards rather than left at what an
    // empty journal said.
    if let Some(talking) = live.talking.as_mut() {
        if let Some(text) = talking.driver.opening(&launch) {
            let at = chrono::Utc::now().to_rfc3339();
            live.journal.append(EventKind::TurnStart { by: super::model::Actor::Person }, at.clone());
            live.journal.append(
                EventKind::Opening { text: opening_text, attachments: opening_attachments.clone() },
                at,
            );
            // Claude's opening prose already names its images, while Codex's
            // app-server additionally receives each path as a localImage.
            // Passing them through here keeps that delivery decision in the
            // driver rather than making a second opening path in the worker.
            let bytes = talking.driver.opening_input(Input::Message { text, attachments: opening_attachments.clone() });
            if talking.stdin.send(bytes).is_err() {
                log::warn!("[session {id}] the child stopped reading before its brief was written");
            }
            live.state = state_of(live.journal.events(), true);
        } else if let Some(bytes) = talking.driver.reopen(&launch) {
            // `ResumeSession` has nothing of a person's own to open on —
            // `opening` above answers `None` for it on every driver — but a
            // resumed or forked conversation still owes its harness a first
            // word right away: the history it is reopening is worth showing
            // the moment the panel attaches, not only once somebody types.
            // Nothing is journalled here: the history itself arrives
            // asynchronously, translated by the driver once its own protocol
            // answers, and reaches the journal through the ordinary
            // `absorb`/`append` path a chunk off the child always takes.
            if talking.stdin.send(bytes).is_err() {
                log::warn!("[session {id}] the child stopped reading before it could reopen its conversation");
            }
        }
    }
    Ok(live)
}

fn write_stdin(
    id: SessionId,
    mut stdin: tokio::process::ChildStdin,
    mut queue: mpsc::UnboundedReceiver<Vec<u8>>,
) {
    tauri::async_runtime::spawn(async move {
        while let Some(bytes) = queue.recv().await {
            if let Err(error) = stdin.write_all(&bytes).await {
                log::warn!("[session {id}] the child stopped reading its input: {error}");
                break;
            }
            if let Err(error) = stdin.flush().await {
                log::warn!("[session {id}] the child's input could not be flushed: {error}");
                break;
            }
        }
    });
}

fn read_stdout(
    id: SessionId,
    mut stdout: tokio::process::ChildStdout,
    chunks: mpsc::UnboundedSender<Chunk>,
) {
    tauri::async_runtime::spawn(async move {
        let mut buf = vec![0u8; READ_CHUNK];
        loop {
            match stdout.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    if chunks.send(Chunk::Data(id, buf[..n].to_vec())).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    log::warn!("[session {id}] the child's output could not be read: {error}");
                    break;
                }
            }
        }
        let _ = chunks.send(Chunk::Eof(id));
    });
}

/// The child's stderr, drained and logged and never journalled: it is the
/// harness's own diagnostics, not the conversation. A warning printed there
/// must not put a row in what a person reads as the agent's words.
///
/// Raw bytes through the same `LineBuffer` the codec cuts stdout with, and not
/// `BufReader::lines()`, because this drain must never stop. `next_line`
/// decodes to a `String` and answers `Err(InvalidData)` at the first byte that
/// is not UTF-8 — one stray byte in a quoted filename — and a drain that gives
/// up fills the pipe at about 64 KB, at which point the child blocks in
/// `write`, stops writing stdout too, and the session freezes with no event, no
/// exit and nothing in the log to say why. `LineBuffer` decodes lossily and
/// caps an unterminated line at `driver::MAX_LINE`, which is the same pair of
/// problems already solved once in this subsystem.
fn read_stderr(id: SessionId, mut stderr: tokio::process::ChildStderr) {
    tauri::async_runtime::spawn(async move {
        let mut buf = vec![0u8; READ_CHUNK];
        let mut lines = LineBuffer::new();
        loop {
            match stderr.read(&mut buf).await {
                Ok(0) => break,
                Ok(n) => {
                    for line in lines.feed(&buf[..n]) {
                        log::warn!(
                            "[session {id}] {}",
                            crate::agents::claude::clip(line.trim(), MAX_STDERR_LINE)
                        );
                    }
                }
                // A read error is the pipe itself, not one line of it: there is
                // nothing left to drain and nothing to be gained by asking
                // again.
                Err(error) => {
                    log::warn!("[session {id}] the child's diagnostics ended: {error}");
                    break;
                }
            }
        }
    });
}

/// Bytes for the child, and whether there was still a way to it. The queue is
/// unbounded, so this fails only when the writer task has already given up —
/// which it does on the first write or flush error, and after which every send
/// would otherwise be swallowed in silence.
fn say(live: &mut Live, bytes: Vec<u8>) -> bool {
    live.talking.as_ref().is_some_and(|talking| talking.stdin.send(bytes).is_ok())
}

/// The child is there but cannot be spoken to any more.
///
/// Both halves matter. The event is so that the conversation says so, and the
/// flag is so that it stops claiming to be `running`: a `TurnStart` that never
/// reached the agent would otherwise stay open for the life of the app, with
/// the row spinning over a turn nobody is taking.
///
/// `talking` is deliberately **not** dropped here, unlike at `Chunk::Eof`: a
/// broken stdin says nothing about stdout, and whatever the agent was in the
/// middle of saying is still worth decoding into the journal.
fn lost(app: &AppHandle, id: SessionId, live: &mut Live) {
    live.child_alive = false;
    append(app, id, live, vec![EventKind::Error { text: UNREACHABLE.into() }]);
}

/// Which intents this road starts. Everything a person talks to, which is
/// everything but a run: nobody is in a run's conversation — the lead works
/// overnight against a queue — and the panel would be drawing a session no one
/// is meant to answer. The brief every other intent carries goes over stdin as
/// the session's opening turn (`Driver::opening`), which is what made the old
/// bare-or-resume refusal unnecessary.
fn drivable(intent: &Intent) -> bool {
    !matches!(intent, Intent::Run { .. })
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CrewTreeChange {
    project: String,
    root: u64,
    nodes: Vec<CrewNode>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct CrewEventsChange {
    root: u64,
    node: u64,
    events: Vec<Event>,
}

fn emit_crew(app: &AppHandle, package: &CrewPackage) {
    let _ = app.emit(
        "crew:tree",
        CrewTreeChange {
            project: package.project.clone(),
            root: package.root,
            nodes: package.tree.nodes(),
        },
    );
}

fn emit_crew_events(app: &AppHandle, root: u64, node: u64, events: Vec<Event>) {
    if !events.is_empty() {
        let _ = app.emit("crew:events", CrewEventsChange { root, node, events });
    }
}

fn record_crew_message(
    app: &AppHandle,
    crews: &mut HashMap<u64, CrewPackage>,
    root: u64,
    node: u64,
    text: String,
) {
    let Some(package) = crews.get_mut(&root) else { return };
    let events = package.append(
        node,
        vec![
            EventKind::TurnStart {
                by: super::model::Actor::Person,
            },
            EventKind::UserMessage {
                text,
                attachments: Vec::new(),
            },
        ],
    );
    if let Some(events) = events {
        emit_crew_events(app, root, node, events);
    }
}

/// Apply only Codex's documented structured app-server records. The driver
/// has already translated turn output into the lead journal; these records are
/// for topology/lifecycle, never a second parsing of rendered output.
fn absorb_codex_crew(
    app: &AppHandle,
    crews: &mut HashMap<u64, CrewPackage>,
    root: u64,
    lead_provider: Option<&str>,
    records: Vec<serde_json::Value>,
) {
    let Some(package) = crews.get_mut(&root) else { return };
    if let Some(lead) = lead_provider {
        package.tree.bind_root(root, lead);
    }
    for record in records {
        for node in crate::agents::codex_crew::nodes(&record) {
            package.apply(node);
        }
        if let Some((id, state, can_message)) = crate::agents::codex_crew::status_change(&record) {
            package.tree.update_provider(&id, state, can_message);
        }
        for (id, state, can_message) in crate::agents::codex_crew::collaboration_states(&record) {
            package.tree.update_provider(&id, state, can_message);
        }
    }
    emit_crew(app, package);
}

/// Route every Codex child record through the backend's provider-id index
/// before appending it. The front end receives only the stable node id and can
/// therefore switch between two simultaneous children without a shared log.
fn absorb_codex_journals(
    app: &AppHandle,
    crews: &mut HashMap<u64, CrewPackage>,
    root: u64,
    records: Vec<serde_json::Value>,
) {
    let Some(package) = crews.get_mut(&root) else { return };
    for record in records {
        let parsed = crate::agents::codex_crew::hydrated_journal(&record)
            .or_else(|| crate::agents::codex_crew::journal(&record));
        let Some((provider, kinds)) = parsed else { continue };
        let node = package
            .tree
            .provider_nodes()
            .into_iter()
            .find_map(|(id, node)| (id == provider).then_some(node));
        let Some(node) = node else { continue };
        if let Some(events) = package.append(node, kinds) {
            if !events.is_empty() {
                emit_crew_events(app, root, node, events);
            }
        }
    }
}

fn codex_hydration_requests(
    package: &CrewPackage,
    hydrated: &mut HashSet<(u64, String)>,
) -> Vec<String> {
    package
        .tree
        .provider_nodes()
        .into_iter()
        .filter_map(|(provider, node)| {
            (node != package.root && hydrated.insert((package.root, provider.clone())))
                .then_some(provider)
        })
        .collect()
}

/// Poll the documented Claude team config. This does not touch the PTY at all:
/// the config is the runtime's structured source for members and lifecycle.
/// A project with two indistinguishable configs is left unchanged until one is
/// unambiguous rather than attaching a package to its neighbour.
fn refresh_claude_crews(
    app: &AppHandle,
    crews: &mut HashMap<u64, CrewPackage>,
    roots: &HashSet<u64>,
    teams: &mut HashMap<u64, PathBuf>,
    baselines: &HashMap<u64, HashSet<PathBuf>>,
    tails: &mut HashMap<(u64, PathBuf), crate::agents::claude_crew::TranscriptTail>,
    lead_tails: &mut HashMap<u64, crate::agents::claude_crew::TranscriptTail>,
    transcript_nodes: &mut HashMap<(u64, PathBuf), u64>,
) {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else { return };
    for root in roots {
        let Some(package) = crews.get_mut(root) else { continue };
        let candidates: Vec<_> = crate::agents::claude_crew::teams_for_project(&home, Path::new(&package.project))
            .into_iter()
            .filter(|(team, _)| teams.get(root) == Some(team) || !baselines.get(root).is_some_and(|baseline| baseline.contains(team)))
            .collect();
        if candidates.len() != 1 {
            continue;
        }
        let (team, config) = candidates.into_iter().next().expect("one candidate");
        let Some(name) = crate::agents::claude_crew::team_name(&config) else {
            package.tree.fail_node(*root);
            emit_crew(app, package);
            continue;
        };
        // The directory was discovered from the runtime; still require that
        // its config names that same directory before retaining an address.
        // This prevents a half-replaced team config from crossing packages.
        if crate::agents::claude_crew::team_config(&home, &name) != team.join("config.json") {
            package.tree.fail_node(*root);
            emit_crew(app, package);
            continue;
        }
        teams.insert(*root, team.clone());
        for node in crate::agents::claude_crew::members(&config) {
            package.apply(node);
        }
        let Some(subagents) = crate::agents::claude_crew::lead_subagents(&home, &config) else {
            emit_crew(app, package);
            continue;
        };
        let Some(lead_session_dir) = subagents.parent() else {
            package.tree.fail_node(*root);
            emit_crew(app, package);
            continue;
        };
        // Version selection happened before the run entered the board loop;
        // this is the runtime half: the actual config, transcript directory,
        // and writable member inboxes supplied by this interactive lead.
        if let Err(error) = crate::agents::claude_crew::preflight(
            "2.1.281",
            &team,
            lead_session_dir,
        ) {
            package.tree.fail_node(*root);
            log::warn!("[crew] Claude runtime preflight failed: {error}");
            emit_crew(app, package);
            continue;
        }
        let Some(lead_transcript) = crate::agents::claude_crew::lead_transcript(&home, &config) else {
            package.tree.fail_node(*root);
            log::warn!("[crew] Claude lead transcript is unavailable");
            emit_crew(app, package);
            continue;
        };
        match lead_tails.entry(*root).or_default().read_new(&lead_transcript) {
            Ok(events) => {
                let kinds = events.into_iter().map(|(kind, _)| kind).collect();
                if let Some(events) = package.append(package.root, kinds) {
                    emit_crew_events(app, *root, package.root, events);
                }
            }
            Err(error) => {
                package.tree.fail_node(*root);
                log::warn!("[crew] {} could not be tailed: {error}", lead_transcript.display());
                emit_crew(app, package);
                continue;
            }
        }
        let Ok(entries) = std::fs::read_dir(&subagents) else {
            emit_crew(app, package);
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(std::ffi::OsStr::to_str) != Some("jsonl") {
                continue;
            }
            let key = (*root, path.clone());
            let node = match transcript_nodes.get(&key).copied() {
                Some(node) => Some(node),
                None => crate::agents::claude_crew::subagent_start_from_file(&path)
                    .ok()
                    .flatten()
                    .and_then(|(internal, name)| {
                        // The hook maps a member name to a private internal
                        // transcript id. Confirm the path is for that exact
                        // id before it can acquire a Smetana node.
                        (crate::agents::claude_crew::transcript(&subagents, &internal) == path)
                            .then(|| package.tree.node_for_label(&name))
                            .flatten()
                    }),
            };
            let Some(node) = node else { continue };
            transcript_nodes.insert(key.clone(), node);
            let tail = tails.entry(key).or_default();
            match tail.read_new(&path) {
                Ok(events) => {
                    let kinds = events.into_iter().map(|(kind, _)| kind).collect();
                    if let Some(events) = package.append(node, kinds) {
                        emit_crew_events(app, *root, node, events);
                    }
                }
                Err(error) => {
                    // Child-tail failures do not fail the root package. Mark
                    // only its node unavailable and leave its existing log.
                    package.tree.fail_node(node);
                    log::warn!("[crew] {} could not be tailed: {error}", path.display());
                }
            }
        }
        emit_crew(app, package);
    }
}

/// Remove one package through the same ownership boundary used by an explicit
/// close and an unexpected interactive-lead exit. A Crew root owns all of its
/// provider subscriptions: keeping a transcript cursor after its root is gone
/// would let a later path reuse a stale node id.
fn clear_crew(
    app: &AppHandle,
    sessions: &mut HashMap<SessionId, Live>,
    crews: &mut HashMap<u64, CrewPackage>,
    crew_ptys: &mut HashMap<u64, crate::terminal::pty::Pty>,
    claude_crews: &mut HashSet<u64>,
    claude_teams: &mut HashMap<u64, PathBuf>,
    claude_team_baselines: &mut HashMap<u64, HashSet<PathBuf>>,
    claude_tails: &mut HashMap<(u64, PathBuf), crate::agents::claude_crew::TranscriptTail>,
    claude_lead_tails: &mut HashMap<u64, crate::agents::claude_crew::TranscriptTail>,
    claude_transcript_nodes: &mut HashMap<(u64, PathBuf), u64>,
    crew_leads: &mut HashMap<SessionId, u64>,
    crew_waiters: &mut HashMap<u64, Vec<oneshot::Sender<crate::terminal::model::Exit>>>,
    crew_send_waiters: &mut HashMap<(u64, String), CrewSendWaiter>,
    root: u64,
    exit: crate::terminal::model::Exit,
) {
    claude_crews.remove(&root);
    claude_teams.remove(&root);
    claude_team_baselines.remove(&root);
    claude_tails.retain(|(crew, _), _| *crew != root);
    claude_lead_tails.remove(&root);
    claude_transcript_nodes.retain(|(crew, _), _| *crew != root);
    if let Some(live) = sessions.get_mut(&root) {
        if let Some(child) = live.child.as_mut() {
            let _ = child.start_kill();
        }
    }
    if let Some(mut pty) = crew_ptys.remove(&root) {
        pty.kill();
    }
    crew_leads.retain(|_, lead_root| *lead_root != root);
    for waiter in crew_waiters.remove(&root).unwrap_or_default() {
        let _ = waiter.send(exit);
    }
    for (_, waiter) in crew_send_waiters.extract_if(|(crew, _), _| *crew == root) {
        let _ = waiter.reply.send(Err(SessionError::Spawn(ENDED.into())));
    }
    if let Some(mut package) = crews.remove(&root) {
        package.tree.clear();
        emit_crew(app, &package);
    }
}

fn handle(
    app: &AppHandle,
    sessions: &mut HashMap<SessionId, Live>,
    crews: &mut HashMap<u64, CrewPackage>,
    crew_ptys: &mut HashMap<u64, crate::terminal::pty::Pty>,
    claude_crews: &mut HashSet<u64>,
    claude_teams: &mut HashMap<u64, PathBuf>,
    claude_team_baselines: &mut HashMap<u64, HashSet<PathBuf>>,
    claude_tails: &mut HashMap<(u64, PathBuf), crate::agents::claude_crew::TranscriptTail>,
    claude_lead_tails: &mut HashMap<u64, crate::agents::claude_crew::TranscriptTail>,
    claude_transcript_nodes: &mut HashMap<(u64, PathBuf), u64>,
    crew_leads: &mut HashMap<SessionId, u64>,
    crew_waiters: &mut HashMap<u64, Vec<oneshot::Sender<crate::terminal::model::Exit>>>,
    crew_send_waiters: &mut HashMap<(u64, String), CrewSendWaiter>,
    next_id: &mut SessionId,
    starting: &mut HashMap<SessionId, oneshot::Sender<Result<SessionId, SessionError>>>,
    crew_starting: &mut HashMap<SessionId, (u64, oneshot::Sender<Result<(u64, SessionId), SessionError>>)>,
    permission: Option<&PermissionServer>,
    chunks: &mpsc::UnboundedSender<Chunk>,
    request: Request,
) {
    match request {
        Request::CrewStart(project, intent, pinned_agent, tx) => {
            let id = *next_id;
            *next_id += 1;
            let agent = pinned_agent.as_str();
            // Claude's native teams require its interactive runtime and are
            // deliberately not sent through ClaudeDriver (`-p`). The caller
            // gets an explicit refusal until that interactive transport has
            // established its team config, rather than a deceptive TUI
            // fallback or a fake pipe session.
            if agent == "claude" {
                let baseline = std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| crate::agents::claude_crew::team_dirs(&home))
                    .unwrap_or_default();
                match spawn_claude_crew(app, &project, intent, agent) {
                    Ok(pty) => {
                        let package = CrewPackage::new(project, id, "Crew lead");
                        emit_crew(app, &package);
                        crews.insert(id, package);
                        crew_ptys.insert(id, pty);
                        claude_crews.insert(id);
                        claude_team_baselines.insert(id, baseline);
                        let _ = tx.send(Ok((id, id)));
                    }
                    Err(error) => {
                        let _ = tx.send(Err(error));
                    }
                }
                return;
            }
            match spawn_session(app, id, &project, intent, permission, chunks, Some(agent)) {
                Ok(live) => {
                    let waits = live.talking.as_ref().is_some_and(|talking| talking.driver.awaits_startup());
                    let package = CrewPackage::new(project, id, "Crew lead");
                    emit_crew(app, &package);
                    crews.insert(id, package);
                    crew_leads.insert(id, id);
                    sessions.insert(id, live);
                    if waits {
                        crew_starting.insert(id, (id, tx));
                    } else {
                        let _ = tx.send(Ok((id, id)));
                    }
                }
                Err(error) => {
                    if let Some(server) = permission {
                        server.forget(id);
                    }
                    let _ = tx.send(Err(error));
                }
            }
        }
        Request::CrewTree(root, tx) => {
            let _ = tx.send(crews.get(&root).map(|package| package.tree.nodes()));
        }
        Request::CrewAttach(root, node, tx) => {
            let answer = crews
                .get(&root)
                .and_then(|package| package.snapshot(node).map(|(events, seq, state)| CrewAttached {
                    events,
                    seq,
                    state,
                    cwd: package.project.clone(),
                }))
                .ok_or(SessionError::NoSuchSession(node));
            let _ = tx.send(answer);
        }
        Request::CrewSend(root, node, text, tx) => {
            let Some(package) = crews.get(&root) else {
                let _ = tx.send(Err(SessionError::NoSuchSession(root)));
                return;
            };
            let Some(selected) = package.tree.node(node) else {
                let _ = tx.send(Err(SessionError::NoSuchSession(node)));
                return;
            };
            // This is the second membership/capability check, immediately
            // before bytes are made. A completed child is never redirected to
            // its root or a sibling; callers retain their draft on this Err.
            if !selected.can_message {
                let _ = tx.send(Err(SessionError::Spawn(ENDED.into())));
                return;
            }
            if let Some(team) = claude_teams.get(&root).cloned() {
                if node == root {
                    // The interactive lead has a normal text input, not a
                    // mailbox addressed to itself. This writes one complete
                    // line to that input; it neither reads nor navigates the
                    // provider TUI. Teammates still use the documented
                    // structured inbox below.
                    let delivered = crew_ptys.get_mut(&root).is_some_and(|pty| {
                        if pty.exit_code().is_some() {
                            false
                        } else {
                            let mut input = text.clone().into_bytes();
                            input.push(b'\n');
                            pty.write(&input);
                            true
                        }
                    });
                    let answer = if delivered {
                        record_crew_message(app, crews, root, node, text);
                        Ok(())
                    } else {
                        Err(SessionError::Spawn(ENDED.into()))
                    };
                    let _ = tx.send(answer);
                    return;
                }
                // Claude's mailbox addresses the member's runtime name, which
                // is the structured config label, not agentId/provider id.
                // `append_message` repeats the membership check under its
                // lock, so a leave between selection and this write remains a
                // refusal with the draft intact.
                let member = selected.label.clone();
                let result = crate::agents::claude_crew::append_message(
                    &team,
                    &member,
                    "team-lead",
                    &text,
                    "Message from Smetana",
                )
                .map_err(|error| SessionError::Spawn(error.to_string()));
                if result.is_ok() {
                    record_crew_message(app, crews, root, node, text);
                }
                let _ = tx.send(result);
                return;
            }
            let Some(provider) = package.tree.provider_id(node).map(str::to_owned) else {
                let _ = tx.send(Err(SessionError::Spawn(UNREACHABLE.into())));
                return;
            };
            let Some(live) = sessions.get_mut(&root) else {
                let _ = tx.send(Err(SessionError::NoSuchSession(root)));
                return;
            };
            if crew_send_waiters.contains_key(&(root, provider.clone())) {
                let _ = tx.send(Err(SessionError::Spawn("a message is already awaiting this Crew agent".into())));
                return;
            }
            let delivered = live
                .talking
                .as_mut()
                .and_then(|talking| talking.driver.crew_send(&provider, text.clone()).ok())
                .is_some_and(|bytes| say(live, bytes));
            if delivered {
                crew_send_waiters.insert((root, provider), CrewSendWaiter { node, text, reply: tx });
            } else {
                let _ = tx.send(Err(SessionError::Spawn(UNREACHABLE.into())));
            }
        }
        Request::CrewAwaitExit(root, tx) => {
            if crews.contains_key(&root) {
                crew_waiters.entry(root).or_default().push(tx);
            } else {
                let _ = tx.send(crate::terminal::model::Exit::Removed);
            }
        }
        Request::CrewStop(root, tx) => {
            if !crews.contains_key(&root) {
                let _ = tx.send(Err(SessionError::NoSuchSession(root)));
                return;
            }
            clear_crew(
                app, sessions, crews, crew_ptys, claude_crews, claude_teams, claude_team_baselines, claude_tails,
                claude_lead_tails, claude_transcript_nodes, crew_leads, crew_waiters, crew_send_waiters, root,
                crate::terminal::model::Exit::Removed,
            );
            let _ = tx.send(Ok(()));
        }
        Request::CrewClear(root) => {
            clear_crew(
                app,
                sessions,
                crews,
                crew_ptys,
                claude_crews,
                claude_teams,
                claude_team_baselines,
                claude_tails,
                claude_lead_tails,
                claude_transcript_nodes,
                crew_leads,
                crew_waiters,
                crew_send_waiters,
                root,
                crate::terminal::model::Exit::Removed,
            );
        }
        Request::Start(project, intent, tx) => {
            if !drivable(&intent) {
                // The same capability tag `driver_for`'s own `None` answers
                // with: nothing was attempted, so the PTY fallback this tag
                // buys is safe — see `SessionError::NotDriven`'s header.
                let _ = tx.send(Err(SessionError::NotDriven(
                    "a run is not a conversation and cannot be driven".into(),
                )));
                return;
            }
            let id = *next_id;
            *next_id += 1;
            let started = spawn_session(app, id, &project, intent, permission, chunks, None);
            match started {
                Ok(live) => {
                    sessions.insert(id, live);
                    /* Codex's app-server has not created its thread yet. Keep
                       the caller's draft and dialog alive until its correlated
                       protocol reply confirms a usable conversation. */
                    if sessions.get(&id).is_some_and(|live| live.talking.as_ref().is_some_and(|talking| talking.driver.awaits_startup())) {
                        starting.insert(id, tx);
                        return;
                    }
                    let _ = tx.send(Ok(id));
                }
                Err(error) => {
                    // The ticket, if one was minted, belongs to a session that
                    // does not exist. Nothing can present it now, and leaving
                    // it registered would leave a token alive for the life of
                    // the app.
                    if let Some(server) = permission {
                        server.forget(id);
                    }
                    let _ = tx.send(Err(error));
                }
            }
        }
        Request::Attach(id, tx) => {
            let _ = tx.send(match sessions.get(&id) {
                Some(live) => {
                    let (events, seq) = live.journal.snapshot();
                    Ok(Attached {
                        events,
                        seq,
                        state: live.state,
                        conversation: live.conversation.clone(),
                        cwd: live.cwd.clone(),
                    })
                }
                None => Err(SessionError::NoSuchSession(id)),
            });
        }
        Request::Since(id, seq, tx) => {
            let _ = tx.send(match sessions.get(&id) {
                Some(live) => Ok(live.journal.since(seq)),
                None => Err(SessionError::NoSuchSession(id)),
            });
        }
        Request::Send(id, text, attachments, tx) => {
            let Some(live) = sessions.get_mut(&id) else {
                let _ = tx.send(Err(SessionError::NoSuchSession(id)));
                return;
            };
            if !live.child_alive {
                // Two halves of one refusal. The `Error` event is so that the
                // conversation says what happened where the answer would have
                // gone; the `Err` is so that the caller knows, because the
                // reply is the only thing it sees synchronously and a store
                // reading `Ok` would clear the composer and lose the paragraph
                // somebody just wrote.
                //
                // The message itself is deliberately not journalled: a
                // `UserMessage` here would open a turn that nothing can close,
                // which `state_of` reads as a crash.
                append(app, id, live, vec![EventKind::Error { text: ENDED.into() }]);
                let _ = tx.send(Err(SessionError::Spawn(ENDED.into())));
                return;
            }
            // Into the journal **before** the bytes go to stdin. The harness
            // does not echo a turn back, and a conversation showing only the
            // agent's half is not one.
            append(
                app,
                id,
                live,
                vec![
                    EventKind::TurnStart { by: super::model::Actor::Person },
                    EventKind::UserMessage {
                        text: text.clone(),
                        attachments: attachments.clone(),
                    },
                ],
            );
            let delivered = match live.talking.as_mut() {
                Some(talking) => {
                    let bytes = talking.driver.send(Input::Message { text, attachments });
                    talking.stdin.send(bytes).is_ok()
                }
                None => false,
            };
            let _ = tx.send(if delivered {
                Ok(())
            } else {
                lost(app, id, live);
                Err(SessionError::Spawn(UNREACHABLE.into()))
            });
        }
        Request::Answer(id, question, decision, answers, tx) => {
            let Some(live) = sessions.get_mut(&id) else {
                let _ = tx.send(Err(SessionError::NoSuchSession(id)));
                return;
            };
            // The question has to be one **this** session is holding open, and
            // the check is here because nothing below it can make it: question
            // ids come from one counter for the whole app, and
            // `PermissionServer::answer` looks them up in one global map. So
            // `session_answer(2, "q1", allow)` would put the answer in session
            // 2's journal and release session **1**'s held tool call — the
            // agent runs the command, while its own journal still holds an
            // unanswered `Permission` that pins it at `needs-you` for good and
            // that `Journal::trim` may never drop.
            if !is_open_question(live.journal.events(), &question) {
                let _ = tx.send(Err(SessionError::NoSuchQuestion(question)));
                return;
            }
            // The journal first and the listener second, in that order and not
            // the other: the journal is what `state_of` reads, so a child that
            // dies between the two steps must not leave the question standing.
            // It also settles a question the harness has already abandoned —
            // the row stops being `needs-you` even though nothing was waiting
            // to hear the answer.
            // Secrets still go to the harness, but a long-lived journal must
            // never retain their plaintext. Non-secret answers stay readable.
            let journal_answers = redact_secret_answers(live.journal.events(), &question, answers.clone());
            append(
                app,
                id,
                live,
                vec![EventKind::PermissionAnswered { id: question.clone(), decision, answers: journal_answers }],
            );
            // Some harnesses take a decision over stdin instead of a channel of
            // their own; Claude Code answers `None` here and is served by the
            // listener below. Nothing here passes `answers` down that road: it
            // exists only for `AskUserQuestion`, which Claude Code serves
            // through the permission listener like every other tool, never
            // over stdin.
            let bytes = match live.talking.as_mut() {
                Some(talking) => talking.driver.answer(&question, decision, answers.clone()),
                None => None,
            };
            let driver_delivered = bytes.is_some();
            if let Some(bytes) = bytes {
                if !say(live, bytes) {
                    lost(app, id, live);
                }
            }
            let delivered = driver_delivered || permission.is_some_and(|server| server.answer(&question, decision, answers));
            let _ = tx.send(if delivered {
                Ok(())
            } else {
                Err(SessionError::NoSuchQuestion(question))
            });
        }
        Request::Stop(id, tx) => {
            let Some(live) = sessions.get_mut(&id) else {
                let _ = tx.send(Err(SessionError::NoSuchSession(id)));
                return;
            };
            // Ask first, kill second. Claude Code sends its stdin control
            // request immediately; Codex may return an empty write while it
            // waits for the correlated `turn/start` response, then queues the
            // app-server interrupt. Both keep the child available for the
            // next turn. A harness with no protocol interrupt falls to
            // `start_kill()` below.
            let bytes = live.talking.as_mut().and_then(|talking| talking.driver.interrupt());
            match bytes {
                Some(bytes) => {
                    if !say(live, bytes) {
                        lost(app, id, live);
                    }
                }
                None => {
                    if let Some(child) = live.child.as_mut() {
                        let _ = child.start_kill();
                    }
                }
            }
            let _ = tx.send(Ok(()));
        }
        Request::Close(id, tx) => {
            let Some(live) = sessions.get_mut(&id) else {
                let _ = tx.send(Err(SessionError::NoSuchSession(id)));
                return;
            };
            // Unconditional, unlike `Stop` above: this is the cross on the
            // row, which means the session ends regardless of what a driver's
            // `interrupt` answers. Killing the child is what makes `Chunk::Eof`
            // arrive, and `Eof`'s own arm is the *only* place that forgets the
            // permission token, drops the `.smetana/agents.json` record and
            // deletes the `--mcp-config` file (`McpConfig::drop`, by dropping
            // `Talking`) — so this reaches all of that by the same road it
            // always has, rather than repeating it here.
            if let Some(child) = live.child.as_mut() {
                let _ = child.start_kill();
            }
            let _ = tx.send(Ok(()));
        }
        // Handled by the caller, which has to `.await` the reply.
        Request::ShutDown(_) => {}
    }
}

/// A chunk off a child, as events.
fn absorb(
    app: &AppHandle,
    sessions: &mut HashMap<SessionId, Live>,
    crews: &mut HashMap<u64, CrewPackage>,
    crew_leads: &mut HashMap<SessionId, u64>,
    crew_waiters: &mut HashMap<u64, Vec<oneshot::Sender<crate::terminal::model::Exit>>>,
    crew_send_waiters: &mut HashMap<(u64, String), CrewSendWaiter>,
    codex_hydrated: &mut HashSet<(u64, String)>,
    codex_reconciled: &mut HashSet<u64>,
    starting: &mut HashMap<SessionId, oneshot::Sender<Result<SessionId, SessionError>>>,
    crew_starting: &mut HashMap<SessionId, (u64, oneshot::Sender<Result<(u64, SessionId), SessionError>>)>,
    permission: Option<&PermissionServer>,
    chunk: Chunk,
) {
    match chunk {
        Chunk::Data(id, bytes) => {
            let Some(live) = sessions.get_mut(&id) else { return };
            let (kinds, crew_records, crew_journal_records, crew_send_results, mut outgoing, startup, discovered) = {
                let Some(talking) = live.talking.as_mut() else { return };
                let kinds = talking.driver.feed(&bytes);
                let crew_records = talking.driver.crew_records();
                let crew_journal_records = talking.driver.crew_journal_records();
                let crew_send_results = talking.driver.crew_send_results();
                let outgoing = talking.driver.outgoing();
                let startup = talking.driver.startup();
            // Asked on every chunk, cheap for the ordinary driver that never
            // answers (`None` is the default `Driver::discovered_id` keeps):
            // a harness that names its own conversation has to be asked
            // somewhere, and the moment its protocol confirms one is exactly
            // the moment this fires.
                let discovered = talking.driver.discovered_id();
                (kinds, crew_records, crew_journal_records, crew_send_results, outgoing, startup, discovered)
            };
            let lead_kinds = kinds.clone();
            append(app, id, live, kinds);
            if let Some(conversation) = discovered.as_ref() {
                note_conversation(app, id, live, conversation.clone());
            }
            if let Some(root) = crew_leads.get(&id).copied() {
                if let Some(package) = crews.get_mut(&root) {
                    if let Some(events) = package.append(package.root, lead_kinds) {
                        if !events.is_empty() {
                            emit_crew_events(app, root, package.root, events);
                        }
                    }
                }
                absorb_codex_crew(app, crews, root, discovered.as_deref(), crew_records);
                absorb_codex_journals(app, crews, root, crew_journal_records);
                for receipt in crew_send_results {
                    // Responses are ordered by one app-server stream. A send
                    // is never reported successful merely because its bytes
                    // reached stdin; only this receipt clears the draft.
                    let Some((key, _)) = crew_send_waiters
                        .iter()
                        .find(|((wait_root, _), _)| *wait_root == root)
                        .map(|(key, value)| (key.clone(), value.node))
                    else { continue };
                    let Some(waiter) = crew_send_waiters.remove(&key) else { continue };
                    match receipt {
                        Ok(()) => {
                            record_crew_message(app, crews, root, waiter.node, waiter.text);
                            let _ = waiter.reply.send(Ok(()));
                        }
                        Err(error) => {
                            let _ = waiter.reply.send(Err(SessionError::Spawn(error)));
                        }
                    }
                }
                if discovered.is_some() && codex_reconciled.insert(root) {
                    if let Some(talking) = live.talking.as_mut() {
                        if let Some(request) = talking.driver.crew_reconcile() {
                            outgoing.push(request);
                        }
                    }
                }
                if let Some(package) = crews.get(&root) {
                    for provider in codex_hydration_requests(package, codex_hydrated) {
                        if let Some(talking) = live.talking.as_mut() {
                            if let Some(request) = talking.driver.crew_hydrate(&provider) {
                                outgoing.push(request);
                            }
                        }
                    }
                }
            }
            for bytes in outgoing {
                if !say(live, bytes) { lost(app, id, live); break; }
            }
            if let Some(result) = startup {
                if let Some((root, tx)) = crew_starting.remove(&id) {
                    match &result {
                        Ok(()) => { let _ = tx.send(Ok((root, id))); }
                        Err(text) => {
                            live.discard_on_eof = true;
                            if let Some(child) = live.child.as_mut() { let _ = child.start_kill(); }
                            crew_leads.remove(&id);
                            if let Some(mut package) = crews.remove(&root) {
                                package.tree.clear();
                                emit_crew(app, &package);
                            }
                            let _ = tx.send(Err(SessionError::Spawn(text.clone())));
                        }
                    }
                }
                if let Some(tx) = starting.remove(&id) {
                    match result {
                        Ok(()) => { let _ = tx.send(Ok(id)); }
                        Err(text) => {
                            live.discard_on_eof = true;
                            if let Some(child) = live.child.as_mut() { let _ = child.start_kill(); }
                            let _ = tx.send(Err(SessionError::Spawn(text)));
                        }
                    }
                }
            }
        }
        Chunk::Eof(id) => {
            if let Some((_root, tx)) = crew_starting.remove(&id) {
                let _ = tx.send(Err(SessionError::Spawn("Codex app-server ended before it created a Crew thread".into())));
            }
            let was_starting = starting.remove(&id);
            let failed_startup = was_starting.is_some();
            if let Some(tx) = was_starting {
                let _ = tx.send(Err(SessionError::Spawn("Codex app-server ended before it created a thread".into())));
            }
            let Some(live) = sessions.get_mut(&id) else { return };
            live.child_alive = false;
            // Nothing can ask this session anything any more, and the token it
            // was registered with should not outlive it.
            if let Some(server) = permission {
                server.forget(id);
            }
            // And nothing left to offer back: the conversation ended, so the
            // row it would draw after the next restart would be an offer to
            // reopen a finished agent. `forget_session` one worker over is the
            // same two lines, and this is deliberately the *only* place a
            // driven session's record is dropped — the cross on the row sends
            // `Request::Close` (smetana-y7mv), which kills the child
            // unconditionally, which ends this stream, which arrives here. The
            // composer's own Stop no longer takes this road for a harness whose
            // `interrupt` answers `Some`: the turn closes and the child lives
            // on, so nothing here runs from a Stop press any more on such a
            // harness. **The app's own exit does not take this path**:
            // `Request::ShutDown` returns out of the worker's loop before any
            // of its kills reaches `absorb`, which is what leaves the records in
            // place for the next launch.
            //
            // **Guarded on the same predicate `sessions.remove` below reads.**
            // A resume or a fork whose `thread/resume`/`thread/fork`/
            // `thread/read` came back an error settles `startup =
            // Some(Err(_))` in `absorb`'s own `Chunk::Data` arm, which sets
            // `discard_on_eof` and kills the child — and that kill is what
            // arrives here as this very `Eof`. The conversation this session
            // was reopening is still perfectly alive on Codex's side; only
            // this app's attempt to talk to it has ended. Dropping the
            // record then would delete the one handle a person has on a
            // thread that never stopped existing, removing the retry this
            // refusal is supposed to leave standing.
            if !never_became_a_conversation(failed_startup, live.discard_on_eof) {
                if let Some(conversation) = live.conversation.as_deref() {
                    crate::terminal::restore::drop_record(Path::new(&live.project), conversation);
                }
            }
            // Reaped on a task of its own: end of stream arrives before the
            // child has necessarily been waited on, and the worker must never
            // wait on a process. Dropping the handle unwaited would leave a
            // zombie for the life of the app.
            if let Some(mut child) = live.child.take() {
                tauri::async_runtime::spawn(async move {
                    match child.wait().await {
                        Ok(status) => log::info!("[session {id}] the child ended: {status}"),
                        Err(error) => {
                            log::warn!("[session {id}] the child could not be waited on: {error}")
                        }
                    }
                });
            }
            // There is nothing left to decode and nowhere left to write, so the
            // codec and the way in are dropped here rather than kept for the
            // life of the app. That deletes the `--mcp-config` file holding
            // this session's bearer token — `McpConfig::drop` is the only thing
            // that does — and ends the task parked on a dead process's stdin.
            // The journal stays: a session that has ended is still one somebody
            // opens a tab on to read.
            live.talking = None;
            refresh_state(app, id, live);
            // The package lifetime ends with its lead. Finished children were
            // retained until here; now drop the complete tree and all provider
            // indexes together so no row is orphaned after Stop/exit.
            if let Some(root) = crew_leads.remove(&id) {
                for (_, waiter) in crew_send_waiters.extract_if(|(crew, _), _| *crew == root) {
                    let _ = waiter.reply.send(Err(SessionError::Spawn(ENDED.into())));
                }
                codex_hydrated.retain(|(crew, _)| *crew != root);
                codex_reconciled.remove(&root);
                for waiter in crew_waiters.remove(&root).unwrap_or_default() {
                    let _ = waiter.send(crate::terminal::model::Exit::NoCode);
                }
                if let Some(mut package) = crews.remove(&root) {
                    package.tree.clear();
                    emit_crew(app, &package);
                }
            }
            // A failed pre-creation startup was never a conversation. Its
            // child has been handed to the reaper above; remove the temporary
            // entry so attach cannot paint a failed empty transcript.
            if never_became_a_conversation(failed_startup, live.discard_on_eof) { sessions.remove(&id); }
        }
    }
}

/// Whether a session that has just ended was ever a conversation worth
/// keeping an offer of — `false` for the two shapes of "this app never
/// actually reached the harness": `failed_startup`, the app-server dying
/// before it ever answered `initialize`, `thread/start`, `thread/resume` or
/// `thread/fork` at all, and `discard_on_eof`, one of those four answering
/// with a protocol error. Both are set before this predicate is ever asked —
/// `failed_startup` by `starting.remove` finding a caller still waiting, and
/// `discard_on_eof` by the startup-error arm in `absorb`'s own `Chunk::Data`
/// match — so this is a pure name for a decision made twice over: once about
/// whether the on-disk record is worth keeping (`Chunk::Eof`'s guard on
/// `drop_record`), and once about whether the temporary `Live` entry itself
/// is worth keeping (`sessions.remove`, right below).
///
/// Pulled out as its own function so the two flags this file's own async
/// worker cannot otherwise be reached by a test — `session::service` carries
/// no unit test for its own I/O and orchestration, this file's header says
/// so — are still checkable without one.
fn never_became_a_conversation(failed_startup: bool, discard_on_eof: bool) -> bool {
    failed_startup || discard_on_eof
}

/// Copy answers for the journal without retaining an `isSecret` answer. The
/// driver still receives the original map immediately afterwards.
fn redact_secret_answers(events: &[Event], id: &str, answers: Option<BTreeMap<String, String>>) -> Option<BTreeMap<String, String>> {
    let mut answers = answers?;
    let secret = events.iter().rev().find_map(|event| match &event.kind {
        EventKind::Permission { id: event_id, input, .. } if event_id == id => Some(input),
        _ => None,
    });
    let Some(questions) = secret.and_then(|input| input.get("questions")).and_then(serde_json::Value::as_array) else { return Some(answers) };
    for question in questions {
        if question.get("isSecret").and_then(serde_json::Value::as_bool) == Some(true) {
            if let Some(key) = question.get("id").or_else(|| question.get("question")).and_then(serde_json::Value::as_str) { answers.remove(key); }
        }
    }
    if answers.is_empty() { None } else { Some(answers) }
}

/// A question from the permission listener. It becomes a `Permission` event and
/// nothing else: the answer is a person's, and until they give one this session
/// says nothing further — the harness is holding its own tool call open.
fn question(app: &AppHandle, sessions: &mut HashMap<SessionId, Live>, asked: Asked) {
    let Asked { session, id, tool, detail, input } = asked;
    let Some(live) = sessions.get_mut(&session) else {
        log::warn!("[session {session}] a question arrived for a session that is not here");
        return;
    };
    // Gated to the short list of tools whose panel actually reads `input`
    // structured — today just `AskUserQuestion` — and never left universal.
    // This event is appended to a journal that lives for the life of a
    // session, is cloned whole on every attach and shipped on every
    // `session:events` batch, and a driven session asks on every `Write`,
    // `Edit`, `MultiEdit` and `Task`: an unclipped `input` on all of them
    // would carry whole file bodies and whole subagent prompts through a
    // budget (`journal::BUDGET`) sized on the promise that each event is
    // small. A second structured tool is a second name added here, in the
    // one place this is gated, rather than a second field on the event.
    let input = if tool == crate::agents::claude::ASK_USER_QUESTION_TOOL {
        input
    } else {
        serde_json::Value::Null
    };
    append(
        app,
        session,
        live,
        vec![EventKind::Permission {
            id,
            tool,
            detail,
            // What this session can actually honour, which is what the field is
            // for. `AllowAlways` is deliberately not offered: nothing here
            // remembers a standing permission, so the button would be one that
            // asked again on the very next tool call.
            options: vec![Decision::Allow, Decision::Deny],
            input,
        }],
    );
}

/// Append to the journal, ship the events, and recompute the state.
///
/// The one door into the journal, and the reason `state_of`'s note about a
/// third producer holds: the stream reader and the permission listener both
/// come through here, in the worker's own order.
fn append(app: &AppHandle, id: SessionId, live: &mut Live, kinds: Vec<EventKind>) {
    if kinds.is_empty() {
        return;
    }
    // One stamp for the batch: they were produced by one read of one stream,
    // and minting a clock reading per event would claim a precision the
    // transport does not have.
    let at = chrono::Utc::now().to_rfc3339();
    let events: Vec<Event> =
        kinds.into_iter().map(|kind| live.journal.append(kind, at.clone())).collect();
    // For every session, attached or not. See this file's header.
    let _ = app.emit("session:events", serde_json::json!({ "id": id, "events": events }));
    refresh_state(app, id, live);
}

fn refresh_state(app: &AppHandle, id: SessionId, live: &mut Live) {
    let state = state_of(live.journal.events(), live.child_alive);
    if state == live.state {
        return;
    }
    live.state = state;
    let _ = app.emit(
        "session:state",
        StateChange { id, state, conversation: live.conversation.clone() },
    );
}

/// The id a harness picked for itself, the moment its own protocol confirms
/// one. The driven road's counterpart to `terminal::service`'s own
/// `Request::SessionIdFound`: a profile told its own id up front
/// (`Launch::session_id`, `Profile::session_id_args`) already has
/// `live.conversation` set at the spawn, so this never fires for one, and it
/// exists for the harness that picks its own instead — Codex's app-server,
/// whose `Driver::discovered_id` hands this the exact string `thread/start`,
/// `thread/resume` or `thread/fork` answered with, no second read of
/// anybody's disk needed the way the PTY road's own discovery is.
///
/// **An id already set wins**, the same guard `Request::SessionIdFound`
/// keeps: nothing here can rename a conversation this app did not name, and
/// a plain resume's redundant report of the id it was already given at the
/// spawn is exactly the case this guard turns into a no-op rather than a
/// second, identical record write.
fn note_conversation(app: &AppHandle, id: SessionId, live: &mut Live, conversation: String) {
    if live.conversation.is_some() {
        return;
    }
    live.conversation = Some(conversation.clone());
    // Emitted unconditionally, unlike `refresh_state`'s own emit above: the
    // state word may not have moved at all — a fresh thread's own
    // `thread/start` reply carries no event of its own — but the id on the
    // wire has, and `noteConversation` on the front end reads exactly this
    // event to key the session's row by it.
    //
    // **Not a fix for a mis-keyed row — there is no ordering under which one
    // happens.** Discovery always precedes the startup promise settling
    // (`Chunk::Data`'s own `discovered` is read before `startup` in the same
    // pass), which precedes `Request::Start` answering the caller, which
    // precedes `session_attach`, so that command's own snapshot already
    // carries this id in the ordinary case — nothing waits on this emit to
    // learn it the first time. What the emit buys instead is that this stops
    // being something the next reader has to reason through: the id reaches
    // every window this session is open in without depending on that chain
    // holding, the same way `refresh_state`'s emit is not the only place a
    // state reaches a window either.

    let _ = app.emit(
        "session:state",
        StateChange { id, state: live.state, conversation: live.conversation.clone() },
    );
    // The record the spawn could not write, written now that there is an id
    // to key it by — the same fields the spawn-time record would have used,
    // taken off `Live` rather than guessed again, which is why they are kept
    // there at all.
    crate::terminal::restore::record(
        Path::new(&live.project),
        crate::terminal::restore::Restorable {
            session_id: conversation,
            agent: live.agent.clone(),
            cwd: live.cwd.clone(),
            project: live.project.clone(),
            work: live.work.clone(),
            started_at: live.started_at.clone(),
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::Intent;

    #[test]
    fn a_run_is_the_one_intent_this_road_refuses() {
        assert!(drivable(&Intent::Bare));
        assert!(drivable(&Intent::Setup));
        assert!(drivable(&Intent::EditTask { id: "x-1".into(), title: "T".into() }));
        assert!(drivable(&Intent::ResumeSession {
            id: "9f1c0a2e-0000-4000-8000-000000000000".into(),
            cwd: "/p".into(),
            title: None,
            fork: false,
        }));
        assert!(!drivable(&Intent::Run {
            settings: crate::runs::model::RunSettings {
                scope: crate::runs::model::RunScope::Queue,
                mode: crate::runs::model::RunMode::Auto,
                target_branch: "main".into(),
                create_target: false,
                min_priority: None,
                max_parallel_tasks: None,
                live_check: false,
                file_findings: false,
            },
            reports: std::path::PathBuf::from("/p/.smetana/runs/7"),
            batch: 2,
            continuation: None,
            remove_worktrees: true,
        }));
    }

    /// The regression matrix smetana-gb7f.2 asks for: every manual intent
    /// this task added to Codex's own driven road — `EditTask`,
    /// `ResolveTask`, `FixTask`, `ResolveConflict`, `RepairTracker`,
    /// `Setup`, `Bootstrap`, `ReviewBranch`, and `ResumeSession` both plain
    /// and forked — beside the two `driver_for` already served before this
    /// task, `Bare` and `NewTask`. Codex used to answer `None` here for
    /// every intent but the first two; the whole point of this table is
    /// that it no longer does for any of them.
    #[test]
    fn codex_now_drives_every_manual_intent_claude_code_already_did() {
        let codex: &'static dyn crate::agents::Profile = &crate::agents::codex::Codex;
        let intents = [
            Intent::Bare,
            Intent::NewTask {
                brainstorm: crate::agents::Stage::Off,
                spec: crate::agents::Stage::Off,
                plan: crate::agents::Stage::Off,
                draft: crate::agents::TaskDraft {
                    text: "Rename the worktree when the branch changes".into(),
                    issue_type: None,
                    priority: None,
                    parent: None,
                    images: Vec::new(),
                },
            },
            Intent::EditTask { id: "x-1".into(), title: "T".into() },
            Intent::ResolveTask { id: "x-1".into(), title: "T".into() },
            Intent::FixTask { id: "x-1".into(), title: "T".into() },
            Intent::ResolveConflict {
                repo: "/p".into(),
                op: crate::vcs::model::OpKind::Merge,
                ours: "main".into(),
                theirs: "feature/x".into(),
                files: vec!["src/main.rs".into()],
            },
            Intent::RepairTracker {
                dir: "/p".into(),
                bd_version: "1.1.2".into(),
                command: "bd sync".into(),
                stderr: "database is older than the binary".into(),
            },
            Intent::Setup,
            Intent::Bootstrap,
            Intent::ReviewBranch {
                pairs: vec![crate::agents::ReviewPair { repo: "/p".into(), base: "main".into(), head: "feature/x".into() }],
                report: ".smetana/reviews/2026-09-21-feature-x".into(),
                fetch_failed: Vec::new(),
            },
            Intent::ResumeSession {
                id: "9f1c0a2e-0000-4000-8000-000000000000".into(),
                cwd: "/p".into(),
                title: None,
                fork: false,
            },
            Intent::ResumeSession {
                id: "9f1c0a2e-0000-4000-8000-000000000000".into(),
                cwd: "/p".into(),
                title: None,
                fork: true,
            },
        ];
        for intent in &intents {
            assert!(drivable(intent), "{intent:?} is not Run and must stay drivable");
            assert!(
                driver_for(codex, intent, None).is_some(),
                "codex refused a codec for {intent:?}, which this task widened it to serve",
            );
        }
    }

    /// A driven Codex resume that fails must not delete the only handle a
    /// person has on a thread that is still perfectly alive on Codex's own
    /// side (finding 2, review pass 1 of smetana-gb7f.2): a refused resume
    /// or fork sets `discard_on_eof` rather than `failed_startup`, since the
    /// app-server did answer — just not with success — and the record has
    /// to survive that refusal exactly as it survives the sibling case, the
    /// app-server dying before any reply at all. The full 2×2 is written out
    /// so each cell reads as the sentence it stands for rather than as a
    /// bare boolean pair.
    #[test]
    fn never_became_a_conversation_covers_both_failure_shapes_and_neither() {
        // The app-server never answered at all: `starting.remove` still held
        // the caller's own sender when `Eof` arrived.
        assert!(never_became_a_conversation(true, false));
        // It answered, but `thread/resume`, `thread/fork` or the
        // `thread/read` behind either came back an error — the exact case
        // this finding is about, and the one a naive `if failed_startup`
        // alone would have missed.
        assert!(never_became_a_conversation(false, true));
        // Both at once is not reachable in practice — `discard_on_eof` is
        // only ever set once `starting` has already answered the caller —
        // but the predicate still has to answer honestly about it.
        assert!(never_became_a_conversation(true, true));
        // An ordinary session, talked to successfully and now ending on its
        // own or by the row's own cross: the record and the offer it draws
        // both survive.
        assert!(!never_became_a_conversation(false, false));
    }
}
