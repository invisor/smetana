//! What the subscription has left, and what a run does about it.
//!
//! The port of `holiday-curb`'s usage gating — the one piece of that script
//! `docs/superpowers/specs/2026-08-05-runs-design.md` deliberately left behind,
//! on the grounds that reading `claude -p "/usage"` is a parse of somebody
//! else's prose that breaks silently. That reasoning stands; what did not is
//! the trade it was made for. A run that exhausts its allowance overnight
//! spends five sessions and a minute of backoff discovering it, then stops with
//! `Crashed` — which says the harness kept failing, when nothing failed at all
//! and the work was never stuck. The two need opposite responses from a person,
//! and the run was giving them the wrong one.
//!
//! So the parse comes back, with its failure mode named rather than assumed:
//! **an unreadable answer never blocks a run**. It reads as `Normal` and the
//! batch goes at full size, which is exactly where things were before this
//! module existed. That is the same shape layer B keeps in `agents/claude.rs`
//! — no match leaves the previous behaviour in place instead of inventing one.
//!
//! The gate runs **before** each batch rather than after a failure, and that is
//! the whole of why it is worth having: an allowance is checked before it is
//! spent, so the exhausted case costs no session at all. `service.rs` asks the
//! same question a second time after a session exits non-zero, for the case the
//! allowance ran out mid-batch — there it is not a gate but a classification,
//! telling a spent limit apart from a harness that fell over.
//!
//! Pure apart from `read`, which is the one function here that spawns anything.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::agents::Profile;

/// At or above this, take no work at all and wait for the reset. The source's
/// number, and it is not 100 for a reason: the reading is approximate — it
/// counts local sessions on this machine and not other devices — so a batch
/// started at 95% is one that runs into the wall halfway through, which costs
/// a killed session and the recovery phase that follows it.
pub const PAUSE_THRESHOLD: u8 = 90;
/// At or above this, take fewer tasks per batch.
pub const REDUCED_THRESHOLD: u8 = 75;
/// How many tasks a batch may take while reduced. Tasks, not agents: a lead
/// spawns whatever teammates a task needs, and this is the count of tasks it
/// may have in flight at once — `[defaults].max_parallel_tasks`, whose own
/// default is 3.
pub const REDUCED_MAX_TASKS: u8 = 2;
/// How often to ask again while paused. A session limit resets in hours and a
/// weekly one in days, so asking oftener than this only spends the machine.
pub const POLL: Duration = Duration::from_secs(10 * 60);
/// A probe that hangs is worse than one that fails: the run would sit between
/// batches with nothing on screen to say why.
const PROBE_TIMEOUT: Duration = Duration::from_secs(60);

/// What the harness said is left. Percentages used, not remaining.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Usage {
    pub session_pct: u8,
    /// When it resets, in the harness's own words. Deliberately a string and
    /// never a moment in time: "Aug 11 at 5:59pm (Europe/Moscow)" is written
    /// for a person to read, and turning it into an instant would add a second
    /// parse of the same prose — one whose failure would be a run that woke at
    /// the wrong hour rather than one that showed a line it could not use.
    pub session_reset: Option<String>,
    pub week_pct: u8,
    pub week_reset: Option<String>,
}

impl Usage {
    /// The limit that is actually in the way. Both are reported, and the run
    /// stops for whichever is nearer its ceiling.
    pub fn pct(&self) -> u8 {
        self.session_pct.max(self.week_pct)
    }

    /// When *that* one resets. A tie goes to the session, which is the sooner
    /// of the two and therefore the more useful thing to put on screen.
    fn reset(&self) -> Option<&str> {
        if self.session_pct >= self.week_pct {
            self.session_reset.as_deref()
        } else {
            self.week_reset.as_deref()
        }
    }
}

/// What to do about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    /// Nothing. Also the answer when the allowance could not be read at all.
    Normal,
    /// Run, but take fewer tasks. Carries the reading for the same reason
    /// `Pause` does: a batch quietly running at half the size somebody chose is
    /// a behaviour with nothing on screen to explain it.
    Reduced { pct: u8 },
    /// Take nothing and wait.
    Pause { pct: u8, resets: Option<String> },
}

/// Three bands, and `None` — an unreadable answer — is deliberately the most
/// permissive of the four. Refusing to work because a probe failed would turn
/// every hiccup in somebody else's CLI into a stopped run, and the failure this
/// module exists to prevent is not that one.
pub fn decide(usage: Option<&Usage>) -> Decision {
    let Some(usage) = usage else { return Decision::Normal };
    let pct = usage.pct();
    if pct >= PAUSE_THRESHOLD {
        return Decision::Pause { pct, resets: usage.reset().map(str::to_owned) };
    }
    if pct >= REDUCED_THRESHOLD {
        return Decision::Reduced { pct };
    }
    Decision::Normal
}

