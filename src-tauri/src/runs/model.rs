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
    pub min_priority: u8,
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

impl RunSettings {
    /// The one rule that is not the dialog's to keep. `Solo` means the agent
    /// does the work itself instead of delegating, which is a coherent thing to
    /// ask of one task and not of a queue or an epic — there it would silently
    /// become something else. Checked here rather than in the dialog because a
    /// dialog gets rewritten and this does not.
    pub fn validate(&self) -> Result<(), RunError> {
        match (&self.mode, &self.scope) {
            (RunMode::Solo, RunScope::Queue | RunScope::Epic { .. }) => Err(RunError::BadSettings(
                "solo mode runs one task; a queue or an epic needs auto or supervised".into(),
            )),
            _ => Ok(()),
        }
    }
}

impl Run {
    pub fn new(project: String, settings: RunSettings) -> Self {
        Run { project, settings, state: RunState::Preflight, session: None, batches: 0, stopping: false }
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

    fn settings(mode: RunMode, scope: RunScope) -> RunSettings {
        RunSettings {
            scope,
            mode,
            target_branch: "main".into(),
            create_target: false,
            min_priority: 2,
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
            "min_priority": 2,
            "live_check": false,
            "file_findings": true
        });
        let parsed: RunSettings = serde_json::from_value(json).expect("the dialog's payload deserializes");
        assert_eq!(parsed.scope, RunScope::Epic { id: "smetana-1".into() });
        assert_eq!(parsed.mode, RunMode::Supervised);
        assert!(!parsed.live_check);
    }
}
