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

/// A threshold that is off. Not a percentage anybody could mean: "pause when 0%
/// of the allowance is used" is "never run at all", so the value is free to
/// carry the other meaning — and it carries it on the wire and in
/// `settings.json` too, because `adopt()` in `src/views/SettingsWindow.vue`
/// skips a field whose value is `null` and an `Option` would therefore never
/// reach the settings window when somebody turned a threshold off.
pub const OFF: u8 = 0;

/// At or above this the allowance is out, whatever the person has set their own
/// gate to. Deliberately a second constant rather than a reuse of
/// `PAUSE_THRESHOLD`, though it ships with the same number: that one is a
/// default somebody may move, and this one is the app's own reading of "the
/// harness will refuse the next session", which is not theirs to move. It is
/// only ever asked *after* a session has already exited non-zero, so it costs
/// nothing when things are going well.
pub const SPENT: u8 = 90;

/// The thresholds a person has chosen, read off `settings.json`.
///
/// Read at every gate check rather than once when the run started, which is the
/// opposite of what `drive` does with `agent` and `remove_worktrees` and for a
/// reason that does not apply to them: changing those mid-run would make a run
/// ask about one subscription and spend another, while changing these only
/// moves when it waits. Somebody watching a paused run and lowering the gate
/// wants that run to go on, not to be stopped and started again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub pause_at: u8,
    pub reduced_at: u8,
}

impl Default for Limits {
    fn default() -> Self {
        Self { pause_at: PAUSE_THRESHOLD, reduced_at: REDUCED_THRESHOLD }
    }
}

/// What the harness said is left. Percentages used, not remaining.
///
/// **A percentage is optional, and that is the whole point of the type.** The
/// harness prints two limit lines and either of them can go missing — a
/// reworded line, a build that prints one of them and not the other — and the
/// half that was not read has no number at all. A zero standing in for it is a
/// claim about an allowance nobody measured, which for a run is merely the
/// benign direction to be wrong in and on the settings window is a sentence
/// the app has no grounds for (smetana-7rp). So the absent half is `None`
/// here and `null` on the wire, and a real `0%` — which the harness does
/// print, on a fresh week — stays `Some(0)` and is drawn.
///
/// `Serialize` because this rides out to the settings window as well as into
/// the run gate, and `camelCase` because that is what every other type crossing
/// that boundary uses — `settings/model.rs` and `git.rs` among them.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Usage {
    pub session_pct: Option<u8>,
    /// When it resets, in the harness's own words. Deliberately a string and
    /// never a moment in time: "Aug 11 at 5:59pm (Europe/Moscow)" is written
    /// for a person to read, and turning it into an instant would add a second
    /// parse of the same prose — one whose failure would be a run that woke at
    /// the wrong hour rather than one that showed a line it could not use.
    pub session_reset: Option<String>,
    pub week_pct: Option<u8>,
    pub week_reset: Option<String>,
}

impl Usage {
    /// The limit that is actually in the way, out of the halves that were read.
    /// Both are reported when both are there, and the run stops for whichever
    /// is nearer its ceiling; one half alone is the answer on its own, and
    /// neither is no answer at all.
    ///
    /// `Ord` on `Option` puts `None` below every `Some`, so the missing half
    /// can never win the comparison and can never be read as a zero either.
    pub fn pct(&self) -> Option<u8> {
        self.session_pct.max(self.week_pct)
    }

