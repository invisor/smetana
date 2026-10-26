//! The restart gateway: what a client may ask of the server about restarting it, and the rule that
//! nothing is killed silently.
//!
//! A restart of the server ends every child it owns — runs, agent sessions, driven sessions, plain
//! shells — so the gate is not "no run" but **no live work of any kind**. A client therefore never
//! says "restart" and gets one. It says *when*:
//!
//! - **`now`** restarts at once if nothing is alive. If something is, it answers
//!   [`Outcome::NeedsConfirmation`] with the list and does nothing. The person's answer is the same
//!   request with `confirmed` set to **that list**, and a confirmation covers only the work it names:
//!   work that started in between is not covered, so the server answers `needs-confirmation` again
//!   with the fresh list. The list is also taken once more right before the restart runs, so work that
//!   appears after the answer is never ended unseen either.
//! - **`when-idle`** never ends anything: it arms a watch and restarts when the list is empty, however
//!   long that takes. A busy server that never goes quiet never restarts, which is the promise.
//! - **`cancel`** disarms a `when-idle` that has not fired.
//!
//! Which work counts and what a restart does are the host's: the gateway is handed a way to list the
//! work and a way to restart, and owns only the rule between them. That keeps the rule testable with no
//! process to kill.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// What kind of live work an entry is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkKind {
    /// An autonomous run, one per project it drives.
    Run,
    /// An agent in a terminal (a PTY session).
    Agent,
    /// A driven session (the structured conversation).
    Session,
    /// A plain shell.
    Shell,
}

/// One piece of work a restart would end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveWork {
    pub kind: WorkKind,
    /// A session's id in its own worker; a run's token. A confirmation of one run therefore does not
    /// cover another started on the same project.
    pub id: Option<u64>,
    pub project: String,
    /// A short phrase for the person: the agent's name, or `shell`.
    pub label: String,
    /// The token of the run this session belongs to, when it is a run's batch. A session its covered
    /// run starts after the person confirmed is covered with it: it is the run's to end.
    #[serde(default)]
    pub owner: Option<u64>,
}

impl LiveWork {
    /// Whether the person's confirmation `covered` includes this work: it is named there, or it is a
    /// batch of a run that is.
    pub fn is_covered_by(&self, covered: &[LiveWork]) -> bool {
        covered.contains(self)
            || self.owner.is_some_and(|token| {
                covered.iter().any(|item| item.kind == WorkKind::Run && item.id == Some(token))
            })
    }
}

/// The event a client hears when a restart it was told was happening is put off because work appeared.
/// Payload: `{ "work": [LiveWork] }`, what the restart now waits for.
pub const DEFERRED_EVENT: &str = "server:restart-deferred";

/// The payload of [`DEFERRED_EVENT`].
pub fn deferred_payload(work: &[LiveWork]) -> serde_json::Value {
    serde_json::json!({ "work": work })
}

/// The commands that start work. **Any new command that starts a long-lived child must be added here**:
/// it pairs with the `calls!` table in `dispatch.rs`, and a command that starts work and is not listed
/// is one the freeze lets through, to be ended unlisted. While a restart is committed the host refuses them, so that nothing
/// can begin between the last look at the list and the end of the workers.
pub const WORK_COMMANDS: &[&str] = &["terminal_create", "terminal_shell", "session_start", "run_start"];

/// When the person wants the restart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    Now,
    WhenIdle,
    Cancel,
}

/// What happened to the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The server is restarting; the connection is about to close.
    Restarting,
    /// Armed: it restarts when no work is alive. `work` is what it is waiting for.
    Scheduled,
    /// Work is alive and the request was not confirmed. Nothing was done; `work` is the list to show.
    NeedsConfirmation,
    /// A scheduled restart was withdrawn.
    Cancelled,
    /// `cancel` with nothing scheduled.
    NothingScheduled,
    /// `cancel` after the restart was committed: the workers are being ended and it cannot be withdrawn.
    AlreadyRestarting,
}

/// The reply of `server_restart`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reply {
    pub outcome: Outcome,
    /// Whether a `when-idle` restart is armed after this request.
    pub scheduled: bool,
    pub work: Vec<LiveWork>,
}

pub type BoxFuture<T> = Pin<Box<dyn Future<Output = T> + Send>>;

