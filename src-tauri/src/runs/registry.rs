//! What a run leaves on disk so that an unclean exit is recoverable, and the
//! rules for reading it — pure, no filesystem and no process table. The disk
//! and the kernel are `recovery.rs` and `procs.rs`; this file is where the
//! decisions live and where the tests are, the same split `gitignore.rs` makes
//! between `amend` and `ensure`.
//!
//! Everything a run knows otherwise lives in memory: `runs::service` keeps its
//! runs in a `HashMap` and writes none of it anywhere, and sessions are
//! deliberately kept out of `settings.json` because a session row with a dead
//! process behind it is worse than an empty list. The orderly ending is
//! `RunEvent::Exit`, which hangs every session up. A crash, a force quit, a
//! `kill -9` — and, in development, every Rust rebuild — reach none of that,
//! and what they strand is tasks claimed by a run that no longer exists and
//! agent processes nobody is going to signal.
//!
//! **The split between the two halves follows what each can see.** The app can
//! see the process table and the tracker cannot; `smetana:running-tasks`
//! Phase R already recovers claimed tasks correctly and has the worktrees in
//! front of it. So the app writes this file and deals with processes, and the
//! tracker half stays with Phase R. Nothing here ever writes to bd.
//!
//! **A record proves its own liveness, and an actor id alone cannot.**
//! `BEADS_ACTOR` is `smetana-run-<session-id>` and session ids restart at 1 on
//! every launch, so after a restart a fresh session takes a dead run's name. A
//! reader therefore decides from the evidence in the record rather than from
//! the name in it: `Proc` is a pid together with the stamp that survives pid
//! reuse. Nothing here reads a date to decide liveness — the one date in the
//! file says when the run started and is used for nothing but ageing a record
//! out.
//!
//! The file is `.smetana/runs.json` in the project folder, beside
//! `project.toml` and outside the repository (`gitignore.rs` keeps `.smetana/`
//! out of it). Phase R reads it with an ordinary file read, needing nothing
//! from the app and no path passed in through a prompt.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

/// The shape of the file. A file claiming anything higher was written by an app
/// newer than this one and is not ours to reason about.
pub const VERSION: u32 = 1;

/// How long a record whose writer is not alive is kept before it is aged out.
///
/// It is kept at all because Phase R is the reader that needs it: a claim whose
/// actor is named under a dead writer is one it may recover, and deleting the
/// record the moment the processes were dealt with would take that evidence
/// away and send Phase R back to its conservative default. A week is far longer
/// than any recovery window and short enough that a machine which crashes often
/// does not accumulate a registry.
pub const ABANDONED_DAYS: i64 = 7;

/// One process, named in a way that survives pid reuse.
///
/// `started` is an opaque stamp: equal means it is the same process, and
/// nothing else about the number is meaningful — the unit is whatever the
/// platform could answer cheaply (microseconds since the epoch on macOS, clock
/// ticks since the epoch on Linux). Nobody may read it as a date, and no rule
/// in this file does.
///
/// `command` is for the other reader. The app compares stamps because it can
/// ask the kernel; a skill with a shell cannot, and `ps -p <pid> -o comm=` is
/// what it has. Two guards for two readers rather than one guard that only
/// half of them can apply.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proc {
    pub pid: i32,
    pub started: u64,
    pub command: String,
}

/// One batch of a run: the name it works under in bd's audit trail, and the
/// process group it started.
///
/// The actor is recorded whether or not the group could be: Phase R matches on
/// the actor, and a platform that cannot answer for a process must not cost the
/// tracker half its evidence as well.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Batch {
    /// `smetana-run-<session-id>` — `terminal::model::run_actor`'s answer, and
    /// the string an issue's assignee carries.
    pub actor: String,
    pub group: Option<Proc>,
}