    /// When *that* one resets. A tie goes to the session, which is the sooner
    /// of the two and therefore the more useful thing to put on screen — and
    /// by the same ordering, a session that was not read loses to a week that
    /// was.
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

/// Three bands, and `None` — an unreadable answer, or one with neither half of
/// it read — is deliberately the most permissive of the four. Refusing to work
/// because a probe failed would turn every hiccup in somebody else's CLI into a
/// stopped run, and the failure this module exists to prevent is not that one.
///
/// The bands are the person's own, out of `settings.json`, and `Limits::default`
/// is what this module used to hold as constants. A threshold set to `OFF` is
/// never entered at all, however high the reading — which is a decision about
/// pre-empting and not about noticing: `spent` below is the rule that keeps on
/// noticing, and `gate` is what puts the two together.
pub fn decide(usage: Option<&Usage>, limits: Limits) -> Decision {
    let Some(usage) = usage else { return Decision::Normal };
    // A reading with neither half in it says nothing about the allowance, so it
    // takes the same answer as no reading at all — the rule this module is
    // built on is that nothing which failed to be read may hold a run up.
    let Some(pct) = usage.pct() else { return Decision::Normal };
    if limits.pause_at != OFF && pct >= limits.pause_at {
        return Decision::Pause { pct, resets: usage.reset().map(str::to_owned) };
    }
    if limits.reduced_at != OFF && pct >= limits.reduced_at {
        return Decision::Reduced { pct };
    }
    Decision::Normal
}

/// Whether the allowance is out, by a rule no setting reaches.
///
/// The classification after a session exits non-zero asks this and nothing
/// else: a limit that ran out mid-batch and a harness that fell over are the
/// same absence to anyone reading an exit code, and they need opposite
/// responses. Were this to follow the person's own pause threshold, turning
/// that threshold off would make every spent allowance read as a crash, and the
/// run would stop with `Crashed` after `MAX_CRASHES` — which is the failure this
/// whole module exists to prevent, arriving through the settings window.
///
/// `None` is not spent. An unreadable probe never holds a run up.
pub fn spent(usage: Option<&Usage>) -> bool {
    usage.and_then(Usage::pct).is_some_and(|pct| pct >= SPENT)
}

/// Whether a pause is the hold above rather than one of the person's own
/// thresholds — the one distinction the run bar needs, because "Run anyway" is
/// worth offering for a threshold and worth refusing for a spent allowance,
/// where the next session would die the moment it started.
///
/// Deliberately not read off the `Decision`: both arrive as `Pause` and the
/// difference is in what produced them. Asked beside `gate` with the same two
/// arguments, so the two answers cannot come from different readings.
///
/// True whenever the hold applies, even where a threshold would have paused the
/// run anyway. Pressing the button in that case would release the threshold and
/// leave the allowance exactly as spent, which is the churn the gate exists to
/// prevent.
pub fn held(usage: Option<&Usage>, after_limited: bool) -> bool {
    after_limited && spent(usage)
}

/// What the run loop's gate does with a reading: the person's own bands, unless
/// the batch before this one died on a spent allowance and the allowance is
/// still spent.
///
/// The second half is what keeps "off" meaning *do not pre-empt* rather than
/// *do not notice*. Without it a run with the gate off would spend a session
/// discovering the wall, be told `LastBatch::Limited`, come straight back here,
/// be told to go, and do it again for as long as the queue lasts.
pub fn gate(usage: Option<&Usage>, limits: Limits, after_limited: bool) -> Decision {
    let decision = decide(usage, limits);
    if !after_limited || matches!(decision, Decision::Pause { .. }) {
        return decision;
    }
    match usage.filter(|reading| spent(Some(reading))) {
        Some(reading) => Decision::Pause {
            // `spent` is only true for a reading with a percentage in it, so the
            // fall-back is unreachable; it is there so this cannot panic.
            pct: reading.pct().unwrap_or(SPENT),
            resets: reading.reset().map(str::to_owned),
        },
        None => decision,
    }
}

/// Which of `decide`'s three bands a reading falls in, and nothing else from it.
///
/// The band travels to the front end while the comparison stays here. Handing
/// the percentages over and comparing them in JS was the alternative, and it
/// was refused for the reason two copies of a threshold are always refused: the
/// second copy drifts from the first with nothing on screen to say it has.
///
/// Which numbers produced the band is now the person's, out of `settings.json`
/// — `PAUSE_THRESHOLD` and `REDUCED_THRESHOLD` are what ships, not what applies.
/// That is what makes the sentence under the percentages in the settings window
/// and what a run actually does one fact rather than two: both come through
/// `decide` with the same `Limits`.
///
/// The reading itself is not repeated inside it the way `Decision` repeats it,
/// since the whole `Usage` is already beside it in the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Band {
    Normal,
    Reduced,
    Pause,
}

impl Band {
    fn of(decision: &Decision) -> Self {
        match decision {
            Decision::Normal => Band::Normal,
            Decision::Reduced { .. } => Band::Reduced,
            Decision::Pause { .. } => Band::Pause,
        }
    }
}

/// What the settings window is told when it asks what is left of the
/// subscription.
///
/// **Three distinguishable states rather than an `Option<Usage>`**, and the
/// third is the whole reason for the type. Through an `Option` the front end
/// could not tell "this agent does not answer that question at all" — Codex has
/// no `usage_command` — from "this agent was asked and could not answer", and
/// those are different sentences for a person and different things for them to
/// do about it. The run gate needs no such distinction, since both of them are
/// `Decision::Normal` there, which is why `decide` keeps taking an `Option` and
/// this is a second reading of the same fact rather than a change to that one.
///
/// **The agent rides in the answer.** `agents::pick` substitutes the first
/// installed profile for a configured one that is not on `PATH`, so the block
/// headed "Claude Code subscription" can be about Codex; the heading has to
/// name whoever actually answered rather than whoever is showing in the
/// dropdown.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "camelCase")]
pub enum AgentUsage {
    /// There is nothing to ask. Either the profile has no `usage_command` of
    /// its own, or no agent is installed at all — and that second case is the
    /// one with no agent to name, which is what the `Option` is for.
    Unsupported { agent: Option<String> },
    /// The probe was made and nothing could be read out of it: not signed in,
    /// not installed, or a CLI that has reworded its own output.
    Unreadable { agent: String },
    /// A reading, with the band it falls in.
    Read { agent: String, usage: Usage, band: Band },
}

