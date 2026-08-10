//! What is left to do, and whether to run another batch.
//!
//! The port of `holiday-curb`'s `loop-state.mjs`, with one substitution that
//! changes its cost rather than its logic: the source shelled out to `bd ready`
//! and `bd list` between every batch, about four seconds each time, while this
//! reads the snapshot the tracker worker already keeps current from its
//! watcher. Pure — issues in, numbers out — which is what lets the decisions
//! below be tested instead of reasoned about.

use std::collections::HashSet;

use crate::runs::model::{RunScope, StopReason};
use crate::tracker::model::Issue;

/// bd's own status for work that has been claimed.
const IN_PROGRESS: &str = "in_progress";
/// Our custom status for work that is reviewed and not yet merged.
const READY_TO_MERGE: &str = "ready_to_merge";
/// Our custom status for a dead end left for a person.
const PARKED: &str = "parked";
/// bd's own status for work that is available.
const OPEN: &str = "open";
const CLOSED: &str = "closed";
/// The dependency kind that actually blocks. bd also records `parent-child`,
/// `related` and `discovered-from`, and none of those means "wait".
const BLOCKS: &str = "blocks";
/// The label on the merge lock — the issue two leads claim to serialize their
/// merges into one target branch (smetana-uox). Only an `open` issue is
/// claimable, so the lock has to sit in `open` while free — which means
/// `bd ready` returns it and, unfiltered, it would count as ready work here;
/// held, it is `in_progress` and would count as unfinished. Either reading
/// turns coordination into work: a lead would try to implement it, and a run
/// would keep taking batches to "recover" it.
const LOCK_LABEL: &str = "smetana-lock";

/// The board, as a run cares about it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct QueueSnapshot {
    /// Open, unblocked, within the scope and no worse than the floor.
    pub ready: Vec<String>,
    /// Claimed or reviewed but not finished: `in_progress` and
    /// `ready_to_merge`. Tracked separately because `bd ready` hides both, and
    /// a run that only watched the ready set would leave a killed batch's
    /// orphans on the board forever.
    pub unfinished: Vec<String>,
    pub closed: usize,
    pub parked: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Run another batch. The reason is for the log, not for the front end.
    Run(RunReason),
    Stop(StopReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunReason {
    ReadyWork,
    /// Nothing new to take, but a killed batch left work behind.
    RecoverUnfinished,
    RetryAfterCrash,
    RetryAfterLimit,
}

/// How the batch before this one ended, as far as the decision cares.
///
/// Three answers rather than "did it crash", and the third is what the type
/// exists for: a batch stopped by a spent subscription allowance changed the
/// board no more than a crashed one did, and reading either as a stuck queue
/// would end a run over something that is not stuck. But they are not the same
/// event and must not be reported as one — a harness that keeps falling over
/// needs somebody to look at it, while an exhausted allowance needs nothing at
/// all except time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LastBatch {
    /// It ran to completion.
    Completed,
    /// Its session exited non-zero — in practice a transient failure of the
    /// harness.
    Crashed,
    /// It could not run, or could not finish: the allowance was spent.
    Limited,
}

/// Is this issue inside what the run was asked to work on?
fn in_scope(issue: &Issue, scope: &RunScope) -> bool {
    match scope {
        RunScope::Queue => true,
        RunScope::Task { id } => &issue.id == id,
        RunScope::Epic { id } => issue.parent.as_ref() == Some(id),
    }
}

/// bd's priority runs 0 (most urgent) to 4, so "no worse than the floor" is
/// `<=`. An issue with no priority at all is taken: bd omits the field rather
/// than defaulting it, and refusing to work on something because nobody graded
/// it would hide it from every run forever.
///
/// The floor is asked only of a queue, and `RunSettings::validate` is what
/// makes anything else `None`. Where a person named the work — one task, or one
/// epic's children — there is nothing to choose between, so the floor could
/// only drop what they picked: a P4 task run from its card under the default P2
/// floor left `ready` empty and stopped the run with `QueueEmpty`, about the
/// task the person was looking at. An epic's children are all of them, whatever
/// they are graded.
fn within_floor(issue: &Issue, scope: &RunScope, min_priority: Option<u8>) -> bool {
    match (scope, min_priority) {
        (RunScope::Queue, Some(floor)) => issue.priority.is_none_or(|p| p <= i64::from(floor)),
        _ => true,
    }
}

