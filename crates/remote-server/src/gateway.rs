//! The restart gateway: what a client may ask of the server about restarting it, and the rule that
//! nothing is killed silently.
//!
//! A restart of the server ends every child it owns — runs, agent sessions, driven sessions, plain
//! shells — so the gate is not "no run" but **no live work of any kind**. A client therefore never
//! says "restart" and gets one. It says *when*:
//!
//! - **`now`** restarts at once if nothing is alive. If something is, it answers
//!   [`Outcome::NeedsConfirmation`] with the list and does nothing; the same request with
//!   `confirm: true` is the person's answer to that list, and only that ends the work.
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
    /// The session's id in its own worker; `None` for a run, which is named by its project.
    pub id: Option<u64>,
    pub project: String,
    /// A short phrase for the person: the agent's name, or `shell`.
    pub label: String,
}

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
    poll: Duration,
    watch: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// How long the reply gets to leave before the restart runs.
const REPLY_GRACE: Duration = Duration::from_millis(300);

impl Gateway {
    pub fn new(
        list: Box<dyn Fn() -> BoxFuture<Vec<LiveWork>> + Send + Sync>,
        restart: Arc<dyn Fn() + Send + Sync>,
        poll: Duration,
    ) -> Arc<Self> {
        Arc::new(Self { list, restart, poll, watch: Mutex::new(None) })
    }

    fn watch(&self) -> std::sync::MutexGuard<'_, Option<tokio::task::JoinHandle<()>>> {
        self.watch.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn scheduled(&self) -> bool {
        self.watch().as_ref().is_some_and(|task| !task.is_finished())
    }

    /// The work alive right now.
    pub async fn live_work(&self) -> Vec<LiveWork> {
        (self.list)().await
    }

    fn fire(&self) {
        let restart = self.restart.clone();
        tokio::spawn(async move {
            tokio::time::sleep(REPLY_GRACE).await;
            restart();
        });
    }

    pub async fn request(self: &Arc<Self>, mode: Mode, confirm: bool) -> Reply {
        match mode {
            Mode::Cancel => {
                let task = self.watch().take();
                let was = task.as_ref().is_some_and(|task| !task.is_finished());
                if let Some(task) = task {
                    task.abort();
                }
                let outcome = if was { Outcome::Cancelled } else { Outcome::NothingScheduled };
                Reply { outcome, scheduled: false, work: Vec::new() }
            }
            Mode::Now => {
                let work = self.live_work().await;
                if work.is_empty() || confirm {
                    self.fire();
                    Reply { outcome: Outcome::Restarting, scheduled: self.scheduled(), work }
                } else {
                    Reply { outcome: Outcome::NeedsConfirmation, scheduled: self.scheduled(), work }
                }
            }
            Mode::WhenIdle => {
                let work = self.live_work().await;
                if work.is_empty() {
                    self.fire();
                    return Reply { outcome: Outcome::Restarting, scheduled: false, work };
                }
                if !self.scheduled() {
                    let gateway = self.clone();
                    let task = tokio::spawn(async move {
                        loop {
                            tokio::time::sleep(gateway.poll).await;
                            if gateway.live_work().await.is_empty() {
                                gateway.fire();
                                return;
                            }
                        }
                    });
                    *self.watch() = Some(task);
                }
                Reply { outcome: Outcome::Scheduled, scheduled: true, work }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn work() -> LiveWork {
        LiveWork { kind: WorkKind::Shell, id: Some(3), project: "/p".into(), label: "shell".into() }
    }

    struct Rig {
        gateway: Arc<Gateway>,
        alive: Arc<Mutex<Vec<LiveWork>>>,
        restarts: Arc<AtomicUsize>,
    }

    fn rig(initial: Vec<LiveWork>) -> Rig {
        let alive = Arc::new(Mutex::new(initial));
        let restarts = Arc::new(AtomicUsize::new(0));
        let (list, count) = (alive.clone(), restarts.clone());
        let gateway = Gateway::new(
            Box::new(move || {
                let list = list.clone();
                Box::pin(async move { list.lock().unwrap().clone() })
            }),
            Arc::new(move || {
                count.fetch_add(1, Ordering::SeqCst);
            }),
            Duration::from_millis(50),
        );
        Rig { gateway, alive, restarts }
    }

    #[tokio::test(start_paused = true)]
    async fn restart_now_with_live_work_lists_it_and_restarts_nothing() {
        let rig = rig(vec![work()]);
        let reply = rig.gateway.request(Mode::Now, false).await;
        assert_eq!(reply.outcome, Outcome::NeedsConfirmation);
        assert_eq!(reply.work, vec![work()]);
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0, "nothing restarts without the confirmation");
    }

    #[tokio::test(start_paused = true)]
    async fn a_confirmed_restart_now_restarts_after_the_reply_has_left() {
        let rig = rig(vec![work()]);
        let reply = rig.gateway.request(Mode::Now, true).await;
        assert_eq!(reply.outcome, Outcome::Restarting);
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0, "the reply goes first");
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn with_nothing_alive_restart_now_needs_no_confirmation() {
        let rig = rig(Vec::new());
        assert_eq!(rig.gateway.request(Mode::Now, false).await.outcome, Outcome::Restarting);
        tokio::time::sleep(REPLY_GRACE * 2).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn when_idle_waits_for_the_work_and_never_ends_it() {
        let rig = rig(vec![work()]);
        let reply = rig.gateway.request(Mode::WhenIdle, false).await;
        assert_eq!((reply.outcome, reply.scheduled), (Outcome::Scheduled, true));
        // Asking again does not arm a second watch.
        assert_eq!(rig.gateway.request(Mode::WhenIdle, false).await.outcome, Outcome::Scheduled);
        tokio::time::sleep(Duration::from_secs(60)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0, "still busy");
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_secs(1)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 1, "restarted once, when the last work ended");
    }

    #[tokio::test(start_paused = true)]
    async fn a_scheduled_restart_can_be_withdrawn() {
        let rig = rig(vec![work()]);
        rig.gateway.request(Mode::WhenIdle, false).await;
        let reply = rig.gateway.request(Mode::Cancel, false).await;
        assert_eq!((reply.outcome, reply.scheduled), (Outcome::Cancelled, false));
        rig.alive.lock().unwrap().clear();
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(rig.restarts.load(Ordering::SeqCst), 0);
        assert_eq!(rig.gateway.request(Mode::Cancel, false).await.outcome, Outcome::NothingScheduled);
    }
}
