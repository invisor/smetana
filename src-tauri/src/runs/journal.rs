//! What the run decided, written down while it is deciding it.
//!
//! A run used to leave nothing at all about its own mechanics. The document
//! `report.rs` writes is about the *work* — which tasks moved, what each batch
//! said about itself — and `.smetana/runs.json` is a registry of the live,
//! rewritten in place and holding no history by definition. So the night of 29
//! August, six batches in two hours of which four did nothing, could not be
//! read back off the disk a day later: whether those four were counted as
//! `LastBatch::Completed` or as `Crashed`, and which `StopReason` finally
//! ended the run, were both consistent with everything that survived, and
//! nothing could tell them apart. This module is the answer to exactly that
//! question, and its line list is closed at the nine things that could not be
//! answered then.
//!
//! **Two destinations, one text.** Every line goes to the app's own log with a
//! `runs:` prefix — so somebody who has only that file open sees the whole of a
//! run — and to a file of the run's own, `.smetana/runs/<token>/journal-<start
//! time>.log`. Neither alone does the job: the app log splices two nights and
//! every other subsystem into one file and gives the report nothing to name,
//! while a file alone is invisible to anybody debugging the app as a whole. One
//! `say` writes both, which is what stops the two drifting into two texts.
//!
//! The name carries the run's **start** time because the directory does not
//! carry the run: `token` counts from zero on every app start, so
//! `.smetana/runs/1/` is reused by a run two launches later, and a fixed name
//! would have one night writing over another's account.
//!
//! **It is a write-through and not a buffer.** The file is opened for appending
//! when the run starts and every line is written and flushed as it happens,
//! because the run this exists for is the one that died: a journal assembled at
//! the end is empty in precisely the case somebody goes looking for it. The
//! cost is a write and a flush per event, which is a few dozen per run.
//!
//! **A journal that cannot be opened never stops a run.** The file is a record
//! of the work and not the work, so a full disk, a read-only project folder or
//! a name already taken by a directory all degrade to the `log::info!` half
//! alone — the same choice `lib.rs` makes about the app log itself, where the
//! thing that failed is the thing that would have reported the failure.
//!
//! Nothing here decides anything. Every function is either a `String` a caller
//! puts on the record or the act of putting it there, and the loop reads none
//! of it back. Journals are ordinary text and nothing cleans them up; that is
//! said out loud so nobody goes looking for the cleaner.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Local};

use super::config::HealthCheck;
use super::model::{Run, StopReason};
use super::preflight::{self, Ran};
use super::queue::{Action, LastBatch, Leftover, QueueSnapshot};
use super::registry::Proc;
use super::service::{Batch, Probe};
use super::usage::{Decision, Usage};

/// The run's own record, open for as long as the loop task lives.
///
/// Clonable and shared rather than owned in one place: the preflight runs its
/// declared commands on a blocking thread, and each of those commands is a line
/// in the list below, so a handle has to be able to cross into that thread. The
/// mutex is held for one `writeln!` and never across an await.
///
/// `None` inside is a journal that could not be opened — see the header. The
/// type stays the same either way, so no call site has an opinion about it.
#[derive(Clone)]
pub struct Journal {
    token: u64,
    file: Option<Arc<Mutex<std::fs::File>>>,
    path: Option<String>,
}

