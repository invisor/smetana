use std::path::PathBuf;
use std::time::Duration;

use notify::RecommendedWatcher;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, MissedTickBehavior};

use crate::project;

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
    // Ничто ещё не отправляет эти два варианта — command-обвязку строит
    // задача 6. `handle` их уже разбирает; атрибут снимет задача 6 вместе с
    // первой командой, которая их сконструирует.
    #[allow(dead_code)]
    SetProject(Option<PathBuf>, oneshot::Sender<Snapshot>),
    #[allow(dead_code)]
    InitTracker(oneshot::Sender<Result<Snapshot, TrackerError>>),
    Resync(oneshot::Sender<Result<Snapshot, TrackerError>>),
    Create(NewIssue, oneshot::Sender<Result<Issue, TrackerError>>),
    Update(String, IssuePatch, oneshot::Sender<Result<Issue, TrackerError>>),
    Close(String, Option<String>, oneshot::Sender<Result<Issue, TrackerError>>),
    Reopen(String, oneshot::Sender<Result<Issue, TrackerError>>),
}

#[derive(Clone)]
pub struct TrackerHandle(pub mpsc::Sender<Request>);

/// Каталог, на который сейчас смотрит воркер.
struct Project {
    dir: PathBuf,
    /// bd нужен и каталогу без трекера: именно им делается init.
    bd: Bd,
    /// Слежение живёт, только пока в каталоге есть трекер. Поле держит
    /// watcher живым — при уничтожении слежение прекращается молча.
    _watcher: Option<RecommendedWatcher>,
    /// Был ли `.beads` в момент открытия каталога.
    tracked: bool,
}

/// Читать и писать можно только там, где трекер есть.
fn tracked(current: &Option<Project>) -> Option<&Bd> {
    current.as_ref().filter(|p| p.tracked).map(|p| &p.bd)
}

/// Health воркера: текущее значение (его же отдаёт `tracker_health`) и две
/// постоянные беды, из которых оно складывается.
///
/// Беды бывают трёх родов, и путать их нельзя. Разовый сбой bd проходит сам:
/// следующий удачный вызов и есть доказательство, что всё снова работает.
/// «Не та версия bd» — про бинарник: она переживает и удачный `bd list`, и
/// смену проекта. «Нет каталога .beads», «слежение умерло», «проект не
/// выбран» — про открытый каталог: они обязаны пережить удачный вызов, но
/// умереть вместе с проектом, к которому относились.
struct HealthReporter {
    app: AppHandle,
    current: Health,
    bd: Option<Health>,
    project: Option<Health>,
}

impl HealthReporter {
    fn new(app: AppHandle) -> Self {
        Self { app, current: Health { state: HealthState::Ok, message: None }, bd: None, project: None }
    }

    fn current(&self) -> Health {
        self.current.clone()
    }

    /// Постоянная беда бинарника: переживает всё, включая смену проекта.
    fn degrade_bd(&mut self, state: HealthState, message: String) {
        self.bd = Some(Health { state, message: Some(message) });
        self.set(self.baseline());
    }

    /// Постоянная беда открытого каталога: живёт ровно пока открыт он.
    fn degrade_project(&mut self, state: HealthState, message: String) {
        self.project = Some(Health { state, message: Some(message) });
        self.set(self.baseline());
    }

    /// Открыли другой каталог — беды прошлого к нему отношения не имеют.
    fn clear_project(&mut self) {
        self.project = None;
        self.set(self.baseline());
    }

    /// Разовый сбой вызова bd.
    fn failed(&mut self, e: &TrackerError) {
        self.set(Health { state: HealthState::Error, message: Some(e.to_string()) });
    }

    /// bd отработал: гасим разовый сбой, но не постоянную беду.
    fn recovered(&mut self) {
        self.set(self.baseline());
    }

