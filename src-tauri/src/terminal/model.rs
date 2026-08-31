//! Terminal vocabulary and pure transition rules. No I/O here — that lives in
//! pty.rs, scheduling lives in service.rs.

pub type SessionId = u64;

/// The name a run's session carries in bd's audit trail. `pty.rs` puts it in
/// the environment as `BEADS_ACTOR`, and the runs worker derives the same
/// string to ask the board what that session claimed — one function so the two
/// cannot drift, because a drifted copy fails silently: the list comes back
/// empty and a parked batch's claims stay `in_progress` forever.
pub fn run_actor(session: SessionId) -> String {
    format!("smetana-run-{session}")
}

/// Session states. What goes out to the front end is a translation to the
/// design system's statuses, done by the store: `running` → running,
/// `needs-you` → needs-you, `idle` → ready, `exited` → done or failed
/// depending on the exit code.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Starting,
    Running,
    Idle,
    NeedsYou,
    Exited,
}

/// One answer option. `send` is what goes into the PTY: for one CLI that's a
/// digit followed by a newline, for another it's arrow keys and Enter. The
/// profile knows which, not the panel — otherwise the panel would have to
/// choose between them.
#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QuestionOption {
    pub label: String,
    pub send: String,
}

#[derive(Clone, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Question {
    pub text: String,
    pub options: Vec<QuestionOption>,
    /// What is highlighted on screen right now.
    pub selected: Option<usize>,
}

