//! The vocabulary of a run: what one was asked to do, where it has got to, and
//! why it stopped. Pure — nothing here touches the tracker, a terminal or the
//! disk.
//!
//! Serialized in snake_case rather than camelCase, deliberately and for the
//! reason `config::ConfigState` records: these names also appear in
//! `.smetana/project.toml` and in what an agent is told, and one spelling
//! across all three is worth more than matching the JavaScript convention on
//! the way past.

use serde::{Deserialize, Serialize};

/// How much of the board a run is allowed to take.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunScope {
    /// Everything ready, subject to the priority floor.
    Queue,
    /// One issue and nothing else.
    Task { id: String },
    /// The issues whose parent is this one.
    Epic { id: String },
}

/// What happens where the process needs a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunMode {
    /// Park the task, note why, carry on. Nobody is watching.
    Auto,
    /// Ask. The session turns `needs-you` and the run waits for an answer.
    Supervised,
    /// One task, done by the agent itself rather than delegated, asking freely.
    Solo,
}

/// What the person chose in the dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSettings {
    pub scope: RunScope,
    pub mode: RunMode,
    /// Where finished work merges. Defaults come from `[defaults].target_branch`,
    /// but the run carries its own copy: the config may change under a run that
    /// is already going, and a run that silently retargets halfway through is
    /// worse than one that is wrong from the start and says so.
    pub target_branch: String,
    /// The target branch does not exist yet and has to be cut before anything
    /// merges into it. Decided by the dialog, which is the only place that
    /// knows what the branch list held — the worker never sees it, and the
    /// agent asking `git` itself would be asking after the run had already
    /// decided where things go.
    #[serde(default)]
    pub create_target: bool,
    /// Nothing worse than this is taken automatically. bd's scale runs 0 (most
    /// urgent) to 4.
    ///
    /// `None` for every scope but the queue, and an `Option` rather than a
    /// number nobody reads: a floor only means something where the run picks
    /// its own work. Where a person pointed at a task or an epic the work is
    /// already named, and a field sent and silently ignored is one somebody
    /// starts honouring again in six months. The same choice, for the same
    /// reason, is already made for Auto in `TaskDraft`.
    pub min_priority: Option<u8>,
    /// How many tasks the lead may have in flight at once. Chosen in the
    /// dialog, offered from `[defaults].max_parallel_tasks`, and it wins over
    /// that key rather than only being allowed to lower it — the file is not
    /// rewritten, so the choice lives exactly one run.
    ///
    /// `None` in `Solo`, and an `Option` for the reason `min_priority` is one:
    /// there the lead delegates to nobody, so "do the work yourself" and "keep
    /// this many agents" would be two instructions contradicting each other in
    /// the same prompt.
    #[serde(default)]
    pub max_parallel_tasks: Option<u8>,
    pub live_check: bool,
    /// Whether a finding may become a `deferred` issue at all. Off means
    /// everything goes to the digest — see the `running-tasks` skill.
    pub file_findings: bool,
}

/// Why a run is not running any more.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StopReason {
    /// Nothing ready and nothing unfinished. The ordinary ending.
    QueueEmpty,
    /// A whole batch ran to completion and changed neither set. Something is
    /// stuck: unmergeable, unfinishable, and not even parked.
    NoProgress,
    /// The backstop against endless churn.
    MaxIterations,
    /// The session exited non-zero this many times in a row. Not a stuck queue
    /// — usually a transient failure of the harness — and the report says so,
    /// because the two need different responses from a person.
    Crashed { attempts: u32 },
    /// The tracker could not be read twice running.
    Unreadable,
    /// Somebody pressed stop. The batch in flight was allowed to finish.
    Cancelled,
    /// The session the batch was running in was removed from the agents panel.
    /// Deliberately not `Cancelled` — that one is the stop button, and the two
    /// are different acts: this one ended a batch mid-flight, so what it left
    /// behind is the recovery phase's to clean up.
    SessionRemoved,
    /// The batch's session stopped to ask a person something, in a run that has
    /// no person in it. The process is still there and still waiting; the run
    /// is what ends, because a wait on that process would be a wait for ever.
    ///
    /// None of its three neighbours: nothing fell over, so it is not `Crashed`;
    /// nobody pressed the button, so it is not `Cancelled`; nobody took the
    /// session away, so it is not `SessionRemoved`. What happened is that the
    /// agent asked, and an unattended run is the one place there is nobody to
    /// answer.
    ///
    /// The question the profile read travels with it, because "the agent is
    /// waiting" without it sends somebody to the terminal to find out what for.
    /// A `String` and not an `Option`: this ending is only ever raised off a
    /// question a profile actually read — see `service::watch_batch` for why a
    /// bell alone is not enough to end a run over.
    NeedsAnswer { question: String },
    /// The project would not come up; the string names what failed.
    Preflight { detail: String },
}

