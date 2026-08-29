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
use crate::tracker::model::{Issue, IssuePatch};

/// bd's own status for work that has been claimed.
const IN_PROGRESS: &str = "in_progress";
/// Our custom status for work that is reviewed and not yet merged.
const READY_TO_MERGE: &str = "ready_to_merge";
/// Our custom status for a dead end left for a person. `pub` because parking is
/// also something the run itself does to a stuck batch's claims — see
/// `service::park_claims` — and a second copy of the string would drift.
pub const PARKED: &str = "parked";
/// bd's own status for work that is available. `pub` because giving a dead
/// batch's unfinished work back is putting it in exactly this status — see
/// `release` — and a second copy of the string would drift.
pub const OPEN: &str = "open";
/// bd's own status for work that is finished. `pub(super)` because
/// `summary.rs` diffs the board against this very word, and a second copy of
/// the string would drift.
pub(super) const CLOSED: &str = "closed";
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
pub(super) const LOCK_LABEL: &str = "smetana-lock";

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
    /// The batch before this one ended having done nothing at all, and the run
    /// has not yet seen enough of those in a row to give up.
    RetryAfterEmpty,
    /// The batch before this one stopped on a question and its claims were
    /// parked; this batch takes what is left.
    AfterQuestion,
}

/// How the batch before this one ended, as far as the decision cares.
///
/// More answers than "did it crash", and the reason is the same one every time
/// the type has grown: a batch stopped by a spent subscription allowance
/// changed the board no more than a crashed one did, and reading either as a
/// stuck queue would end a run over something that is not stuck. But they are
/// not the same event and must not be reported as one — a harness that keeps
/// falling over needs somebody to look at it, while an exhausted allowance
/// needs nothing at all except time.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LastBatch {
    /// It ran to completion.
    Completed,
    /// Its session exited non-zero — in practice a transient failure of the
    /// harness.
    Crashed,
    /// It could not run, or could not finish: the allowance was spent.
    Limited,
    /// Its session stopped on a question nobody was there to answer, so the
    /// run killed it and parked its claims (smetana-8pe). Like `Crashed` and
    /// `Limited` it must not read as a stuck queue — the batch never got to
    /// finish its work — but it is neither of them: nothing fell over and
    /// nothing needs waiting out, so it is retried at once and reported as
    /// itself. The spin a repeating question could cause is not this type's to
    /// stop: `RepeatedQuestion` in `model.rs` ends the run on the second
    /// identical question before the decision here is ever asked.
    Asked,
    /// It ended having done nothing at all: no account of itself, and a board
    /// exactly where it stood when the batch started — `did_nothing` is the
    /// rule, and it is what a batch that died on its first request to the API
    /// looks like from here.
    ///
    /// Its own answer rather than `Completed`, and that is the whole of
    /// smetana-0t4: the harness exits with zero both when it did the work and
    /// when it fell over before starting, so a clean exit alone said nothing
    /// about whether a batch had happened, and a night of eight-minute batches
    /// read as a run doing its job. Like `Crashed` it must not read as a stuck
    /// queue — the board is unmoved because nothing ran, not because the work
    /// is unfinishable — and, like a crash, it is the run's own count of them
    /// in a row that ends the night, in `service.rs` beside the crash count.
    Empty,
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

