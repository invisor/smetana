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
    pub fn new(id: SessionId, agent: &str, cwd: &str, project: &str) -> Self {
        Self {
            id,
            agent: agent.to_owned(),
            cwd: cwd.to_owned(),
            project: project.to_owned(),
            state: SessionState::Starting,
            question: None,
            started_at: chrono::Utc::now().to_rfc3339(),
            exit_code: None,
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
        Session::new(1, "claude", "/p", "/p")
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
