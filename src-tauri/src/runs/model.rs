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
///
/// Equality is the whole of what "the same scope" means to `service::admit`:
/// two queue runs race for the same tasks, two runs on one task are the same
/// work twice, while a queue run beside a task run — or two runs over
/// different epics — divide the board rather than fight over it. The derived
/// `PartialEq` says exactly that, so there is no second function to keep in
/// step with it.
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

impl RunScope {
    /// The scope as a person reads it, for the refusal that has to name which
    /// run is already going. The front end composes the same words
    /// (`components/run/runScopes.js`), so a person meets one vocabulary
    /// whichever side of the wire the sentence was made on.
    pub fn describe(&self) -> String {
        match self {
            RunScope::Queue => "the queue".to_string(),
            RunScope::Task { id } => format!("task {id}"),
            RunScope::Epic { id } => format!("epic {id}"),
        }
    }
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
    /// Permission to cut the target branch wherever it turns out to be
    /// missing — not a claim that it is.
    ///
    /// Permission rather than a fact because a project of several repositories
    /// can carry the branch in some of them and not others, and which ones is
    /// a question only each repository can answer at the moment it is cut. The
    /// dialog's answer is a snapshot of what the list held when it opened, and
    /// a run lives for hours: so `provisioning` re-asks git per repository
    /// (`show-ref --verify`) and this flag decides only what it may do when the
    /// answer is no — make the branch from that repository's own HEAD, or STOP
    /// because nobody sanctioned inventing a base for the whole batch.
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
    /// The run was allowed one batch, that batch ran to completion, and this is
    /// it: the run did what it was asked to do. Its own reason rather than a
    /// reuse, because both neighbours would lie — `QueueEmpty` with tasks still
    /// sitting in Ready says the work ran out, and `NoProgress` says something
    /// is stuck when nothing is.
    BatchDone,
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
    /// The same question stopped two batches in a row, in a run that has no
    /// person in it. One question costs one batch, not the run: the first time
    /// a session stops to ask, the run kills it, parks what it claimed with the
    /// question as the note, and takes the next batch (smetana-8pe). Coming
    /// back to the very same question is a machine that cannot start — a
    /// harness dialog no batch will get past — and that needs a person rather
    /// than more batches. The second session is left alive and still at its
    /// prompt, because the terminal is where the person answers it.
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
    /// Which run this is, for a project that may hold several at once. The
    /// worker's map is keyed by it, a stop names one, and every `run:state`
    /// event carries it — the same job `generation` does for the tracker, and
    /// the same defect without it: a late report from an ended run written
    /// over the one that replaced it.
    pub token: u64,
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
    /// What the run did, filled in on the one transition into `Stopped` and
    /// `None` in every other state — there is nothing to summarize about a run
    /// that is still going, and a half-filled one on the wire would be read as
    /// the whole account. `advance` is what holds that: only the `Stopped` arm
    /// keeps it, so a field set by accident anywhere else clears itself on the
    /// next transition rather than travelling under a live run.
    ///
    /// `runs::service::finish` is the single writer, which is the whole reason
    /// that helper exists: a dozen exits into `Stopped` is how the next ending
    /// somebody adds quietly arrives with no report behind it.
    #[serde(default)]
    pub summary: Option<crate::runs::summary::RunSummary>,
}

