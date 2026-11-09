//! Server-owned Kanban synchronization. A single coordinator keeps separate state per project.
pub mod command;
mod observe;
pub mod policy;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::{watch, Mutex};
use tokio::time::Instant;

use super::service::TrackerHandle;
use crate::context::Context;
use crate::project;
use crate::settings::{file, model::KanbanSync};

pub const EVENT: &str = "kanban:sync";

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct State {
    pub project: String,
    pub pull_error: Option<command::Error>,
    pub push_error: Option<command::Error>,
}

#[derive(Clone)]
pub struct Handle(Weak<Coordinator>);

impl Handle {
    pub async fn presence(&self, client: u64, projects: Vec<String>) {
        if let Some(coordinator) = self.0.upgrade() {
            let roots = projects
                .into_iter()
                .filter(|path| !path.trim().is_empty() && path.len() <= 4096)
                .take(crate::settings::model::MAX_OPEN)
                .map(|path| canonical(Path::new(&path)))
                .collect();
            coordinator.clients.lock().await.insert(client, roots);
            coordinator.reconcile().await;
        }
    }

    pub async fn client_gone(&self, client: u64) {
        if let Some(coordinator) = self.0.upgrade() {
            coordinator.clients.lock().await.remove(&client);
            coordinator.reconcile().await;
        }
    }

    pub fn enabled(&self, project: &str) -> bool {
        let Some(coordinator) = self.0.upgrade() else {
            return false;
        };
        let key = canonical(Path::new(project));
        coordinator
            .settings()
            .and_then(|settings| config_for(&settings, &key).cloned())
            .is_some_and(|config| config.enabled)
    }

    pub async fn pull(&self, project: &str) -> Result<(), command::Error> {
        let Some(coordinator) = self.0.upgrade() else {
            return Ok(());
        };
        coordinator.pull(project).await
    }

    pub async fn changed(&self, project: &Path) {
        if let Some(coordinator) = self.0.upgrade() {
            let key = canonical(project);
            if let Some(runtime) = coordinator.projects.lock().await.get(&key).cloned() {
                runtime.policy.lock().await.changed(Instant::now());
                observe::scan(&runtime).await;
            }
        }
    }

    pub async fn state(&self, project: &str) -> State {
        let key = canonical(Path::new(project));
        if let Some(coordinator) = self.0.upgrade() {
            if let Some(settings) = coordinator.settings() {
                let config = config_for(&settings, &key).cloned().unwrap_or_default();
                if !config.enabled {
                    return State {
                        project: key.to_string_lossy().into_owned(),
                        ..State::default()
                    };
                }
                if let Err(reason) = config.check() {
                    return State {
                        project: key.to_string_lossy().into_owned(),
                        pull_error: Some(command::Error {
                            operation: "pull".into(),
                            code: None,
                            timed_out: false,
                            stderr: reason.into(),
                        }),
                        push_error: None,
                    };
                }
            }
            if let Some(runtime) = coordinator.projects.lock().await.get(&key).cloned() {
                return runtime.state.lock().await.clone();
            }
            if let Some(state) = coordinator.states.lock().await.get(&key).cloned() {
                return state;
            }
        }
        State {
            project: key.to_string_lossy().into_owned(),
            ..State::default()
        }
    }
}

fn canonical(path: &Path) -> PathBuf {
    let root = project::nearest_tracked_ancestor(path).unwrap_or_else(|| path.to_path_buf());
    root.canonicalize().unwrap_or(root)
}

fn config_for<'a>(
    settings: &'a crate::settings::model::Settings,
    root: &Path,
) -> Option<&'a KanbanSync> {
    settings
        .projects
        .iter()
        .find(|(path, _)| canonical(Path::new(path)) == root)
        .map(|(_, state)| &state.kanban_sync)
}

