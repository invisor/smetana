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
    Health(oneshot::Sender<Health>),
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
///
/// Функция не умеет падать намеренно. Если рабочий каталог прочитать не
/// удалось (его удалили, песочница не пускает), берём `.`: отказаться
/// запускаться хуже, чем запуститься и сказать, что не так.
pub fn project_dir() -> PathBuf {
    let start = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    nearest_beads_ancestor(&start).unwrap_or(start)
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
        let mut store = Store::default();
        let mut health = Health { state: HealthState::Ok, message: None };

        let beads_dir = project_dir.join(".beads");
        if !beads_dir.is_dir() {
            set_health(
                &app,
                &mut health,
                HealthState::NotABeadsRepo,
                Some(format!("в {} нет каталога .beads", project_dir.display())),
            );
            // Событие уходит раньше, чем фронт успевает на него подписаться,
            // поэтому воркер остаётся жив: tracker_health переспросит, а
            // tracker_snapshot получит пустой снимок вместо ошибки «воркер не
            // запущен». Ни bd, ни watcher здесь не трогаем и пересканов не
            // делаем — смена каталога придёт вместе с пикером.
            while let Some(request) = rx_req.recv().await {
                handle(&app, None, &mut store, &mut health, request).await;
            }
            return;
        }

        let bd = Bd::new(app.clone(), project_dir.clone());

        match bd.version().await {
            Ok(Some(version)) if version == EXPECTED_BD_VERSION => {
                set_health(&app, &mut health, HealthState::Ok, None)
            }
            Ok(other) => set_health(
                &app,
                &mut health,
                HealthState::BdVersionMismatch,
                Some(format!(
                    "ожидалась версия bd {EXPECTED_BD_VERSION}, получена {other:?}"
                )),
            ),
            Err(e) => set_health(&app, &mut health, HealthState::Error, Some(e.to_string())),
        }

        // Держим watcher живым до конца работы воркера.
        let _watcher = match watcher::spawn(beads_dir, tx_tick.clone()) {
            Ok(w) => Some(w),
            Err(e) => {
                set_health(
                    &app,
                    &mut health,
                    HealthState::Error,
                    Some(format!(
                        "не удалось следить за .beads: {e}; остаётся только периодическая сверка"
                    )),
                );
                None
            }
        };

        // Первую сверку никто не ждёт: о неудаче уже рассказал health.
        let _ = full_sync(&app, &bd, &mut store, &mut health).await;

        let mut ticker = tokio::time::interval(FULL_RESYNC);
        ticker.tick().await; // первый срабатывает мгновенно

        loop {
            tokio::select! {
                request = rx_req.recv() => {
                    // Отправители кончились — фронта больше нет, работать не для кого.
                    let Some(request) = request else { break };
                    handle(&app, Some(&bd), &mut store, &mut health, request).await;
                }
                Some(()) = rx_tick.recv() => {
                    tokio::time::sleep(DEBOUNCE).await;
                    while rx_tick.try_recv().is_ok() {}
                    incremental_sync(&app, &bd, &mut store, &mut health).await;
                }
                _ = ticker.tick() => {
                    // Периодическую сверку тоже никто не ждёт.
                    let _ = full_sync(&app, &bd, &mut store, &mut health).await;
                }
            }
        }
    });

    TrackerHandle(tx_req)
}

/// Health одновременно и запоминается, и рассылается: событие — быстрый путь
/// для того, кто уже слушает, а сохранённое значение — ответ тому, кто
/// подписаться не успел и спросит командой tracker_health.
fn set_health(app: &AppHandle, health: &mut Health, state: HealthState, message: Option<String>) {
    *health = Health { state, message };
    let _ = app.emit("tracker:health", health.clone());
}

/// Писать в каталог без трекера некуда. Это не сбой запуска bd — bd мы и не
/// пытались звать, — поэтому и ошибка отдельная.
fn no_tracker(health: &Health) -> TrackerError {
    TrackerError::NoTracker(
        health
            .message
            .clone()
            .unwrap_or_else(|| "трекер недоступен".to_string()),
    )
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

/// Неудача уходит по обоим каналам: событием — тому, кто слушает, и
/// значением — тому, кто позвал tracker_resync и ждёт ответа. Колонки и
/// задачи независимы, поэтому спрашиваем и то и другое, а наружу отдаём
/// первую ошибку.
async fn full_sync(
    app: &AppHandle,
    bd: &Bd,
    store: &mut Store,
    health: &mut Health,
) -> Result<(), TrackerError> {
    let columns = match bd.columns().await {
        Ok(columns) => {
            if store.set_columns(columns) {
                emit_delta(app, store.columns_delta());
            }
            Ok(())
        }
        Err(e) => {
            set_health(app, health, HealthState::Error, Some(e.to_string()));
            Err(e)
        }
    };
    let issues = match bd.list_all().await {
        Ok(issues) => {
            emit_delta(app, store.apply_full(issues));
            Ok(())
        }
        Err(e) => {
            set_health(app, health, HealthState::Error, Some(e.to_string()));
            Err(e)
        }
    };
    columns.and(issues)
}

async fn incremental_sync(app: &AppHandle, bd: &Bd, store: &mut Store, health: &mut Health) {
    let since = since_with_overlap(store.last_seen());
    match bd.list_updated_after(&since).await {
        Ok(issues) => emit_delta(app, store.apply_incremental(issues)),
        Err(e) => set_health(app, health, HealthState::Error, Some(e.to_string())),
    }
}

/// `bd` отсутствует ровно в одном случае — каталог без трекера. Тогда о
/// состоянии рассказать по-прежнему можно, а изменить нечего.
async fn handle(
    app: &AppHandle,
    bd: Option<&Bd>,
    store: &mut Store,
    health: &mut Health,
    request: Request,
) {
    match request {
        Request::Health(reply) => {
            let _ = reply.send(health.clone());
        }
        Request::Snapshot(reply) => {
            let _ = reply.send(store.snapshot());
        }
        Request::Resync(reply) => {
            let result = match bd {
                Some(bd) => full_sync(app, bd, store, health).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(result.map(|()| store.snapshot()));
        }
        Request::Create(new, reply) => {
            let result = match bd {
                Some(bd) => bd.create(&new).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
        }
        Request::Update(id, patch, reply) => {
            let result = match bd {
                Some(bd) => bd.update(&id, &patch).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
        }
        Request::Close(id, reason, reply) => {
            let result = match bd {
                Some(bd) => bd.close(&id, reason.as_deref()).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
        }
        Request::Reopen(id, reply) => {
            let result = match bd {
                Some(bd) => bd.reopen(&id).await,
                None => Err(no_tracker(health)),
            };
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
