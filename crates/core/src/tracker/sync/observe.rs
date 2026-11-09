//! Watch every enabled open project, with a periodic full sweep as backup.
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::Instant;

use super::ProjectSync;
use crate::tracker::{
    bd::Bd,
    model::Issue,
    watcher::{self, WatchEvent},
};

fn same_tasks(before: &[Issue], after: &[Issue]) -> bool {
    if before.len() != after.len() {
        return false;
    }
    let by_id: std::collections::HashMap<&str, &Issue> = before
        .iter()
        .map(|issue| (issue.id.as_str(), issue))
        .collect();
    after.iter().all(|issue| {
        by_id
            .get(issue.id.as_str())
            .is_some_and(|old| *old == issue)
    })
}

pub(super) fn start(runtime: Arc<ProjectSync>) {
    crate::context::spawn(async move {
        let (tx, mut rx) = mpsc::channel(16);
        let _watcher = watcher::spawn(runtime.root.join(".beads"), tx).ok();
        let mut watching = _watcher.is_some();
        scan(&runtime).await;
        let mut sweep = tokio::time::interval(Duration::from_secs(60));
        let mut due = tokio::time::interval(Duration::from_secs(1));
        let mut interval = runtime.config.lock().await.pull_interval_minutes;
        let mut next_pull = Instant::now() + Duration::from_secs(interval as u64 * 60);
        while runtime.open.load(Ordering::SeqCst) {
            tokio::select! {
                event = rx.recv(), if watching => {
                    match event {
                        Some(WatchEvent::Changed) => scan(&runtime).await,
                        Some(WatchEvent::Failed(reason)) => {
                            log::warn!("kanban sync watcher failed for {}: {reason}", runtime.root.display());
                            watching = false;
                        }
                        None => watching = false,
                    }
                }
                _ = sweep.tick() => scan(&runtime).await,
                _ = due.tick() => {
                    let pushing = runtime.clone();
                    crate::context::spawn(async move { pushing.push_if_due().await; });
                    let now = Instant::now();
                    let configured = runtime.config.lock().await.pull_interval_minutes;
                    if configured != interval {
                        interval = configured;
                        next_pull = now + Duration::from_secs(interval as u64 * 60);
                    }
                    if now >= next_pull {
                        next_pull = now + Duration::from_secs(interval as u64 * 60);
                        let pulling = runtime.clone();
                        crate::context::spawn(async move { let _ = pulling.pull().await; });
                    }
                }
            }
        }
    });
}

pub(super) async fn scan(runtime: &ProjectSync) {
    let _guard = runtime.scan_lock.lock().await;
    scan_unlocked(runtime).await;
}

pub(super) async fn scan_unlocked(runtime: &ProjectSync) {
    if runtime.pulling.load(Ordering::SeqCst) {
        return;
    }
    if let Ok(issues) = Bd::new(runtime.ctx.clone(), runtime.root.clone())
        .list_all()
        .await
    {
        let mut baseline = runtime.baseline.lock().await;
        if let Some(before) = baseline.as_ref() {
            if !same_tasks(before, &issues) {
                runtime.policy.lock().await.changed(Instant::now());
            }
        }
        *baseline = Some(issues);
    }
}
