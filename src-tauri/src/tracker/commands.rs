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

/// Переезд на другой каталог. `None` означает «проектов не осталось»:
/// доска пустеет, а воркер продолжает жить и ждать следующего проекта.
/// Ответ — снимок нового проекта целиком: дельты, пришедшие по дороге,
/// фронт на время переключения не слушает.
#[tauri::command]
pub async fn tracker_set_project(
    handle: State<'_, TrackerHandle>,
    path: Option<String>,
) -> Result<Snapshot, TrackerError> {
    let dir = path.map(std::path::PathBuf::from);
    ask(&handle, |tx| Request::SetProject(dir, tx)).await
}

/// `bd init` в каталоге активного проекта. Успех сразу возвращает доску:
/// каталог стал репозиторием, и воркер уже перечитал его.
#[tauri::command]
pub async fn tracker_init(handle: State<'_, TrackerHandle>) -> Result<Snapshot, TrackerError> {
    ask(&handle, Request::InitTracker).await?
}

/// Есть ли трекер в этих каталогах. Вопрос про файловую систему, а не про bd:
/// воркер сюда не зовётся, вызов стоит один `is_dir` на путь. Без него про
/// каталог без трекера человек узнавал бы, только кликнув по нему.
#[tauri::command]
pub async fn tracker_probe(paths: Vec<String>) -> Vec<ProjectProbe> {
    paths
        .into_iter()
        .map(|path| {
            let tracked = crate::project::has_tracker(std::path::Path::new(&path));
            ProjectProbe { path, tracked }
        })
        .collect()
}

/// Каталог, который на самом деле открывают. Ткнули в подкаталог
/// отслеживаемого репозитория — проектом становится его корень: иначе доска
/// сказала бы «здесь нет трекера» про репозиторий, у которого он есть, а
/// кнопка рядом завела бы второй `.beads` внутри первого.
///
/// Вопрос к файловой системе, а не к bd, — воркер сюда не зовётся. Ничего
/// отслеживаемого выше нет: возвращаем путь как есть, это законное «пока не
/// репозиторий», и предложение `bd init` относится именно к нему.
#[tauri::command]
pub async fn project_root(path: String) -> String {
    crate::project::nearest_tracked_ancestor(std::path::Path::new(&path))
        .map(|dir| dir.to_string_lossy().into_owned())
        .unwrap_or(path)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectProbe {
    pub path: String,
    pub tracked: bool,
}