/// bd writes `last-touched` on a local task mutation. A measured clean
/// `bd dolt pull` changes Dolt's manifest but leaves this file alone.
fn touch_stamp(path: &Path) -> Option<(Vec<u8>, Option<std::time::SystemTime>)> {
    let bytes = std::fs::read(path).ok()?;
    let modified = std::fs::metadata(path)
        .ok()
        .and_then(|metadata| metadata.modified().ok());
    Some((bytes, modified))
}

pub struct Coordinator {
    ctx: Context,
    tracker: TrackerHandle,
    runner: Arc<dyn command::Runner>,
    projects: Mutex<HashMap<PathBuf, Arc<ProjectSync>>>,
    clients: Mutex<HashMap<u64, Vec<PathBuf>>>,
    states: Arc<Mutex<HashMap<PathBuf, State>>>,
}

impl Drop for Coordinator {
    fn drop(&mut self) {
        if let Ok(projects) = self.projects.try_lock() {
            for runtime in projects.values() {
                runtime.open.store(false, Ordering::SeqCst);
            }
        }
    }
}

pub fn start(ctx: Context, tracker: TrackerHandle) -> Arc<Coordinator> {
    let coordinator = Arc::new(Coordinator {
        ctx: ctx.clone(),
        tracker,
        runner: Arc::new(command::Shell),
        projects: Mutex::new(HashMap::new()),
        clients: Mutex::new(HashMap::new()),
        states: Arc::new(Mutex::new(HashMap::new())),
    });
    ctx.install_kanban_sync(Handle(Arc::downgrade(&coordinator)));
    let weak = Arc::downgrade(&coordinator);
    crate::context::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3));
        loop {
            tick.tick().await;
            let Some(coordinator) = weak.upgrade() else {
                break;
            };
            coordinator.reconcile().await;
        }
    });
    coordinator
}

impl Coordinator {
    fn settings(&self) -> Option<crate::settings::model::Settings> {
        self.ctx.settings_path().map(|path| file::load(&path).0)
    }

    async fn reconcile(&self) {
        let Some(settings) = self.settings() else {
            return;
        };
        let mut wanted = HashMap::new();
        let mut invalid = Vec::new();
        for path in &settings.open_projects {
            let key = canonical(Path::new(path));
            let config = config_for(&settings, &key).cloned().unwrap_or_default();
            if config.enabled {
                match config.check() {
                    Ok(()) => {
                        wanted.insert(key, config);
                    }
                    Err(reason) => invalid.push((key, reason)),
                }
            }
        }
        for paths in self.clients.lock().await.values() {
            for key in paths {
                let config = config_for(&settings, key).cloned().unwrap_or_default();
                if config.enabled {
                    match config.check() {
                        Ok(()) => {
                            wanted.insert(key.clone(), config);
                        }
                        Err(reason) => {
                            if !invalid.iter().any(|(old, _)| old == key) {
                                invalid.push((key.clone(), reason));
                            }
                        }
                    }
                }
            }
        }
        let mut projects = self.projects.lock().await;
        let mut created = Vec::new();
        for (key, config) in &wanted {
            if let Some(runtime) = projects.get(key) {
                *runtime.config.lock().await = config.clone();
            } else {
                let runtime = ProjectSync::new(
                    self.ctx.clone(),
                    self.tracker.clone(),
                    key.clone(),
                    config.clone(),
                    self.runner.clone(),
                    self.states.clone(),
                    true,
                );
                observe::start(runtime.clone());
                projects.insert(key.clone(), runtime);
                created.push(key.clone());
            }
        }
        let mut cleared = Vec::new();
        projects.retain(|key, runtime| {
            if wanted.contains_key(key) {
                true
            } else {
                runtime.open.store(false, Ordering::SeqCst);
                if !invalid.iter().any(|(invalid_key, _)| invalid_key == key) {
                    cleared.push(key.clone());
                }
                false
            }
        });
        drop(projects);
        for key in created {
            let state = State { project: key.to_string_lossy().into_owned(), ..State::default() };
            if self.states.lock().await.insert(key, state.clone()).is_some_and(|old| old != state) {
                self.ctx.emit(EVENT, &state);
            }
        }
        for key in cleared {
            let state = State {
                project: key.to_string_lossy().into_owned(),
                ..State::default()
            };
            self.states.lock().await.insert(key, state.clone());
            self.ctx.emit(EVENT, &state);
        }
        for (key, reason) in invalid {
            let state = State {
                project: key.to_string_lossy().into_owned(),
                pull_error: Some(command::Error {
                    operation: "pull".into(),
                    code: None,
                    timed_out: false,
                    stderr: reason.into(),
                }),
                push_error: None,
            };
            let mut states = self.states.lock().await;
            if states.get(&key) != Some(&state) {
                states.insert(key, state.clone());
                self.ctx.emit(EVENT, &state);
            }
        }
    }