    /// Беда каталога важнее беды бинарника: она конкретнее и с ней человеку
    /// есть что делать прямо сейчас.
    fn baseline(&self) -> Health {
        self.project
            .clone()
            .or_else(|| self.bd.clone())
            .unwrap_or(Health { state: HealthState::Ok, message: None })
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

/// Открывает каталог: чистит снимок, поднимает bd и слежение, ставит health и
/// делает первую сверку.
///
/// Дельты при этом уходят как обычно — снимок для того, кто позвал команду,
/// собирается уже после. Фронт на время переключения перестаёт слушать
/// дельты и берёт ответ команды целиком; иначе задачи нового проекта легли бы
/// поверх задач старого.
async fn open(
    app: &AppHandle,
    dir: Option<PathBuf>,
    store: &mut Store,
    health: &mut HealthReporter,
    tx_tick: &mpsc::Sender<WatchEvent>,
) -> Option<Project> {
    store.reset();

    let Some(dir) = dir else {
        health.degrade_project(HealthState::NoProject, "проект не выбран".to_string());
        return None;
    };

    let bd = Bd::new(app.clone(), dir.clone());

    if !project::has_tracker(&dir) {
        health.degrade_project(
            HealthState::NotABeadsRepo,
            format!("в {} нет каталога .beads", dir.display()),
        );
        // Каталог всё равно остаётся открытым: bd init делается в нём.
        return Some(Project { dir, bd, _watcher: None, tracked: false });
    }

    health.clear_project();

    // Слежение — не условие работы: без него остаётся сверка раз в минуту, и
    // знать об этом должен не только лог.
    let watcher = match watcher::spawn(dir.join(".beads"), tx_tick.clone()) {
        Ok(w) => Some(w),
        Err(e) => {
            health.degrade_project(
                HealthState::Error,
                format!("не удалось следить за .beads: {e}; остаётся только периодическая сверка"),
            );
            None
        }
    };

    let project = Project { dir, bd, _watcher: watcher, tracked: true };
    // Первую сверку никто не ждёт: о неудаче уже рассказал health.
    let _ = full_sync(app, &project.bd, store, health).await;
    Some(project)
}

/// Версия bd — свойство бинарника, а не каталога: спрашиваем один раз за
/// запуск и любым рабочим каталогом, `bd --version` трекер не читает.
async fn check_version(app: &AppHandle, health: &mut HealthReporter) {
    let probe = Bd::new(app.clone(), std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    match probe.version().await {
        Ok(Some(version)) if version == EXPECTED_BD_VERSION => {}
        Ok(other) => health.degrade_bd(
            HealthState::BdVersionMismatch,
            format!("ожидалась версия bd {EXPECTED_BD_VERSION}, получена {other:?}"),
        ),
        Err(e) => health.failed(&e),
    }
}

/// Единственное место с изменяемым состоянием — и оно однопоточное.
/// Вызов bd стоит около двух секунд, поэтому очередь запросов даёт понятный
/// порядок вместо непредсказуемых блокировок на мьютексе.
pub fn start(app: AppHandle, initial: Option<PathBuf>) -> TrackerHandle {
    let (tx_req, mut rx_req) = mpsc::channel::<Request>(32);
    let (tx_tick, mut rx_tick) = mpsc::channel::<WatchEvent>(16);

    tauri::async_runtime::spawn(async move {
        let mut store = Store::default();
        let mut health = HealthReporter::new(app.clone());

        check_version(&app, &mut health).await;
        let mut current = open(&app, initial, &mut store, &mut health, &tx_tick).await;

        let mut ticker = tokio::time::interval(FULL_RESYNC);
        // По умолчанию пропущенные тики (машина спала, вызов bd затянулся)
        // срабатывают подряд — несколько двухсекундных полных сверок одна за
        // другой сразу после затыка. Delay сдвигает расписание вместо этого.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        ticker.tick().await; // первый срабатывает мгновенно

        // Срок отложенной догрузки — состояние цикла, а не await внутри ветки:
        // пока идёт debounce, воркер обязан отвечать на команды.
        let mut due: Option<Instant> = None;

        loop {
            tokio::select! {
                request = rx_req.recv() => {
                    // Отправители кончились — фронта больше нет, работать не для кого.
                    let Some(request) = request else { break };
                    handle(&app, &mut current, &mut store, &mut health, &tx_tick, request).await;
                }
                Some(event) = rx_tick.recv() => match event {
                    // Срок задаёт первое событие, остальные к нему прилипают:
                    // пачка записей схлопывается в одну догрузку.
                    WatchEvent::Changed => { due.get_or_insert_with(|| Instant::now() + DEBOUNCE); }
                    WatchEvent::Failed(message) => health.degrade_project(
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
                    if let Some(bd) = tracked(&current) {
                        incremental_sync(&app, bd, &mut store, &mut health).await;
                    }
                }
                _ = ticker.tick() => {
                    // Периодическую сверку тоже никто не ждёт.
                    if let Some(bd) = tracked(&current) {
                        let _ = full_sync(&app, bd, &mut store, &mut health).await;
                    }
                }
            }
        }
    });

    TrackerHandle(tx_req)
}

/// Трекера может не быть по двум причинам — проект не выбран или в каталоге
/// нет `.beads`. Рассказать о состоянии можно и там, и там; изменить нечего.
async fn handle(
    app: &AppHandle,
    current: &mut Option<Project>,
    store: &mut Store,
    health: &mut HealthReporter,
    tx_tick: &mpsc::Sender<WatchEvent>,
    request: Request,
) {
    match request {
        Request::Health(reply) => {
            let _ = reply.send(health.current());
        }
        Request::Snapshot(reply) => {
            let _ = reply.send(store.snapshot());
        }
        Request::SetProject(dir, reply) => {
            *current = open(app, dir, store, health, tx_tick).await;
            let _ = reply.send(store.snapshot());
        }
        Request::InitTracker(reply) => {
            let result = match current.as_ref() {
                Some(p) if !p.tracked => p.bd.init().await,
                Some(_) => Err(TrackerError::NoTracker("в этом каталоге трекер уже есть".into())),
                None => Err(TrackerError::NoTracker("проект не выбран".into())),
            };
            match result {
                Ok(()) => {
                    let dir = current.as_ref().map(|p| p.dir.clone());
                    *current = open(app, dir, store, health, tx_tick).await;
                    let _ = reply.send(Ok(store.snapshot()));
                }
                // Health намеренно не трогаем: «здесь нет трекера» осталось
                // правдой, и на месте доски должна остаться кнопка, а не
                // «bd ломается». О неудаче расскажет ответ команды.
                Err(e) => {
                    let _ = reply.send(Err(e));
                }
            }
        }
        Request::Resync(reply) => {
            let result = match tracked(current) {
                Some(bd) => full_sync(app, bd, store, health).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(result.map(|()| store.snapshot()));
        }
        Request::Create(new, reply) => {
            let result = match tracked(current) {
                Some(bd) => bd.create(&new).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
        }
        Request::Update(id, patch, reply) => {
            let result = match tracked(current) {
                Some(bd) => bd.update(&id, &patch).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
        }
        Request::Close(id, reason, reply) => {
            let result = match tracked(current) {
                Some(bd) => bd.close(&id, reason.as_deref()).await,
                None => Err(no_tracker(health)),
            };
            let _ = reply.send(finish(app, store, result));
        }
        Request::Reopen(id, reply) => {
            let result = match tracked(current) {
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