/// Coordination, not work: the merge lock never enters the snapshot's counts —
/// free (`open`) it is not ready work, held (`in_progress`) it is not a killed
/// batch's orphan to recover. It deliberately stays in the `not_finished`
/// blocking set above: nothing should ever depend on the lock — it never
/// closes — and if something does by mistake, holding the dependent back fails
/// closed, where releasing it would take work whose premise was a wiring
/// error.
///
/// `pub(super)` because `summary.rs` has to make the same exclusion — a lock is
/// coordination there too, and it must never appear in a report — and sharing
/// the predicate is what stops the two from drifting apart.
pub(super) fn is_lock(issue: &Issue) -> bool {
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

/// What a run's batch has claimed and not finished: `in_progress` under the
/// batch's own bd actor. Exact because of smetana-4fh — every claim a run's
/// session makes carries `BEADS_ACTOR=smetana-run-<session-id>`, and
/// `bd update --claim` writes that actor into **`assignee`** — so this is
/// `bd list --status in_progress -a <actor>` read off the snapshot.
///
/// `owner` is a different field holding a different person and a claim never
/// touches it (smetana-a5b): this filter read `owner` for a while, matched
/// nothing on every board there has ever been, and so silently parked nothing at
/// all — a stuck batch left its tasks `in_progress` overnight with no note
/// saying why.
///
/// Deliberately not `ready_to_merge`, although the batch also holds those: a
/// reviewed task waiting for its merge is finished work, and parking it would
/// throw the review away — the recovery phase's `unfinished` set is what picks
/// it up. Parking is only for work the stuck lead never got to settle.
pub fn claimed_by(issues: &[Issue], actor: &str) -> Vec<String> {
    issues
        .iter()
        .filter(|i| i.status == IN_PROGRESS && i.assignee.as_deref() == Some(actor))
        .map(|i| i.id.clone())
        .collect()
}

/// One thing a batch's actor was still holding on the board when the batch
/// ended. Read only, and named in the run's document — the app writes to the
/// tracker nowhere as part of recovery (`recovery.rs`), so this is evidence for
/// a person and never a step taken on their behalf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Leftover {
    pub id: String,
    /// The board's own word: `in_progress` or `ready_to_merge`. Carried rather
    /// than reduced to a flag, because the two mean opposite things to whoever
    /// picks the work up — one is unfinished, the other is finished and unmerged.
    pub status: String,
    /// The merge lock, which is coordination rather than work and is excluded
    /// from every count in this file for that reason. It is exactly the thing
    /// worth naming here, though: a lock left claimed by a dead actor stops the
    /// *next* run rather than this one, and until smetana-pmj nothing said so.
    pub lock: bool,
}

/// What a batch's actor still holds: `in_progress` and `ready_to_merge` under
/// its own bd actor, the merge lock among them.
///
/// Wider than `claimed_by` in both directions it is wider in, and each is
/// deliberate. `ready_to_merge` is here because this is a record rather than a
/// parking list — reviewed work nobody merged is precisely what a killed batch
/// strands — and the lock is here because it is the one leftover that costs
/// somebody else their night.
///
/// Nothing is filtered by scope: an actor's claim is the actor's claim, and a
/// task that fell outside this run's scope while it was being held is a stranger
/// finding to hide.
pub fn left_behind(issues: &[Issue], actor: &str) -> Vec<Leftover> {
    issues
        .iter()
        .filter(|i| i.assignee.as_deref() == Some(actor))
        .filter(|i| matches!(i.status.as_str(), IN_PROGRESS | READY_TO_MERGE))
        .map(|i| Leftover { id: i.id.clone(), status: i.status.clone(), lock: is_lock(i) })
        .collect()
}

/// The note a parked task carries, in the wording the `running-tasks` skill
/// already uses for the lead's own parking (`parked: <one concrete line>`), so
/// a person scanning notes reads one vocabulary whoever did the parking. The
/// question is the whole of what the run knows: a lead stuck at a harness
/// dialog has not told anybody which of its tasks it was thinking about.
pub fn parking_note(question: &str) -> String {
    format!("parked: {question}")
}