/// What a session was started to do, in as much detail as the two places that
/// draw it need: the row in the agents panel, which names the work rather than
/// the process, and the panel on the right, which opens that work when the row
/// is picked.
///
/// One variant per `Intent`, and deliberately not the `Intent` itself: that
/// carries the paths of the images a person attached and a run's entire
/// settings, and neither is ever drawn. **What crosses the boundary here is
/// what gets drawn, and nothing else.**
///
/// That rule is why `NewTask` carries the person's own prose, which it did not
/// before. A filed task becomes a card and the card is what the right panel
/// opens; a task still being filed has no card and no id — the agent has not
/// run `bd create` yet, and when it does, the issue arrives through the watcher
/// with nothing tying it back to this session. So the draft the dialog
/// collected is the only thing there is to show for a filing agent, and this is
/// the only copy of it on the front end. It rides here rather than in a map
/// beside the start ticket because a start becomes a session about a second
/// later: carried here it survives that handover for free, and the placeholder
/// row and the session's own row draw the same words.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SessionWork {
    Bare,
    /// The draft the new-task dialog collected, read back read-only. Not an
    /// issue: it has no status, nothing deletes it, and nothing edits it here.
    ///
    /// The `rename_all` on the enum above renames the *variants*; a struct
    /// variant's fields need their own, which is what this one is for. The
    /// front end reads `text`, `issueType`, `priority` and `parent`.
    #[serde(rename_all = "camelCase")]
    NewTask {
        text: String,
        /// `None` is the dialog's Auto, the same invariant `TaskDraft` holds:
        /// Auto travels as absence and never as the word, so the panel draws it
        /// as Auto rather than inventing a type bd would reject.
        issue_type: Option<String>,
        priority: Option<u8>,
        /// The issue this is a follow-up to, or `None` for an ordinary filing.
        /// The panel draws it as an identifier, the way `EditTask` draws its
        /// own id: a filing agent's draft would otherwise be silent about the
        /// one thing that makes this task different from any other.
        parent: Option<String>,
    },
    /// The issue being edited, by id rather than by title: the panel draws it
    /// as an identifier, and a title would not fit a row anyway. The right
    /// panel looks the issue itself up by this id.
    EditTask { id: String },
    /// The parked issue whose open questions this session is asking about.
    /// Its own variant rather than an `EditTask`, because a row is captioned by
    /// the work and this is different work: an edit is a person's own change to
    /// an issue, this is a run's unanswered question being put to them.
    ResolveTask { id: String },
    /// The closed issue whose finished work is being corrected. By id, like
    /// the two above and for the same reason: a row draws an identifier, and
    /// the right panel looks the issue itself up by this id.
    ///
    /// Its own variant rather than an `EditTask` for the reason the caption
    /// gives: an edit changes what the task says, this changes what was built
    /// for it, and a row that called both "Editing" would name the wrong one
    /// over a session rewriting sources.
    FixTask { id: String },
    /// A conflicted working tree the Git panel produced, and the merge or
    /// rebase that is to be finished in it. The only work in this list that is
    /// about a repository rather than about an issue, which is why it carries a
    /// path: a project can hold several, and "resolving a conflict" without
    /// saying where would name none of them.
    ///
    /// The conflicted paths stay behind, the way `NewTask`'s images do: they
    /// are the agent's briefing, a row has nowhere to draw a dozen of them, and
    /// the list is out of date the moment the agent resolves the first one.
    ResolveConflict { repo: String, theirs: String },
    /// A tracker the app's own repair could not fix, handed to an agent whole.
    ///
    /// It carries nothing, and there is nothing it could carry: the failure —
    /// the folder, the bd version, the command and its stderr — is a briefing
    /// for the agent, and the row it would be drawn in is 252px wide. The
    /// project is already named by the panel the row sits in, so the caption
    /// alone says what this session is for.
    RepairTracker,
    /// A Claude Code session read off disk and started again, and the one work
    /// in this list with nothing of this app's behind it: no issue, no
    /// repository, no run — the conversation existed before this window did.
    ///
    /// It carries the session's title and neither its id nor its working
    /// directory, and that is the same reading `NewTask` takes of a draft: what
    /// crosses this boundary is what gets drawn. A row is 252px wide, the id is
    /// a 36-character UUID and the directory is an absolute path, while the
    /// title is the sentence somebody recognises the conversation by. Both of
    /// the others are already on the card in the Sessions tab, in full.
    ///
    /// `None` is a transcript with no human message in it — a session opened
    /// and abandoned — and the row then says what it is and nothing more,
    /// rather than inventing a name for it.
    ResumeSession { title: Option<String> },
    Setup,
    /// A branch review, by the path its report is written to, and the one work
    /// in this list that names a file which does not exist yet: the two
    /// documents appear when the agent is finished, and this is where the app
    /// looks for them.
    ///
    /// The pairs stay behind, the way `ResolveConflict`'s conflicted paths do.
    /// They are the agent's briefing, and a review of several repositories
    /// carries two refs each — nothing a row 252px wide has anywhere to put.
    ReviewBranch { report: String },
    /// One batch of a run. Which issues it has taken is not known here and
    /// cannot be: the agent claims them by running `bd update --claim` itself,
    /// and nothing reports that back. The front end crosses the run's session
    /// with what the tracker holds in progress.
    Run,
    /// The person's own shell, and the one entry in this list that is not an
    /// agent at all: there is no `Intent` behind it, no profile, and nothing
    /// this app asked it to do.
    ///
    /// It carries nothing, and there is nothing it could carry — a shell is for
    /// whatever gets typed into it. What makes the variant worth existing is
    /// that it is the **only** thing by which the rest of the app tells the two
    /// kinds of session apart: `agentRows` in `src/stores/terminals.js` filters
    /// it out of the agents panel, the presence of the centre's Agent tab is
    /// counted on everything that is not this, and each of these gets a centre
    /// tab of its own instead. The project rail asks the same question of a
    /// `SessionMark`, which carries `WorkKind` rather than this enum.
    Shell,
}

/// Which variant of `SessionWork` a session was started for, and nothing of
/// what that variant carries.
///
/// A second enum rather than a flag, and rather than `SessionWork` itself: what
/// the project rail needs is "is this an agent", but a boolean answering only
/// that would be a second vocabulary for a question the front end already
/// spells one way — `kind === 'shell'`. Mirroring the variants keeps the one
/// wire word, and keeps the door open for a reader that cares which agent.
///
/// The duplication is held two ways and neither covers the other. *Whether* a
/// variant is mapped is the compiler's: `kind()`'s match is exhaustive, and so
/// is `sample` in the tests, so a variant added to either enum does not build.
/// *Which word* a mapped variant gets is
/// `every_work_variant_reports_its_own_kind`'s, against the `kind` tag serde
/// writes for the work itself — but only for the variants in that test's own
/// list, so one added and not appended there has its word asserted by nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkKind {
    Bare,
    NewTask,
    EditTask,
    ResolveTask,
    FixTask,
    ResolveConflict,
    RepairTracker,
    ResumeSession,
    Setup,
    ReviewBranch,
    Run,
    Shell,
}