/// One run, as the file remembers it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    /// The run's own token. Tokens restart at 1 on every launch, so a token is
    /// only a name *within* one writer — which is why every lookup here takes
    /// the writer as well.
    pub token: u64,
    pub project: String,
    pub target_branch: String,
    /// When the run started, RFC 3339. Not evidence of anything being alive —
    /// see the module note — and read by exactly one rule, the ageing out of a
    /// record nobody will ever want again.
    pub started_at: String,
    /// The app process that wrote this record.
    pub writer: Proc,
    pub batches: Vec<Batch>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Registry {
    pub version: u32,
    pub runs: Vec<Record>,
}

impl Default for Registry {
    fn default() -> Self {
        Self { version: VERSION, runs: Vec::new() }
    }
}

/// What the process table says about one pid, and the whole of what the rules
/// here are allowed to know about it.
///
/// `Unknown` is not a rounding error to be folded into one of the other two: a
/// platform with no way to ask, or a pid held by another user, has to leave
/// every decision here exactly where it was before the module existed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seen {
    /// Nothing holds that pid.
    Gone,
    /// Something holds it, and this is when it started.
    Running { started: u64 },
    /// This platform cannot say.
    Unknown,
}

/// Whether a recorded process is still the process that was recorded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Liveness {
    Alive,
    Dead,
    Unknown,
}

/// The one rule everything else here is built from.
///
/// A stamp that differs is the pid-reuse case and reads as `Dead`, which is the
/// whole reason a bare pid would not do: pids are handed out again, and after a
/// restart the machine is busy handing them out.
pub fn liveness(recorded: &Proc, seen: Seen) -> Liveness {
    match seen {
        Seen::Gone => Liveness::Dead,
        Seen::Unknown => Liveness::Unknown,
        Seen::Running { started } if started == recorded.started => Liveness::Alive,
        Seen::Running { .. } => Liveness::Dead,
    }
}

/// May this group be signalled at all, whatever the process table says?
///
/// `killpg(0)` signals *our own* process group and `killpg(1)` aims at init's;
/// a pid at or below 1 in a record can only be damage or a platform that
/// answered nonsense, and either way it is not something the app started.
fn signalable(group: &Proc) -> bool {
    group.pid > 1
}

/// An agent process a dead run left running, and the name it was working
/// under. The actor rides along for the log line: a pid on its own says
/// nothing a person could line up against the board afterwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Orphan {
    pub actor: String,
    pub group: Proc,
}

/// What the app's start-up sweep should do about one project's file.
pub struct Sweep {
    /// The process groups to hang up — those belonging to a record whose writer
    /// is provably dead and which are themselves provably still the process
    /// that was recorded. Anything else is left alone: an unreadable answer, a
    /// pid that has been reused, a group under a writer that is alive or that
    /// this platform cannot ask about.
    pub signal: Vec<Orphan>,
    /// What the file should hold afterwards.
    pub next: Registry,
    /// Whether `next` says anything different from what was read. A sweep that
    /// changed nothing must not rewrite the file — another app instance may be
    /// working in it, and a write that says the same thing is a chance to lose
    /// somebody's record for nothing.
    pub changed: bool,
}

/// Finish the shutdown a dead app never got to: which groups to signal, and
/// what the file should hold when that is done.
///
/// The list of records is only ever shortened here, and only for records that
/// are both not alive and long past being useful to Phase R. A record whose
/// writer is alive — this app's own, or another instance's — comes through
/// untouched, which is what keeps a second start from truncating a live app's
/// work.
pub fn sweep(registry: &Registry, now: DateTime<Utc>, table: &impl Fn(i32) -> Seen) -> Sweep {
    let mut signal = Vec::new();
    let mut keep: Vec<Record> = Vec::new();
    for record in &registry.runs {
        let writer = liveness(&record.writer, table(record.writer.pid));
        if writer == Liveness::Dead {
            for batch in &record.batches {
                let Some(group) = batch.group.as_ref() else { continue };
                if signalable(group) && liveness(group, table(group.pid)) == Liveness::Alive {
                    signal.push(Orphan { actor: batch.actor.clone(), group: group.clone() });
                }
            }
        }
        if writer != Liveness::Alive && aged_out(record, now) {
            continue;
        }
        keep.push(record.clone());
    }
    let changed = keep != registry.runs || registry.version != VERSION;
    Sweep { signal, next: Registry { version: VERSION, runs: keep }, changed }
}

