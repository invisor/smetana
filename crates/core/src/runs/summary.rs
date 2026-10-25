//! What a run did, as the app can honestly know it.
//!
//! The board diffed between the run's first read and its last: a task is this
//! run's work when its status moved into `closed` or `parked` while the run was
//! going. Pure — issues in, lists out — which is what lets the rules below be
//! tested rather than reasoned about, the same shape `queue.rs` keeps.
//!
//! Attribution by actor was the alternative and it misses two real cases: an
//! orphan recovered from a previous killed run in Phase R carries that dead
//! run's actor, and an epic closed in Phase 3 was never claimed by anybody. The
//! diff's own cost is named instead rather than hidden — a task a person closes
//! by hand in another window while the run is going is credited to the run. The
//! report is a statement about what moved on the board while the run was going,
//! and it should be read as one.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::runs::model::RunScope;
use crate::runs::queue::{is_lock, CLOSED, PARKED};
use crate::tracker::model::Issue;

/// One row of a report: what the board calls it, and nothing the board does not
/// know. What was *done* comes from the batch's own file — see `report.rs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLine {
    pub id: String,
    pub title: String,
}

/// What moved, split by where it ended up.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tasks {
    pub closed: Vec<TaskLine>,
    pub parked: Vec<TaskLine>,
}

/// What a finished run has to say about itself.
///
/// `tasks` is `Option` and that is the whole point of the type: `None` means
/// the diff could not be computed — the run died in its preflight, so there is
/// no baseline, or the board could not be read at the end. It is never rendered
/// as "0 closed, 0 parked". An unreadable board and an empty board are opposite
/// facts, which is the rule `projectBytes` and `cleanup::refusal` already hold
/// everywhere else in this app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    /// Wall clock from the start to `Stopped`, preflight, pauses and backoff
    /// included. The only number about a run's time that cannot be computed
    /// wrongly, and the one somebody means when they ask how long the night
    /// took.
    pub seconds: u64,
    pub tasks: Option<Tasks>,
    /// Absolute path of the document. `None` when it could not be written — a
    /// card offering a report that is not there is worse than one without a
    /// button.
    pub report: Option<String>,
}

/// The board as the run first saw it: id → status, over the report's scope.
#[derive(Debug, Clone, Default)]
pub struct Baseline(HashMap<String, String>);

impl Baseline {
    pub fn of(issues: &[Issue], scope: &RunScope) -> Self {
        Baseline(
            issues
                .iter()
                .filter(|i| in_report_scope(i, scope))
                .map(|i| (i.id.clone(), i.status.clone()))
                .collect(),
        )
    }

    fn was(&self, id: &str, status: &str) -> bool {
        self.0.get(id).is_some_and(|held| held == status)
    }
}

/// Deliberately wider than `queue::in_scope`, which answers "may this run take
/// that task". For an epic that one means its children only; Phase 3 closes the
/// epic itself, and a summary leaving it out is missing the ending. The
/// priority floor is not applied for the same kind of reason: it decides what
/// may be taken, and this is about what moved — Phase R already reaches past
/// it, and a report dropping a recovered orphan would be missing real work.
///
/// The merge lock is excluded exactly as `queue.rs` excludes it, through the
/// very same predicate rather than a second copy of the label: it is
/// coordination, never work.
fn in_report_scope(issue: &Issue, scope: &RunScope) -> bool {
    if is_lock(issue) {
        return false;
    }
    match scope {
        RunScope::Queue => true,
        RunScope::Task { id } => &issue.id == id,
        RunScope::Epic { id } => &issue.id == id || issue.parent.as_ref() == Some(id),
    }
}