/// Where a run has got to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunState {
    /// Bringing the project up, before the first batch.
    Preflight,
    /// A batch is running; `iteration` counts from zero.
    Working { iteration: u32 },
    /// Between batches: reading the board and deciding.
    Deciding,
    /// The subscription's allowance is spent, and the run is waiting for it to
    /// reset rather than spending sessions finding that out again.
    ///
    /// A state and not a sleep inside the loop, deliberately: a run that had
    /// simply gone quiet for three hours is indistinguishable from one that
    /// hung, and the bar is where somebody looks to tell those apart. Being a
    /// state is also what makes the stop button reach it — `request_stop` ends
    /// a run with no session in flight at once, and a pause has none.
    ///
    /// `resets` is the harness's own sentence about when the allowance clears
    /// ("Aug 11 at 5:59pm (Europe/Moscow)"), passed through untouched. The app
    /// never turns it into a moment in time — see `usage::Usage`.
    Paused { pct: u8, resets: Option<String> },
    Stopped { reason: StopReason },
}

/// A run, whole. Every event carries one of these and `run_state` returns one,
/// so the front end never reconstructs a run from pieces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Run {
    pub project: String,
    pub settings: RunSettings,
    pub state: RunState,
    /// The session working right now. `None` between batches, and `None` once
    /// the run has stopped — a row pointing at a dead session is worse than no
    /// row, which is the same reasoning that keeps sessions out of settings.
    pub session: Option<u64>,
    /// How many batches have been started, including the one running.
    pub batches: u32,
    /// Set when stop was asked for and the batch in flight is still going. The
    /// interface says "stopping after this batch" from this, and it is a
    /// separate field rather than a state because the run is still working.
    pub stopping: bool,
    /// How much of the subscription's allowance was spent when the batch in
    /// flight was started, if that was enough to make it take fewer tasks than
    /// the person asked for (`usage::REDUCED_THRESHOLD`). `None` is the
    /// ordinary case: full size.
    ///
    /// A field beside `stopping` rather than a state, for exactly the reason
    /// that one is: the run is working, and this only qualifies how. What it
    /// buys is that the reduction is not silent — somebody who chose four tasks
    /// and is watching two has something on screen that says why.
    #[serde(default)]
    pub reduced: Option<u8>,
}

/// What a run cannot start for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum RunError {
    #[error("a run is already going in this project")]
    AlreadyRunning,
    #[error("this project has no .smetana/project.toml")]
    NotConfigured,
    /// The config exists and could not be read. The message is the one
    /// `config::load` produced, and this is the first place in the app it is
    /// ever shown to anybody.
    #[error("{0}")]
    BrokenConfig(String),
    #[error("{0}")]
    BadSettings(String),
    #[error("{0}")]
    Terminal(String),
}

impl RunMode {
    /// Whether a session that stops to ask has anybody to answer it.
    ///
    /// `Auto` alone is nobody's: its own prompt says "you are on your own —
    /// there is no one to ask" (`agents::prompt`), and it is the only mode
    /// `Profile::autonomy` hands the bypass switches to. The other two say "ask
    /// me" in as many words, and the person who is sitting there answers in the
    /// terminal — ending their run out from under them at the first question
    /// would take away the whole of what they chose the mode for.
    pub fn unattended(self) -> bool {
        match self {
            RunMode::Auto => true,
            RunMode::Supervised | RunMode::Solo => false,
        }
    }
}

/// A question a session has stopped on, believed only once it has been seen
/// twice running.
///
/// Layer B is somebody else's interface read off a screen, and `agents/codex.rs`
/// records by name the one way it fails open: a scrolled screen with no anchor
/// left on it, where indented prose above a numbered list still reads as a
/// dialog. Ending an unattended run over one such frame would cost the run;
/// looking twice costs one poll. A real dialog stands until somebody answers
/// it, so it survives the second look, while a frame that only looked like one
/// has scrolled on.
///
/// The text has to match, not merely be present: two different questions a poll
/// apart is an agent that answered the first one itself and is therefore not
/// stuck at all.
#[derive(Default)]
pub struct Asked {
    last: Option<String>,
}

