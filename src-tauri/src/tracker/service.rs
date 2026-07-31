use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};

use super::bd::Bd;
use super::model::{Delta, Health, HealthState, Issue, IssuePatch, NewIssue, Snapshot, TrackerError};
use super::store::Store;
use super::watcher;

/// Ожидаемая версия bd. Держится в одной строке с BD_VERSION из
/// scripts/fetch-bd.mjs — расхождение видно в health.
const EXPECTED_BD_VERSION: &str = "1.1.2";
/// Записи прилетают пачками; ждём, пока поток утихнет.
const DEBOUNCE: Duration = Duration::from_millis(250);
/// Страховочная полная сверка: ловит удаления и пропущенные события.
const FULL_RESYNC: Duration = Duration::from_secs(60);
/// Запас на округление updated_at до секунды. Пропуск дороже повтора,
/// а дифф идемпотентен.
const OVERLAP_SECONDS: i64 = 5;

pub enum Request {
    Snapshot(oneshot::Sender<Snapshot>),
    Resync(oneshot::Sender<Result<Snapshot, TrackerError>>),
    Create(NewIssue, oneshot::Sender<Result<Issue, TrackerError>>),
    Update(String, IssuePatch, oneshot::Sender<Result<Issue, TrackerError>>),
    Close(String, Option<String>, oneshot::Sender<Result<Issue, TrackerError>>),
    Reopen(String, oneshot::Sender<Result<Issue, TrackerError>>),
}

#[derive(Clone)]
pub struct TrackerHandle(pub mpsc::Sender<Request>);

/// Каталог проекта — ближайший предок рабочего каталога, в котором лежит
/// `.beads`.
///
/// Наивный вариант (просто `current_dir`) не работает ни в одном реальном
/// запуске: под `npm run tauri dev` бинарник стартует из `src-tauri/`, где
/// никакого `.beads` нет, а у собранного macOS-приложения, открытого из
/// Finder, рабочий каталог вообще `/`. Поэтому идём вверх по предкам.
///
/// Это не выбор проекта: открытый проект по-прежнему один, а настоящий
/// выбор каталога появится отдельно. Если `.beads` не нашёлся нигде,
/// возвращаем исходный каталог — воркер честно сообщит `not-a-beads-repo`.
pub fn find_project_dir() -> std::io::Result<PathBuf> {
    let start = std::env::current_dir()?;
    Ok(nearest_beads_ancestor(&start).unwrap_or(start))
}

fn nearest_beads_ancestor(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|dir| dir.join(".beads").is_dir())
        .map(Path::to_path_buf)
}

/// Единственное место с изменяемым состоянием — и оно однопоточное.
/// Вызов bd стоит около двух секунд, поэтому очередь запросов даёт
/// понятный порядок вместо непредсказуемых блокировок на мьютексе.
pub fn start(app: AppHandle, project_dir: PathBuf) -> TrackerHandle {
    let (tx_req, mut rx_req) = mpsc::channel::<Request>(32);
    let (tx_tick, mut rx_tick) = mpsc::channel::<()>(1);

    tauri::async_runtime::spawn(async move {
        let beads_dir = project_dir.join(".beads");
        if !beads_dir.is_dir() {
            emit_health(
                &app,
                HealthState::NotABeadsRepo,
                Some(format!("в {} нет каталога .beads", project_dir.display())),
            );
            return;
        }

        let bd = Bd::new(app.clone(), project_dir.clone());
        let mut store = Store::default();

        match bd.version().await {
            Ok(Some(version)) if version == EXPECTED_BD_VERSION => {
                emit_health(&app, HealthState::Ok, None)
            }
            Ok(other) => emit_health(
                &app,
                HealthState::BdVersionMismatch,
                Some(format!(
                    "ожидалась версия bd {EXPECTED_BD_VERSION}, получена {other:?}"
                )),
            ),
            Err(e) => emit_health(&app, HealthState::Error, Some(e.to_string())),
        }

        // Держим watcher живым до конца работы воркера.
        let _watcher = match watcher::spawn(beads_dir, tx_tick.clone()) {
            Ok(w) => Some(w),
            Err(e) => {
                emit_health(
                    &app,
                    HealthState::Error,
                    Some(format!(
                        "не удалось следить за .beads: {e}; остаётся только периодическая сверка"
                    )),
                );
                None
            }
        };

        full_sync(&app, &bd, &mut store).await;

        let mut ticker = tokio::time::interval(FULL_RESYNC);
        ticker.tick().await; // первый срабатывает мгновенно

        loop {
            tokio::select! {
                request = rx_req.recv() => {
                    // Отправители кончились — фронта больше нет, работать не для кого.
                    let Some(request) = request else { break };
                    handle(&app, &bd, &mut store, request).await;
                }
                Some(()) = rx_tick.recv() => {
                    tokio::time::sleep(DEBOUNCE).await;
                    while rx_tick.try_recv().is_ok() {}
                    incremental_sync(&app, &bd, &mut store).await;
                }
                _ = ticker.tick() => {
                    full_sync(&app, &bd, &mut store).await;
                }
            }
        }
    });

    TrackerHandle(tx_req)
}