    async fn pull(&self, project: &str) -> Result<(), command::Error> {
        let key = canonical(Path::new(project));
        let Some(settings) = self.settings() else {
            return Ok(());
        };
        let config = config_for(&settings, &key).cloned().unwrap_or_default();
        if !config.enabled {
            return Ok(());
        }
        if let Err(error) = config.check() {
            let failure = command::Error {
                operation: "pull".into(),
                code: None,
                timed_out: false,
                stderr: error.into(),
            };
            self.ctx.emit(
                EVENT,
                &State {
                    project: key.to_string_lossy().into_owned(),
                    pull_error: Some(failure.clone()),
                    push_error: None,
                },
            );
            return Err(failure);
        }
        let runtime = {
            let mut projects = self.projects.lock().await;
            if let Some(runtime) = projects.get(&key) {
                runtime.clone()
            } else {
                let managed = settings
                    .open_projects
                    .iter()
                    .any(|path| canonical(Path::new(path)) == key)
                    || self.clients.lock().await.values().any(|paths| paths.contains(&key));
                let runtime = ProjectSync::new(
                    self.ctx.clone(),
                    self.tracker.clone(),
                    key.clone(),
                    config,
                    self.runner.clone(),
                    self.states.clone(),
                    managed,
                );
                if managed {
                    observe::start(runtime.clone());
                    projects.insert(key.clone(), runtime.clone());
                }
                runtime
            }
        };
        match tokio::time::timeout(Duration::from_secs(120), runtime.pull()).await {
            Ok(result) => result,
            Err(_) => {
                runtime.pull_timed_out.store(true, Ordering::SeqCst);
                let failure = command::Error {
                    operation: "pull".into(),
                    code: None,
                    timed_out: true,
                    stderr: "Kanban sync waited more than 120 seconds before agent launch".into(),
                };
                runtime.publish("pull", Some(failure.clone())).await;
                Err(failure)
            }
        }
    }
}

struct ProjectSync {
    ctx: Context,
    tracker: TrackerHandle,
    root: PathBuf,
    runner: Arc<dyn command::Runner>,
    states: Arc<Mutex<HashMap<PathBuf, State>>>,
    config: Mutex<KanbanSync>,
    policy: Mutex<policy::Policy>,
    baseline: Mutex<Option<Vec<super::model::Issue>>>,
    scan_lock: Mutex<()>,
    command_lock: Mutex<()>,
    pull_wait: Mutex<Option<watch::Receiver<Option<Result<(), command::Error>>>>>,
    state: Mutex<State>,
    open: AtomicBool,
    managed: bool,
    pulling: AtomicBool,
    pull_timed_out: AtomicBool,
}

impl ProjectSync {
    fn new(
        ctx: Context,
        tracker: TrackerHandle,
        root: PathBuf,
        config: KanbanSync,
        runner: Arc<dyn command::Runner>,
        states: Arc<Mutex<HashMap<PathBuf, State>>>,
        managed: bool,
    ) -> Arc<Self> {
        Arc::new(Self {
            ctx,
            tracker,
            state: Mutex::new(State {
                project: root.to_string_lossy().into_owned(),
                ..State::default()
            }),
            root,
            runner,
            states,
            config: Mutex::new(config),
            policy: Mutex::new(policy::Policy::default()),
            baseline: Mutex::new(None),
            scan_lock: Mutex::new(()),
            command_lock: Mutex::new(()),
            pull_wait: Mutex::new(None),
            open: AtomicBool::new(managed),
            managed,
            pulling: AtomicBool::new(false),
            pull_timed_out: AtomicBool::new(false),
        })
    }