impl Journal {
    /// Open this run's journal in `dir` — `.smetana/runs/<token>/`, which the
    /// caller has already made. `started` is the run's own start, and it names
    /// the file rather than stamping it: two runs share the directory over the
    /// life of a project, and only the moment tells them apart.
    ///
    /// Append rather than truncate, for the one case the name does not cover: a
    /// clock that went backwards, or a second run starting inside the same
    /// second, would otherwise silently take the first one's file away.
    pub fn open(dir: &Path, token: u64, started: DateTime<Local>) -> Journal {
        let path = dir.join(file_name(started));
        match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(file) => Journal {
                token,
                file: Some(Arc::new(Mutex::new(file))),
                path: Some(path.to_string_lossy().into_owned()),
            },
            Err(err) => {
                // Said in the half that still works, and then forgotten: the
                // run goes on without a file, which is the whole rule this
                // module is built on.
                log::warn!("runs: could not open {}: {err}", path.display());
                Journal { token, file: None, path: None }
            }
        }
    }

    /// Where this run's journal is, for the report's footer. `None` when there
    /// is no file to name, and the document then says nothing rather than
    /// pointing at a path that is not there.
    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// Put one line on the record: stamped, logged, appended, flushed.
    ///
    /// The single call site for both destinations. Two calls per event — one to
    /// the file and one to the log — is how the two texts come to differ, and a
    /// journal that disagrees with the log is worse than either alone, since
    /// nothing says which of them was the one that was right.
    pub fn say(&self, line: &str) {
        let stamped = stamp(Local::now(), self.token, line);
        log::info!("runs: {stamped}");
        let Some(file) = &self.file else { return };
        // A poisoned mutex means a thread panicked mid-write. The record is
        // worth less than the run, so this gives up on the line rather than
        // taking the loop down with it.
        let Ok(mut file) = file.lock() else { return };
        // Flushed rather than left to the buffer: a run killed a moment later
        // is exactly the run this file is kept for. Both failures are dropped —
        // there is nowhere left to report a journal that cannot be written.
        let _ = writeln!(file, "{stamped}");
        let _ = file.flush();
    }
}

/// The file's name, from the moment the run started.
///
/// Seconds and no separator inside them, matching `write_report`'s own stem, so
/// a person listing `.smetana` reads one date format whichever kind of file
/// they are looking at.
pub fn file_name(started: DateTime<Local>) -> String {
    format!("journal-{}.log", started.format("%Y-%m-%d-%H%M%S"))
}

/// One line as both readers see it: a local timestamp, the run it belongs to,
/// and the text.
///
/// The stamp is **local**, like the run report's and unlike the app log's own
/// UTC prefix (`lib.rs` records that offset). The line therefore carries a
/// second stamp when it is read in the app log, and that is deliberate: it is
/// the same text in both places, so a line found in one can be searched for in
/// the other.
///
/// The token is in every line rather than in a header, because the app log
/// interleaves every run in every project and a line that cannot say which run
/// it belongs to is a line nobody can use.
pub fn stamp(now: DateTime<Local>, token: u64, line: &str) -> String {
    format!("{} run {token} {line}", now.format("%Y-%m-%d %H:%M:%S"))
}

/// 1. The run, as it was asked for.
pub fn started(run: &Run, max_iterations: u32) -> String {
    format!(
        "start project={} scope={:?} mode={:?} target={} max-iterations={max_iterations} \
         max-tasks={} min-priority={}",
        run.project,
        run.settings.scope,
        run.settings.mode,
        run.settings.target_branch,
        opt(run.settings.max_parallel_tasks),
        opt(run.settings.min_priority),
    )
}

/// 2. One declared command and how it went. `Err` carries what the run will
/// stop on, so the line and the bar say the same thing.
pub fn preflight_command(command: &str, ran: Result<Ran, &str>) -> String {
    match ran {
        Ok(Ran::Done) => format!("preflight command {command:?} done"),
        Ok(Ran::Cancelled) => format!("preflight command {command:?} cancelled"),
        Err(detail) => format!("preflight command {command:?} failed: {detail}"),
    }
}

/// 2, the other half. One declared health check and what it settled on.
///
/// `pub(super)`, like `batch_ended` below and for the same reason: it names a
/// type of the loop's own, and nothing outside `runs/` has one to name.
pub(super) fn preflight_check(check: &HealthCheck, probe: Probe) -> String {
    let what = match probe {
        Probe::Up => "up",
        Probe::Down => "down",
        Probe::Cancelled => "cancelled",
    };
    format!("preflight check {:?} {what}", preflight::describe(check))
}