/// The one mapping from "who would answer, and what did they say" to what the
/// settings window draws. Pure, and the whole of the command behind it: the
/// command's own body is the two blocking calls that produce these arguments.
pub fn report(
    profile: Option<&'static dyn Profile>,
    reading: Option<Usage>,
    limits: Limits,
) -> AgentUsage {
    let Some(profile) = profile else { return AgentUsage::Unsupported { agent: None } };
    let agent = profile.id().to_owned();
    // Asked before the reading is looked at, because a profile that cannot be
    // asked and one that was asked and said nothing both arrive here as `None`
    // — `read` answers that for every way of failing, this one included.
    if profile.usage_command().is_none() {
        return AgentUsage::Unsupported { agent: Some(agent) };
    }
    let Some(usage) = reading else { return AgentUsage::Unreadable { agent } };
    let band = Band::of(&decide(Some(&usage), limits));
    AgentUsage::Read { agent, usage, band }
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
            session_pct: Some(session),
            session_reset: Some("Aug 7 at 8pm".into()),
            week_pct: Some(week),
            week_reset: Some("Aug 11 at 5:59pm".into()),
        }
    }

    /// A reading with only the session in it: the shape `agents/claude.rs`
    /// hands over when one of the two lines it looks for has been reworded.
    fn session_only(session: u8) -> Usage {
        Usage { week_pct: None, week_reset: None, ..usage(session, 0) }
    }

    #[test]
    fn an_allowance_that_could_not_be_read_never_holds_a_run_up() {
        // The whole reason this module is allowed to parse somebody else's
        // prose: when the parse fails, things are exactly where they were
        // before it existed.
        assert_eq!(decide(None, Limits::default()), Decision::Normal);
    }

    #[test]
    fn a_reading_with_neither_half_in_it_is_no_reading_either() {
        // `claude.rs` does not produce this one — it answers `None` rather than
        // an empty reading — but `decide` is the place the rule is written, and
        // a second caller must not be able to stop a run by handing over an
        // allowance nobody read.
        assert_eq!(decide(Some(&Usage::default()), Limits::default()), Decision::Normal);
        assert_eq!(Usage::default().pct(), None);
    }

    #[test]
    fn one_half_of_a_reading_is_the_whole_of_the_decision() {
        // The bug this shape exists for: with the week unread, a `0` standing
        // in for it used to be compared against the session and lose, which is
        // harmless here and a lie on the settings window. The half that
        // arrived is the number, not its maximum with an invented zero.
        assert_eq!(session_only(80).pct(), Some(80));
        assert_eq!(decide(Some(&session_only(80)), Limits::default()), Decision::Reduced { pct: 80 });
        assert_eq!(session_only(0).pct(), Some(0), "a real zero is a reading");

        let week_only = Usage { session_pct: None, session_reset: None, ..usage(0, 95) };
        assert_eq!(week_only.pct(), Some(95));
        assert_eq!(
            decide(Some(&week_only), Limits::default()),
            Decision::Pause { pct: 95, resets: Some("Aug 11 at 5:59pm".into()) },
            "the reset named is the one of the half that is in the way"
        );
    }

    #[test]
    fn the_three_bands_are_read_off_whichever_limit_is_nearer_its_ceiling() {
        assert_eq!(decide(Some(&usage(0, 0)), Limits::default()), Decision::Normal);
        assert_eq!(decide(Some(&usage(74, 74)), Limits::default()), Decision::Normal);
        assert_eq!(decide(Some(&usage(REDUCED_THRESHOLD, 0)), Limits::default()), Decision::Reduced { pct: REDUCED_THRESHOLD });
        assert_eq!(decide(Some(&usage(0, REDUCED_THRESHOLD)), Limits::default()), Decision::Reduced { pct: REDUCED_THRESHOLD });
        assert_eq!(decide(Some(&usage(89, 89)), Limits::default()), Decision::Reduced { pct: 89 });
        assert!(matches!(decide(Some(&usage(PAUSE_THRESHOLD, 0)), Limits::default()), Decision::Pause { .. }));
        assert!(matches!(decide(Some(&usage(0, PAUSE_THRESHOLD)), Limits::default()), Decision::Pause { .. }));
    }

    #[test]
    fn a_pause_names_the_reset_of_the_limit_that_is_in_the_way() {
        // Showing the session's reset while it is the week that is exhausted
        // would send somebody back in an hour to find the run still paused.
        let Decision::Pause { pct, resets } = decide(Some(&usage(10, 95)), Limits::default()) else {
            panic!("95% of the week is a pause");
        };
        assert_eq!(pct, 95);
        assert_eq!(resets.as_deref(), Some("Aug 11 at 5:59pm"));

        let Decision::Pause { pct, resets } = decide(Some(&usage(95, 10)), Limits::default()) else {
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
    fn an_agent_with_no_way_to_be_asked_is_unsupported_rather_than_a_failed_read() {
        // Codex overrides neither half of the pair, so the question cannot be
        // put to it at all. Reading that as a failed probe would send somebody
        // to check a login that has nothing to do with it.
        assert_eq!(
            report(Some(&crate::agents::codex::Codex), None, Limits::default()),
            AgentUsage::Unsupported { agent: Some("codex".into()) }
        );
    }

    #[test]
    fn a_machine_with_no_agent_at_all_has_nobody_to_name() {
        assert_eq!(report(None, None, Limits::default()), AgentUsage::Unsupported { agent: None });
    }

    #[test]
    fn a_probe_that_gave_nothing_back_is_unreadable_and_never_a_reading_of_zero() {
        // The state this whole type exists for: the same `None` a profile with
        // no command produces, from a profile that has one. A `Usage::default`
        // here would put "0% used" on the screen of somebody who is simply not
        // signed in.
        assert_eq!(
            report(Some(&crate::agents::claude::Claude), None, Limits::default()),
            AgentUsage::Unreadable { agent: "claude".into() }
        );
    }

    #[test]
    fn a_reading_carries_the_agent_that_answered_and_the_band_it_falls_in() {
        let AgentUsage::Read { agent, usage: read, band } =
            report(Some(&crate::agents::claude::Claude), Some(usage(10, 80)), Limits::default())
        else {
            panic!("a reading from a profile that can be asked");
        };
        assert_eq!(agent, "claude");
        assert_eq!(read, usage(10, 80));
        assert_eq!(band, Band::Reduced);
    }

    #[test]
    fn the_wire_shape_is_the_one_the_settings_window_reads() {
        // The names are load-bearing and nothing else pins them: the front end
        // reads `state`, `agent`, `band` and the four camelCase fields of the
        // reading, and a rename here would empty the block with every gate
        // still green.
        let json = serde_json::to_value(report(
            Some(&crate::agents::claude::Claude),
            Some(usage(10, 20)),
            Limits::default(),
        ))
        .expect("the answer serializes");
        assert_eq!(json["state"], "read");
        assert_eq!(json["agent"], "claude");
        assert_eq!(json["band"], "normal");
        assert_eq!(json["usage"]["sessionPct"], 10);
        assert_eq!(json["usage"]["sessionReset"], "Aug 7 at 8pm");
        assert_eq!(json["usage"]["weekPct"], 20);
        assert_eq!(json["usage"]["weekReset"], "Aug 11 at 5:59pm");

        // A half that was not read travels as an explicit `null` under the key
        // it would have had, rather than by the key going missing: the front
        // end reads it with `Number.isFinite`, which refuses both, but the two
        // are not the same promise and only one of them is testable from here.
        let json = serde_json::to_value(report(
            Some(&crate::agents::claude::Claude),
            Some(session_only(10)),
            Limits::default(),
        ))
        .expect("the answer serializes");
        assert_eq!(json["usage"]["sessionPct"], 10);
        assert!(json["usage"]["weekPct"].is_null(), "an unread half is null and never a zero");
        assert!(json["usage"].as_object().expect("a reading is an object").contains_key("weekPct"));

        let json = serde_json::to_value(report(Some(&crate::agents::codex::Codex), None, Limits::default()))
            .expect("the answer serializes");
        assert_eq!(json["state"], "unsupported");
        assert_eq!(json["agent"], "codex");
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

    #[test]
    fn a_threshold_that_is_off_is_never_entered() {
        let limits = Limits { pause_at: OFF, reduced_at: REDUCED_THRESHOLD };
        // 99% used, and the person has said not to pause on it.
        assert_eq!(decide(Some(&usage(99, 0)), limits), Decision::Reduced { pct: 99 });
        let limits = Limits { pause_at: PAUSE_THRESHOLD, reduced_at: OFF };
        assert_eq!(decide(Some(&usage(80, 0)), limits), Decision::Normal);
    }

    #[test]
    fn both_thresholds_off_is_always_normal() {
        let limits = Limits { pause_at: OFF, reduced_at: OFF };
        assert_eq!(decide(Some(&usage(100, 100)), limits), Decision::Normal);
    }

    #[test]
    fn the_shipped_limits_are_the_bands_this_module_had() {
        let limits = Limits::default();
        assert_eq!(decide(Some(&usage(74, 0)), limits), Decision::Normal);
        assert_eq!(decide(Some(&usage(75, 0)), limits), Decision::Reduced { pct: 75 });
        assert!(matches!(decide(Some(&usage(90, 0)), limits), Decision::Pause { .. }));
    }

    #[test]
    fn a_spent_allowance_is_read_at_ninety_and_above() {
        assert!(spent(Some(&usage(SPENT, 0))));
        assert!(spent(Some(&usage(0, 99))));
        assert!(!spent(Some(&usage(89, 89))));
    }

    #[test]
    fn nothing_that_could_not_be_read_is_ever_spent() {
        assert!(!spent(None));
        assert!(!spent(Some(&Usage::default())));
    }

    #[test]
    fn the_gate_holds_after_a_limited_batch_with_every_threshold_off() {
        let limits = Limits { pause_at: OFF, reduced_at: OFF };
        let reading = usage(95, 0);
        // Nothing was limited yet: the person's own thresholds are the whole
        // answer, and they say go.
        assert_eq!(gate(Some(&reading), limits, false), Decision::Normal);
        // A batch has just died on a spent allowance, so the run waits it out
        // rather than spending another session finding out again.
        assert!(matches!(gate(Some(&reading), limits, true), Decision::Pause { pct: 95, .. }));
    }

    #[test]
    fn the_gate_lets_a_run_through_once_the_reading_has_dropped() {
        let limits = Limits { pause_at: OFF, reduced_at: OFF };
        assert_eq!(gate(Some(&usage(3, 40)), limits, true), Decision::Normal);
    }

    #[test]
    fn the_gate_carries_the_reset_of_the_limit_that_is_in_the_way() {
        let limits = Limits { pause_at: OFF, reduced_at: OFF };
        let reading = Usage {
            session_pct: Some(96),
            session_reset: Some("Sep 1 at 6pm (Europe/Moscow)".into()),
            week_pct: Some(20),
            week_reset: Some("Sep 4 at 9am (Europe/Moscow)".into()),
        };
        assert_eq!(
            gate(Some(&reading), limits, true),
            Decision::Pause { pct: 96, resets: Some("Sep 1 at 6pm (Europe/Moscow)".into()) }
        );
    }

    #[test]
    fn a_reduced_band_the_person_chose_still_stands_after_a_limited_batch() {
        // The hold is only ever a `Pause`, so a reading inside somebody's own
        // reduced band comes back reduced rather than being promoted.
        let limits = Limits { pause_at: OFF, reduced_at: 50 };
        assert_eq!(gate(Some(&usage(60, 0)), limits, true), Decision::Reduced { pct: 60 });
    }

    #[test]
    fn the_band_the_settings_window_draws_is_the_persons_own() {
        // 80% with the pause threshold moved down to 80: the window must say a
        // run would stop here, not that it would merely take fewer tasks.
        let limits = Limits { pause_at: 80, reduced_at: 50 };
        let answer = report(Some(&crate::agents::claude::Claude), Some(usage(80, 0)), limits);
        assert!(matches!(answer, AgentUsage::Read { band: Band::Pause, .. }));
    }

    #[test]
    fn a_hold_is_told_from_a_threshold_by_what_produced_it() {
        let reading = usage(95, 0);
        // Nobody's batch has died yet: whatever the bands say, this is the
        // person's own gate and the button is worth offering.
        assert!(!held(Some(&reading), false));
        // The batch before this one died on it, and it is still spent.
        assert!(held(Some(&reading), true));
        // True even where a threshold would have paused the run anyway:
        // releasing the threshold would leave the allowance just as spent.
        assert!(held(Some(&reading), true));
        // Dropped back under the line: the hold is over, and a pause here can
        // only be somebody's own threshold again.
        assert!(!held(Some(&usage(60, 0)), true));
        // Nothing that could not be read ever holds a run.
        assert!(!held(None, true));
    }
}