    async fn publish(&self, operation: &str, error: Option<command::Error>) {
        let mut state = self.state.lock().await;
        let slot = if operation == "pull" {
            &mut state.pull_error
        } else {
            &mut state.push_error
        };
        if *slot != error {
            *slot = error;
            self.states
                .lock()
                .await
                .insert(self.root.clone(), state.clone());
            self.ctx.emit(EVENT, &*state);
        }
    }

    fn enabled_now(&self) -> bool {
        let Some(path) = self.ctx.settings_path() else {
            return false;
        };
        let settings = file::load(&path).0;
        config_for(&settings, &self.root)
            .is_some_and(|config| config.enabled && config.check().is_ok())
    }

    async fn push_if_due(&self) {
        if !self.open.load(Ordering::SeqCst) || !self.enabled_now() {
            return;
        }
        if !self.policy.lock().await.due(Instant::now()) {
            return;
        }
        let Ok(_guard) = self.command_lock.try_lock() else {
            return;
        };
        self.push_if_due_locked().await;
    }

    async fn push_if_due_locked(&self) {
        if !self.open.load(Ordering::SeqCst) || !self.enabled_now() {
            return;
        }
        if !self.policy.lock().await.due(Instant::now()) {
            return;
        }
        let Some(generation) = self.policy.lock().await.begin_push() else {
            return;
        };
        let command = self.config.lock().await.push_command.clone();
        let result = self
            .runner
            .run(&self.ctx, &self.root, "push", &command)
            .await;
        self.policy
            .lock()
            .await
            .finish_push(generation, Instant::now(), result.is_ok());
        self.publish("push", result.err()).await;
    }

    async fn pull(self: &Arc<Self>) -> Result<(), command::Error> {
        let mut pending = self.pull_wait.lock().await;
        let mut rx = if let Some(rx) = pending.as_ref() {
            rx.clone()
        } else {
            let (tx, rx) = watch::channel(None);
            *pending = Some(rx.clone());
            let this = Arc::clone(self);
            crate::context::spawn(async move {
                let result = this.pull_once().await;
                *this.pull_wait.lock().await = None;
                let _ = tx.send(Some(result));
            });
            rx
        };
        drop(pending);
        loop {
            if let Some(result) = rx.borrow().clone() {
                return result;
            }
            if rx.changed().await.is_err() {
                return Err(command::Error {
                    operation: "pull".into(),
                    code: None,
                    timed_out: false,
                    stderr: "pull worker stopped".into(),
                });
            }
        }
    }

