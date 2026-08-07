//! Terminal vocabulary and pure transition rules. No I/O here — that lives in
//! pty.rs, scheduling lives in service.rs.

pub type SessionId = u64;

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

/// What a session was started to do, in as much detail as a row in the agents
/// panel needs to name the work rather than the process.
///
/// One variant per `Intent`, and deliberately not the `Intent` itself: that
/// carries the whole of what a person typed, the paths of the images they
/// attached and a run's entire settings. None of it is a caption, and what
/// crosses the boundary here is only what gets drawn.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SessionWork {
    Bare,
    NewTask,
    /// The issue being edited, by id rather than by title: the panel draws it
    /// as an identifier, and a title would not fit a row anyway.
    EditTask { id: String },
    Setup,
    /// One batch of a run. Which issues it has taken is not known here and
    /// cannot be: the agent claims them by running `bd update --claim` itself,
    /// and nothing reports that back. The front end crosses the run's session
    /// with what the tracker holds in progress.
    Run,
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
        assert_eq!(
            serde_json::to_string(&SessionWork::NewTask).expect("serializes"),
            r#"{"kind":"newTask"}"#
        );
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