/// How one thing a batch left claimed is given back, as a patch for the
/// tracker — or `None` for the one leftover that is never given back.
///
/// Three rules, and each of them is a way of being wrong that costs a night:
///
/// - **`in_progress` goes back to `open`, unclaimed.** Returned rather than
///   parked: parking is for a task carrying a question to a person, and this
///   one carries none — it is simply not finished — so parking it would hide it
///   from every run that comes after (smetana-0t4).
/// - **`ready_to_merge` keeps its status and loses only the claim.** That is
///   reviewed work waiting to be merged, and putting it back in `open` would
///   throw the review away and have it done again.
/// - **The merge lock is never touched.** Releasing it behind a batch that is
///   still alive is releasing it in the middle of somebody's merge, which is
///   the half-merged target branch the lock exists to prevent.
///
/// A status this rule has never seen is left alone too: `left_behind` produces
/// only the two above, and guessing at a third is not this function's to do.
/// The note is an ordinary one — deliberately not the `parked:` or `resolved:`
/// the `running-tasks` skill writes, since neither happened here — and it names
/// the batch, its actor and, where the app could find them, the branch the work
/// was left on and the commit at its tip.
pub fn release(
    left: &Leftover,
    batch: u32,
    actor: &str,
    work: Option<(&str, &str)>,
) -> Option<IssuePatch> {
    if left.lock {
        return None;
    }
    let (status, said) = match left.status.as_str() {
        IN_PROGRESS => (
            Some(OPEN.to_string()),
            format!(
                "batch {batch} ({actor}) ended without finishing this; \
                 it is open again and unclaimed"
            ),
        ),
        READY_TO_MERGE => (
            None,
            format!(
                "batch {batch} ({actor}) ended before this was merged; \
                 the claim is released and the review stands"
            ),
        ),
        _ => return None,
    };
    let found = match work {
        Some((branch, commit)) => format!(", work so far on {branch} at {commit}"),
        None => String::new(),
    };
    Some(IssuePatch {
        status,
        // An empty assignee is how bd clears the field — checked against the
        // binary rather than assumed, since `bd update -a ""` could as easily
        // have been a no-op or an error.
        assignee: Some(String::new()),
        append_notes: Some(format!("{said}{found}")),
        ..Default::default()
    })
}

/// Did this batch do anything at all?
///
/// The cheap signal smetana-0t4 asked for, and it is made of facts the loop is
/// already holding rather than anything new: the batch left no account of
/// itself, and the board is where it was when the batch started. A batch that
/// died on its first request to the API looks exactly like this, and until this
/// rule existed nothing could tell it apart from a batch that had nothing to do
/// — `StopReason::NoProgress` costs a whole extra iteration and needs the board
/// unmoved *twice* running, which a batch that got as far as claiming one task
/// defeats.
///
/// `reported` is the account file having **parsed**, which is `read_batch`'s own
/// answer and never a file that merely exists — an agent's write is not atomic.
/// A batch that said anything at all about itself is not empty whatever the
/// board says: it may honestly have found there was nothing to take, and that
/// is a batch reporting rather than a batch dying.
///
/// "The board moved" is stricter here than progress is in `next_action`, and
/// deliberately so: this rule ends runs, so every reading under which the batch
/// did something has to count. Hence the closed and parked counts as well as the
/// two sets — a task closed out of `parked`, or parked from a status this run
/// never looks at, moves neither set and is plainly not nothing.
pub fn did_nothing(reported: bool, before: &QueueSnapshot, after: &QueueSnapshot) -> bool {
    !reported && same_board(before, after)
}

