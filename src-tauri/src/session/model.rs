//! The vocabulary of a driven session and the pure rules over it. No I/O here:
//! the process lives in `service.rs`, the codec in a driver.
//!
//! Every type here is what the front end sees, so the serde shape is part of
//! the contract rather than an implementation detail.

pub type SessionId = u64;

/// Whose turn produced this.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Actor {
    Person,
    Agent,
}

/// What a person may answer a permission request with. The list an agent
/// actually offers travels on the request, because not every harness offers
/// all three.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Decision {
    Allow,
    /// Allow this tool for the rest of the session without asking again.
    AllowAlways,
    Deny,
}

/// The closed list. A driver maps its harness's protocol onto exactly this and
/// the front end never learns which harness it was talking to.
///
/// **An event type a driver does not recognise produces no event.** That is the
/// choice `agents::claude::transcript_line` already made, for the reason it
/// records: a missing row costs a person nothing the CLI's own logs do not
/// still hold, while a wall of raw protocol costs them the panel.
#[derive(Clone, PartialEq, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EventKind {
    TurnStart { by: Actor },
    UserMessage { text: String, attachments: Vec<String> },
    /// Markdown as the agent wrote it. Rendering is the front end's business.
    Text { text: String },
    Reasoning { text: String },
    ToolUse { id: String, name: String, detail: String },
    ToolResult { id: String, ok: bool, summary: String },
    Permission { id: String, tool: String, detail: String, options: Vec<Decision> },
    PermissionAnswered { id: String, decision: Decision },
    Result { tokens_in: u64, tokens_out: u64, cost_usd: Option<f64>, ms: u64 },
    Error { text: String },
}

// No `rename_all` here on purpose: `kind` is flattened in, and its fields go
// on the wire as `tokens_in` and `cost_usd`. A casing declared on this struct
// would apply to its own fields alone, so the first field added beside `seq`
// would arrive as `sessionId` next to `tokens_in` and the object would carry
// two conventions with nothing to say which one is right.
#[derive(Clone, PartialEq, Debug, serde::Serialize)]
pub struct Event {
    pub seq: u64,
    /// RFC 3339, minted by the worker when the event is appended.
    pub at: String,
    #[serde(flatten)]
    pub kind: EventKind,
}

/// Translated by the store into the design system's statuses, the same way
/// `terminal::model::SessionState` is.
#[derive(Clone, Copy, PartialEq, Eq, Debug, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SessionState {
    Starting,
    Running,
    Ready,
    NeedsYou,
    Exited,
    Failed,
}

#[derive(Clone, Debug, thiserror::Error, serde::Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "message")]
pub enum SessionError {
    #[error("the session could not be started: {0}")]
    Spawn(String),
    #[error("there is no session {0}")]
    NoSuchSession(SessionId),
    #[error("there is no question {0} waiting for an answer")]
    NoSuchQuestion(String),
}

/// The whole of what a session's state is: a fold over its journal.
///
/// There is no bell here and no silence timer. Both existed to guess at what
/// the wire now says outright, and neither is reimplemented.
///
/// The fold reads forward, so it takes the journal's order as true: a
/// `PermissionAnswered` appended ahead of the `Permission` it answers would
/// leave the question standing here, while `journal::Journal::trim`'s answered
/// set is order-free and would already count it settled. Nothing produces that
/// today because the worker is the one place where both the stream reader and
/// the permission listener append — whoever writes a third producer has to keep
/// it that way.
pub fn state_of(events: &[Event], child_alive: bool) -> SessionState {
    let mut open_turn = false;
    let mut pending: Vec<&str> = Vec::new();
    let mut seen_anything = false;
    for event in events {
        seen_anything = true;
        match &event.kind {
            EventKind::TurnStart { .. } => open_turn = true,
            EventKind::Result { .. } => open_turn = false,
            EventKind::Permission { id, .. } => pending.push(id),
            // By id rather than by count: an answer that arrives out of order
            // must settle its own question and leave the others standing.
            EventKind::PermissionAnswered { id, .. } => pending.retain(|open| open != id),
            _ => {}
        }
    }
    if !child_alive {
        // A turn still open when the process went is the honest reading of a
        // crash; a turn closed is an ordinary end.
        return if open_turn { SessionState::Failed } else { SessionState::Exited };
    }
    // A question outranks a turn in flight: the agent is not working, it waits.
    if !pending.is_empty() {
        return SessionState::NeedsYou;
    }
    if open_turn {
        return SessionState::Running;
    }
    if seen_anything {
        SessionState::Ready
    } else {
        SessionState::Starting
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(seq: u64, kind: EventKind) -> Event {
        Event { seq, at: "2026-09-10T12:00:00Z".into(), kind }
    }

    fn permission(id: &str) -> EventKind {
        EventKind::Permission {
            id: id.into(),
            tool: "Bash".into(),
            detail: "rm -rf /tmp/x".into(),
            options: vec![Decision::Allow, Decision::Deny],
        }
    }

    #[test]
    fn an_unanswered_permission_is_the_loudest_thing_a_journal_can_say() {
        // It outranks a turn in flight: the agent is not working, it is waiting.
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Agent }),
            ev(2, permission("p1")),
        ];
        assert_eq!(state_of(&events, true), SessionState::NeedsYou);
    }

    #[test]
    fn a_permission_that_was_answered_is_not_a_question_any_more() {
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Agent }),
            ev(2, permission("p1")),
            ev(3, EventKind::PermissionAnswered { id: "p1".into(), decision: Decision::Allow }),
        ];
        assert_eq!(state_of(&events, true), SessionState::Running);
    }

    #[test]
    fn two_questions_are_settled_one_at_a_time() {
        // Answering the first leaves the second standing. Matching by id rather
        // than by count is what makes an out-of-order answer harmless.
        let events = vec![
            ev(1, permission("p1")),
            ev(2, permission("p2")),
            ev(3, EventKind::PermissionAnswered { id: "p2".into(), decision: Decision::Deny }),
        ];
        assert_eq!(state_of(&events, true), SessionState::NeedsYou);
    }

    #[test]
    fn a_turn_closed_by_its_result_leaves_the_session_ready() {
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Agent }),
            ev(2, EventKind::Text { text: "done".into() }),
            ev(3, EventKind::Result { tokens_in: 10, tokens_out: 20, cost_usd: None, ms: 400 }),
        ];
        assert_eq!(state_of(&events, true), SessionState::Ready);
    }

    #[test]
    fn a_child_that_died_with_a_turn_open_failed_rather_than_finished() {
        let events = vec![ev(1, EventKind::TurnStart { by: Actor::Agent })];
        assert_eq!(state_of(&events, false), SessionState::Failed);
    }

    #[test]
    fn a_child_that_died_between_turns_simply_ended() {
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Agent }),
            ev(2, EventKind::Result { tokens_in: 1, tokens_out: 1, cost_usd: None, ms: 1 }),
        ];
        assert_eq!(state_of(&events, false), SessionState::Exited);
    }

    #[test]
    fn a_session_that_has_produced_nothing_yet_is_starting() {
        assert_eq!(state_of(&[], true), SessionState::Starting);
    }
}