/// What a run cannot start for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum RunError {
    /// A live run over the very same scope. Not "this project is busy" any
    /// more — a project holds several runs at once, and only two runs told to
    /// take the same work are refused. `scope` is `RunScope::describe`'s
    /// sentence fragment, because a refusal that cannot say *which* run is in
    /// the way sends somebody to the bar to guess.
    #[error("a run over {scope} is already going in this project")]
    AlreadyRunning { scope: String },
    /// The previous run over this scope has stopped and the loop task carrying
    /// it has not finished winding down — it is inside a board read or a usage
    /// probe and has not looked at the stop channel yet. The preflight used to
    /// be the longest of these by far and is not one any more: `bring_up`
    /// watches that channel and kills the command in flight (smetana-16w).
    ///
    /// Deliberately not `AlreadyRunning`, and the difference is the whole
    /// reason it exists: nothing is going, the bar says so in the same breath,
    /// and a refusal claiming otherwise reads as the stop not having taken. The
    /// scope is not free yet, which is a different sentence and a true one.
    #[error("the previous run in this project is still finishing")]
    WindingDown,
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

    /// Whether the run is one batch and no more.
    ///
    /// `Supervised` alone: a lead and its team take one batch of independent
    /// tasks, merge it, and that is the end — wanting more is answered by
    /// starting another run, not by this one taking another batch. `Auto` is
    /// the unattended loop that keeps taking batches until the queue is empty.
    /// `Solo` is one task, and the queue itself ends it: the scope holds that
    /// task and nothing else, so finishing it is `QueueEmpty` — calling it a
    /// one-batch run here would change its recovery, not its ending.
    pub fn one_batch(self) -> bool {
        match self {
            RunMode::Supervised => true,
            RunMode::Auto | RunMode::Solo => false,
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

/// What one confirmed question costs: the batch, or the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnQuestion {
    /// Kill the session, park what it claimed with the question as the note,
    /// and take the next batch. The lead's own layer already says park and
    /// carry on; a lead stuck at a harness dialog just cannot do it itself.
    Park,
    /// The same question has now ended two batches in a row: a machine that
    /// cannot start. The run stops with `NeedsAnswer`.
    Stop,
}

/// Batches ended by an unanswered question, counted in a row.
///
/// The spin this exists to prevent: a harness dialog — Codex's folder trust is
/// the recorded one (smetana-wnl) — reappears in every batch, and parking a
/// whole batch each time would chew the queue and call it progress. So the
/// first question costs one batch, and the *same* question a second time in a
/// row costs the run. A different question is not the spin: the machine got
/// past the first one, so batches are getting somewhere.
///
/// A batch that ends any other way breaks the row — "in a row" is literal,
/// because a completed batch in between proves the question was not standing
/// in front of everything.
#[derive(Default)]
pub struct RepeatedQuestion {
    last: Option<String>,
}

impl RepeatedQuestion {
    /// A batch just stopped on this question.
    pub fn ended_by(&mut self, question: &str) -> OnQuestion {
        if self.last.as_deref() == Some(question) {
            OnQuestion::Stop
        } else {
            self.last = Some(question.to_owned());
            OnQuestion::Park
        }
    }

    /// A batch ended some other way — exit, crash, spent allowance. The row is
    /// broken, so the next question is again worth one batch.
    pub fn cleared(&mut self) {
        self.last = None;
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
    pub fn new(token: u64, project: String, settings: RunSettings) -> Self {
        Run {
            token,
            project,
            settings,
            state: RunState::Preflight,
            session: None,
            batches: 0,
            stopping: false,
            reduced: None,
            summary: None,
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
        } else {
            // A summary belongs to an ending and to nothing else. Clearing it
            // here is what makes "None in every state but Stopped" a property
            // of the type rather than a habit of the one caller that fills it.
            self.summary = None;
        }
        self.state = state;
    }

    /// Take on where the loop says it has got to. Everything the loop knows is
    /// adopted; `stopping` is the one field it does not know, because stop is
    /// asked for on this side and never travels to the loop task at all — its
    /// copy carries `false` whatever the truth is. Overwriting with that is how
    /// a stop gets lost: the run then reads as unstopped to `may_start_batch`,
    /// which is the check standing between it and another batch (smetana-0kb).
    pub fn adopt(&mut self, reported: Run) {
        let asked_to_stop = self.stopping;
        *self = reported;
        self.stopping |= asked_to_stop;
    }

    /// Take the account of itself a run that was stopped from this side could
    /// not have made, and answer whether anything was taken.
    ///
    /// `request_stop` ends a run with nothing in flight **at once** — that is
    /// what makes the button immediate and what lets it reach a paused run — so
    /// the worker's copy reaches `Stopped` before the loop task has looked at
    /// the stop channel. Only the loop runs `finish`, and `finish` is the one
    /// place a summary is ever made, so the account arrives afterwards, in a
    /// report `adopt` must not take: a stopped run is never revived, and the
    /// loop's copy would put a live state back on the screen.
    ///
    /// Hence this, which is `adopt`'s opposite number and deliberately much
    /// narrower — **the summary and nothing else.** The ending does not travel
    /// with it: somebody pressed stop and was told `Cancelled`, while the loop
    /// may have got as far as reading the board and finding the queue empty a
    /// moment later, and rewriting the reason under them would be a different
    /// run's story. Without this the document sits on disk correctly written
    /// and the `Run` on the wire says there is none — for stop-while-deciding,
    /// stop-while-paused and stop-during-preflight, which is the whole of what
    /// the immediate branch exists for.
    pub fn take_summary_from(&mut self, reported: Run) -> bool {
        if !self.is_over() || self.summary.is_some() {
            return false;
        }
        // A summary is only ever set beside a `Stopped`, and `advance` clears
        // it on every other transition, so its presence is the whole evidence
        // that this report is an ending carrying an account.
        match reported.summary {
            Some(summary) => {
                self.summary = Some(summary);
                true
            }
            None => false,
        }
    }

    /// May another batch go out? Everything a stop has touched says no: a run
    /// already over, and — the half that is easy to miss — one that was asked
    /// to stop and is still finishing the batch in flight. "The batch in flight
    /// finishes" has always meant that one and no more, and the loop's own
    /// reading of the stop channel cannot enforce it: a stop landing just after
    /// it looks starts a whole further round, board read and all, and by the
    /// time the loop looks again a second batch is out and merging (smetana-0kb).
    pub fn may_start_batch(&self) -> bool {
        !self.is_over() && !self.stopping
    }

    /// Stop was asked for. The batch in flight finishes: a run interrupted
    /// between a merge and a close is exactly the state the recovery phase
    /// exists to clean up, and killing a session mid-merge is how you get there
    /// deliberately. A run that has not started a batch yet stops at once.
    ///
    /// `starting` is the one fact this struct cannot hold for itself: a batch
    /// the loop has been cleared to spawn and has not reported yet, so
    /// `session` is still `None` and a batch is on its way regardless. It has
    /// to count as a batch in flight, or a stop landing in that window ends the
    /// run on the screen while the batch it did not know about runs on and
    /// merges (smetana-0kb). Who knows that is the worker, which is why it is a
    /// parameter and not a field: it is true for a window measured in the time
    /// between two messages, and a serialized field would be a second copy of
    /// it for the front end to read wrong.
    pub fn request_stop(&mut self, starting: bool) {
        if self.is_over() {
            return;
        }
        self.stopping = true;
        if self.session.is_none() && !starting {
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
        let mut run = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        run.advance(RunState::Stopped { reason: StopReason::QueueEmpty });
        run.advance(RunState::Working { iteration: 7 });
        assert_eq!(run.state, RunState::Stopped { reason: StopReason::QueueEmpty });
    }

    #[test]
    fn stopping_forgets_the_session() {
        let mut run = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        run.session = Some(3);
        run.advance(RunState::Stopped { reason: StopReason::Cancelled });
        assert_eq!(run.session, None, "a row pointing at a dead session is worse than no row");
    }

    #[test]
    fn stop_between_batches_is_immediate_and_stop_mid_batch_waits() {
        let mut idle = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        idle.request_stop(false);
        assert!(idle.is_over(), "nothing is in flight, so there is nothing to wait for");

        let mut working = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        working.session = Some(1);
        working.advance(RunState::Working { iteration: 0 });
        working.request_stop(false);
        assert!(working.stopping);
        assert!(!working.is_over(), "the batch in flight finishes — see request_stop");
    }

    #[test]
    fn stop_while_a_batch_is_starting_waits_like_one_already_in_flight() {
        // The window between the loop being cleared to spawn and the session
        // id coming back: `session` is still None and a batch is on its way
        // regardless. Reading that as "nothing in flight" is smetana-0kb — the
        // run reads as stopped everywhere while the batch runs and merges.
        let mut starting = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        starting.advance(RunState::Deciding);
        starting.request_stop(true);
        assert!(starting.stopping);
        assert!(!starting.is_over(), "a batch is on its way, so this stop waits for it");

        // And the flag alone is what changed: the same run without it stops on
        // the spot, which is what makes the stop button reach a paused run.
        let mut idle = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        idle.advance(RunState::Deciding);
        idle.request_stop(false);
        assert!(idle.is_over());
    }

    #[test]
    fn a_report_from_the_loop_cannot_unask_a_stop() {
        // The loop's copy of the run was cloned before anybody pressed
        // anything and never hears about it, so it reports `stopping: false`
        // for the rest of its life. Adopting that wholesale hands the next
        // batch a run that looks unstopped.
        let mut worker = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        worker.session = Some(1);
        worker.advance(RunState::Working { iteration: 0 });
        worker.request_stop(false);

        let mut loops_own = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        loops_own.advance(RunState::Deciding);
        assert!(!loops_own.stopping);

        worker.adopt(loops_own);
        assert_eq!(worker.state, RunState::Deciding, "where the loop is, is the loop's to say");
        assert!(worker.stopping, "and whether it was asked to stop is not");
        assert!(!worker.may_start_batch());
    }

    #[test]
    fn a_run_that_was_asked_to_stop_starts_no_further_batch() {
        let mut run = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        run.session = Some(1);
        run.advance(RunState::Working { iteration: 0 });
        assert!(run.may_start_batch(), "nothing has been asked of it yet");

        run.request_stop(false);
        assert!(!run.is_over(), "the batch in flight finishes");
        assert!(
            !run.may_start_batch(),
            "that batch and no more — the next round must not start another"
        );

        // And a run that is over is refused for the plainer reason.
        let mut done = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        done.advance(RunState::Stopped { reason: StopReason::QueueEmpty });
        assert!(!done.may_start_batch());
    }

    #[test]
    fn a_second_stop_changes_nothing_about_a_run_already_stopped() {
        // The entry now outlives the stop that ended it — it stays in the
        // worker's map until the loop task is gone — so the stop button can be
        // pressed again against a run that is already over.
        let mut run = Run::new(1, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        run.request_stop(false);
        run.request_stop(true);
        assert_eq!(run.state, RunState::Stopped { reason: StopReason::Cancelled });
    }

    #[test]
    fn a_stop_reason_reaches_the_front_end_as_a_kind_and_its_detail() {
        let json = serde_json::to_value(StopReason::Crashed { attempts: 5 }).expect("serialize");
        assert_eq!(json["kind"], "crashed");
        assert_eq!(json["attempts"], 5);

        let json = serde_json::to_value(StopReason::QueueEmpty).expect("serialize");
        assert_eq!(json["kind"], "queue_empty");

        // The front end keys `stopReason.js` on this exact string.
        let json = serde_json::to_value(StopReason::BatchDone).expect("serialize");
        assert_eq!(json["kind"], "batch_done");
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
    fn only_a_supervised_run_is_one_batch_and_no_more() {
        // Auto is the loop that empties the queue; solo is one task, which the
        // queue itself ends when the task closes.
        assert!(RunMode::Supervised.one_batch());
        assert!(!RunMode::Auto.one_batch());
        assert!(!RunMode::Solo.one_batch());
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
    fn the_first_question_costs_the_batch_and_the_same_one_again_costs_the_run() {
        // The park-and-continue half and the two-in-a-row stop, which are the
        // whole of what smetana-8pe changes: a lead stuck at a dialog loses its
        // batch, and only a dialog that stands in front of every batch — the
        // same question twice running — ends the run.
        let mut questions = RepeatedQuestion::default();
        assert_eq!(questions.ended_by("Do you trust this directory?"), OnQuestion::Park);
        assert_eq!(
            questions.ended_by("Do you trust this directory?"),
            OnQuestion::Stop,
            "a machine that cannot start needs a person, not more batches"
        );
    }

    #[test]
    fn a_different_question_is_not_the_spin() {
        // The machine got past the first dialog, so batches are getting
        // somewhere: each new question is again worth one batch, however many
        // there are.
        let mut questions = RepeatedQuestion::default();
        assert_eq!(questions.ended_by("Run this command?"), OnQuestion::Park);
        assert_eq!(questions.ended_by("Apply this patch?"), OnQuestion::Park);
        assert_eq!(questions.ended_by("Run this command?"), OnQuestion::Park);
    }

    #[test]
    fn a_batch_that_ended_some_other_way_breaks_the_row() {
        // "In a row" is literal: a completed batch between two identical
        // questions proves the dialog was not standing in front of everything.
        let mut questions = RepeatedQuestion::default();
        assert_eq!(questions.ended_by("Do you trust this directory?"), OnQuestion::Park);
        questions.cleared();
        assert_eq!(questions.ended_by("Do you trust this directory?"), OnQuestion::Park);
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
    fn a_run_reaches_the_front_end_with_its_token() {
        // The token is how the front end tells one run's event from another's
        // now that a project holds several; a `Run` that serialized without it
        // would leave every event unattributable.
        let run = Run::new(7, "/p".into(), settings(RunMode::Auto, RunScope::Queue));
        let json = serde_json::to_value(&run).expect("serialize");
        assert_eq!(json["token"], 7);
    }

    #[test]
    fn a_scope_describes_itself_the_way_the_refusal_needs_it() {
        assert_eq!(RunScope::Queue.describe(), "the queue");
        assert_eq!(RunScope::Task { id: "smetana-1".into() }.describe(), "task smetana-1");
        assert_eq!(RunScope::Epic { id: "smetana-2".into() }.describe(), "epic smetana-2");
    }

    #[test]
    fn the_refusal_reaches_the_front_end_with_the_scope_it_names() {
        // `runFailure` in DesktopApp.vue reads `kind` and `detail.scope` off
        // this exact shape to compose its own sentence.
        let json = serde_json::to_value(RunError::AlreadyRunning { scope: "the queue".into() })
            .expect("serialize");
        assert_eq!(json["kind"], "already_running");
        assert_eq!(json["detail"]["scope"], "the queue");
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
