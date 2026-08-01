use std::path::PathBuf;
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, MissedTickBehavior};

use super::bd::Bd;
use super::model::{Delta, Health, HealthState, Issue, IssuePatch, NewIssue, Snapshot, TrackerError};
use super::store::Store;
use super::watcher::{self, WatchEvent};

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

/// Единственное место с изменяемым состоянием — и оно однопоточное.
/// Вызов bd стоит около двух секунд, поэтому очередь запросов даёт
/// понятный порядок вместо непредсказуемых блокировок на мьютексе.
pub fn start(app: AppHandle, project_dir: PathBuf) -> TrackerHandle {
    let (tx_req, mut rx_req) = mpsc::channel::<Request>(32);
    let (tx_tick, mut rx_tick) = mpsc::channel::<WatchEvent>(16);

    tauri::async_runtime::spawn(async move {
        let mut store = Store::default();
        let mut health = HealthReporter::new(app.clone());

        let beads_dir = project_dir.join(".beads");
        if !beads_dir.is_dir() {
            health.degrade(
                HealthState::NotABeadsRepo,
                format!("в {} нет каталога .beads", project_dir.display()),
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
            Ok(Some(version)) if version == EXPECTED_BD_VERSION => {}
            Ok(other) => health.degrade(
                HealthState::BdVersionMismatch,
                format!("ожидалась версия bd {EXPECTED_BD_VERSION}, получена {other:?}"),
            ),
            Err(e) => health.failed(&e),
        }

        // Держим watcher живым до конца работы воркера.
        let _watcher = match watcher::spawn(beads_dir, tx_tick.clone()) {
            Ok(w) => Some(w),
            Err(e) => {
                health.degrade(
                    HealthState::Error,
                    format!(
                        "не удалось следить за .beads: {e}; остаётся только периодическая сверка"
                    ),
                );
                None
            }
        };

        // Первую сверку никто не ждёт: о неудаче уже рассказал health.
        let _ = full_sync(&app, &bd, &mut store, &mut health).await;

        let mut ticker = tokio::time::interval(FULL_RESYNC);
        // По умолчанию пропущенные тики (машина спала, вызов bd затянулся)
        // срабатывают подряд — несколько двухсекундных полных сверок одна за
        // другой сразу после затыка. Delay сдвигает расписание вместо этого.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await; // первый срабатывает мгновенно

        // Срок отложенной догрузки — состояние цикла, а не await внутри ветки:
        // пока идёт debounce, воркер обязан отвечать на команды. Каждая запись
        // из интерфейса будит watcher, и раньше следующее действие
        // пользователя ждало конца этой паузы.
        let mut due: Option<Instant> = None;

        loop {
            tokio::select! {
                request = rx_req.recv() => {
                    // Отправители кончились — фронта больше нет, работать не для кого.
                    let Some(request) = request else { break };
                    handle(&app, Some(&bd), &mut store, &mut health, request).await;
                }
                Some(event) = rx_tick.recv() => match event {
                    // Срок задаёт первое событие, остальные к нему прилипают:
                    // пачка записей схлопывается в одну догрузку.
                    WatchEvent::Changed => { due.get_or_insert_with(|| Instant::now() + DEBOUNCE); }
                    WatchEvent::Failed(message) => health.degrade(
                        HealthState::Error,
                        format!(
                            "слежение за .beads прекратилось: {message}; \
                             остаётся только периодическая сверка"
                        ),
                    ),
                },
                // Значение по умолчанию не используется никогда: без срока
                // ветка выключена, а условие select! вычисляет раньше выражения.
                _ = tokio::time::sleep_until(due.unwrap_or_else(Instant::now)), if due.is_some() => {
                    due = None;
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

/// Health воркера: текущее значение (его же отдаёт `tracker_health`) и база —
/// состояние, в которое health возвращается после удачного вызова bd.
///
/// Разделение нужно потому, что беды бывают двух родов. Разовый сбой bd
/// проходит сам: следующий удачный вызов и есть доказательство, что всё снова
/// работает. А «не та версия bd», «нет каталога .beads» и «слежение умерло»
/// удачным `bd list` не опровергаются и обязаны его пережить — иначе первая же
/// успешная сверка стёрла бы предупреждение, которое остаётся верным.
struct HealthReporter {
    app: AppHandle,
    current: Health,
    baseline: Health,
}

impl HealthReporter {
    fn new(app: AppHandle) -> Self {
        let ok = Health { state: HealthState::Ok, message: None };
        Self { app, current: ok.clone(), baseline: ok }
    }

    fn current(&self) -> Health {
        self.current.clone()
    }

    /// Постоянная беда: сама не пройдёт, поэтому становится и базой.
    fn degrade(&mut self, state: HealthState, message: String) {
        self.baseline = Health { state, message: Some(message) };
        self.set(self.baseline.clone());
    }

    /// Разовый сбой вызова bd.
    fn failed(&mut self, e: &TrackerError) {
        self.set(Health { state: HealthState::Error, message: Some(e.to_string()) });
    }

    /// bd отработал: гасим разовый сбой, но не постоянную беду.
    fn recovered(&mut self) {
        self.set(self.baseline.clone());
    }

    /// Health одновременно и запоминается, и рассылается: событие — быстрый
    /// путь для того, кто уже слушает, а сохранённое значение — ответ тому,
    /// кто подписаться не успел и спросит командой tracker_health. Событие
    /// уходит только на смену значения: health на каждом удачном тике — шум,
    /// за которым не видно настоящей беды.
    fn set(&mut self, next: Health) {
        if self.current == next {
            return;
        }
        self.current = next;
        let _ = self.app.emit("tracker:health", self.current.clone());
    }
}

/// Писать в каталог без трекера некуда. Это не сбой запуска bd — bd мы и не
/// пытались звать, — поэтому и ошибка отдельная.
fn no_tracker(health: &HealthReporter) -> TrackerError {
    TrackerError::NoTracker(
        health
            .current
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
/// первую ошибку. Удача тоже событие: без неё один сорвавшийся вызов bd
/// оставлял бы health в `error` до конца жизни процесса.
async fn full_sync(
    app: &AppHandle,
    bd: &Bd,
    store: &mut Store,
    health: &mut HealthReporter,
) -> Result<(), TrackerError> {
    let columns = match bd.columns().await {
        Ok(columns) => {
            if store.set_columns(columns) {
                emit_delta(app, store.columns_delta());
            }
            Ok(())
        }
        Err(e) => Err(e),
    };
    let issues = match bd.list_all().await {
        Ok(issues) => {
            emit_delta(app, store.apply_full(issues));
            Ok(())
        }
        Err(e) => Err(e),
    };

    let result = columns.and(issues);
    match &result {
        Ok(()) => health.recovered(),
        Err(e) => health.failed(e),
    }
    result
}

async fn incremental_sync(app: &AppHandle, bd: &Bd, store: &mut Store, health: &mut HealthReporter) {
    let since = since_with_overlap(store.last_seen());
    match bd.list_updated_after(&since).await {
        Ok(issues) => {
            emit_delta(app, store.apply_incremental(issues));
            health.recovered();
        }
        Err(e) => health.failed(&e),
    }
}

/// `bd` отсутствует ровно в одном случае — каталог без трекера. Тогда о
/// состоянии рассказать по-прежнему можно, а изменить нечего.
async fn handle(
    app: &AppHandle,
    bd: Option<&Bd>,
    store: &mut Store,
    health: &mut HealthReporter,
    request: Request,
) {
    match request {
        Request::Health(reply) => {
            let _ = reply.send(health.current());
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