impl SessionWork {
    /// The variant alone, for the types that cross the boundary carrying what
    /// gets drawn and nothing else.
    pub fn kind(&self) -> WorkKind {
        match self {
            SessionWork::Bare => WorkKind::Bare,
            SessionWork::NewTask { .. } => WorkKind::NewTask,
            SessionWork::EditTask { .. } => WorkKind::EditTask,
            SessionWork::ResolveTask { .. } => WorkKind::ResolveTask,
            SessionWork::FixTask { .. } => WorkKind::FixTask,
            SessionWork::ResolveConflict { .. } => WorkKind::ResolveConflict,
            SessionWork::RepairTracker => WorkKind::RepairTracker,
            SessionWork::ResumeSession { .. } => WorkKind::ResumeSession,
            SessionWork::Setup => WorkKind::Setup,
            SessionWork::ReviewBranch { .. } => WorkKind::ReviewBranch,
            SessionWork::Run => WorkKind::Run,
            SessionWork::Shell => WorkKind::Shell,
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: SessionId,
    pub agent: String,
    pub cwd: String,
    /// The project directory the session belongs to. Matches cwd for as long
    /// as agents spawn at the root, and will diverge once worktrees exist.
    pub project: String,
    pub state: SessionState,
    pub question: Option<Question>,
    pub started_at: String,
    pub exit_code: Option<i32>,
    /// Why this session exists. Fixed at the spawn and never revised — an
    /// agent handed one job does not acquire another halfway through, and a
    /// row whose caption changed under a person would be describing a session
    /// they are no longer looking at.
    pub work: SessionWork,
}

/// What the project rail needs to know about a session, and nothing else: a
/// tile draws one dot, decided by the state — of the sessions the rail counts.
///
/// A separate type rather than `Session`, for the reason `Request::Group`
/// gives about the pid — what crosses the boundary is what gets drawn. Every
/// project's sessions cross here, and `Session` carries `work`, which for a
/// filing agent holds the whole of the person's own draft prose.
///
/// `kind` is why the rail can say *which* sessions it counts: it is the
/// variant of that `work` and none of its payload, so a person's own shell is
/// told from an agent here without the draft prose crossing with it. Without
/// it, a shell that rang the bell lit its project's tile loud while the scope
/// bar's agent counter, which filters, read zero.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMark {
    pub id: SessionId,
    pub project: String,
    pub state: SessionState,
    pub kind: WorkKind,
}

/// How a session ended, as far as whoever was waiting on it is concerned.
///
/// Three answers rather than an `Option<i32>`, and the third is the whole
/// reason this type exists: a session a person took out of the agents panel and
/// a session whose process fell over are the same absence to anyone reading an
/// exit code, and they need opposite responses from a run — one is somebody
/// saying "stop", the other is a harness to retry. The worker is the only place
/// that knows which happened, so it is the worker that says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Exit {
    /// The process exited and this is its code.
    Code(i32),
    /// The process is gone and no code ever arrived before the grace ran out —
    /// in practice it was signalled. A crash, for anybody counting them.
    NoCode,
    /// The session was removed while somebody waited on it. Not a crash: the
    /// process did not fail, a person took it away.
    Removed,
}

