use std::collections::BTreeMap;

use smetana_core::agents::Intent;
use smetana_core::session::commands as logic;
use smetana_core::session::crew::CrewNode;
use smetana_core::session::model::{Decision, Event, SessionError, SessionId};
use smetana_core::session::service::{Attached, CrewAttached, SessionHandle};
use tauri::State;

/// Start a driven session in `project`. Every intent a person talks to is
/// accepted; a run is refused by the worker with a sentence saying so, since
/// nobody is in a run's conversation.
#[tauri::command]
pub async fn session_start(
    handle: State<'_, SessionHandle>,
    project: String,
    intent: Intent,
) -> Result<SessionId, SessionError> {
    logic::plain::start(&handle, project, intent).await
}

/// Snapshot a Crew package's backend-owned hierarchy. Individual updates use
/// `crew:tree`; this command closes the subscribe-before-first-event gap.
#[tauri::command]
pub async fn crew_tree(handle: State<'_, SessionHandle>, root: u64) -> Result<Option<Vec<CrewNode>>, SessionError> {
    logic::plain::crew_tree(&handle, root).await
}

#[tauri::command]
pub async fn crew_attach(
    handle: State<'_, SessionHandle>,
    root: u64,
    node: u64,
) -> Result<CrewAttached, SessionError> {
    logic::plain::crew_attach(&handle, root, node).await
}

/// Address the selected native Crew node. A failed send is intentionally an
/// error so the composer keeps its draft; it is never retried against the lead.
#[tauri::command]
pub async fn crew_send(
    handle: State<'_, SessionHandle>,
    root: u64,
    node: u64,
    text: String,
) -> Result<(), SessionError> {
    logic::plain::crew_send(&handle, root, node, text).await
}

/// Stop a Crew root. The native provider owns its children, so this terminates
/// the complete package rather than attempting an unsafe child interrupt.
#[tauri::command]
pub async fn crew_stop(handle: State<'_, SessionHandle>, root: u64) -> Result<(), SessionError> {
    logic::plain::crew_stop(&handle, root).await
}

/// See `session::commands::crew_clear` in the core crate.
#[tauri::command]
pub async fn crew_clear(handle: State<'_, SessionHandle>, root: u64) -> Result<(), SessionError> {
    logic::crew_clear(&handle, root).await
}

/// The whole conversation, the sequence number to continue from, and where the
/// session stands. Asked whenever a window opens on a session, however many
/// times that is: the journal lives in the worker, so the second attach hands
/// back exactly what the first did.
#[tauri::command]
pub async fn session_attach(handle: State<'_, SessionHandle>, id: SessionId) -> Result<Attached, SessionError> {
    logic::plain::attach(&handle, id).await
}

/// Everything after `seq`. `None` is not an error — it means the journal no
/// longer holds that far back, and the front end should take a fresh snapshot
/// rather than draw a conversation with a hole in it.
#[tauri::command]
pub async fn session_since(
    handle: State<'_, SessionHandle>,
    id: SessionId,
    seq: u64,
) -> Result<Option<Vec<Event>>, SessionError> {
    logic::plain::since(&handle, id, seq).await
}

/// A person's turn. The journal gets it before the bytes reach the child, so
/// the conversation shows both halves — the harness does not echo it back.
#[tauri::command]
pub async fn session_send(
    handle: State<'_, SessionHandle>,
    id: SessionId,
    text: String,
    attachments: Vec<String>,
) -> Result<(), SessionError> {
    logic::plain::send(&handle, id, text, attachments).await
}

/// A person's answer to a permission question. `question` is the id carried by
/// the `Permission` event; `noSuchQuestion` means nothing was waiting for it —
/// it was answered already, or the harness gave up and went.
///
/// `answers` is `AskUserQuestion`'s own: the text of each question mapped to
/// what was chosen or typed, or `None` for an ordinary allow/deny and for a
/// decline to answer. Every other tool's card never sends one.
#[tauri::command]
pub async fn session_answer(
    handle: State<'_, SessionHandle>,
    id: SessionId,
    question: String,
    decision: Decision,
    answers: Option<BTreeMap<String, String>>,
) -> Result<(), SessionError> {
    logic::plain::answer(&handle, id, question, decision, answers).await
}

/// Stop the turn in flight, by whatever means this harness leaves open. For
/// Claude Code that is a `control_request` over stdin (smetana-y7mv,
/// `ClaudeDriver::interrupt`) that ends the turn without killing the child; a
/// harness with no such answer still loses the child. This is never what ends
/// a session outright — see `session_close` below for that.
#[tauri::command]
pub async fn session_stop(handle: State<'_, SessionHandle>, id: SessionId) -> Result<(), SessionError> {
    logic::plain::stop(&handle, id).await
}

/// See `session::commands::session_close` in the core crate.
#[tauri::command]
pub async fn session_close(handle: State<'_, SessionHandle>, id: SessionId) -> Result<(), SessionError> {
    logic::session_close(&handle, id).await
}