impl Asked {
    /// What this look saw. `Some` only when it is the same question the look
    /// before saw.
    pub fn confirm(&mut self, question: Option<&str>) -> Option<String> {
        let before = self.last.take();
        self.last = question.map(str::to_owned);
        match (before, question) {
            (Some(before), Some(now)) if before == now => Some(now.to_owned()),
            _ => None,
        }
    }
}

/// The most agents a run may be told to keep in flight. The ceiling is not the
/// app's — it is the subscription's and the machine's — so it is a wide one, and
/// it exists at all so that a number nobody could honour cannot reach a prompt.
pub const MAX_PARALLEL: u8 = 8;

impl RunSettings {
    /// The three rules that are not the dialog's to keep. `Solo` means the agent
    /// does the work itself instead of delegating, which is a coherent thing to
    /// ask of one task and not of a queue or an epic — there it would silently
    /// become something else. And a priority floor is the queue's alone: it
    /// decides what a run picks up for itself, so against work a person already
    /// pointed at it can only drop it. Checked here rather than in the dialog
    /// because a dialog gets rewritten and this does not.
    pub fn validate(&self) -> Result<(), RunError> {
        if matches!(self.mode, RunMode::Solo)
            && matches!(self.scope, RunScope::Queue | RunScope::Epic { .. })
        {
            return Err(RunError::BadSettings(
                "solo mode runs one task; a queue or an epic needs auto or supervised".into(),
            ));
        }
        if self.min_priority.is_some() && !matches!(self.scope, RunScope::Queue) {
            return Err(RunError::BadSettings(
                "a priority floor belongs to the queue; a named task or epic is taken whatever \
                 its priority"
                    .into(),
            ));
        }
        // The same shape as the floor, one rule down: a number of agents only
        // means something where the lead delegates, and `Solo` is the mode in
        // which it does not.
        if self.max_parallel_tasks.is_some() && matches!(self.mode, RunMode::Solo) {
            return Err(RunError::BadSettings(
                "solo mode does the work itself and delegates to nobody; a number of agents \
                 belongs to the other two modes"
                    .into(),
            ));
        }
        if let Some(agents) = self.max_parallel_tasks {
            if agents == 0 || agents > MAX_PARALLEL {
                return Err(RunError::BadSettings(format!(
                    "between 1 and {MAX_PARALLEL} agents at once, not {agents}"
                )));
            }
        }
        Ok(())
    }
}

impl Run {
    pub fn new(project: String, settings: RunSettings) -> Self {
        Run {
            project,
            settings,
            state: RunState::Preflight,
            session: None,
            batches: 0,
            stopping: false,
            reduced: None,
        }
    }

    pub fn is_over(&self) -> bool {
        matches!(self.state, RunState::Stopped { .. })
    }

    /// Nothing revives a stopped run — the same rule `Session::apply` keeps, and
    /// for the same reason: a late event from a batch that was already on its
    /// way out must not put a finished run back on the screen.
    pub fn advance(&mut self, state: RunState) {
        if self.is_over() {
            return;
        }
        if matches!(state, RunState::Stopped { .. }) {
            self.session = None;
        }
        self.state = state;
    }