#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "message")]
pub enum TerminalError {
    #[error("the agent did not start: {0}")]
    Spawn(String),
    #[error("no session {0}")]
    NoSession(SessionId),
    #[error("the session is waiting for a person's answer")]
    Busy,
    #[error("the session did not answer in the time allowed")]
    Timeout,
    #[error("no agent is installed: looked for {0}")]
    NoAgent(String),
    /// A working directory a session was asked to start in that is not a folder
    /// inside the project: outside the root, gone from disk, or a file. Its own
    /// variant rather than a `Spawn`, because nothing was spawned and nothing
    /// tried to be — the request was refused before any process existed.
    #[error("that folder cannot be a working directory: {0}")]
    BadCwd(String),
    /// A recorded session asked to be picked up again by a harness that cannot
    /// be told to do it — `Profile::resume_args` answered `None`.
    ///
    /// Its own variant for the same reason `BadCwd` is one: nothing was
    /// spawned. Starting the agent anyway is the outcome this refusal exists to
    /// prevent — a fresh session in the worktree, under a card promising the
    /// conversation somebody left. The front end greys the row before anybody
    /// can press it (`resumeAvailability` in
    /// `src/components/agent/sessionMenu.js`); this is the guard standing next
    /// to the spawn, where a rule about spawning belongs.
    #[error("{0} cannot pick a recorded session up by its id")]
    NoResume(String),
    /// The same refusal one verb over: a recorded session asked to be carried
    /// on in a *new* session of its own by a harness that cannot be told to do
    /// it — `Profile::fork_args` answered `None`.
    ///
    /// Its own variant rather than a second use of `NoResume`, because the two
    /// are two capabilities: a harness that reopens a transcript and cannot
    /// branch one is an ordinary shape, and a sentence saying it cannot resume
    /// would be untrue about the row nobody pressed.
    #[error("{0} cannot carry a recorded session on in a new one")]
    NoFork(String),
}

impl Session {
    pub fn new(id: SessionId, agent: &str, cwd: &str, project: &str, work: SessionWork) -> Self {
        Self {
            id,
            agent: agent.to_owned(),
            cwd: cwd.to_owned(),
            project: project.to_owned(),
            state: SessionState::Starting,
            question: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            exit_code: None,
            work,
        }
    }

    /// Exit is final: the process is gone, and no detection logic is entitled
    /// to bring the row back to life — otherwise the list would show as alive
    /// something that has died.
    pub fn finish(&mut self, code: Option<i32>) {
        self.state = SessionState::Exited;
        self.exit_code = code;
        self.question = None;
    }

    pub fn apply(&mut self, state: SessionState, question: Option<Question>) {
        if self.state == SessionState::Exited {
            return;
        }
        self.state = state;
        // A question lives exactly as long as the needs-you state: an agent
        // that went back to work already got its answer to the previous
        // question, and a phrase stuck in the panel would offer to answer it
        // a second time.
        self.question = if state == SessionState::NeedsYou { question } else { None };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        Session::new(1, "claude", "/p", "/p", SessionWork::Bare)
    }