/// The board, under a scope and — for a queue — a floor.
///
/// `closed` and `parked` are counted across the scope and reported, but they
/// deliberately take no part in the decision below — see `next_action`.
pub fn snapshot(issues: &[Issue], scope: &RunScope, min_priority: Option<u8>) -> QueueSnapshot {
    // What satisfies a dependency is the blocker being *finished*, and finished
    // means `closed` — so the blocking set is everything on the board that is
    // not. Naming what finishes rather than listing what blocks is the only
    // form that survives a status set this app does not control: a parked or
    // blocked blocker, a deferred one, and any custom status bd grows all mean
    // the work is not done and the dependent must wait (smetana-6sl). Listing
    // three blocking statuses here read every other status as satisfied, and an
    // unattended run took work whose parked premise was never built.
    let not_finished: HashSet<&str> = issues
        .iter()
        .filter(|i| i.status != CLOSED)
        .map(|i| i.id.as_str())
        .collect();

    let mut out = QueueSnapshot::default();
    for issue in issues.iter().filter(|i| in_scope(i, scope) && !is_lock(i)) {
        match issue.status.as_str() {
            CLOSED => out.closed += 1,
            PARKED => out.parked += 1,
            IN_PROGRESS | READY_TO_MERGE => out.unfinished.push(issue.id.clone()),
            OPEN if within_floor(issue, scope, min_priority) && !blocked(issue, &not_finished) => {
                out.ready.push(issue.id.clone());
            }
            _ => {}
        }
    }
    out
}

/// Coordination, not work: the merge lock is invisible to the queue whatever
/// its status — free (`open`) it is not ready work, held (`in_progress`) it is
/// not a killed batch's orphan to recover.
fn is_lock(issue: &Issue) -> bool {
    issue.labels.iter().any(|l| l == LOCK_LABEL)
}

/// Waiting on something that has not finished. A dependency on a closed issue
/// is satisfied; one on an issue that is not on the board at all is treated as
/// satisfied too, since the alternative is a run that stalls on a reference
/// nobody can resolve and says nothing about why.
fn blocked(issue: &Issue, not_finished: &HashSet<&str>) -> bool {
    issue
        .dependencies
        .iter()
        .any(|d| d.kind == BLOCKS && not_finished.contains(d.depends_on_id.as_str()))
}

/// Order-independent equality: bd's ordering is not stable across calls, and
/// two passes returning the same work in a different order is not progress.
fn same_set(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && {
        let set: HashSet<&str> = b.iter().map(String::as_str).collect();
        a.iter().all(|id| set.contains(id.as_str()))
    }
}