pub struct Gateway {
    list: Box<dyn Fn() -> BoxFuture<Vec<LiveWork>> + Send + Sync>,
    restart: Arc<dyn Fn() + Send + Sync>,
    deferred: Arc<dyn Fn(&[LiveWork]) + Send + Sync>,
    poll: Duration,
    control: Mutex<Control>,
}

/// Where a restart has got to. **One piece of state under one lock**, because the freeze, the cancel
/// counter and the commit have to change together: a freeze raised by a task whose cancel has already
/// happened, or lifted after the restart is committed, are each a way to refuse work for ever or to
/// accept it just before it is ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Nothing is committed; work-creating commands are accepted.
    Idle,
    /// A restart task has taken its last look and is about to commit; the commands are refused. Carries
    /// the epoch of the task that raised it, and only that task (or a cancel) lowers it.
    Frozen(u64),
    /// `restart` has been called. There is no way back: the workers are being ended, the commands stay
    /// refused and a cancel is told it is too late.
    Committed,
}

struct Control {
    /// Raised by every cancel. A task carries the value it started under and may not freeze, commit,
    /// re-arm or announce anything once it has moved.
    epoch: u64,
    phase: Phase,
    /// The pending restart: the `when-idle` watch, or a restart on its way out. One slot, so that
    /// `cancel` withdraws whichever it is.
    task: Option<tokio::task::JoinHandle<()>>,
}

/// Lowers the freeze a task raised, however the task ends — finished, returned early or aborted by a
/// cancel — unless it was committed. It does nothing to a freeze that is not this task's.
struct FreezeGuard {
    gateway: Arc<Gateway>,
    epoch: u64,
}

impl Drop for FreezeGuard {
    fn drop(&mut self) {
        self.gateway.unfreeze(self.epoch);
    }
}

/// How long the reply gets to leave before the restart runs.
const REPLY_GRACE: Duration = Duration::from_millis(300);