/// What moved. A task counts once, by the status it holds now: parked and then
/// closed is closed, which is the true reading and needs no ordering to be kept.
pub fn diff(baseline: &Baseline, issues: &[Issue], scope: &RunScope) -> Tasks {
    let mut tasks = Tasks::default();
    for issue in issues.iter().filter(|i| in_report_scope(i, scope)) {
        let line = TaskLine { id: issue.id.clone(), title: issue.title.clone() };
        match issue.status.as_str() {
            CLOSED if !baseline.was(&issue.id, CLOSED) => tasks.closed.push(line),
            PARKED if !baseline.was(&issue.id, PARKED) => tasks.parked.push(line),
            _ => {}
        }
    }
    tasks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::model::Issue;

    fn issue(id: &str, status: &str) -> Issue {
        Issue {
            id: id.into(),
            title: format!("{id} title"),
            status: status.into(),
            ..Default::default()
        }
    }

    fn child(id: &str, status: &str, parent: &str) -> Issue {
        Issue { parent: Some(parent.into()), ..issue(id, status) }
    }

    #[test]
    fn a_task_closed_since_the_baseline_is_this_runs_work() {
        let before = [issue("a-1", "open")];
        let after = [issue("a-1", "closed")];
        let base = Baseline::of(&before, &RunScope::Queue);
        let tasks = diff(&base, &after, &RunScope::Queue);
        assert_eq!(tasks.closed.len(), 1, "the one task that moved");
        assert_eq!(tasks.closed[0].id, "a-1");
        assert!(tasks.parked.is_empty(), "nothing was parked");
    }

    #[test]
    fn a_task_already_closed_before_the_run_is_not_credited_to_it() {
        let before = [issue("a-1", "closed")];
        let after = [issue("a-1", "closed")];
        let base = Baseline::of(&before, &RunScope::Queue);
        assert!(diff(&base, &after, &RunScope::Queue).closed.is_empty());
    }

    #[test]
    fn a_task_created_during_the_run_and_closed_counts_as_closed() {
        let before: [Issue; 0] = [];
        let after = [issue("a-2", "closed")];
        let base = Baseline::of(&before, &RunScope::Queue);
        assert_eq!(diff(&base, &after, &RunScope::Queue).closed.len(), 1);
    }

    #[test]
    fn a_task_parked_then_closed_is_counted_once_by_where_it_ended() {
        // The baseline holds the status as it was, so nothing about the middle
        // of the run is remembered — and nothing has to be, since a task ends
        // in exactly one place.
        let before = [issue("a-1", "parked")];
        let after = [issue("a-1", "closed")];
        let base = Baseline::of(&before, &RunScope::Queue);
        let tasks = diff(&base, &after, &RunScope::Queue);
        assert_eq!(tasks.closed.len(), 1);
        assert!(tasks.parked.is_empty(), "the status now is the whole answer");
    }

    #[test]
    fn an_epic_run_reports_the_epic_itself_as_well_as_its_children() {
        let before = [issue("e-1", "open"), child("c-1", "open", "e-1")];
        let after = [issue("e-1", "closed"), child("c-1", "closed", "e-1")];
        let scope = RunScope::Epic { id: "e-1".into() };
        let base = Baseline::of(&before, &scope);
        let tasks = diff(&base, &after, &scope);
        let ids: Vec<&str> = tasks.closed.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, ["e-1", "c-1"], "Phase 3 closes the epic and the report says so");
    }

    #[test]
    fn another_epics_children_are_not_in_this_epics_report() {
        let before = [child("c-9", "open", "e-2")];
        let after = [child("c-9", "closed", "e-2")];
        let scope = RunScope::Epic { id: "e-1".into() };
        let base = Baseline::of(&before, &scope);
        assert!(diff(&base, &after, &scope).closed.is_empty());
    }

    #[test]
    fn a_task_run_reports_that_task_and_nothing_beside_it() {
        let before = [issue("a-1", "open"), issue("a-2", "open")];
        let after = [issue("a-1", "closed"), issue("a-2", "closed")];
        let scope = RunScope::Task { id: "a-1".into() };
        let base = Baseline::of(&before, &scope);
        let tasks = diff(&base, &after, &scope);
        assert_eq!(tasks.closed.len(), 1);
        assert_eq!(tasks.closed[0].id, "a-1");
    }

    #[test]
    fn the_merge_lock_is_coordination_and_never_appears_in_a_report() {
        let mut lock = issue("lock-1", "open");
        lock.labels = vec![crate::runs::queue::LOCK_LABEL.into()];
        let before = [lock.clone()];
        let mut closed = lock;
        closed.status = "closed".into();
        let base = Baseline::of(&before, &RunScope::Queue);
        assert!(diff(&base, &[closed], &RunScope::Queue).closed.is_empty());
    }

    #[test]
    fn the_priority_floor_does_not_narrow_a_report() {
        // The floor decides what may be taken. Phase R reaches past it, and a
        // report that dropped a recovered orphan would be missing real work.
        let mut low = issue("a-4", "open");
        low.priority = Some(4);
        let before = [low.clone()];
        let mut done = low;
        done.status = "closed".into();
        let base = Baseline::of(&before, &RunScope::Queue);
        assert_eq!(diff(&base, &[done], &RunScope::Queue).closed.len(), 1);
    }

    #[test]
    fn a_parked_task_carries_its_id_and_title() {
        let before = [issue("a-1", "in_progress")];
        let after = [issue("a-1", "parked")];
        let base = Baseline::of(&before, &RunScope::Queue);
        let tasks = diff(&base, &after, &RunScope::Queue);
        assert_eq!(tasks.parked[0].id, "a-1");
        assert_eq!(tasks.parked[0].title, "a-1 title");
    }

    #[test]
    fn a_task_already_parked_before_the_run_is_not_credited_to_it() {
        let before = [issue("a-1", "parked")];
        let after = [issue("a-1", "parked")];
        let base = Baseline::of(&before, &RunScope::Queue);
        assert!(diff(&base, &after, &RunScope::Queue).parked.is_empty());
    }

    #[test]
    fn work_that_is_still_going_is_in_neither_list() {
        // The report says what moved into a resting place, not what is halfway.
        let before = [issue("a-1", "open")];
        let after = [issue("a-1", "in_progress")];
        let base = Baseline::of(&before, &RunScope::Queue);
        let tasks = diff(&base, &after, &RunScope::Queue);
        assert!(tasks.closed.is_empty() && tasks.parked.is_empty());
    }
}