/// Another batch, or an ending.
///
/// Five decisions, each of them a defect somebody already paid for:
///
/// - Work remains while **either** set is non-empty. A run that stopped on an
///   empty ready queue would abandon the orphans a killed batch left behind,
///   and nothing else ever picks those up.
/// - **Progress is either set changing**, not the closed or parked counts. A
///   batch that only moves tasks to `in_progress`, merges an orphan or parks a
///   stuck one has made progress, and stopping there would call a working run
///   stuck.
/// - **A batch that did not run to completion suppresses the no-progress
///   stop.** An unchanged board after a crash, or after an allowance ran out,
///   means the batch never got to move anything — not that the board is stuck.
/// - **A run allowed one batch stops once that batch has run to completion**
///   (`once`, derived from the mode — the decision cares about whether a second
///   batch may go out, not about who answers a question). `prev` is what says a
///   batch has gone out at all: it is `None` on the first look, so recovering a
///   killed run's orphans is the same first batch and not a second one. A crash
///   or a spent allowance means the batch never got to do its work, so the
///   retry below still runs — only a completed batch is the one batch. Both
///   stops above still outrank it: a board the batch emptied is honestly
///   `QueueEmpty`, and a completed batch that moved nothing is honestly stuck.
/// - The iteration cap is a backstop and nothing more.
pub fn next_action(
    now: &QueueSnapshot,
    prev: Option<&QueueSnapshot>,
    iteration: u32,
    max_iterations: u32,
    last: LastBatch,
    once: bool,
) -> Action {
    if now.ready.is_empty() && now.unfinished.is_empty() {
        return Action::Stop(StopReason::QueueEmpty);
    }
    if iteration >= max_iterations {
        return Action::Stop(StopReason::MaxIterations);
    }
    let unchanged = prev
        .is_some_and(|p| same_set(&now.ready, &p.ready) && same_set(&now.unfinished, &p.unfinished));
    if unchanged && matches!(last, LastBatch::Completed) {
        return Action::Stop(StopReason::NoProgress);
    }
    if once && prev.is_some() && matches!(last, LastBatch::Completed) {
        return Action::Stop(StopReason::BatchDone);
    }
    match last {
        LastBatch::Crashed => return Action::Run(RunReason::RetryAfterCrash),
        LastBatch::Limited => return Action::Run(RunReason::RetryAfterLimit),
        LastBatch::Completed => {}
    }
    Action::Run(if now.ready.is_empty() {
        RunReason::RecoverUnfinished
    } else {
        RunReason::ReadyWork
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tracker::model::Dependency;

    fn issue(id: &str, status: &str) -> Issue {
        // `..Default::default()` rather than every field spelled out: this is
        // what `Issue`'s `Default` exists for, and an exhaustive literal here
        // breaks the build every time the tracker's vocabulary grows a field.
        Issue {
            id: id.into(),
            title: id.into(),
            status: status.into(),
            updated_at: "2026-08-05".into(),
            priority: Some(1),
            ..Default::default()
        }
    }

    fn snap(ready: &[&str], unfinished: &[&str]) -> QueueSnapshot {
        QueueSnapshot {
            ready: ready.iter().map(|s| (*s).to_string()).collect(),
            unfinished: unfinished.iter().map(|s| (*s).to_string()).collect(),
            closed: 0,
            parked: 0,
        }
    }

    #[test]
    fn only_open_issues_are_ready() {
        // deferred is where findings go and parked is a dead end: a run that
        // took either would be feeding itself, which is the whole reason both
        // statuses exist.
        let board = vec![
            issue("a", "open"),
            issue("b", "deferred"),
            issue("c", "parked"),
            issue("d", "closed"),
            issue("e", "in_progress"),
            issue("f", "ready_to_merge"),
        ];
        let s = snapshot(&board, &RunScope::Queue, Some(4));
        assert_eq!(s.ready, vec!["a"]);
        assert_eq!(s.unfinished, vec!["e", "f"]);
        assert_eq!(s.closed, 1);
        assert_eq!(s.parked, 1);
    }

    #[test]
    fn the_merge_lock_is_neither_ready_nor_unfinished() {
        // Free, the lock sits `open` because only an open issue is claimable;
        // held, it is `in_progress`. The first must not enter `ready` — a lead
        // would take it as work — and the second must not enter `unfinished`,
        // or a run would keep taking batches to "recover" its own lock.
        let mut free = issue("lock-free", "open");
        free.labels = vec!["smetana-lock".into()];
        let mut held = issue("lock-held", "in_progress");
        held.labels = vec!["smetana-lock".into()];
        let board = vec![free, held, issue("real", "open")];

        let s = snapshot(&board, &RunScope::Queue, Some(4));
        assert_eq!(s.ready, vec!["real"]);
        assert!(s.unfinished.is_empty(), "a held lock is not a killed batch's orphan");
    }

    #[test]
    fn the_merge_lock_is_invisible_whatever_its_status() {
        // The rule is the label, not the status pair above: whatever state a
        // lock ends up in — somebody parking it or closing it by hand included
        // — it never enters the snapshot anywhere.
        for status in ["open", "in_progress", "ready_to_merge", "parked", "closed"] {
            let mut lock = issue("lock", status);
            lock.labels = vec!["smetana-lock".into()];
            assert_eq!(
                snapshot(&[lock], &RunScope::Queue, Some(4)),
                QueueSnapshot::default(),
                "a lock in `{status}` leaked into the snapshot"
            );
        }
    }

    #[test]
    fn an_ordinary_label_hides_nothing() {
        // `spawned` is the label filed findings already carry; only the lock's
        // own label may make an issue invisible to the queue.
        let mut labelled = issue("labelled", "open");
        labelled.labels = vec!["spawned".into()];
        assert_eq!(snapshot(&[labelled], &RunScope::Queue, Some(4)).ready, vec!["labelled"]);
    }

    #[test]
    fn the_floor_drops_what_is_worse_and_keeps_what_is_equal() {
        let mut low = issue("low", "open");
        low.priority = Some(3);
        let mut edge = issue("edge", "open");
        edge.priority = Some(2);
        let mut ungraded = issue("ungraded", "open");
        ungraded.priority = None;

        let s = snapshot(&[low, edge, ungraded], &RunScope::Queue, Some(2));
        assert_eq!(s.ready, vec!["edge", "ungraded"], "bd omits priority rather than defaulting it");
    }

    #[test]
    fn the_floor_does_not_reach_work_a_person_named() {
        // The defect this pins: a P4 task run from its own card under the
        // default P2 floor came back with an empty ready set, so the run
        // stopped at once and said the queue was empty — about the one task
        // the person had just pointed at. An epic's children are all of them,
        // whatever they are graded.
        let mut low = issue("low", "open");
        low.priority = Some(4);
        let mut child = issue("child", "open");
        child.priority = Some(4);
        child.parent = Some("epic".into());
        let board = vec![low, child, issue("epic", "open")];

        // Passed a floor anyway — validate refuses this payload, and the rule
        // is the scope's, not the caller's remembering to send None.
        assert_eq!(snapshot(&board, &RunScope::Task { id: "low".into() }, Some(2)).ready, vec!["low"]);
        assert_eq!(
            snapshot(&board, &RunScope::Epic { id: "epic".into() }, Some(2)).ready,
            vec!["child"]
        );

        // The same board as a queue is where the floor still bites.
        let queued = snapshot(&board, &RunScope::Queue, Some(2)).ready;
        assert!(
            !queued.contains(&"low".to_string()) && !queued.contains(&"child".to_string()),
            "a run choosing its own work still stops at the floor: {queued:?}"
        );
    }

    #[test]
    fn a_task_scope_is_that_issue_alone_and_an_epic_scope_is_its_children() {
        let mut child = issue("child", "open");
        child.parent = Some("epic".into());
        let board = vec![issue("other", "open"), child, issue("epic", "open")];

        assert_eq!(snapshot(&board, &RunScope::Task { id: "other".into() }, None).ready, vec!["other"]);
        assert_eq!(snapshot(&board, &RunScope::Epic { id: "epic".into() }, None).ready, vec!["child"]);
    }

    #[test]
    fn an_issue_waiting_on_unfinished_work_is_not_ready() {
        let mut waiting = issue("waiting", "open");
        waiting.dependencies = vec![Dependency {
            issue_id: "waiting".into(),
            depends_on_id: "earlier".into(),
            kind: "blocks".into(),
        }];
        let board = vec![waiting.clone(), issue("earlier", "open")];
        assert!(snapshot(&board, &RunScope::Queue, Some(4)).ready.iter().all(|id| id != "waiting"));

        // The same issue once what it waited for has closed.
        let board = vec![waiting, issue("earlier", "closed")];
        assert_eq!(snapshot(&board, &RunScope::Queue, Some(4)).ready, vec!["waiting"]);
    }

    #[test]
    fn a_blocker_in_any_unfinished_status_keeps_its_dependent_waiting() {
        // The defect this pins (smetana-6sl): the blocking set used to list
        // three statuses, so a dependency on anything else read as satisfied.
        // An unattended run parks what it cannot settle, and the very next
        // batch took the dependents of the parked blocker. The rule is the
        // other way round — only `closed` finishes — so every one of these,
        // including a custom status this app has never heard of, must block.
        for status in ["parked", "blocked", "deferred", "pinned", "someday"] {
            let mut waiting = issue("waiting", "open");
            waiting.dependencies = vec![Dependency {
                issue_id: "waiting".into(),
                depends_on_id: "blocker".into(),
                kind: "blocks".into(),
            }];
            let board = vec![waiting, issue("blocker", status)];
            assert!(
                snapshot(&board, &RunScope::Queue, Some(4)).ready.is_empty(),
                "a blocker in `{status}` is not finished, and its dependent must wait"
            );
        }
    }

    #[test]
    fn a_dependency_on_an_issue_not_on_the_board_is_satisfied() {
        // The alternative is a run stalled on a reference nobody can resolve,
        // with nothing anywhere to say why.
        let mut waiting = issue("waiting", "open");
        waiting.dependencies = vec![Dependency {
            issue_id: "waiting".into(),
            depends_on_id: "gone".into(),
            kind: "blocks".into(),
        }];
        assert_eq!(snapshot(&[waiting], &RunScope::Queue, Some(4)).ready, vec!["waiting"]);
    }

    #[test]
    fn a_chain_of_three_holds_at_every_link() {
        // a blocks b, b blocks c. While a is anything but closed, only a is
        // ready; closing a releases b and nothing further, because b itself is
        // still unfinished. Under the old three-status set the chain broke at
        // the first parked or blocked link and everything below came loose.
        let chain = |a_status: &str, b_status: &str| {
            let a = issue("a", a_status);
            let mut b = issue("b", b_status);
            b.dependencies = vec![Dependency {
                issue_id: "b".into(),
                depends_on_id: "a".into(),
                kind: "blocks".into(),
            }];
            let mut c = issue("c", "open");
            c.dependencies = vec![Dependency {
                issue_id: "c".into(),
                depends_on_id: "b".into(),
                kind: "blocks".into(),
            }];
            snapshot(&[a, b, c], &RunScope::Queue, Some(4)).ready
        };

        assert_eq!(chain("open", "open"), vec!["a"], "only the head of the chain is ready");
        assert!(chain("parked", "open").is_empty(), "a parked head releases nothing below it");
        assert!(chain("blocked", "blocked").is_empty(), "a blocked middle holds the tail");
        assert_eq!(chain("closed", "open"), vec!["b"], "closing the first releases only the second");
    }

    #[test]
    fn only_a_blocking_dependency_blocks() {
        // bd records parent-child, related and discovered-from in the same
        // list; reading any of them as "wait" would stall every child of an
        // open epic.
        let mut child = issue("child", "open");
        child.dependencies = vec![Dependency {
            issue_id: "child".into(),
            depends_on_id: "epic".into(),
            kind: "parent-child".into(),
        }];
        let board = vec![child, issue("epic", "open")];
        assert!(snapshot(&board, &RunScope::Queue, Some(4)).ready.contains(&"child".to_string()));
    }

    #[test]
    fn an_empty_board_ends_the_run() {
        assert_eq!(
            next_action(&snap(&[], &[]), None, 0, 20, LastBatch::Completed, false),
            Action::Stop(StopReason::QueueEmpty)
        );
    }

    #[test]
    fn unfinished_work_keeps_a_run_going_with_nothing_ready() {
        // A killed batch leaves in_progress and ready_to_merge behind, and
        // `bd ready` hides both. Stopping here would abandon them.
        assert_eq!(
            next_action(&snap(&[], &["orphan"]), None, 0, 20, LastBatch::Completed, false),
            Action::Run(RunReason::RecoverUnfinished)
        );
    }

    #[test]
    fn a_whole_batch_that_changed_neither_set_is_stuck() {
        let before = snap(&["a"], &["b"]);
        let after = snap(&["a"], &["b"]);
        assert_eq!(
            next_action(&after, Some(&before), 1, 20, LastBatch::Completed, false),
            Action::Stop(StopReason::NoProgress)
        );
    }

    #[test]
    fn an_unchanged_board_after_a_crash_is_retried_not_called_stuck() {
        // The batch never ran. Reading that as a stuck queue would end a run
        // over a transient failure of the harness.
        let before = snap(&["a"], &[]);
        assert_eq!(
            next_action(&snap(&["a"], &[]), Some(&before), 1, 20, LastBatch::Crashed, false),
            Action::Run(RunReason::RetryAfterCrash)
        );
    }

    #[test]
    fn an_unchanged_board_after_a_spent_allowance_is_not_called_stuck_either() {
        // The batch could not run, so of course it moved nothing. Reading that
        // as a stuck queue is the defect smetana-bvn is about: an overnight run
        // would end on `NoProgress` while the work was untouched and the only
        // thing missing was time.
        let before = snap(&["a"], &[]);
        assert_eq!(
            next_action(&snap(&["a"], &[]), Some(&before), 1, 20, LastBatch::Limited, false),
            Action::Run(RunReason::RetryAfterLimit),
            "and not RetryAfterCrash — nothing crashed, and the log must not say it did"
        );
    }

    #[test]
    fn a_spent_allowance_does_not_outrank_an_empty_board_or_the_cap() {
        // Both endings are about the run being over rather than about waiting,
        // so neither is worth pausing for: there would be nothing to come back
        // to when the limit cleared.
        assert_eq!(
            next_action(&snap(&[], &[]), None, 0, 20, LastBatch::Limited, false),
            Action::Stop(StopReason::QueueEmpty)
        );
        assert_eq!(
            next_action(&snap(&["a"], &[]), None, 20, 20, LastBatch::Limited, false),
            Action::Stop(StopReason::MaxIterations)
        );
    }

    #[test]
    fn moving_a_task_into_flight_counts_as_progress() {
        // Nothing closed and nothing parked, and the run must not stop: this is
        // exactly the pass the source's earlier closed-count check got wrong.
        let before = snap(&["a"], &[]);
        assert_eq!(
            next_action(&snap(&[], &["a"]), Some(&before), 1, 20, LastBatch::Completed, false),
            Action::Run(RunReason::RecoverUnfinished)
        );
    }

    #[test]
    fn the_same_work_in_a_different_order_is_not_progress() {
        let before = snap(&["a", "b"], &[]);
        assert_eq!(
            next_action(&snap(&["b", "a"], &[]), Some(&before), 1, 20, LastBatch::Completed, false),
            Action::Stop(StopReason::NoProgress)
        );
    }

    #[test]
    fn the_iteration_cap_is_a_backstop() {
        assert_eq!(
            next_action(&snap(&["a"], &[]), None, 20, 20, LastBatch::Completed, false),
            Action::Stop(StopReason::MaxIterations)
        );
    }

    #[test]
    fn an_empty_board_wins_over_the_cap() {
        // Finishing the work is not a churn stop, and reporting it as one would
        // send somebody looking for a problem that is not there.
        assert_eq!(
            next_action(&snap(&[], &[]), None, 99, 20, LastBatch::Completed, false),
            Action::Stop(StopReason::QueueEmpty)
        );
    }

    #[test]
    fn a_one_batch_run_stops_after_its_completed_batch_with_work_still_ready() {
        // More ready tasks than fit one batch: the batch took some, merged
        // them, and the rest still sit in Ready. `QueueEmpty` would be a lie
        // about them and `NoProgress` an accusation — the run did what it was
        // asked to do, and its ending says so.
        let before = snap(&["a", "b", "c"], &[]);
        assert_eq!(
            next_action(&snap(&["b", "c"], &[]), Some(&before), 1, 20, LastBatch::Completed, true),
            Action::Stop(StopReason::BatchDone)
        );
    }

    #[test]
    fn a_one_batch_run_still_starts_its_first_batch() {
        // `prev` is None on the first look: no batch has gone out yet, so the
        // one batch is still owed — including when the board already holds a
        // killed run's orphans, which the same first batch recovers rather
        // than a second one.
        assert_eq!(
            next_action(&snap(&["a"], &[]), None, 0, 20, LastBatch::Completed, true),
            Action::Run(RunReason::ReadyWork)
        );
        assert_eq!(
            next_action(&snap(&[], &["orphan"]), None, 0, 20, LastBatch::Completed, true),
            Action::Run(RunReason::RecoverUnfinished)
        );
    }

    #[test]
    fn a_crashed_or_limited_batch_is_not_the_one_batch() {
        // Neither got to do its work, so the retry each already has is still
        // right: the next attempt is the same first batch, not a second one.
        let before = snap(&["a"], &[]);
        assert_eq!(
            next_action(&snap(&["a"], &[]), Some(&before), 1, 20, LastBatch::Crashed, true),
            Action::Run(RunReason::RetryAfterCrash)
        );
        assert_eq!(
            next_action(&snap(&["a"], &[]), Some(&before), 1, 20, LastBatch::Limited, true),
            Action::Run(RunReason::RetryAfterLimit)
        );
    }

    #[test]
    fn a_one_batch_run_that_emptied_the_board_ends_as_queue_empty() {
        // Both endings are true here and the emptier one is the more useful:
        // nothing is left to take, which is what somebody reading the bar
        // wants to know before starting another run.
        let before = snap(&["a"], &[]);
        assert_eq!(
            next_action(&snap(&[], &[]), Some(&before), 1, 20, LastBatch::Completed, true),
            Action::Stop(StopReason::QueueEmpty)
        );
    }

    #[test]
    fn a_completed_batch_that_moved_nothing_is_stuck_even_in_a_one_batch_run() {
        // "The batch is done" would be a lie of its own: it ran to completion
        // and changed neither set, so nothing merged and nothing was even
        // parked. That is the stuck ending, whatever the mode.
        let before = snap(&["a"], &["b"]);
        assert_eq!(
            next_action(&snap(&["a"], &["b"]), Some(&before), 1, 20, LastBatch::Completed, true),
            Action::Stop(StopReason::NoProgress)
        );
    }
}