/// 3. A board read, with the ids and not only the counts: the counts say a run
/// had work, and only the ids say *which*, which is what a person comparing a
/// journal against the board a day later is doing.
///
/// `resync` marks the second read the loop pays for when the first said the
/// queue was empty — it is the same event, and which of the two a decision came
/// from is worth being able to see.
pub fn board(snapshot: &QueueSnapshot, resync: bool) -> String {
    let mark = if resync { " (resync)" } else { "" };
    format!(
        "board{mark} ready={} {} unfinished={} {} closed={} parked={}",
        snapshot.ready.len(),
        ids(&snapshot.ready),
        snapshot.unfinished.len(),
        ids(&snapshot.unfinished),
        snapshot.closed,
        snapshot.parked,
    )
}

/// 3, when the read failed. An unreadable board is still a board read, and it
/// is the one whose absence from the record would read as a run that simply
/// stopped deciding.
pub fn unreadable_board(in_a_row: u32) -> String {
    format!("board unreadable ({in_a_row} in a row)")
}

/// 4. The spend gate: what the harness said, and what was made of it.
///
/// The reading is carried beside the decision because `Decision::Normal` holds
/// no percentage at all — an answer of "run at full size" is the same word for
/// a fresh week and for an allowance nobody could read, and those are different
/// nights.
pub fn gate(usage: Option<&Usage>, decision: &Decision) -> String {
    let (session, week) = match usage {
        Some(usage) => (pct(usage.session_pct), pct(usage.week_pct)),
        None => ("unread".to_string(), "unread".to_string()),
    };
    format!("usage session={session} week={week} decision={decision:?}")
}

/// 5. `queue::next_action`'s answer, whole, with both of the things it was
/// decided from: the ending of the batch before, and whether this board is the
/// board the last decision saw.
///
/// The pair is the point. A `Run(RetryAfterEmpty)` on its own says a batch
/// happened; beside `last=Empty` it says why another one followed rather than a
/// stop, which is the question of 29 August.
pub fn decision(
    action: &Action,
    last: LastBatch,
    iteration: u32,
    previous: Option<&QueueSnapshot>,
    now: &QueueSnapshot,
) -> String {
    let board = match previous {
        None => "first",
        Some(before) if before == now => "same",
        Some(_) => "moved",
    };
    format!("decide action={action:?} last={last:?} iteration={iteration} board={board}")
}

/// 6. A batch going out.
///
/// `ready` is the set the batch may take from and deliberately not "the tasks
/// it was given": nothing hands a batch a list of ids — it is handed a scope
/// and a ceiling, and the lead picks inside them — so a line claiming otherwise
/// would be an invention in the one document written to be trusted about
/// mechanics.
pub fn batch_started(
    n: u32,
    session: u64,
    actor: &str,
    group: Option<&Proc>,
    tasks: Option<u8>,
    ready: &[String],
) -> String {
    format!(
        "batch {n} start session={session} actor={actor} group={} max-tasks={} ready={}",
        group.map(|proc| proc.pid.to_string()).unwrap_or_else(|| "none".to_string()),
        opt(tasks),
        ids(ready),
    )
}

/// 7. A batch ending, in the loop's own words rather than the document's.
///
/// `Exit` is printed as itself — `Code(0)`, `NoCode`, `Removed` — because the
/// three are what a person reading this back has to tell apart, and every prose
/// rendering of them loses one of the distinctions. `report.rs` says the same
/// endings in sentences for a different reader; this is the machine's version
/// and the two are allowed to differ in wording.
pub(super) fn batch_ended(
    n: u32,
    batch: &Batch,
    seconds: u64,
    account: bool,
    held: &[Leftover],
) -> String {
    let how = match batch {
        Batch::Ended(exit) => format!("exit={exit:?}"),
        Batch::HandedBack => "ending=handed-back".to_string(),
        Batch::Unanswered { question } => format!("ending=unanswered question={question:?}"),
    };
    format!(
        "batch {n} end {how} seconds={seconds} account={} held={}",
        if account { "yes" } else { "no" },
        leftovers(held),
    )
}