/// The board twice over, as far as "did anything happen at all" cares.
/// Order-independent for the reason `same_set` is.
fn same_board(a: &QueueSnapshot, b: &QueueSnapshot) -> bool {
    same_set(&a.ready, &b.ready)
        && same_set(&a.unfinished, &b.unfinished)
        && a.closed == b.closed
        && a.parked == b.parked
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
///   stop.** An unchanged board after a crash, after an allowance ran out,
///   after a session was killed at a question it stopped on, or after a batch
///   that did nothing at all, means the batch never got to move anything — not
///   that the board is stuck.
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
        LastBatch::Asked => return Action::Run(RunReason::AfterQuestion),
        // An empty batch never got to do its work either, so it is retried the
        // way a crash is, and the count of them in a row is what ends the run
        // — `service.rs`, beside the crash count. A one-batch run never arrives
        // here with one, because that count ends it at the first.
        LastBatch::Empty => return Action::Run(RunReason::RetryAfterEmpty),
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
    fn a_blocks_dependency_on_the_lock_keeps_its_dependent_waiting() {
        // Nothing should ever depend on the lock — it never closes — so a
        // dependency on it is a wiring error, and it fails closed: the lock is
        // filtered out of the snapshot's counts, not out of the blocking set.
        // Moving the filter up into the `not_finished` collection has to be
        // done on purpose, against this test.
        let mut waiting = issue("waiting", "open");
        waiting.dependencies = vec![Dependency {
            issue_id: "waiting".into(),
            depends_on_id: "lock".into(),
            kind: "blocks".into(),
        }];
        let mut lock = issue("lock", "open");
        lock.labels = vec!["smetana-lock".into()];
        assert!(snapshot(&[waiting, lock], &RunScope::Queue, Some(4)).ready.is_empty());
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
    fn an_unchanged_board_after_a_parked_batch_is_taken_again_not_called_stuck() {
        // The session was killed at its question before it could move
        // anything, so an unchanged board says nothing about the queue. The
        // spin a repeating question could cause is ended by `RepeatedQuestion`
        // in the loop, not by this decision.
        let before = snap(&["a"], &[]);
        assert_eq!(
            next_action(&snap(&["a"], &[]), Some(&before), 1, 20, LastBatch::Asked, false),
            Action::Run(RunReason::AfterQuestion)
        );
    }

    #[test]
    fn a_parked_batch_does_not_outrank_an_empty_board_or_the_cap() {
        // The same rule Limited keeps: both endings say the run is over, and
        // there is nothing for another batch to do about either.
        assert_eq!(
            next_action(&snap(&[], &[]), None, 1, 20, LastBatch::Asked, false),
            Action::Stop(StopReason::QueueEmpty)
        );
        assert_eq!(
            next_action(&snap(&["a"], &[]), None, 20, 20, LastBatch::Asked, false),
            Action::Stop(StopReason::MaxIterations)
        );
    }

    #[test]
    fn only_the_stuck_leads_own_claims_are_its_batchs_to_park() {
        // `bd list --status in_progress -a <actor>` read off the snapshot:
        // in_progress under this run's actor and nothing else. Another
        // session's claims are another batch's business, a person's claim is
        // nobody's to park, and ready_to_merge is finished work whose review
        // parking would throw away.
        //
        // Every claimed fixture carries the actor in `assignee` and a *different*
        // value in `owner`, which is what bd actually emits after a `--claim`
        // (smetana-a5b). That is deliberately the shape a filter on `owner`
        // cannot pass, and `claimed_by_refuses_the_actor_in_owner_alone` below
        // pins the other direction.
        let actor = "smetana-run-42";
        let owner = "merazent@gmail.com";
        let mut mine = issue("mine", "in_progress");
        mine.assignee = Some(actor.into());
        mine.owner = Some(owner.into());
        let mut merged = issue("merged", "ready_to_merge");
        merged.assignee = Some(actor.into());
        merged.owner = Some(owner.into());
        let mut theirs = issue("theirs", "in_progress");
        theirs.assignee = Some("smetana-run-43".into());
        theirs.owner = Some(owner.into());
        let mut hand = issue("hand", "in_progress");
        hand.assignee = Some("flexo".into());
        hand.owner = Some(owner.into());
        let unclaimed = issue("unclaimed", "in_progress");
        let mut open = issue("open", "open");
        open.assignee = Some(actor.into());
        open.owner = Some(owner.into());

        let board = vec![mine, merged, theirs, hand, unclaimed, open];
        assert_eq!(claimed_by(&board, actor), vec!["mine"]);
    }

    #[test]
    fn claimed_by_refuses_the_actor_in_owner_alone() {
        // smetana-a5b, from the other side: an issue whose `owner` happens to be
        // the run's actor was never claimed by it — a claim writes `assignee`.
        // While this filter read `owner`, it matched nothing on any real board
        // and `park_claims` therefore parked nothing; a fixture setting only
        // `owner` must never make it pass again.
        let actor = "smetana-run-42";
        let mut owner_only = issue("owner-only", "in_progress");
        owner_only.owner = Some(actor.into());

        assert!(claimed_by(&[owner_only], actor).is_empty());
    }

    #[test]
    fn what_a_batch_left_holding_is_wider_than_what_it_would_have_parked() {
        // The record the report draws, and it is deliberately not the parking
        // list: `ready_to_merge` is reviewed work nobody merged, which parking
        // refuses to touch and a person reading a report has to know about, and
        // the merge lock is the leftover that costs the *next* run its night
        // rather than this one — the whole of smetana-pmj.
        let actor = "smetana-run-6";
        let owner = "merazent@gmail.com";
        let mut mine = issue("mine", "in_progress");
        mine.assignee = Some(actor.into());
        mine.owner = Some(owner.into());
        let mut merged = issue("merged", "ready_to_merge");
        merged.assignee = Some(actor.into());
        let mut lock = issue("lock", "in_progress");
        lock.assignee = Some(actor.into());
        lock.labels = vec![LOCK_LABEL.into()];
        let mut theirs = issue("theirs", "in_progress");
        theirs.assignee = Some("smetana-run-7".into());
        let mut done = issue("done", "closed");
        done.assignee = Some(actor.into());
        let mut parked = issue("parked", "parked");
        parked.assignee = Some(actor.into());

        let board = vec![mine, merged, lock, theirs, done, parked];
        assert_eq!(
            left_behind(&board, actor),
            vec![
                Leftover { id: "mine".into(), status: "in_progress".into(), lock: false },
                Leftover { id: "merged".into(), status: "ready_to_merge".into(), lock: false },
                Leftover { id: "lock".into(), status: "in_progress".into(), lock: true },
            ]
        );
    }

    #[test]
    fn a_batch_that_left_the_board_clean_left_nothing_to_name() {
        // The ordinary ending, and the one that must stay silent: a line saying
        // an actor held nothing would be printed under every batch in every
        // report, and a line that is always there is a line nobody reads.
        let actor = "smetana-run-6";
        let mut closed = issue("closed", "closed");
        closed.assignee = Some(actor.into());
        assert!(left_behind(&[closed, issue("open", "open")], actor).is_empty());
    }

    #[test]
    fn the_parking_note_speaks_the_skills_own_vocabulary() {
        // `running-tasks` writes `parked: <one concrete line>`; the run's own
        // parking has to read as the same act to the person scanning notes.
        assert_eq!(
            parking_note("Do you trust the contents of this directory?"),
            "parked: Do you trust the contents of this directory?"
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
    fn a_batch_that_left_no_account_and_an_unmoved_board_did_nothing() {
        // The night smetana-0t4 was filed about: the session exited with zero
        // having died on its first request to the API, so nothing was written
        // and nothing moved. Nothing else in the run could see that.
        let before = snap(&["a", "b"], &["c"]);
        let after = snap(&["b", "a"], &["c"]);
        assert!(did_nothing(false, &before, &after), "order is not movement either");
    }

    #[test]
    fn a_batch_that_left_an_account_is_never_empty() {
        // A batch that said something about itself may honestly have found
        // nothing to take, and that is a batch reporting rather than a batch
        // dying. The board is identical here and it still does not count.
        let before = snap(&["a"], &[]);
        assert!(!did_nothing(true, &before, &snap(&["a"], &[])));
    }

    #[test]
    fn a_batch_that_moved_the_board_is_never_empty() {
        let before = snap(&["a", "b"], &[]);
        // Taken: `a` left the ready set for the unfinished one.
        assert!(!did_nothing(false, &before, &snap(&["b"], &["a"])));
        // Recovered: an orphan is gone from the unfinished set.
        let held = snap(&["a"], &["orphan"]);
        assert!(!did_nothing(false, &held, &snap(&["a"], &[])));
        // And the counts move where neither set does — a task closed out of
        // `parked` touches nothing this run takes work from, and is plainly
        // not nothing. Stricter than `next_action`'s idea of progress, on
        // purpose: this rule is what ends the run.
        let mut closed_one = before.clone();
        closed_one.closed += 1;
        assert!(!did_nothing(false, &before, &closed_one));
    }

    #[test]
    fn an_empty_batch_is_retried_and_does_not_read_as_a_stuck_queue() {
        // The board is unmoved, which after a *completed* batch is the stuck
        // ending — but this batch never ran, so `NoProgress` would send
        // somebody to the board when the agent is what needs looking at. The
        // count of them in a row is `service.rs`'s.
        let before = snap(&["a"], &[]);
        assert_eq!(
            next_action(&snap(&["a"], &[]), Some(&before), 1, 20, LastBatch::Empty, false),
            Action::Run(RunReason::RetryAfterEmpty)
        );
    }

    #[test]
    fn unfinished_work_goes_back_to_open_with_its_claim_dropped() {
        let left = Leftover {
            id: "smetana-08f".into(),
            status: "in_progress".into(),
            lock: false,
        };
        let patch = release(&left, 2, "smetana-run-10", None).expect("work is given back");
        assert_eq!(patch.status.as_deref(), Some("open"));
        assert_eq!(patch.assignee.as_deref(), Some(""), "bd clears the field on an empty assignee");
        let note = patch.append_notes.expect("a note saying what happened");
        assert!(note.contains("batch 2"), "{note}");
        assert!(note.contains("smetana-run-10"), "{note}");
        // Not the lead's own vocabulary: nothing was parked and no question was
        // answered, and a `parked:` line puts an answer in the trail that
        // nobody gave.
        assert!(!note.starts_with("parked:") && !note.starts_with("resolved:"), "{note}");
        assert!(!note.contains('\n'), "one line: {note}");
    }

    #[test]
    fn reviewed_work_keeps_its_status_and_loses_only_the_claim() {
        // `ready_to_merge` is work somebody already reviewed. Putting it back
        // in `open` would throw that away and have it done a second time.
        let left = Leftover {
            id: "smetana-2ya".into(),
            status: "ready_to_merge".into(),
            lock: false,
        };
        let patch = release(&left, 3, "smetana-run-10", None).expect("the claim is released");
        assert_eq!(patch.status, None);
        assert_eq!(patch.assignee.as_deref(), Some(""));
    }

    #[test]
    fn the_merge_lock_is_never_given_back() {
        // Releasing it behind a batch that is still alive is releasing it in
        // the middle of somebody's merge — the half-merged target branch the
        // lock exists to prevent. It is refused in whichever status it is held.
        for status in ["in_progress", "ready_to_merge"] {
            let lock = Leftover { id: "smetana-uox".into(), status: status.into(), lock: true };
            assert!(release(&lock, 4, "smetana-run-10", None).is_none(), "in `{status}`");
        }
    }

    #[test]
    fn a_status_this_rule_has_never_seen_is_left_alone() {
        // `left_behind` produces exactly two, and guessing at a third is not
        // this function's to do.
        let odd = Leftover { id: "x".into(), status: "hooked".into(), lock: false };
        assert!(release(&odd, 1, "smetana-run-10", None).is_none());
    }

    #[test]
    fn the_note_names_the_branch_and_the_commit_when_the_app_found_them() {
        // What somebody reading the note the next morning actually needs: work
        // that was committed and never merged is on a branch, and the note is
        // the only place the board will ever say so.
        let left = Leftover { id: "smetana-08f".into(), status: "in_progress".into(), lock: false };
        let patch = release(&left, 2, "smetana-run-10", Some(("fix/smetana-08f-a-task", "d3a4309")))
            .expect("work is given back");
        let note = patch.append_notes.expect("a note");
        assert!(note.contains("fix/smetana-08f-a-task"), "{note}");
        assert!(note.contains("d3a4309"), "{note}");
        assert!(!note.contains('\n'), "one line: {note}");
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