    async fn pull_once(&self) -> Result<(), command::Error> {
        let _guard = self.command_lock.lock().await;
        if self.managed && !self.open.load(Ordering::SeqCst) {
            return Ok(());
        }
        if !self.enabled_now() {
            return Ok(());
        }
        self.push_if_due_locked().await;
        // Hold the scanner out until the post-pull baseline is installed. A
        // scan started before the pull must not publish its older snapshot.
        let _scan_guard = self.scan_lock.lock().await;
        let last_touched = self.root.join(".beads/last-touched");
        let before = touch_stamp(&last_touched);
        observe::scan_unlocked(self).await;
        self.pulling.store(true, Ordering::SeqCst);
        if self.managed && !self.open.load(Ordering::SeqCst) {
            self.pulling.store(false, Ordering::SeqCst);
            return Ok(());
        }
        if !self.enabled_now() {
            self.pulling.store(false, Ordering::SeqCst);
            return Ok(());
        }
        let command = self.config.lock().await.pull_command.clone();
        let result = self
            .runner
            .run(&self.ctx, &self.root, "pull", &command)
            .await;
        if let Ok(issues) = super::bd::Bd::new(self.ctx.clone(), self.root.clone())
            .list_all()
            .await
        {
            *self.baseline.lock().await = Some(issues);
        }
        // The final stamp follows the baseline read: a CLI write during that
        // read must still be counted even if its task is in the new snapshot.
        let after = touch_stamp(&last_touched);
        self.pulling.store(false, Ordering::SeqCst);
        if before != after {
            self.policy.lock().await.changed(Instant::now());
        }
        // Tracker worker writes call `Handle::changed`, which scans under this
        // lock. Release it before asking that worker for Current or Resync.
        drop(_scan_guard);
        if result.is_ok() {
            if let Ok((Some(selected), _, _)) = super::commands::current(&self.tracker).await {
                if canonical(&selected) == self.root {
                    let _ = super::commands::tracker_resync(&self.tracker).await;
                }
            }
        }
        let late = self.pull_timed_out.swap(false, Ordering::SeqCst);
        if !(late && result.is_ok()) {
            self.publish("pull", result.clone().err()).await;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use tokio::sync::Notify;

    struct Fake {
        calls: AtomicUsize,
        release: Option<Arc<Notify>>,
    }

    impl command::Runner for Fake {
        fn run<'a>(
            &'a self,
            _: &'a Context,
            _: &'a Path,
            _: &'a str,
            _: &'a str,
        ) -> command::CommandFuture<'a> {
            Box::pin(async move {
                let number = self.calls.fetch_add(1, Ordering::SeqCst);
                if number == 0 {
                    if let Some(release) = &self.release {
                        release.notified().await;
                    }
                }
                Ok(())
            })
        }
    }

    struct Sink;
    impl crate::context::Sink for Sink {
        fn emit(&self, _: &str, _: serde_json::Value) {}
        fn push(&self, _: crate::remote::push::Message) {}
    }