/// 8. What this batch did to the counters, and what the run will wait out
/// before the next one.
///
/// Both counters on every batch, whichever of them moved: the pair is what says
/// a night of eight-minute sessions was read as empty rather than as crashed,
/// and a line that only appears when a counter moves cannot say that.
///
/// It is written where the loop has just settled `last_batch`, which is every
/// batch the run goes on from and the two whose counter itself ended the run.
/// The endings that come from somewhere else — a session somebody removed, a
/// question asked twice — leave no counted line, because neither counter moved
/// and `last_batch` there is the batch *before* this one: naming it would put a
/// stale ending on the record. The `end reason=` line is what those endings
/// have.
pub fn counted(
    n: u32,
    last: LastBatch,
    crashes: u32,
    max_crashes: u32,
    empties: u32,
    max_empties: u32,
    backoff: Option<Duration>,
) -> String {
    let mut line = format!(
        "batch {n} counted last={last:?} crashes={crashes}/{max_crashes} \
         empties={empties}/{max_empties}"
    );
    if let Some(backoff) = backoff {
        let _ = write!(line, " backoff={}s", backoff.as_secs());
    }
    line
}

/// 9. The ending, with the document it was written into — the one line that
/// ties a journal to the report beside it, in the direction the report cannot
/// go on its own.
pub fn ended(reason: &StopReason, seconds: u64, report: Option<&str>) -> String {
    format!("end reason={reason:?} seconds={seconds} report={}", report.unwrap_or("none"))
}

/// A list of ids as one field. Empty stays `[]` rather than disappearing: a
/// missing field reads as a fact nobody looked up.
fn ids(ids: &[String]) -> String {
    format!("[{}]", ids.join(", "))
}

/// What an actor still held, with the board's own word for each and the lock
/// called out — the same three facts `report.rs` draws, in one field.
fn leftovers(held: &[Leftover]) -> String {
    let mut out = String::from("[");
    for (nth, left) in held.iter().enumerate() {
        if nth > 0 {
            out.push_str(", ");
        }
        let _ = write!(out, "{} ({}", left.id, left.status);
        if left.lock {
            out.push_str(", lock");
        }
        out.push(')');
    }
    out.push(']');
    out
}

/// A number that may not have been chosen. `none` and never a zero: the
/// difference between "three tasks" and "whatever the lead takes" is a
/// difference this file exists to preserve.
fn opt(value: Option<u8>) -> String {
    value.map(|n| n.to_string()).unwrap_or_else(|| "none".to_string())
}

/// One half of a usage reading. `unread` rather than `none`, because the half
/// that went missing is a line the harness did not print, not an allowance of
/// zero.
fn pct(value: Option<u8>) -> String {
    value.map(|n| format!("{n}%")).unwrap_or_else(|| "unread".to_string())
}