    #[test]
    fn the_work_a_session_was_started_for_serializes_as_the_front_end_reads_it() {
        // The panel switches on `kind` and reads `id` for an edit; both
        // spellings are written out again in `src/stores/terminals.js`, which
        // is the only other place they exist, so this is what catches a rename
        // on one side of the boundary before a row goes blank on the other.
        let json = serde_json::to_string(&SessionWork::EditTask { id: "smetana-42".into() })
            .expect("serializes");
        assert_eq!(json, r#"{"kind":"editTask","id":"smetana-42"}"#);
    }

    #[test]
    fn a_fix_names_its_issue_on_the_wire() {
        // Read in `src/stores/terminals.js` exactly as an edit's is: `workOf`
        // builds the same shape for the placeholder row, and `captionOf`
        // reaches for `id` to put the issue beside "Fixing".
        let json = serde_json::to_string(&SessionWork::FixTask { id: "smetana-42".into() })
            .expect("serializes");
        assert_eq!(json, r#"{"kind":"fixTask","id":"smetana-42"}"#);
    }

    #[test]
    fn a_resumed_session_names_the_conversation_and_nothing_else() {
        // `captionOf` in `src/stores/terminals.js` reads `title` to put the
        // conversation beside "Resumed session", and `workOf` builds this same
        // shape for the second the start ticket is on screen. A rename here
        // goes quiet on the other side: the row keeps drawing and stops saying
        // which session it is.
        let json =
            serde_json::to_string(&SessionWork::ResumeSession { title: Some("Move it".into()) })
                .expect("serializes");
        assert_eq!(json, r#"{"kind":"resumeSession","title":"Move it"}"#);
    }

    #[test]
    fn a_resumed_session_nobody_typed_in_carries_no_title() {
        // A transcript with no human message in it is an ordinary outcome, and
        // absence travels as `null` rather than as an invented name — the same
        // reading `SessionSummary::title` takes one module over.
        let json = serde_json::to_string(&SessionWork::ResumeSession { title: None })
            .expect("serializes");
        assert_eq!(json, r#"{"kind":"resumeSession","title":null}"#);
    }

    #[test]
    fn a_draft_reaches_the_front_end_camel_cased_and_whole() {
        // `issueType`, not `issue_type`: the enum's own `rename_all` renames
        // variants only, so the variant carries a second one for its fields.
        // Drop that attribute and the right panel silently draws Auto over a
        // type the person did choose.
        let json = serde_json::to_string(&SessionWork::NewTask {
            text: "The log drops lines above 10k".into(),
            issue_type: Some("bug".into()),
            priority: Some(1),
            parent: None,
        })
        .expect("serializes");
        assert_eq!(
            json,
            r#"{"kind":"newTask","text":"The log drops lines above 10k","issueType":"bug","priority":1,"parent":null}"#
        );
    }

    #[test]
    fn a_draft_left_on_auto_reaches_the_front_end_as_null() {
        // Auto is absence on this side of the boundary too, and the panel draws
        // the word from the absence. A default substituted anywhere along the
        // way would have the panel claim a choice nobody made.
        let json =
            serde_json::to_string(&SessionWork::NewTask {
                text: "x".into(),
                issue_type: None,
                priority: None,
                parent: None,
            })
            .expect("serializes");
        assert_eq!(
            json,
            r#"{"kind":"newTask","text":"x","issueType":null,"priority":null,"parent":null}"#
        );
    }

    #[test]
    fn a_follow_up_draft_names_its_parent_on_the_front_end() {
        // The right panel draws this as a row of its own, so a rename here goes
        // quiet rather than loud: the row simply stops being drawn.
        let json = serde_json::to_string(&SessionWork::NewTask {
            text: "the tooltip clips".into(),
            issue_type: None,
            priority: None,
            parent: Some("smetana-3uv".into()),
        })
        .expect("serializes");
        assert_eq!(
            json,
            r#"{"kind":"newTask","text":"the tooltip clips","issueType":null,"priority":null,"parent":"smetana-3uv"}"#
        );
    }

    #[test]
    fn a_conflict_reaches_the_front_end_as_the_repository_and_the_branch() {
        // Both spellings are read in `src/stores/terminals.js`, where the row's
        // caption is built: `repo` becomes the folder's name in the caption and
        // `theirs` the branch beside it, so a rename on this side goes quiet
        // rather than loud — the row keeps drawing, saying "Agent".
        let json = serde_json::to_string(&SessionWork::ResolveConflict {
            repo: "/p/backend".into(),
            theirs: "develop".into(),
        })
        .expect("serializes");
        assert_eq!(json, r#"{"kind":"resolveConflict","repo":"/p/backend","theirs":"develop"}"#);
    }

    #[test]
    fn session_mark_serializes_camel_case_with_kebab_state() {
        // The rail reads all three of these names off the wire, and this is a
        // second type carrying the same facts as `Session` — a rename here goes
        // quiet on the other side: every tile's dot falls back to idle and the
        // rail simply looks like an app with nothing running in it.
        let mark = SessionMark {
            id: 7,
            project: "/p".into(),
            state: SessionState::NeedsYou,
            kind: WorkKind::Run,
        };
        let json = serde_json::to_string(&mark).expect("serializes");
        assert_eq!(json, r#"{"id":7,"project":"/p","state":"needs-you","kind":"run"}"#);
    }

    #[test]
    fn a_shell_reports_the_shell_kind_and_an_agent_does_not() {
        // The whole of what lets the rail leave a person's own shell out of a
        // project's dots. Both directions, because a field that answered
        // "shell" for everything would pass a one-sided assertion and turn
        // every tile grey.
        let shell = SessionWork::Shell.kind();
        assert_eq!(shell, WorkKind::Shell);
        for agent in
            [SessionWork::Bare, SessionWork::Run, SessionWork::Setup, SessionWork::EditTask { id: "x".into() }]
        {
            assert_ne!(agent.kind(), WorkKind::Shell, "an agent's work is not a shell: {agent:?}");
        }
    }

    /// One work per kind, and the match is exhaustive on purpose: with `kind()`
    /// exhaustive over `SessionWork` and this over `WorkKind`, a variant added
    /// to either enum stops the test file compiling, which is what gets the
    /// next person to the list below rather than past it.
    fn sample(kind: WorkKind) -> SessionWork {
        match kind {
            WorkKind::Bare => SessionWork::Bare,
            WorkKind::NewTask => {
                SessionWork::NewTask {
                    text: "x".into(),
                    issue_type: None,
                    priority: None,
                    parent: None,
                }
            }
            WorkKind::EditTask => SessionWork::EditTask { id: "smetana-42".into() },
            WorkKind::ResolveTask => SessionWork::ResolveTask { id: "smetana-42".into() },
            WorkKind::FixTask => SessionWork::FixTask { id: "smetana-42".into() },
            WorkKind::ResolveConflict => {
                SessionWork::ResolveConflict { repo: "/p".into(), theirs: "develop".into() }
            }
            WorkKind::RepairTracker => SessionWork::RepairTracker,
            WorkKind::ResumeSession => {
                SessionWork::ResumeSession { title: Some("Move the card to done".into()) }
            }
            WorkKind::Setup => SessionWork::Setup,
            WorkKind::ReviewBranch => {
                SessionWork::ReviewBranch { report: ".smetana/reviews/2026-08-31-main".into() }
            }
            WorkKind::Run => SessionWork::Run,
            WorkKind::Shell => SessionWork::Shell,
        }
    }

    #[test]
    fn every_work_variant_reports_its_own_kind() {
        // `WorkKind` duplicates `SessionWork`'s variants, and this is what
        // holds the words together: the `kind` tag serde writes for a work has
        // to be what `kind()` serializes to, or a shell reaches the rail as
        // somebody else's kind.
        //
        // What this cannot see is a variant nobody added to the list below.
        // The two exhaustive matches make an *unmapped* variant a compile
        // error, which is what brings the next person to this file; appending
        // it here is still theirs to do. A wrong arm in a listed variant is
        // what fails here.
        let all = [
            WorkKind::Bare,
            WorkKind::NewTask,
            WorkKind::EditTask,
            WorkKind::ResolveTask,
            WorkKind::FixTask,
            WorkKind::ResolveConflict,
            WorkKind::RepairTracker,
            WorkKind::ResumeSession,
            WorkKind::Setup,
            WorkKind::ReviewBranch,
            WorkKind::Run,
            WorkKind::Shell,
        ];
        for kind in all {
            let work = sample(kind);
            let tagged = serde_json::to_value(&work).expect("serializes");
            let tag = tagged.get("kind").expect("the work carries its variant as `kind`");
            let narrow = serde_json::to_value(work.kind()).expect("serializes");
            // The words first, so that each fault is reported by the message
            // that names it. A wrong arm in `kind()` shows up here as two
            // different words; a wrong arm in `sample` cannot be seen here at
            // all — both words are then taken from the same substituted work —
            // and falls through to the line below.
            assert_eq!(tag, &narrow, "the narrow kind disagrees with the work's own tag: {work:?}");
            assert_eq!(work.kind(), kind, "the sample for {kind:?} is another variant's work");
        }
    }

    #[test]
    fn a_new_session_starts() {
        assert_eq!(session().state, SessionState::Starting);
    }

    #[test]
    fn nothing_revives_a_session_that_has_exited() {
        let mut s = session();
        s.finish(Some(0));
        s.apply(SessionState::Running, None);
        assert_eq!(s.state, SessionState::Exited);
        assert_eq!(s.exit_code, Some(0));
    }

    #[test]
    fn an_answer_clears_the_question() {
        let mut s = session();
        let q = Question {
            text: "Do you want to proceed?".into(),
            options: vec![QuestionOption { label: "Yes".into(), send: "1\r".into() }],
            selected: Some(0),
        };
        s.apply(SessionState::NeedsYou, Some(q));
        assert!(s.question.is_some());
        s.apply(SessionState::Running, None);
        assert!(s.question.is_none(), "the question does not survive the return to work");
    }
}