/// Is this record old enough that nobody will want its actors again?
///
/// A timestamp that cannot be read counts as aged out. It cannot be shown
/// fresh, and the alternative — keeping every undatable record for ever — is a
/// file that only grows.
fn aged_out(record: &Record, now: DateTime<Utc>) -> bool {
    match DateTime::parse_from_rfc3339(&record.started_at) {
        Ok(started) => now.signed_duration_since(started) > Duration::days(ABANDONED_DAYS),
        Err(_) => true,
    }
}

/// The actors a reader should treat as dead: every one named under a record
/// whose writer is provably gone.
///
/// This is Phase R's rule written in Rust. The reader that actually applies it
/// is a skill with a shell, which checks `ps -p <writer.pid>` rather than a
/// start-time stamp — so this is not the code that runs for it, and it is here
/// because a rule stated twice in prose and nowhere in a test is a rule that
/// drifts. An actor named under a live writer, an actor named under a writer
/// this platform cannot ask about, and an actor named nowhere at all are all
/// the same answer: not provably dead, so left alone.
pub fn abandoned_actors(registry: &Registry, table: &impl Fn(i32) -> Seen) -> Vec<String> {
    registry
        .runs
        .iter()
        .filter(|record| liveness(&record.writer, table(record.writer.pid)) == Liveness::Dead)
        .flat_map(|record| record.batches.iter().map(|batch| batch.actor.clone()))
        .collect()
}

/// Put a run in the file, replacing whatever stood under the same writer and
/// token.
///
/// Keyed by both, and that is not belt and braces: tokens restart at 1 on every
/// launch, so a second app instance working in the same project has a run
/// called 1 as well, and keying by the token alone would have each of them
/// writing over the other's record.
pub fn note_run(registry: &mut Registry, record: Record) {
    registry.runs.retain(|held| !same_run(held, &record.writer, record.token));
    registry.runs.push(record);
}

/// Add a batch to a run that is already in the file. Answers whether there was
/// one to add it to — there may not be, if the record was aged out or the file
/// was replaced under us, and inventing a record from a batch would leave one
/// with no target branch and no start time.
pub fn note_batch(registry: &mut Registry, writer: &Proc, token: u64, batch: Batch) -> bool {
    let Some(record) = registry.runs.iter_mut().find(|held| same_run(held, writer, token)) else {
        return false;
    };
    record.batches.retain(|held| held.actor != batch.actor);
    record.batches.push(batch);
    true
}

/// The run's loop task is gone — **however it went**, which is not the same as
/// "it finished". `Report::Ended` comes from a `Drop` guard, so a cancellation,
/// a crash, a failed preflight and a run that stopped on an unanswered question
/// all arrive here. Answers whether the file changed.
///
/// The record goes only when nothing it names is still running, and that
/// condition is the point rather than caution. `runs::service` ends a run with
/// `NeedsAnswer` **without killing the session** — the person is being sent to
/// that terminal to answer, and killing would take away the very thing they
/// were sent to. Deleting the record there would leave a live agent, still
/// claiming tasks under its actor, named nowhere: a `kill -9` a minute later
/// orphans exactly the process this file exists to reclaim, and the next start
/// has nothing to find it by.
///
/// A condition on the processes rather than on the stop reason, because the
/// reasons are a list somebody adds to: a future ending that leaves a session
/// alive is covered by this and would not be covered by a `match`. What is left
/// behind is one record with one batch in it, which the next start sweeps or
/// ages out like any other.
pub fn forget_run(
    registry: &mut Registry,
    writer: &Proc,
    token: u64,
    table: &impl Fn(i32) -> Seen,
) -> bool {
    let before = registry.runs.clone();
    let mut kept: Vec<Record> = Vec::with_capacity(before.len());
    for mut record in registry.runs.drain(..) {
        if !same_run(&record, writer, token) {
            kept.push(record);
            continue;
        }
        // What is worth keeping is the batches that are still running, and one
        // this platform cannot ask about counts as running: keeping a record
        // too long costs it ageing out in a week, dropping one too early costs
        // an orphan nobody can name. A batch whose group could not be read when
        // it was written down is dropped, because that only happens when the
        // session was already gone by the time it was asked about.
        record.batches.retain(|batch| {
            batch
                .group
                .as_ref()
                .is_some_and(|group| liveness(group, table(group.pid)) != Liveness::Dead)
        });
        if !record.batches.is_empty() {
            kept.push(record);
        }
    }
    registry.runs = kept;
    registry.runs != before
}

