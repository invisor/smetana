//! Словарь терминала и чистые правила переходов. Ввода-вывода здесь нет —
//! он в pty.rs, планирование в service.rs.

pub type SessionId = u64;

/// Состояния сессии. Наружу, во фронт, едет перевод в статусы дизайн-системы,
/// и делает его стор: `running` → running, `needs-you` → needs-you,
/// `idle` → ready, `exited` → done или failed по коду возврата.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Starting,
    Running,
    Idle,
    NeedsYou,
    Exited,
}

/// Вариант ответа. `send` — то, что уйдёт в PTY: у одного CLI это цифра с
/// переводом строки, у другого стрелки и Enter. Знает это профиль, а не
/// панель, иначе панели пришлось бы выбирать между ними.
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
    /// Что подсвечено на экране прямо сейчас.
    pub selected: Option<usize>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: SessionId,
    pub agent: String,
    pub cwd: String,
    /// Каталог проекта, которому принадлежит сессия. Совпадает с cwd, пока
    /// агент запускается в корне, и разойдётся, когда появятся worktree.
    pub project: String,
    pub state: SessionState,
    pub question: Option<Question>,
    pub started_at: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "message")]
pub enum TerminalError {
    #[error("агент не запустился: {0}")]
    Spawn(String),
    #[error("сессии {0} нет")]
    NoSession(SessionId),
    #[error("сессия ждёт ответа человека")]
    Busy,
    #[error("сессия не ответила за отведённое время")]
    Timeout,
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

    /// Выход финален: процесса больше нет, и никакое распознавание не вправе
    /// вернуть строку в работу — иначе список показал бы живым того, кто умер.
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
        // Вопрос живёт ровно столько, сколько состояние needs-you: агент,
        // вернувшийся к работе, на прошлый вопрос уже получил ответ, а
        // застрявшая в панели фраза предлагала бы отвечать второй раз.
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
    fn новая_сессия_запускается() {
        assert_eq!(session().state, SessionState::Starting);
    }

    #[test]
    fn вышедшую_сессию_ничто_не_оживляет() {
        let mut s = session();
        s.finish(Some(0));
        s.apply(SessionState::Running, None);
        assert_eq!(s.state, SessionState::Exited);
        assert_eq!(s.exit_code, Some(0));
    }

    #[test]
    fn ответ_гасит_вопрос() {
        let mut s = session();
        let q = Question {
            text: "Do you want to proceed?".into(),
            options: vec![QuestionOption { label: "Yes".into(), send: "1\r".into() }],
            selected: Some(0),
        };
        s.apply(SessionState::NeedsYou, Some(q));
        assert!(s.question.is_some());
        s.apply(SessionState::Running, None);
        assert!(s.question.is_none(), "вопрос не переживает возврат в работу");
    }
}