    fn runtime(fake: Arc<dyn command::Runner>) -> (tempfile::TempDir, Arc<ProjectSync>) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let mut settings = crate::settings::model::Settings::default();
        settings.projects.insert(
            root.to_string_lossy().into_owned(),
            crate::settings::model::ProjectState {
                kanban_sync: KanbanSync {
                    enabled: true,
                    ..KanbanSync::default()
                },
                ..Default::default()
            },
        );
        file::save(&dir.path().join("settings.json"), &settings).unwrap();
        let ctx = Context::new(
            Err("x".into()),
            Ok(dir.path().to_path_buf()),
            Err("x".into()),
            None,
            Arc::new(Sink),
        );
        let (sender, _) = tokio::sync::mpsc::channel(1);
        (
            dir,
            ProjectSync::new(
                ctx,
                TrackerHandle(sender),
                root,
                KanbanSync {
                    enabled: true,
                    ..KanbanSync::default()
                },
                fake,
                Arc::new(Mutex::new(HashMap::new())),
                true,
            ),
        )
    }

    #[tokio::test(start_paused = true)]
    async fn fixed_window_runs_one_push_at_sixty_seconds() {
        let fake = Arc::new(Fake {
            calls: AtomicUsize::new(0),
            release: None,
        });
        let (_dir, runtime) = runtime(fake.clone());
        runtime.policy.lock().await.changed(Instant::now());
        tokio::time::advance(Duration::from_secs(20)).await;
        runtime.policy.lock().await.changed(Instant::now());
        tokio::time::advance(Duration::from_secs(39)).await;
        runtime.policy.lock().await.changed(Instant::now());
        runtime.push_if_due_locked().await;
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
        tokio::time::advance(Duration::from_secs(1)).await;
        runtime.push_if_due_locked().await;
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_write_during_push_opens_its_own_window() {
        let release = Arc::new(Notify::new());
        let fake = Arc::new(Fake {
            calls: AtomicUsize::new(0),
            release: Some(release.clone()),
        });
        let (_dir, runtime) = runtime(fake.clone());
        runtime.policy.lock().await.changed(Instant::now());
        tokio::time::advance(Duration::from_secs(60)).await;
        let pushing = runtime.clone();
        let first = tokio::spawn(async move { pushing.push_if_due_locked().await });
        tokio::task::yield_now().await;
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(10)).await;
        runtime.policy.lock().await.changed(Instant::now());
        release.notify_one();
        first.await.unwrap();
        tokio::time::advance(Duration::from_secs(49)).await;
        runtime.push_if_due_locked().await;
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(11)).await;
        runtime.push_if_due_locked().await;
        assert_eq!(fake.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn overlapping_pulls_share_one_attempt_and_a_later_request_starts_another() {
        let release = Arc::new(Notify::new());
        let fake = Arc::new(Fake {
            calls: AtomicUsize::new(0),
            release: Some(release.clone()),
        });
        let (_dir, runtime) = runtime(fake.clone());
        let first_runtime = runtime.clone();
        let first = tokio::spawn(async move { first_runtime.pull().await });
        tokio::task::yield_now().await;
        let second_runtime = runtime.clone();
        let second = tokio::spawn(async move { second_runtime.pull().await });
        tokio::task::yield_now().await;
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
        release.notify_one();
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();
        runtime.pull().await.unwrap();
        assert_eq!(fake.calls.load(Ordering::SeqCst), 2);
    }

    struct FailsOnce(AtomicUsize);
    impl command::Runner for FailsOnce {
        fn run<'a>(
            &'a self,
            _: &'a Context,
            _: &'a Path,
            operation: &'a str,
            _: &'a str,
        ) -> command::CommandFuture<'a> {
            Box::pin(async move {
                if self.0.fetch_add(1, Ordering::SeqCst) == 0 {
                    Err(command::Error {
                        operation: operation.into(),
                        code: Some(7),
                        timed_out: false,
                        stderr: "network unavailable".into(),
                    })
                } else {
                    Ok(())
                }
            })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn failed_push_retains_batch_and_retries_after_sixty_seconds() {
        let fake = Arc::new(FailsOnce(AtomicUsize::new(0)));
        let (_dir, runtime) = runtime(fake.clone());
        runtime.policy.lock().await.changed(Instant::now());
        tokio::time::advance(Duration::from_secs(60)).await;
        runtime.push_if_due_locked().await;
        assert_eq!(
            runtime.state.lock().await.push_error.as_ref().unwrap().code,
            Some(7)
        );
        tokio::time::advance(Duration::from_secs(59)).await;
        runtime.policy.lock().await.changed(Instant::now());
        runtime.push_if_due_locked().await;
        assert_eq!(fake.0.load(Ordering::SeqCst), 1);
        tokio::time::advance(Duration::from_secs(1)).await;
        runtime.push_if_due_locked().await;
        assert_eq!(fake.0.load(Ordering::SeqCst), 2);
        assert!(runtime.state.lock().await.push_error.is_none());
    }

    #[tokio::test]
    async fn a_busy_project_does_not_block_another_projects_command() {
        let release = Arc::new(Notify::new());
        let slow = Arc::new(Fake {
            calls: AtomicUsize::new(0),
            release: Some(release.clone()),
        });
        let fast = Arc::new(Fake {
            calls: AtomicUsize::new(0),
            release: None,
        });
        let (_a, first) = runtime(slow.clone());
        let (_b, second) = runtime(fast.clone());
        let waiting = tokio::spawn(async move { first.pull().await });
        tokio::task::yield_now().await;
        second.pull().await.unwrap();
        assert_eq!(slow.calls.load(Ordering::SeqCst), 1);
        assert_eq!(fast.calls.load(Ordering::SeqCst), 1);
        release.notify_one();
        waiting.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn closing_a_project_cancels_queued_pull_before_shell() {
        let fake = Arc::new(Fake {
            calls: AtomicUsize::new(0),
            release: None,
        });
        let (_dir, runtime) = runtime(fake.clone());
        let guard = runtime.command_lock.lock().await;
        let requested = runtime.clone();
        let pending = tokio::spawn(async move { requested.pull().await });
        tokio::task::yield_now().await;
        runtime.open.store(false, Ordering::SeqCst);
        drop(guard);
        pending.await.unwrap().unwrap();
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn closing_one_of_two_desktops_keeps_the_other_projects_watcher() {
        let fake = Arc::new(Fake { calls: AtomicUsize::new(0), release: None });
        let (dir, runtime) = runtime(fake.clone());
        let root = runtime.root.to_string_lossy().into_owned();
        let mut settings = file::load(&dir.path().join("settings.json")).0;
        settings.open_projects = vec![root.clone()];
        file::save(&dir.path().join("settings.json"), &settings).unwrap();
        let coordinator = Arc::new(Coordinator {
            ctx: runtime.ctx.clone(), tracker: runtime.tracker.clone(), runner: fake,
            projects: Mutex::new(HashMap::new()), clients: Mutex::new(HashMap::new()),
            states: runtime.states.clone(),
        });
        let handle = Handle(Arc::downgrade(&coordinator));
        handle.presence(1, vec![root.clone()]).await;
        handle.presence(2, vec![root.clone()]).await;
        assert!(coordinator.projects.lock().await[&runtime.root].open.load(Ordering::SeqCst));
        settings.open_projects.clear();
        file::save(&dir.path().join("settings.json"), &settings).unwrap();
        handle.presence(1, vec![]).await;
        assert!(coordinator.projects.lock().await[&runtime.root].open.load(Ordering::SeqCst));
        handle.client_gone(2).await;
        assert!(!coordinator.projects.lock().await.contains_key(&runtime.root));
    }

    #[tokio::test]
    async fn pull_waits_for_a_started_scan_before_replacing_its_baseline() {
        let fake = Arc::new(Fake { calls: AtomicUsize::new(0), release: None });
        let (_dir, runtime) = runtime(fake.clone());
        let scan = runtime.scan_lock.lock().await;
        let pulling = runtime.clone();
        let pending = tokio::spawn(async move { pulling.pull().await });
        tokio::task::yield_now().await;
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0);
        drop(scan);
        pending.await.unwrap().unwrap();
        assert_eq!(fake.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn tracker_worker_can_scan_while_answering_pull_resync() {
        let fake = Arc::new(Fake { calls: AtomicUsize::new(0), release: None });
        let (_dir, original) = runtime(fake.clone());
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        let runtime = ProjectSync::new(
            original.ctx.clone(), TrackerHandle(sender), original.root.clone(),
            original.config.lock().await.clone(), fake, original.states.clone(), true,
        );
        let scanning = runtime.clone();
        let worker = tokio::spawn(async move {
            if let Some(super::super::service::Request::Current(reply)) = receiver.recv().await {
                observe::scan(&scanning).await;
                drop(reply);
            }
        });
        tokio::time::timeout(Duration::from_secs(2), runtime.pull_once()).await.unwrap().unwrap();
        worker.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn agent_wait_timeout_is_visible_and_late_success_does_not_erase_it() {
        let release = Arc::new(Notify::new());
        let fake = Arc::new(Fake {
            calls: AtomicUsize::new(0),
            release: Some(release.clone()),
        });
        let (_dir, runtime) = runtime(fake.clone());
        let coordinator = Arc::new(Coordinator {
            ctx: runtime.ctx.clone(),
            tracker: runtime.tracker.clone(),
            runner: fake,
            projects: Mutex::new(HashMap::from([(runtime.root.clone(), runtime.clone())])),
            clients: Mutex::new(HashMap::new()),
            states: runtime.states.clone(),
        });
        let handle = Handle(Arc::downgrade(&coordinator));
        let project = runtime.root.to_string_lossy().into_owned();
        let waiting_handle = handle.clone();
        let waiting_project = project.clone();
        let waiting = tokio::spawn(async move { waiting_handle.pull(&waiting_project).await });
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(120)).await;
        assert!(waiting.await.unwrap().unwrap_err().timed_out);
        assert!(handle.state(&project).await.pull_error.unwrap().timed_out);
        release.notify_one();
        tokio::task::yield_now().await;
        assert!(handle.state(&project).await.pull_error.unwrap().timed_out);
        handle.pull(&project).await.unwrap();
        assert!(handle.state(&project).await.pull_error.is_none());
    }

    #[cfg(target_os = "macos")]
    #[tokio::test]
    async fn real_bd_remote_import_and_concurrent_local_write_keep_the_right_origin() {
        use std::process::Command;
        let bd = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../src-tauri/binaries/bd-aarch64-apple-darwin")
            .canonicalize();
        let Ok(bd) = bd else {
            return;
        };
        let dir = tempfile::tempdir_in("/tmp").unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let remote = dir.path().join("remote");
        for path in [&a, &b, &remote] {
            std::fs::create_dir(path).unwrap();
        }
        let run = |cwd: &Path, args: &[&str]| {
            let output = Command::new(&bd)
                .args(args)
                .current_dir(cwd)
                .env("BEADS_DIR", cwd.join(".beads"))
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "bd {} in {}: {}",
                args.join(" "),
                cwd.display(),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run(&a, &["init", "-p", "kbs", "--non-interactive"]);
        assert!(a.join(".beads").is_dir(), "stop before touching any remote");
        let url = format!("file://{}", remote.display());
        run(&a, &["dolt", "remote", "add", "origin", &url]);
        run(&a, &["create", "First", "--json"]);
        run(&a, &["dolt", "push"]);
        run(
            &b,
            &["init", "-p", "kbs", "--remote", &url, "--non-interactive"],
        );
        assert!(b.join(".beads").is_dir(), "stop before testing pull");

        let mut settings = crate::settings::model::Settings::default();
        let root = b.canonicalize().unwrap();
        settings.projects.insert(
            root.to_string_lossy().into_owned(),
            crate::settings::model::ProjectState {
                kanban_sync: KanbanSync {
                    enabled: true,
                    ..KanbanSync::default()
                },
                ..Default::default()
            },
        );
        file::save(&dir.path().join("settings.json"), &settings).unwrap();
        let ctx = Context::new(
            Err("x".into()),
            Ok(dir.path().to_path_buf()),
            Err("x".into()),
            Some(bd.clone()),
            Arc::new(Sink),
        );
        let (sender, _) = tokio::sync::mpsc::channel(1);
        let runtime = ProjectSync::new(
            ctx,
            TrackerHandle(sender),
            root.clone(),
            KanbanSync {
                enabled: true,
                ..KanbanSync::default()
            },
            Arc::new(command::Shell),
            Arc::new(Mutex::new(HashMap::new())),
            true,
        );
        observe::scan(&runtime).await;
        run(&a, &["create", "Remote second", "--json"]);
        run(&a, &["dolt", "push"]);
        runtime.pull_once().await.unwrap();
        observe::scan(&runtime).await; // a late watcher event is an empty diff
        assert!(!runtime
            .policy
            .lock()
            .await
            .due(Instant::now() + Duration::from_secs(61)));
        assert_eq!(runtime.baseline.lock().await.as_ref().unwrap().len(), 2);

        run(&a, &["create", "Remote third", "--json"]);
        run(&a, &["dolt", "push"]);
        runtime.config.lock().await.pull_command =
            "touch .sync-started; sleep 2; bd dolt pull".into();
        let pulling = runtime.clone();
        let attempt = tokio::spawn(async move { pulling.pull_once().await });
        let signal = b.join(".sync-started");
        let until = std::time::Instant::now() + Duration::from_secs(10);
        while !signal.exists() && std::time::Instant::now() < until {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert!(signal.exists(), "pull shell did not start");
        run(&b, &["create", "Concurrent local", "--json"]);
        attempt.await.unwrap().unwrap();
        assert!(
            runtime
                .policy
                .lock()
                .await
                .due(Instant::now() + Duration::from_secs(61)),
            "local write during imported pull must remain pending"
        );
    }
}
