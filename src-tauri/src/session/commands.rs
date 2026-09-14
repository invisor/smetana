//! The commands are deliberately thin: they put a request on the worker's
//! queue and wait for the reply — exactly as the terminal's and the tracker's
//! do. The outer `Result` is about delivery to the worker, the inner one about
//! the operation itself.

use std::collections::BTreeMap;

use tauri::State;
use tokio::sync::oneshot;

use super::model::{Decision, Event, SessionError, SessionId};
use super::service::{Attached, Request, SessionHandle};
use crate::agents::Intent;

async fn ask<T>(
    handle: &SessionHandle,
    make: impl FnOnce(oneshot::Sender<T>) -> Request,
) -> Result<T, SessionError> {
    let (tx, rx) = oneshot::channel();
    handle
        .0
        .send(make(tx))
        .await
        .map_err(|_| SessionError::Spawn("the session worker is not running".into()))?;
    rx.await.map_err(|_| SessionError::Spawn("the session worker did not answer".into()))
}

/// Start a driven session in `project`. Every intent a person talks to is
/// accepted; a run is refused by the worker with a sentence saying so, since
/// nobody is in a run's conversation.
#[tauri::command]
pub async fn session_start(
    handle: State<'_, SessionHandle>,
    project: String,
    intent: Intent,
) -> Result<SessionId, SessionError> {
    ask(&handle, |tx| Request::Start(project, intent, tx)).await?
}

/// The whole conversation, the sequence number to continue from, and where the
/// session stands. Asked whenever a window opens on a session, however many
/// times that is: the journal lives in the worker, so the second attach hands
/// back exactly what the first did.
#[tauri::command]
pub async fn session_attach(
    handle: State<'_, SessionHandle>,
    id: SessionId,
) -> Result<Attached, SessionError> {
    ask(&handle, |tx| Request::Attach(id, tx)).await?
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
    ask(&handle, |tx| Request::Since(id, seq, tx)).await?
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
    ask(&handle, |tx| Request::Send(id, text, attachments, tx)).await?
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
    ask(&handle, |tx| Request::Answer(id, question, decision, answers, tx)).await?
}

/// Stop the turn in flight, by whatever means this harness leaves open. For
/// Claude Code that is killing the child: it has no documented way of being
/// asked to stop one.
#[tauri::command]
pub async fn session_stop(
    handle: State<'_, SessionHandle>,
    id: SessionId,
) -> Result<(), SessionError> {
    ask(&handle, |tx| Request::Stop(id, tx)).await?
}