fn same_run(record: &Record, writer: &Proc, token: u64) -> bool {
    record.token == token && record.writer == *writer
}

/// The file's text as a `Registry`, or `None` when it is not one this app may
/// act on: damaged, or written by a newer app.
///
/// Both refusals are the same answer on purpose. A file we cannot read is a
/// file whose records we cannot show alive, and acting on records we cannot
/// read is the one thing this module must never do.
pub fn parse(text: &str) -> Option<Registry> {
    let registry: Registry = serde_json::from_str(text).ok()?;
    (registry.version <= VERSION).then_some(registry)
}

/// The text to write. Pretty-printed because both readers are people or agents
/// reading it in a terminal.
pub fn render(registry: &Registry) -> Result<String, serde_json::Error> {
    let mut text = serde_json::to_string_pretty(registry)?;
    text.push('\n');
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn stamp(pid: i32, started: u64) -> Proc {
        Proc { pid, started, command: "smetana".into() }
    }

    /// A process table a test can state outright: a pid answers what is put
    /// here for it, and anything not listed is gone.
    fn table(entries: &[(i32, Seen)]) -> impl Fn(i32) -> Seen + '_ {
        let map: HashMap<i32, Seen> = entries.iter().copied().collect();
        move |pid| map.get(&pid).copied().unwrap_or(Seen::Gone)
    }

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-08-11T12:00:00+00:00").expect("a fixed now").into()
    }

    fn ago(days: i64) -> String {
        (now() - Duration::days(days)).to_rfc3339()
    }

    fn record(token: u64, writer: Proc, groups: &[Option<Proc>]) -> Record {
        Record {
            token,
            project: "/p".into(),
            target_branch: "develop".into(),
            started_at: ago(0),
            writer,
            batches: groups
                .iter()
                .enumerate()
                .map(|(n, group)| Batch { actor: format!("smetana-run-{n}"), group: group.clone() })
                .collect(),
        }
    }

    fn registry(runs: Vec<Record>) -> Registry {
        Registry { version: VERSION, runs }
    }

    #[test]
    fn a_pid_that_now_belongs_to_something_else_reads_as_dead() {
        // The whole reason a bare pid is not enough. `BEADS_ACTOR` restarts at
        // `smetana-run-1` on every launch and so do the pids the machine hands
        // out, so a record naming pid 4213 says nothing until the stamp agrees.
        let recorded = stamp(4213, 1_000);
        assert_eq!(liveness(&recorded, Seen::Running { started: 1_000 }), Liveness::Alive);
        assert_eq!(liveness(&recorded, Seen::Running { started: 9_999 }), Liveness::Dead);
        assert_eq!(liveness(&recorded, Seen::Gone), Liveness::Dead);
        assert_eq!(liveness(&recorded, Seen::Unknown), Liveness::Unknown);
    }

    #[test]
    fn a_live_apps_record_is_left_exactly_as_it_was() {
        // The criterion a second start has to meet: it may not truncate a
        // record whose writer is still there, and it may not signal anything
        // that record named.
        let held = registry(vec![record(1, stamp(10, 1), &[Some(stamp(20, 2))])]);
        let swept = sweep(&held, now(), &table(&[(10, Seen::Running { started: 1 }), (20, Seen::Running { started: 2 })]));

        assert!(swept.signal.is_empty(), "a live app's agents are its own to signal");
        assert_eq!(swept.next.runs, held.runs);
        assert!(!swept.changed, "and the file is not rewritten at all");
    }

    #[test]
    fn only_the_groups_of_a_dead_writer_are_signalled() {
        let held = registry(vec![
            record(1, stamp(10, 1), &[Some(stamp(20, 2))]),
            record(1, stamp(11, 3), &[Some(stamp(21, 4)), Some(stamp(22, 5))]),
        ]);
        // Writer 10 is alive; writer 11 is gone, and both of its agents are
        // still running — exactly the state a `kill -9` leaves.
        let swept = sweep(
            &held,
            now(),
            &table(&[
                (10, Seen::Running { started: 1 }),
                (20, Seen::Running { started: 2 }),
                (21, Seen::Running { started: 4 }),
                (22, Seen::Running { started: 5 }),
            ]),
        );

        assert_eq!(
            swept.signal.iter().map(|orphan| orphan.group.pid).collect::<Vec<_>>(),
            vec![21, 22],
            "the live app's own agent is not among them"
        );
        assert_eq!(
            swept.signal[0].actor, "smetana-run-0",
            "and each one carries the name it was claiming under, for the log"
        );
    }

    #[test]
    fn a_group_this_app_cannot_show_is_ours_is_never_signalled() {
        // Three ways for the app to be unable to prove it started the process
        // under that pid, and all three end the same way. Anything the registry
        // does not name is not in this file at all and so cannot reach here.
        let held = registry(vec![record(
            1,
            stamp(10, 1),
            &[
                Some(stamp(20, 2)), // gone: nothing to signal
                Some(stamp(21, 3)), // the pid was reused by something else
                Some(stamp(22, 4)), // this platform cannot say
                None,               // the pid could not be read when it was written
            ],
        )]);
        let swept = sweep(
            &held,
            now(),
            &table(&[
                (10, Seen::Gone),
                (20, Seen::Gone),
                (21, Seen::Running { started: 999 }),
                (22, Seen::Unknown),
            ]),
        );

        assert!(swept.signal.is_empty());
    }

    #[test]
    fn a_group_that_could_name_our_own_process_group_is_never_signalled() {
        // `killpg(0)` is the app signalling itself and everything it started,
        // and `killpg(1)` aims at init. A record holding either can only be
        // damage, and the sweep runs before anything else in the app.
        for pid in [-1, 0, 1] {
            let held = registry(vec![record(1, stamp(10, 1), &[Some(stamp(pid, 2))])]);
            let swept =
                sweep(&held, now(), &table(&[(10, Seen::Gone), (pid, Seen::Running { started: 2 })]));
            assert!(swept.signal.is_empty(), "pid {pid} is not a group we started");
        }
    }

    #[test]
    fn a_writer_this_platform_cannot_ask_about_leaves_everything_alone() {
        // Windows has no way to answer, and a pid held by another user does not
        // answer either. Neither is a reason to signal anything or to forget a
        // record, which is where things were before this module existed.
        let held = registry(vec![record(1, stamp(10, 1), &[Some(stamp(20, 2))])]);
        let swept = sweep(&held, now(), &table(&[(10, Seen::Unknown), (20, Seen::Unknown)]));

        assert!(swept.signal.is_empty());
        assert_eq!(swept.next.runs, held.runs);
        assert!(!swept.changed);
    }

    #[test]
    fn an_abandoned_record_is_kept_for_phase_r_and_then_aged_out() {
        // It outlives its own processes on purpose: the tracker half of the
        // recovery is Phase R's, it runs whenever the next run does, and a
        // record deleted the moment the processes were dealt with would send it
        // back to leaving every claim in place.
        let mut fresh = record(1, stamp(10, 1), &[Some(stamp(20, 2))]);
        fresh.started_at = ago(1);
        let mut old = record(2, stamp(11, 3), &[Some(stamp(21, 4))]);
        old.started_at = ago(ABANDONED_DAYS + 1);
        let held = registry(vec![fresh.clone(), old]);

        let swept = sweep(&held, now(), &table(&[]));

        assert_eq!(swept.next.runs, vec![fresh], "yesterday's abandoned run is still evidence");
        assert!(swept.changed);
    }

    #[test]
    fn a_record_nobody_can_date_is_aged_out() {
        // It cannot be shown fresh, and keeping every undatable record for ever
        // is a file that only grows.
        let mut damaged = record(1, stamp(10, 1), &[]);
        damaged.started_at = "yesterday afternoon".into();
        let swept = sweep(&registry(vec![damaged]), now(), &table(&[]));

        assert!(swept.next.runs.is_empty());
        assert!(swept.changed);
    }

    #[test]
    fn what_a_reader_concludes_is_the_actors_of_a_dead_writer() {
        // Phase R's own rule: this actor's claim may be recovered, that one's
        // belongs to a run still going, and anything the file does not name at
        // all keeps the conservative default by not appearing here.
        let held = registry(vec![
            record(1, stamp(10, 1), &[Some(stamp(20, 2))]),
            record(1, stamp(11, 3), &[Some(stamp(21, 4))]),
            record(2, stamp(12, 5), &[None]),
        ]);
        let dead = abandoned_actors(
            &held,
            &table(&[(10, Seen::Running { started: 1 }), (12, Seen::Unknown)]),
        );

        assert_eq!(dead, vec!["smetana-run-0".to_string()], "only the run whose app is gone");
    }

    #[test]
    fn a_run_is_remembered_while_it_lives_and_forgotten_when_it_ends_cleanly() {
        let writer = stamp(10, 1);
        let mut held = Registry::default();

        note_run(&mut held, record(1, writer.clone(), &[]));
        assert_eq!(held.runs.len(), 1);
        assert!(note_batch(
            &mut held,
            &writer,
            1,
            Batch { actor: "smetana-run-4".into(), group: Some(stamp(20, 2)) }
        ));
        assert_eq!(held.runs[0].batches[0].actor, "smetana-run-4");

        assert!(forget_run(&mut held, &writer, 1, &table(&[])));
        assert!(held.runs.is_empty(), "an ending with nothing left running takes the record");
        assert!(!forget_run(&mut held, &writer, 1, &table(&[])), "and the second finds nothing");
    }

    #[test]
    fn a_run_that_ended_with_a_session_still_at_its_prompt_keeps_its_record() {
        // The ending `runs::service` reaches on an unanswered question: the
        // session is deliberately left alive, because the person is being sent
        // to that terminal to answer it. Forgetting the record there would
        // leave a live agent — still claiming under its actor — named nowhere,
        // and a `kill -9` a minute later would orphan the one process this
        // whole file exists to reclaim.
        let writer = stamp(10, 1);
        let mut held = Registry::default();
        note_run(&mut held, record(1, writer.clone(), &[]));
        note_batch(
            &mut held,
            &writer,
            1,
            Batch { actor: "smetana-run-4".into(), group: Some(stamp(20, 2)) },
        );
        note_batch(
            &mut held,
            &writer,
            1,
            Batch { actor: "smetana-run-9".into(), group: Some(stamp(21, 3)) },
        );

        // The earlier batch's agent is long gone; the one at the prompt is not.
        let ended = forget_run(&mut held, &writer, 1, &table(&[(21, Seen::Running { started: 3 })]));

        assert!(ended, "the file changed");
        assert_eq!(held.runs.len(), 1, "the record stays while it names something alive");
        assert_eq!(
            held.runs[0].batches.iter().map(|batch| batch.actor.as_str()).collect::<Vec<_>>(),
            vec!["smetana-run-9"],
            "and it is trimmed to what is actually still running"
        );
    }

    #[test]
    fn an_ending_leaves_every_other_run_alone() {
        let ours = stamp(10, 1);
        let mut held = Registry::default();
        note_run(&mut held, record(1, ours.clone(), &[Some(stamp(20, 2))]));
        note_run(&mut held, record(2, ours.clone(), &[Some(stamp(21, 3))]));

        forget_run(&mut held, &ours, 1, &table(&[(21, Seen::Running { started: 3 })]));

        assert_eq!(held.runs.len(), 1);
        assert_eq!(held.runs[0].token, 2, "the other run's live batch is not this ending's to read");
    }

    #[test]
    fn another_apps_run_under_the_same_token_is_a_different_run() {
        // Tokens restart at 1 on every launch, so two app instances in one
        // project both have a run called 1. Keyed by the token alone, each
        // would write over the other's record and a stop in one would delete
        // the other's evidence.
        let ours = stamp(10, 1);
        let theirs = stamp(11, 2);
        let mut held = Registry::default();
        note_run(&mut held, record(1, ours.clone(), &[]));
        note_run(&mut held, record(1, theirs.clone(), &[]));
        assert_eq!(held.runs.len(), 2);

        assert!(!note_batch(
            &mut held,
            &stamp(12, 3),
            1,
            Batch { actor: "smetana-run-1".into(), group: None }
        ), "a batch belongs to the run of the app that started it");
        assert!(forget_run(&mut held, &ours, 1, &table(&[])));
        assert_eq!(held.runs.len(), 1);
        assert_eq!(held.runs[0].writer, theirs, "the other app's record is untouched");
    }

    #[test]
    fn a_batch_reported_twice_is_recorded_once() {
        // The actor is the name that matters and it is one per session, so a
        // repeated report replaces rather than stacks.
        let writer = stamp(10, 1);
        let mut held = Registry::default();
        note_run(&mut held, record(1, writer.clone(), &[]));
        for started in [2, 3] {
            note_batch(
                &mut held,
                &writer,
                1,
                Batch { actor: "smetana-run-4".into(), group: Some(stamp(20, started)) },
            );
        }
        assert_eq!(held.runs[0].batches.len(), 1);
        assert_eq!(held.runs[0].batches[0].group.as_ref().expect("a group").started, 3);
    }

    #[test]
    fn the_shape_the_other_reader_matches_on_is_pinned() {
        // `smetana:running-tasks` Phase R reads these names out of the file
        // with a shell, so a rename here is a silent failure over there: the
        // skill finds nothing, concludes nothing is provably dead, and quietly
        // goes back to leaving every claim in place.
        let held = registry(vec![record(3, stamp(4213, 17), &[Some(stamp(4890, 19))])]);
        let text = render(&held).expect("renders");

        for key in [
            "\"version\"",
            "\"runs\"",
            "\"token\"",
            "\"project\"",
            "\"targetBranch\"",
            "\"startedAt\"",
            "\"writer\"",
            "\"batches\"",
            "\"actor\"",
            "\"group\"",
            "\"pid\"",
            "\"started\"",
            "\"command\"",
        ] {
            assert!(text.contains(key), "{key} is what the skill looks for");
        }
        assert_eq!(parse(&text).expect("reads back"), held);
    }

    #[test]
    fn a_file_this_app_cannot_read_is_not_acted_on() {
        assert!(parse("{not json").is_none());
        assert!(parse(r#"{"version":99,"runs":[]}"#).is_none(), "a newer app wrote it");
        assert_eq!(parse(r#"{"version":1,"runs":[]}"#), Some(Registry::default()));
    }
}