    /// Stop was asked for. The batch in flight finishes: a run interrupted
    /// between a merge and a close is exactly the state the recovery phase
    /// exists to clean up, and killing a session mid-merge is how you get there
    /// deliberately. A run that has not started a batch yet stops at once.
    pub fn request_stop(&mut self) {
        if self.is_over() {
            return;
        }
        self.stopping = true;
        if self.session.is_none() {
            self.advance(RunState::Stopped { reason: StopReason::Cancelled });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A floor only where the scope allows one and a parallel count only where
    /// the mode allows one, which is what every test below that is not about
    /// either of them wants.
    fn settings(mode: RunMode, scope: RunScope) -> RunSettings {
        let min_priority = matches!(scope, RunScope::Queue).then_some(2);
        let max_parallel_tasks = (!matches!(mode, RunMode::Solo)).then_some(3);
        RunSettings {
            scope,
            mode,
            target_branch: "main".into(),
            create_target: false,
            min_priority,
            max_parallel_tasks,
            live_check: true,
            file_findings: true,
        }
    }

    #[test]
    fn solo_is_a_single_task_and_nothing_else() {
        assert!(settings(RunMode::Solo, RunScope::Task { id: "a-1".into() }).validate().is_ok());
        assert!(settings(RunMode::Solo, RunScope::Queue).validate().is_err());
        assert!(settings(RunMode::Solo, RunScope::Epic { id: "a-1".into() }).validate().is_err());
    }

    #[test]
    fn the_other_modes_take_any_scope() {
        for mode in [RunMode::Auto, RunMode::Supervised] {
            for scope in
                [RunScope::Queue, RunScope::Task { id: "a-1".into() }, RunScope::Epic { id: "a-2".into() }]
            {
                assert!(settings(mode, scope).validate().is_ok());
            }
        }
    }

    #[test]
    fn a_priority_floor_belongs_to_the_queue_and_to_nothing_else() {
        // A floor against work somebody pointed at can only take it away: the
        // run has nothing to choose, so a P4 task under a P2 floor would come
        // back as an empty queue naming the task the person had just picked.
        let floored = |scope| RunSettings { min_priority: Some(2), ..settings(RunMode::Auto, scope) };
        assert!(floored(RunScope::Queue).validate().is_ok());
        assert!(floored(RunScope::Task { id: "a-1".into() }).validate().is_err());
        assert!(floored(RunScope::Epic { id: "a-2".into() }).validate().is_err());

        // And no floor is fine everywhere, the queue included — the dialog
        // always sends one there, but nothing here depends on it.
        for scope in
            [RunScope::Queue, RunScope::Task { id: "a-1".into() }, RunScope::Epic { id: "a-2".into() }]
        {
            let bare = RunSettings { min_priority: None, ..settings(RunMode::Auto, scope) };
            assert!(bare.validate().is_ok());
        }
    }

    #[test]
    fn a_number_of_agents_belongs_to_every_mode_but_solo() {
        // Solo does the work itself, so "keep three agents going" beside "do it
        // yourself" is one prompt contradicting itself.
        let solo = RunSettings {
            max_parallel_tasks: Some(3),
            ..settings(RunMode::Solo, RunScope::Task { id: "a-1".into() })
        };
        assert!(solo.validate().is_err());

        // And none at all is fine anywhere: the dialog always sends one outside
        // solo, but nothing here depends on it — the lead then falls back to
        // the config, which is what the skill says.
        for mode in [RunMode::Auto, RunMode::Supervised] {
            let bare = RunSettings { max_parallel_tasks: None, ..settings(mode, RunScope::Queue) };
            assert!(bare.validate().is_ok());
        }
    }

    #[test]
    fn a_number_of_agents_outside_the_range_is_refused() {
        // Not the app's ceiling — the subscription's and the machine's — but a
        // number nobody could honour must not reach a prompt.
        let with = |agents| RunSettings {
            max_parallel_tasks: Some(agents),
            ..settings(RunMode::Auto, RunScope::Queue)
        };
        assert!(with(0).validate().is_err(), "zero agents is not a run, it is a stop");
        assert!(with(1).validate().is_ok());
        assert!(with(MAX_PARALLEL).validate().is_ok());
        assert!(with(MAX_PARALLEL + 1).validate().is_err());
    }

    #[test]
    fn a_stopped_run_stays_stopped() {
        // A batch on its way out can still report; putting a finished run back
        // on the screen is the defect this prevents.
        let mut run = Run::new("/p".into(), settings(RunMode::Auto, RunScope::Queue));
        run.advance(RunState::Stopped { reason: StopReason::QueueEmpty });
        run.advance(RunState::Working { iteration: 7 });
        assert_eq!(run.state, RunState::Stopped { reason: StopReason::QueueEmpty });
    }

    #[test]
    fn stopping_forgets_the_session() {
        let mut run = Run::new("/p".into(), settings(RunMode::Auto, RunScope::Queue));
        run.session = Some(3);
        run.advance(RunState::Stopped { reason: StopReason::Cancelled });
        assert_eq!(run.session, None, "a row pointing at a dead session is worse than no row");
    }

    #[test]
    fn stop_between_batches_is_immediate_and_stop_mid_batch_waits() {
        let mut idle = Run::new("/p".into(), settings(RunMode::Auto, RunScope::Queue));
        idle.request_stop();
        assert!(idle.is_over(), "nothing is in flight, so there is nothing to wait for");

        let mut working = Run::new("/p".into(), settings(RunMode::Auto, RunScope::Queue));
        working.session = Some(1);
        working.advance(RunState::Working { iteration: 0 });
        working.request_stop();
        assert!(working.stopping);
        assert!(!working.is_over(), "the batch in flight finishes — see request_stop");
    }

    #[test]
    fn a_stop_reason_reaches_the_front_end_as_a_kind_and_its_detail() {
        let json = serde_json::to_value(StopReason::Crashed { attempts: 5 }).expect("serialize");
        assert_eq!(json["kind"], "crashed");
        assert_eq!(json["attempts"], 5);

        let json = serde_json::to_value(StopReason::QueueEmpty).expect("serialize");
        assert_eq!(json["kind"], "queue_empty");
    }

    #[test]
    fn settings_round_trip_through_the_shape_the_front_end_sends() {
        // The one place either suite crosses the IPC boundary: this is the JSON
        // the dialog will actually put on the wire.
        let json = serde_json::json!({
            "scope": { "kind": "epic", "id": "smetana-1" },
            "mode": "supervised",
            "target_branch": "staging",
            "min_priority": null,
            "max_parallel_tasks": 4,
            "live_check": false,
            "file_findings": true
        });
        let parsed: RunSettings = serde_json::from_value(json).expect("the dialog's payload deserializes");
        assert_eq!(parsed.scope, RunScope::Epic { id: "smetana-1".into() });
        assert_eq!(parsed.mode, RunMode::Supervised);
        assert_eq!(parsed.min_priority, None, "the dialog shows no floor outside the queue");
        assert_eq!(parsed.max_parallel_tasks, Some(4));
        assert!(!parsed.live_check);
    }

    #[test]
    fn only_an_auto_run_has_nobody_to_answer_it() {
        // The whole of what decides whether a session waiting on a person ends
        // the run. Supervised and solo both promise the agent it may ask, and
        // there is somebody sitting in front of the terminal to be asked.
        assert!(RunMode::Auto.unattended());
        assert!(!RunMode::Supervised.unattended());
        assert!(!RunMode::Solo.unattended());
    }

    #[test]
    fn a_question_is_believed_only_when_it_is_still_there_a_look_later() {
        // One frame is not a dialog: layer B's known way of failing open is a
        // scrolled screen that reads like one, and it scrolls on. A real dialog
        // stands until it is answered.
        let mut asked = Asked::default();
        assert_eq!(asked.confirm(Some("Do you trust the contents of this directory?")), None);
        assert_eq!(
            asked.confirm(Some("Do you trust the contents of this directory?")),
            Some("Do you trust the contents of this directory?".to_string())
        );
    }

    #[test]
    fn a_question_that_went_away_or_changed_starts_the_count_again() {
        let mut asked = Asked::default();
        asked.confirm(Some("Trust this directory?"));
        assert_eq!(asked.confirm(None), None, "the dialog was gone by the second look");
        assert_eq!(asked.confirm(Some("Trust this directory?")), None, "and the count starts over");

        // A different question a poll later is an agent that answered the first
        // one itself, which is not an agent that is stuck.
        let mut moved = Asked::default();
        moved.confirm(Some("Run this command?"));
        assert_eq!(moved.confirm(Some("Apply this patch?")), None);
    }

    #[test]
    fn an_unanswered_question_reaches_the_front_end_with_what_was_asked() {
        // The bar draws the question as the detail line; a reason with nothing
        // in it would say the agent is waiting and not what for.
        let json = serde_json::to_value(StopReason::NeedsAnswer {
            question: "Do you trust the contents of this directory?".into(),
        })
        .expect("serialize");
        assert_eq!(json["kind"], "needs_answer");
        assert_eq!(json["question"], "Do you trust the contents of this directory?");
    }

    #[test]
    fn a_stop_reason_a_person_caused_is_not_the_stop_button() {
        // Both endings are somebody's doing and neither is a crash, but they
        // are different acts and the bar has to be able to say which: removing
        // the session ended a batch mid-flight, pressing stop let it finish.
        let removed = serde_json::to_value(StopReason::SessionRemoved).expect("serialize");
        assert_eq!(removed["kind"], "session_removed");
        assert_ne!(removed, serde_json::to_value(StopReason::Cancelled).expect("serialize"));
    }
}