impl Gateway {
    pub fn new(
        list: Box<dyn Fn() -> BoxFuture<Vec<LiveWork>> + Send + Sync>,
        restart: Arc<dyn Fn() + Send + Sync>,
        deferred: Arc<dyn Fn(&[LiveWork]) + Send + Sync>,
        poll: Duration,
    ) -> Arc<Self> {
        Arc::new(Self {
            list,
            restart,
            deferred,
            poll,
            control: Mutex::new(Control { epoch: 0, phase: Phase::Idle, task: None }),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Control> {
        self.control.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn epoch(&self) -> u64 {
        self.lock().epoch
    }

    /// Whether work-creating commands are being refused: a restart is about to commit, or has.
    pub fn frozen(&self) -> bool {
        self.lock().phase != Phase::Idle
    }

    fn scheduled(&self) -> bool {
        self.lock().task.as_ref().is_some_and(|task| !task.is_finished())
    }

    /// Raises the freeze for the task of `epoch`. Refused if a cancel has moved the epoch, or if
    /// something is already frozen or committed.
    fn freeze(&self, epoch: u64) -> bool {
        let mut control = self.lock();
        if control.epoch == epoch && control.phase == Phase::Idle {
            control.phase = Phase::Frozen(epoch);
            true
        } else {
            false
        }
    }

    /// Lowers the freeze `epoch` raised. A committed restart's is not lowered.
    fn unfreeze(&self, epoch: u64) {
        let mut control = self.lock();
        if control.phase == Phase::Frozen(epoch) {
            control.phase = Phase::Idle;
        }
    }

    /// Commits the restart of `epoch`: only from its own freeze, and only if no cancel came.
    fn commit(&self, epoch: u64) -> bool {
        let mut control = self.lock();
        if control.epoch == epoch && control.phase == Phase::Frozen(epoch) {
            control.phase = Phase::Committed;
            true
        } else {
            false
        }
    }

    /// The work alive right now.
    pub async fn live_work(&self) -> Vec<LiveWork> {
        (self.list)().await
    }

    /// Arms the `when-idle` watch for `epoch`. `force` replaces the pending restart, for the restart
    /// that has just found work again. Nothing is armed if a cancel has moved the epoch since, or the
    /// restart is committed.
    fn arm(self: &Arc<Self>, force: bool, epoch: u64) {
        let mut control = self.lock();
        if control.epoch != epoch || control.phase == Phase::Committed {
            return;
        }
        if !force && control.task.as_ref().is_some_and(|task| !task.is_finished()) {
            return;
        }
        let gateway = self.clone();
        let task = tokio::spawn(async move {
            loop {
                tokio::time::sleep(gateway.poll).await;
                if gateway.epoch() != epoch {
                    return;
                }
                if gateway.live_work().await.is_empty() {
                    gateway.fire(Vec::new(), epoch);
                    return;
                }
            }
        });
        if let Some(old) = control.task.replace(task) {
            old.abort();
        }
    }

    /// Restarts after the reply has left, **if nothing is alive that `covered` does not name**.
    ///
    /// The list is taken again here, and the work-creating commands are then refused and the list taken
    /// once more, so the last look is the one the workers are ended on: work that started after the
    /// answer the person saw was never shown to them. When there is such work the restart does not
    /// happen; it becomes a `when-idle` one, and the clients are told (`DEFERRED_EVENT`). The task is the
    /// pending restart, so `cancel` stops it, and every step that changes anything checks the epoch it
    /// began under, under the lock the cancel takes.
    fn fire(self: &Arc<Self>, covered: Vec<LiveWork>, epoch: u64) {
        let mut control = self.lock();
        if control.epoch != epoch || control.phase == Phase::Committed {
            return;
        }
        let gateway = self.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(REPLY_GRACE).await;
            let uncovered = |work: &[LiveWork]| work.iter().any(|item| !item.is_covered_by(&covered));
            let mut now = gateway.live_work().await;
            let mut _guard = None;
            if !uncovered(&now) {
                if !gateway.freeze(epoch) {
                    return;
                }
                _guard = Some(FreezeGuard { gateway: gateway.clone(), epoch });
                now = gateway.live_work().await;
            }
            if uncovered(&now) {
                gateway.unfreeze(epoch);
                if gateway.epoch() != epoch {
                    return;
                }
                log::warn!("restart: work started after the answer was given; waiting for it instead");
                (gateway.deferred)(&now);
                gateway.arm(true, epoch);
            } else if gateway.commit(epoch) {
                (gateway.restart)();
            }
        });
        if let Some(old) = control.task.replace(task) {
            old.abort();
        }
    }

    pub async fn request(self: &Arc<Self>, mode: Mode, confirmed: Option<Vec<LiveWork>>) -> Reply {
        if mode == Mode::Cancel {
            let mut control = self.lock();
            if control.phase == Phase::Committed {
                // Too late, and said so: the workers are being ended.
                return Reply { outcome: Outcome::AlreadyRestarting, scheduled: false, work: Vec::new() };
            }
            control.epoch += 1;
            control.phase = Phase::Idle;
            let task = control.task.take();
            drop(control);
            let was = task.as_ref().is_some_and(|task| !task.is_finished());
            if let Some(task) = task {
                task.abort();
            }
            let outcome = if was { Outcome::Cancelled } else { Outcome::NothingScheduled };
            return Reply { outcome, scheduled: false, work: Vec::new() };
        }
        let epoch = self.epoch();
        let work = self.live_work().await;
        if self.lock().phase == Phase::Committed {
            return Reply { outcome: Outcome::Restarting, scheduled: false, work };
        }
        match mode {
            Mode::Now => {
                // Covered: everything alive now is something the person has seen and said yes to.
                let covered = confirmed.unwrap_or_default();
                if work.iter().all(|item| item.is_covered_by(&covered)) {
                    self.fire(covered, epoch);
                    Reply { outcome: Outcome::Restarting, scheduled: self.scheduled(), work }
                } else {
                    Reply { outcome: Outcome::NeedsConfirmation, scheduled: self.scheduled(), work }
                }
            }
            Mode::WhenIdle => {
                if work.is_empty() {
                    self.fire(Vec::new(), epoch);
                    return Reply { outcome: Outcome::Restarting, scheduled: false, work };
                }
                self.arm(false, epoch);
                Reply { outcome: Outcome::Scheduled, scheduled: true, work }
            }
            Mode::Cancel => unreachable!("handled above"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn work() -> LiveWork {
        LiveWork { kind: WorkKind::Shell, id: Some(3), project: "/p".into(), label: "shell".into(), owner: None }
    }

    struct Rig {
        gateway: Arc<Gateway>,
        alive: Arc<Mutex<Vec<LiveWork>>>,
        restarts: Arc<AtomicUsize>,
        deferred: Arc<AtomicUsize>,
    }

    fn rig(initial: Vec<LiveWork>) -> Rig {
        let alive = Arc::new(Mutex::new(initial));
        let restarts = Arc::new(AtomicUsize::new(0));
        let deferred = Arc::new(AtomicUsize::new(0));
        let (list, count, told) = (alive.clone(), restarts.clone(), deferred.clone());
        let gateway = Gateway::new(
            Box::new(move || {
                let list = list.clone();
                Box::pin(async move { list.lock().unwrap().clone() })
            }),
            Arc::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
            }),
            Arc::new(move |_| {
                told.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::from_millis(50),
        );
        Rig { gateway, alive, restarts, deferred }
    }

    #[tokio::test(start_paused = true)]
    async fn restart_now_with_live_work_lists_it_and_restarts_nothing() {
        let rig = rig(vec![work()]);
        let reply = rig.gateway.request(Mode::Now, None).await;
        assert_eq!(reply.outcome, Outcome::NeedsConfirmation);
        assert_eq!(reply.work, vec![work()]);
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0, "nothing restarts without the confirmation");
    }

    #[tokio::test(start_paused = true)]
    async fn a_confirmed_restart_now_restarts_after_the_reply_has_left() {
        let rig = rig(vec![work()]);
        let reply = rig.gateway.request(Mode::Now, Some(vec![work()])).await;
        assert_eq!(reply.outcome, Outcome::Restarting);
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0, "the reply goes first");
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn with_nothing_alive_restart_now_needs_no_confirmation() {
        let rig = rig(Vec::new());
        assert_eq!(rig.gateway.request(Mode::Now, None).await.outcome, Outcome::Restarting);
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn when_idle_waits_for_the_work_and_never_ends_it() {
        let rig = rig(vec![work()]);
        let reply = rig.gateway.request(Mode::WhenIdle, None).await;
        assert_eq!((reply.outcome, reply.scheduled), (Outcome::Scheduled, true));
        // Asking again does not arm a second watch.
        assert_eq!(rig.gateway.request(Mode::WhenIdle, None).await.outcome, Outcome::Scheduled);
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0, "still busy");
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1, "restarted once, when the last work ended");
    }

    #[tokio::test(start_paused = true)]
    async fn a_scheduled_restart_can_be_withdrawn() {
        let rig = rig(vec![work()]);
        rig.gateway.request(Mode::WhenIdle, None).await;
        let reply = rig.gateway.request(Mode::Cancel, None).await;
        assert_eq!((reply.outcome, reply.scheduled), (Outcome::Cancelled, false));
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0);
        assert_eq!(rig.gateway.request(Mode::Cancel, None).await.outcome, Outcome::NothingScheduled);
    }

    fn other() -> LiveWork {
        LiveWork { kind: WorkKind::Agent, id: Some(9), project: "/p".into(), label: "claude".into(), owner: None }
    }

    #[tokio::test(start_paused = true)]
    async fn a_confirmation_covers_only_the_work_it_names() {
        let rig = rig(vec![work()]);
        // The person saw `work()`; before they answered, another session started.
        rig.alive.lock().unwrap().push(other());
        let reply = rig.gateway.request(Mode::Now, Some(vec![work()])).await;
        assert_eq!(reply.outcome, Outcome::NeedsConfirmation);
        assert_eq!(reply.work, vec![work(), other()], "the fresh list is what they are shown");
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0);
        // Confirming the fresh list is enough.
        let reply = rig.gateway.request(Mode::Now, Some(reply.work)).await;
        assert_eq!(reply.outcome, Outcome::Restarting);
    }

    #[tokio::test(start_paused = true)]
    async fn work_that_starts_after_an_empty_answer_is_not_ended_unseen() {
        let rig = rig(Vec::new());
        assert_eq!(rig.gateway.request(Mode::Now, None).await.outcome, Outcome::Restarting);
        // Inside the grace period before the restart runs, a session starts.
        rig.alive.lock().unwrap().push(other());
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0, "it did not restart over the new session");
        // It waits for it instead, and then goes.
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn a_confirmed_restart_does_not_cover_work_that_appears_before_it_runs() {
        let rig = rig(vec![work()]);
        assert_eq!(rig.gateway.request(Mode::Now, Some(vec![work()])).await.outcome, Outcome::Restarting);
        rig.alive.lock().unwrap().push(other());
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn when_idle_looks_again_before_it_fires() {
        let rig = rig(vec![work()]);
        rig.gateway.request(Mode::WhenIdle, None).await;
        rig.alive.lock().unwrap().clear();
        // The watch sees an empty list and fires; new work arrives during the grace period.
        tokio::time::sleep(Duration::from_millis(60)).await;
        rig.alive.lock().unwrap().push(other());
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0);
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
    }

    fn run(token: u64) -> LiveWork {
        LiveWork { kind: WorkKind::Run, id: Some(token), project: "/p".into(), label: "run".into(), owner: None }
    }

    #[tokio::test(start_paused = true)]
    async fn cancel_reaches_a_restart_that_is_already_on_its_way() {
        // (a) the watch saw an empty list and fired; the person cancels inside the grace period.
        let rig = rig(vec![work()]);
        rig.gateway.request(Mode::WhenIdle, None).await;
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_millis(60)).await;
        let reply = rig.gateway.request(Mode::Cancel, None).await;
        assert_eq!(reply.outcome, Outcome::Cancelled, "an in-flight restart counts as scheduled");
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0);
        // And a `now` that has been answered but not yet run is withdrawn the same way.
        assert_eq!(rig.gateway.request(Mode::Now, None).await.outcome, Outcome::Restarting);
        assert_eq!(rig.gateway.request(Mode::Cancel, None).await.outcome, Outcome::Cancelled);
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancelled_restart_is_not_re_armed_by_work_that_appears_afterwards() {
        // (b) same window, and work appears too: the restart the person withdrew must not come back.
        let rig = rig(vec![work()]);
        rig.gateway.request(Mode::WhenIdle, None).await;
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_millis(60)).await;
        rig.alive.lock().unwrap().push(other());
        assert_eq!(rig.gateway.request(Mode::Cancel, None).await.outcome, Outcome::Cancelled);
        tokio::time::sleep(REPLY_GRACE * 2).await;
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0);
        assert_eq!(rig.gateway.request(Mode::Cancel, None).await.outcome, Outcome::NothingScheduled);
        assert!(!rig.gateway.frozen());
    }

    #[tokio::test(start_paused = true)]
    async fn work_is_refused_from_the_last_look_and_a_late_arrival_lifts_it() {
        let rig = rig(Vec::new());
        rig.gateway.request(Mode::Now, None).await;
        tokio::time::sleep(REPLY_GRACE + Duration::from_millis(10)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
        assert!(rig.gateway.frozen(), "once committed, work-creating commands are refused");

        // Work that gets in between the first look and the freeze is caught by the second one.
        let rig = rig_with_arrival();
        rig.gateway.request(Mode::Now, None).await;
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0, "the final look found it");
        assert!(!rig.gateway.frozen(), "and the freeze is lifted");
        assert_eq!(rig.deferred.load(Ordering::SeqCst), 1, "the clients are told it was put off");
    }

    /// A list that is empty the first time it is read and has a session in it from the second.
    fn rig_with_arrival() -> Rig {
        let reads = Arc::new(AtomicUsize::new(0));
        let rig = rig(Vec::new());
        let (restarts, told) = (rig.restarts.clone(), rig.deferred.clone());
        let (count, deferred) = (restarts.clone(), told.clone());
        let gateway = Gateway::new(
            Box::new(move || {
                let first = reads.fetch_add(1, Ordering::SeqCst) == 0;
                Box::pin(async move { if first { Vec::new() } else { vec![other()] } })
            }),
            Arc::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
            }),
            Arc::new(move |_| {
                deferred.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::from_secs(3600),
        );
        Rig { gateway, alive: rig.alive, restarts, deferred: told }
    }

    #[tokio::test(start_paused = true)]
    async fn a_confirmation_of_one_run_does_not_cover_another_on_the_same_project() {
        let rig = rig(vec![run(2)]);
        let reply = rig.gateway.request(Mode::Now, Some(vec![run(1)])).await;
        assert_eq!(reply.outcome, Outcome::NeedsConfirmation);
        assert_eq!(rig.gateway.request(Mode::Now, Some(vec![run(2)])).await.outcome, Outcome::Restarting);
    }

    #[tokio::test(start_paused = true)]
    async fn a_session_of_a_covered_run_is_covered_with_it() {
        let batch = LiveWork { owner: Some(7), ..other() };
        let rig = rig(vec![run(7), batch.clone()]);
        let reply = rig.gateway.request(Mode::Now, Some(vec![run(7)])).await;
        assert_eq!(reply.outcome, Outcome::Restarting, "the batch is the run's to end");
        let rig = rig_batches(run(8), batch);
        assert_eq!(rig.gateway.request(Mode::Now, Some(vec![run(8)])).await.outcome, Outcome::NeedsConfirmation);
    }

    fn rig_batches(run: LiveWork, batch: LiveWork) -> Rig {
        rig(vec![run, batch])
    }

    /// A gateway whose list is always empty, except that on its `call`th read (counting the request's
    /// own) the person's cancel lands, in the middle of the task that is reading it. That is the
    /// interleaving a second thread would produce, placed exactly instead of waited for.
    fn rig_cancelling_on(call: usize) -> (Arc<Gateway>, Arc<AtomicUsize>) {
        let restarts = Arc::new(AtomicUsize::new(0));
        let cell: Arc<std::sync::OnceLock<Arc<Gateway>>> = Arc::default();
        let (reads, count, handle) = (Arc::new(AtomicUsize::new(0)), restarts.clone(), cell.clone());
        let gateway = Gateway::new(
            Box::new(move || {
                let n = reads.fetch_add(1, Ordering::SeqCst) + 1;
                let handle = handle.clone();
                Box::pin(async move {
                    if n == call {
                        let reply = handle.get().unwrap().request(Mode::Cancel, None).await;
                        assert_eq!(reply.outcome, Outcome::Cancelled);
                    }
                    Vec::new()
                })
            }),
            Arc::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
            }),
            Arc::new(|_| {}),
            Duration::from_secs(3600),
        );
        let _ = cell.set(gateway.clone());
        (gateway, restarts)
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancel_between_the_first_look_and_the_freeze_leaves_nothing_frozen() {
        let (gateway, restarts) = rig_cancelling_on(2);
        assert_eq!(gateway.request(Mode::Now, None).await.outcome, Outcome::Restarting);
        tokio::time::sleep(REPLY_GRACE * 3).await;
        assert!(!gateway.frozen(), "the task's freeze was refused: its epoch had moved");
        assert_eq!(restarts.load(Ordering::SeqCst), 0);
        assert_eq!(gateway.request(Mode::Cancel, None).await.outcome, Outcome::NothingScheduled);
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancel_between_the_freeze_and_the_commit_lifts_it_and_commits_nothing() {
        let (gateway, restarts) = rig_cancelling_on(3);
        assert_eq!(gateway.request(Mode::Now, None).await.outcome, Outcome::Restarting);
        tokio::time::sleep(REPLY_GRACE * 3).await;
        assert!(!gateway.frozen());
        assert_eq!(restarts.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn a_freeze_belongs_to_the_epoch_that_raised_it() {
        let rig = rig(Vec::new());
        let gateway = &rig.gateway;
        let epoch = gateway.epoch();
        // A cancel moves the epoch; the task that began before it may not freeze afterwards.
        gateway.lock().epoch += 1;
        assert!(!gateway.freeze(epoch));
        assert!(!gateway.frozen());
        // Its own freeze is lowered by its guard, however it ends.
        let epoch = gateway.epoch();
        assert!(gateway.freeze(epoch));
        assert!(!gateway.freeze(epoch), "not twice");
        drop(FreezeGuard { gateway: gateway.clone(), epoch });
        assert!(!gateway.frozen(), "an aborted task lifts its own freeze");
        // Another task's freeze is not its to lower.
        assert!(gateway.freeze(epoch));
        gateway.unfreeze(epoch + 1);
        assert!(gateway.frozen());
    }

    #[tokio::test(start_paused = true)]
    async fn once_committed_a_cancel_is_too_late_and_the_freeze_stays() {
        let rig = rig(Vec::new());
        rig.gateway.request(Mode::Now, None).await;
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
        let reply = rig.gateway.request(Mode::Cancel, None).await;
        assert_eq!(reply.outcome, Outcome::AlreadyRestarting, "not NothingScheduled");
        assert!(rig.gateway.frozen(), "work is still refused while the workers are ended");
        // And asking again does not restart twice.
        assert_eq!(rig.gateway.request(Mode::Now, None).await.outcome, Outcome::Restarting);
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
    }
}
