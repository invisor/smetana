use tauri::State;
use tokio::sync::oneshot;

use super::model::{Health, Issue, IssuePatch, NewIssue, Snapshot, TrackerError};
use super::service::{Request, TrackerHandle};

/// Команды намеренно тонкие: всё, что они делают, — кладут запрос в очередь
/// воркера и ждут ответ. Внешний Result — про доставку до воркера, внутренний
/// (там, где он есть) — про сам вызов bd.
async fn ask<T>(
    handle: &TrackerHandle,
    make: impl FnOnce(oneshot::Sender<T>) -> Request,
) -> Result<T, TrackerError> {
    let (tx, rx) = oneshot::channel();
    handle
        .0
        .send(make(tx))
        .await
        .map_err(|_| TrackerError::Spawn("воркер трекера не запущен".into()))?;
    rx.await
        .map_err(|_| TrackerError::Spawn("воркер трекера не ответил".into()))
}

/// Событие tracker:health может уйти раньше, чем фронт подпишется, — эта
/// команда отдаёт последнее состояние тому, кто его пропустил.
#[tauri::command]
pub async fn tracker_health(handle: State<'_, TrackerHandle>) -> Result<Health, TrackerError> {
    ask(&handle, Request::Health).await
}

#[tauri::command]
pub async fn tracker_snapshot(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::Snapshot).await
}

#[tauri::command]
pub async fn tracker_resync(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::Resync).await?
}

#[tauri::command]
pub async fn tracker_create(
    handle: State<'_, TrackerHandle>,
    issue: NewIssue,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Create(issue, tx)).await?
}

#[tauri::command]
pub async fn tracker_update(
    handle: State<'_, TrackerHandle>,
    id: String,
    patch: IssuePatch,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Update(id, patch, tx)).await?
}

#[tauri::command]
pub async fn tracker_close(
    handle: State<'_, TrackerHandle>,
    id: String,
    reason: Option<String>,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Close(id, reason, tx)).await?
}

#[tauri::command]
pub async fn tracker_reopen(
    handle: State<'_, TrackerHandle>,
    id: String,
) -> Result<Issue, TrackerError> {
    ask(&handle, |tx| Request::Reopen(id, tx)).await?
}