/// How many tasks the next batch may take.
///
/// The number a person chose is never rewritten — `RunSettings` keeps it and
/// the report names it — and this is what one batch is run with instead. The
/// same split `views/panelWidths.js` makes between the width that is stored and
/// the width that is drawn, for the same reason: a condition of the moment must
/// not silently become a preference.
///
/// `None` stays `None`: that is `Solo`, where the lead does the work itself and
/// a number of tasks would be a second instruction contradicting the first.
pub fn cap(chosen: Option<u8>, decision: &Decision) -> Option<u8> {
    match decision {
        Decision::Reduced { .. } => chosen.map(|n| n.min(REDUCED_MAX_TASKS)),
        Decision::Normal | Decision::Pause { .. } => chosen,
    }
}

/// Ask the harness. `None` means the question could not be asked or the answer
/// could not be read, which `decide` treats as no reason to hold anything up.
///
/// Blocking, and called from `spawn_blocking`. The output is small — a couple
/// of kilobytes — so reading it after the wait cannot deadlock on a full pipe
/// the way a large one would.
pub fn read(profile: &'static dyn Profile) -> Option<Usage> {
    let args = profile.usage_command()?;
    let mut command = Command::new(profile.binary());
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    // The login shell's PATH, for the reason `terminal/service.rs` records at
    // its own `agents::pick`: a bundled app started from Finder inherits
    // launchd's, where nothing a person installed is reachable.
    if let Some(path) = crate::shell_env::path() {
        command.env("PATH", path);
    }

    let mut child = command.spawn().ok()?;
    let deadline = Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => break,
            // A non-zero probe says nothing about the allowance — it says the
            // probe failed — so it is the same answer as no probe at all.
            Ok(Some(_)) => return None,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(_) => return None,
        }
    }
    let output = child.wait_with_output().ok()?;
    profile.parse_usage(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn usage(session: u8, week: u8) -> Usage {
        Usage {
            session_pct: session,
            session_reset: Some("Aug 7 at 8pm".into()),
            week_pct: week,
            week_reset: Some("Aug 11 at 5:59pm".into()),
        }
    }

    #[test]
    fn an_allowance_that_could_not_be_read_never_holds_a_run_up() {
        // The whole reason this module is allowed to parse somebody else's
        // prose: when the parse fails, things are exactly where they were
        // before it existed.
        assert_eq!(decide(None), Decision::Normal);
    }

    #[test]
    fn the_three_bands_are_read_off_whichever_limit_is_nearer_its_ceiling() {
        assert_eq!(decide(Some(&usage(0, 0))), Decision::Normal);
        assert_eq!(decide(Some(&usage(74, 74))), Decision::Normal);
        assert_eq!(decide(Some(&usage(REDUCED_THRESHOLD, 0))), Decision::Reduced { pct: REDUCED_THRESHOLD });
        assert_eq!(decide(Some(&usage(0, REDUCED_THRESHOLD))), Decision::Reduced { pct: REDUCED_THRESHOLD });
        assert_eq!(decide(Some(&usage(89, 89))), Decision::Reduced { pct: 89 });
        assert!(matches!(decide(Some(&usage(PAUSE_THRESHOLD, 0))), Decision::Pause { .. }));
        assert!(matches!(decide(Some(&usage(0, PAUSE_THRESHOLD))), Decision::Pause { .. }));
    }

    #[test]
    fn a_pause_names_the_reset_of_the_limit_that_is_in_the_way() {
        // Showing the session's reset while it is the week that is exhausted
        // would send somebody back in an hour to find the run still paused.
        let Decision::Pause { pct, resets } = decide(Some(&usage(10, 95))) else {
            panic!("95% of the week is a pause");
        };
        assert_eq!(pct, 95);
        assert_eq!(resets.as_deref(), Some("Aug 11 at 5:59pm"));

        let Decision::Pause { pct, resets } = decide(Some(&usage(95, 10))) else {
            panic!("95% of the session is a pause");
        };
        assert_eq!(pct, 95);
        assert_eq!(resets.as_deref(), Some("Aug 7 at 8pm"));
    }

    #[test]
    fn a_reduced_batch_takes_fewer_tasks_and_never_more_than_was_asked_for() {
        // Reduced is a ceiling, not a number: somebody who chose one task at a
        // time must not find two running because the allowance ran low.
        assert_eq!(cap(Some(8), &Decision::Reduced { pct: 80 }), Some(REDUCED_MAX_TASKS));
        assert_eq!(cap(Some(3), &Decision::Reduced { pct: 80 }), Some(REDUCED_MAX_TASKS));
        assert_eq!(cap(Some(1), &Decision::Reduced { pct: 80 }), Some(1));
    }

    #[test]
    fn nothing_but_reduced_touches_the_number_of_tasks() {
        for decision in [Decision::Normal, Decision::Pause { pct: 99, resets: None }] {
            assert_eq!(cap(Some(4), &decision), Some(4));
        }
    }

    #[test]
    fn solo_has_no_number_of_tasks_to_reduce() {
        // `RunSettings::validate` is what makes it `None` there, and a batch
        // that suddenly grew one would be told to delegate work it was
        // started to do itself.
        for decision in
            [Decision::Normal, Decision::Reduced { pct: 80 }, Decision::Pause { pct: 99, resets: None }]
        {
            assert_eq!(cap(None, &decision), None);
        }
    }
}