/// The lines themselves, which are the whole of what this module is: a line
/// that loses a distinction is a night that cannot be read back, and every test
/// here is one of those distinctions. Nothing in it spawns a process or waits
/// on a runtime.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::terminal::model::Exit;
    use crate::runs::model::{RunMode, RunScope, RunSettings};
    use chrono::TimeZone;

    fn at(hour: u32, minute: u32, second: u32) -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 8, 29, hour, minute, second).unwrap()
    }

    fn run() -> Run {
        Run::new(
            7,
            "/p".to_string(),
            RunSettings {
                scope: RunScope::Queue,
                mode: RunMode::Auto,
                target_branch: "main".to_string(),
                create_target: false,
                min_priority: Some(2),
                max_parallel_tasks: Some(3),
                live_check: false,
                file_findings: true,
            },
        )
    }

    fn snapshot(ready: &[&str], unfinished: &[&str]) -> QueueSnapshot {
        QueueSnapshot {
            ready: ready.iter().map(|id| id.to_string()).collect(),
            unfinished: unfinished.iter().map(|id| id.to_string()).collect(),
            closed: 4,
            parked: 1,
        }
    }

    #[test]
    fn every_line_carries_the_time_and_the_run_it_belongs_to() {
        // Both halves are load-bearing and for different readers: without the
        // stamp a journal cannot be lined up against anything else that
        // happened that night, and without the token the app log — where every
        // run in every project is interleaved — cannot say whose line it is.
        assert_eq!(stamp(at(3, 4, 5), 7, "board ready=0 []"), "2026-08-29 03:04:05 run 7 board ready=0 []");
    }

    #[test]
    fn the_file_is_named_for_the_moment_the_run_started() {
        // The directory is `.smetana/runs/<token>/` and the token counts from
        // zero on every app start, so two runs share it across restarts. The
        // name is the only thing keeping one night's account off another's.
        assert_eq!(file_name(at(22, 1, 9)), "journal-2026-08-29-220109.log");
        assert_ne!(file_name(at(22, 1, 9)), file_name(at(22, 1, 10)));
    }

    #[test]
    fn a_journal_that_cannot_be_opened_is_still_a_journal() {
        // A file where the run's directory should be: `open` has nowhere to
        // write and must not take the run down with it. The line still reaches
        // the app log, and the report is told there is no path to name.
        let dir = std::env::temp_dir().join(format!("smetana-journal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a temporary directory");
        let blocked = dir.join("not-a-directory");
        std::fs::write(&blocked, b"").expect("a file in the way");

        let journal = Journal::open(&blocked, 1, at(1, 0, 0));

        assert!(journal.path().is_none(), "there is no file to name in the report");
        journal.say("start project=/p");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_line_reaches_the_file_as_it_happens_rather_than_at_the_end() {
        // The whole reason the file is opened at the start and flushed per
        // line: what is on disk while the run is still going is what a run that
        // died leaves behind.
        let dir = std::env::temp_dir().join(format!("smetana-journal-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a temporary directory");

        let journal = Journal::open(&dir, 2, at(1, 2, 3));
        journal.say("start project=/p");
        journal.say("batch 1 start session=9");
        let written = std::fs::read_to_string(dir.join("journal-2026-08-29-010203.log"))
            .expect("the journal is on disk before the run is over");

        assert_eq!(written.lines().count(), 2, "one line per event: {written}");
        assert!(written.contains("run 2 start project=/p"), "{written}");
        assert!(written.contains("run 2 batch 1 start session=9"), "{written}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_start_line_carries_what_the_run_was_asked_for() {
        let line = started(&run(), 40);
        assert!(line.starts_with("start project=/p "), "{line}");
        assert!(line.contains("scope=Queue"), "{line}");
        assert!(line.contains("mode=Auto"), "{line}");
        assert!(line.contains("target=main"), "{line}");
        assert!(line.contains("max-iterations=40"), "{line}");
        assert!(line.contains("max-tasks=3"), "{line}");
        assert!(line.contains("min-priority=2"), "{line}");
    }

    #[test]
    fn a_ceiling_nobody_chose_is_none_and_never_a_zero() {
        // "The lead takes what it takes" and "the lead may take nothing" are
        // opposite instructions, and a zero standing in for the first would
        // read as the second.
        let mut run = run();
        run.settings.max_parallel_tasks = None;
        run.settings.min_priority = None;
        let line = started(&run, 40);
        assert!(line.contains("max-tasks=none"), "{line}");
        assert!(line.contains("min-priority=none"), "{line}");
    }

    #[test]
    fn a_preflight_names_the_command_and_what_became_of_it() {
        assert_eq!(
            preflight_command("npm install", Ok(Ran::Done)),
            "preflight command \"npm install\" done"
        );
        assert_eq!(
            preflight_command("npm install", Ok(Ran::Cancelled)),
            "preflight command \"npm install\" cancelled"
        );
        assert_eq!(
            preflight_command("docker compose up -d", Err("exited with code 127")),
            "preflight command \"docker compose up -d\" failed: exited with code 127"
        );
    }

    #[test]
    fn a_health_check_is_named_the_way_the_bar_names_it() {
        let url = HealthCheck::Url { url: "http://localhost:3000/health".into() };
        assert_eq!(
            preflight_check(&url, Probe::Up),
            "preflight check \"http://localhost:3000/health\" up"
        );
        assert_eq!(
            preflight_check(&HealthCheck::Tcp { tcp: 5433 }, Probe::Down),
            "preflight check \"port 5433\" down"
        );
    }

    #[test]
    fn a_board_read_names_the_ids_and_not_only_the_counts() {
        let line = board(&snapshot(&["a-1", "a-2"], &["a-3"]), false);
        assert_eq!(line, "board ready=2 [a-1, a-2] unfinished=1 [a-3] closed=4 parked=1");
    }

    #[test]
    fn the_second_read_of_an_empty_queue_says_it_is_the_second() {
        // The resync is what a `QueueEmpty` is settled on, and a journal
        // showing two reads with no mark on either invites the reading that
        // the loop went round twice.
        let line = board(&snapshot(&[], &[]), true);
        assert!(line.starts_with("board (resync) ready=0 []"), "{line}");
    }

    #[test]
    fn an_unreadable_board_is_a_read_that_happened() {
        assert_eq!(unreadable_board(2), "board unreadable (2 in a row)");
    }

    #[test]
    fn the_gate_keeps_the_reading_beside_the_decision() {
        // `Normal` is the answer both to a fresh week and to a probe that could
        // not be read at all, so the decision alone does not say which night
        // this was.
        let usage = Usage {
            session_pct: Some(12),
            session_reset: None,
            week_pct: Some(40),
            week_reset: None,
        };
        assert_eq!(
            gate(Some(&usage), &Decision::Normal),
            "usage session=12% week=40% decision=Normal"
        );
        assert_eq!(gate(None, &Decision::Normal), "usage session=unread week=unread decision=Normal");
    }

    #[test]
    fn a_pause_carries_the_percentage_it_was_taken_on() {
        let usage =
            Usage { session_pct: None, session_reset: None, week_pct: Some(96), week_reset: None };
        let line = gate(Some(&usage), &Decision::Pause { pct: 96, resets: None });
        assert!(line.contains("session=unread"), "a line the harness did not print: {line}");
        assert!(line.contains("week=96%"), "{line}");
        assert!(line.contains("decision=Pause"), "{line}");
    }

    #[test]
    fn a_decision_names_the_batch_it_came_out_of() {
        // The whole of the 29 August question. `Run(RetryAfterEmpty)` beside
        // `last=Empty` is a run that read four dead batches as empty; the same
        // action beside `last=Crashed` is a run that read them as a harness
        // falling over, and the two ended in different places.
        let now = snapshot(&["a-1"], &[]);
        let line = decision(
            &Action::Run(crate::runs::queue::RunReason::RetryAfterEmpty),
            LastBatch::Empty,
            3,
            Some(&now),
            &now,
        );
        assert_eq!(line, "decide action=Run(RetryAfterEmpty) last=Empty iteration=3 board=same");
    }

    #[test]
    fn a_board_that_moved_is_told_from_one_that_did_not_and_from_the_first() {
        let before = snapshot(&["a-1"], &[]);
        let after = snapshot(&["a-2"], &[]);
        let stop = Action::Stop(StopReason::QueueEmpty);
        assert!(decision(&stop, LastBatch::Completed, 0, None, &before).ends_with("board=first"));
        assert!(decision(&stop, LastBatch::Completed, 1, Some(&before), &after)
            .ends_with("board=moved"));
    }

    #[test]
    fn a_batch_start_names_the_actor_the_claims_will_carry() {
        let group = Proc { pid: 4321, started: 9, command: "node".into() };
        let line = batch_started(
            2,
            9,
            "smetana-run-9",
            Some(&group),
            Some(2),
            &["a-1".to_string(), "a-2".to_string()],
        );
        assert_eq!(
            line,
            "batch 2 start session=9 actor=smetana-run-9 group=4321 max-tasks=2 \
             ready=[a-1, a-2]"
        );
    }

    #[test]
    fn a_batch_whose_group_could_not_be_read_says_none() {
        let line = batch_started(1, 3, "smetana-run-3", None, None, &[]);
        assert!(line.contains("group=none"), "{line}");
        assert!(line.contains("ready=[]"), "an empty list, not a missing field: {line}");
    }

    #[test]
    fn a_clean_exit_is_distinguishable_from_a_signal_and_from_a_removal() {
        // The acceptance criterion of smetana-7di, and the reason the exit is
        // printed as the enum rather than described: every prose rendering of
        // these three loses one of the distinctions.
        let held = vec![];
        assert!(batch_ended(1, &Batch::Ended(Exit::Code(0)), 480, true, &held)
            .contains("exit=Code(0)"));
        assert!(batch_ended(1, &Batch::Ended(Exit::Code(1)), 480, true, &held)
            .contains("exit=Code(1)"));
        assert!(batch_ended(1, &Batch::Ended(Exit::NoCode), 480, true, &held)
            .contains("exit=NoCode"));
        assert!(batch_ended(1, &Batch::Ended(Exit::Removed), 480, true, &held)
            .contains("exit=Removed"));
    }

    #[test]
    fn a_batch_end_says_whether_there_was_an_account_and_what_was_left_behind() {
        let held = vec![
            Leftover { id: "a-1".into(), status: "in_progress".into(), lock: false },
            Leftover { id: "smetana-lock".into(), status: "in_progress".into(), lock: true },
        ];
        let line = batch_ended(3, &Batch::Ended(Exit::Code(0)), 482, false, &held);
        assert_eq!(
            line,
            "batch 3 end exit=Code(0) seconds=482 account=no \
             held=[a-1 (in_progress), smetana-lock (in_progress, lock)]"
        );
    }

    #[test]
    fn the_two_endings_that_are_not_an_exit_say_what_they_are() {
        let held = vec![];
        assert!(batch_ended(1, &Batch::HandedBack, 60, true, &held).contains("ending=handed-back"));
        let asked = Batch::Unanswered { question: "Do you trust this directory?".into() };
        let line = batch_ended(1, &asked, 60, false, &held);
        assert!(
            line.contains("ending=unanswered question=\"Do you trust this directory?\""),
            "{line}"
        );
    }

    #[test]
    fn both_counters_are_on_every_batch_whichever_of_them_moved() {
        assert_eq!(
            counted(4, LastBatch::Empty, 0, 5, 2, 3, None),
            "batch 4 counted last=Empty crashes=0/5 empties=2/3"
        );
        assert_eq!(
            counted(4, LastBatch::Crashed, 2, 5, 0, 3, Some(Duration::from_secs(10))),
            "batch 4 counted last=Crashed crashes=2/5 empties=0/3 backoff=10s"
        );
    }

    #[test]
    fn the_ending_names_the_reason_and_the_document() {
        assert_eq!(
            ended(&StopReason::Crashed { attempts: 5 }, 7200, Some("/p/.smetana/reports/x.html")),
            "end reason=Crashed { attempts: 5 } seconds=7200 report=/p/.smetana/reports/x.html"
        );
        assert!(
            ended(&StopReason::QueueEmpty, 60, None).ends_with("report=none"),
            "a report that could not be written is named as absent"
        );
    }
}
