//! The commands are deliberately thin: they put a request on the worker's
//! queue and wait for the reply — exactly as the terminal's and the tracker's
//! do. The outer `Result` is about delivery to the worker, the inner one about
//! the operation itself.

use std::collections::BTreeMap;

use tokio::sync::oneshot;

use super::model::{Decision, Event, SessionError, SessionId};
use super::crew::CrewNode;
use super::service::{Attached, CrewAttached, Request, SessionHandle};
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






/// End a Crew package from its row's close control. Kept separate from
/// `crew_stop` at the IPC boundary so the UI never mistakes a node id for an
/// ordinary terminal session id.
pub async fn crew_clear(handle: &SessionHandle, root: u64) -> Result<(), SessionError> {
    handle
        .0
        .send(Request::CrewClear(root))
        .await
        .map_err(|_| SessionError::Spawn("the session worker is not running".into()))
}






/// End the session outright — the cross on a driven agent row, never the
/// composer's Stop (smetana-y7mv). Always kills the child, whatever a
/// driver's `interrupt` answers, so the cleanup that used to follow every
/// Stop unconditionally — forgetting the permission token, dropping the
/// `.smetana/agents.json` record, deleting the `--mcp-config` file — still
/// runs for the one gesture that is actually asking for it.
pub async fn session_close(handle: &SessionHandle, id: SessionId) -> Result<(), SessionError> {
    ask(&handle, |tx| Request::Close(id, tx)).await?
}

/// The bodies of the commands the phone may call, as plain functions over the
/// worker's handle: the `#[tauri::command]` wrappers above and `remote::bridge`
/// both call these, so the phone reaches exactly the code the desktop front end
/// does. A command that takes `State` cannot be called without Tauri, and this
/// is what the wrapper becomes for the allowlisted ones.
pub mod plain {
    use super::*;

    pub async fn start(
        handle: &SessionHandle,
        project: String,
        intent: Intent,
    ) -> Result<SessionId, SessionError> {
        ask(handle, |tx| Request::Start(project, intent, tx)).await?
    }

    pub async fn crew_tree(
        handle: &SessionHandle,
        root: u64,
    ) -> Result<Option<Vec<CrewNode>>, SessionError> {
        ask(handle, |tx| Request::CrewTree(root, tx)).await
    }

    pub async fn crew_attach(
        handle: &SessionHandle,
        root: u64,
        node: u64,
    ) -> Result<CrewAttached, SessionError> {
        ask(handle, |tx| Request::CrewAttach(root, node, tx)).await?
    }

    pub async fn crew_send(
        handle: &SessionHandle,
        root: u64,
        node: u64,
        text: String,
    ) -> Result<(), SessionError> {
        ask(handle, |tx| Request::CrewSend(root, node, text, tx)).await?
    }

    pub async fn crew_stop(handle: &SessionHandle, root: u64) -> Result<(), SessionError> {
        ask(handle, |tx| Request::CrewStop(root, tx)).await?
    }

    pub async fn attach(handle: &SessionHandle, id: SessionId) -> Result<Attached, SessionError> {
        ask(handle, |tx| Request::Attach(id, tx)).await?
    }

    pub async fn since(
        handle: &SessionHandle,
        id: SessionId,
        seq: u64,
    ) -> Result<Option<Vec<Event>>, SessionError> {
        ask(handle, |tx| Request::Since(id, seq, tx)).await?
    }

    pub async fn send(
        handle: &SessionHandle,
        id: SessionId,
        text: String,
        attachments: Vec<String>,
    ) -> Result<(), SessionError> {
        ask(handle, |tx| Request::Send(id, text, attachments, tx)).await?
    }

    pub async fn answer(
        handle: &SessionHandle,
        id: SessionId,
        question: String,
        decision: Decision,
        answers: Option<BTreeMap<String, String>>,
    ) -> Result<(), SessionError> {
        ask(handle, |tx| Request::Answer(id, question, decision, answers, tx)).await?
    }

    pub async fn stop(handle: &SessionHandle, id: SessionId) -> Result<(), SessionError> {
        ask(handle, |tx| Request::Stop(id, tx)).await?
    }
}