fn emit_health(app: &AppHandle, state: HealthState, message: Option<String>) {
    let _ = app.emit("tracker:health", Health { state, message });
}

fn emit_delta(app: &AppHandle, delta: Delta) {
    if !delta.is_empty() {
        let _ = app.emit("tracker:delta", delta);
    }
}

/// updated_at округляется до секунды, поэтому просим с запасом.
fn since_with_overlap(last_seen: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(last_seen) {
        Ok(t) => (t - chrono::Duration::seconds(OVERLAP_SECONDS))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        Err(_) => "1970-01-01T00:00:00Z".to_string(),
    }
}

async fn full_sync(app: &AppHandle, bd: &Bd, store: &mut Store) {
    match bd.columns().await {
        Ok(columns) => {
            if store.set_columns(columns) {
                emit_delta(app, store.columns_delta());
            }
        }
        Err(e) => emit_health(app, HealthState::Error, Some(e.to_string())),
    }
    match bd.list_all().await {
        Ok(issues) => emit_delta(app, store.apply_full(issues)),
        Err(e) => emit_health(app, HealthState::Error, Some(e.to_string())),
    }
}

async fn incremental_sync(app: &AppHandle, bd: &Bd, store: &mut Store) {
    let since = since_with_overlap(store.last_seen());
    match bd.list_updated_after(&since).await {
        Ok(issues) => emit_delta(app, store.apply_incremental(issues)),
        Err(e) => emit_health(app, HealthState::Error, Some(e.to_string())),
    }
}

async fn handle(app: &AppHandle, bd: &Bd, store: &mut Store, request: Request) {
    match request {
        Request::Snapshot(reply) => {
            let _ = reply.send(store.snapshot());
        }
        Request::Resync(reply) => {
            full_sync(app, bd, store).await;
            let _ = reply.send(Ok(store.snapshot()));
        }
        Request::Create(new, reply) => {
            let result = bd.create(&new).await;
            let _ = reply.send(finish(app, store, result));
        }
        Request::Update(id, patch, reply) => {
            let result = bd.update(&id, &patch).await;
            let _ = reply.send(finish(app, store, result));
        }
        Request::Close(id, reason, reply) => {
            let result = bd.close(&id, reason.as_deref()).await;
            let _ = reply.send(finish(app, store, result));
        }
        Request::Reopen(id, reply) => {
            let result = bd.reopen(&id).await;
            let _ = reply.send(finish(app, store, result));
        }
    }
}

/// Результат собственной записи кладём в снимок сразу, не дожидаясь watcher:
/// пришедший следом тик даст пустой дифф.
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
