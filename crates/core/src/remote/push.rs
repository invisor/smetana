//! The desktop's half of push: when a run ends or an agent starts waiting for a person, every paired
//! phone that registered a push token is sent a short message, encrypted to the key it paired with.
//!
//! The gateway (`server/push-gateway`) is the only party in between and sees a device token, the time
//! and the size of a blob — never the text. Delivery is best effort and says so: one failed request is
//! one log line without the token or the content, with no retry and no queue. The run report is still
//! on the desktop and the phone shows it when it next connects, so a lost push costs a nudge and
//! nothing else.
//!
//! This file is what is said and when: the messages, the rule that decides a session has started
//! waiting, and the `notify_*` calls the workers make through the context's sink. Sending — sealing
//! the message to each paired device's key and posting it to the gateway — needs the device list and
//! so lives beside the endpoint, in the desktop shell's `remote/push.rs`.
//!
//! Sending is spawned off the path that produced the event. A run's worker and a session's worker are
//! the places this is called from, and neither may wait on a network that is down.

use std::path::Path;

use serde::Serialize;

use crate::context::Context;
use crate::runs::summary::RunSummary;
use crate::session::model::SessionState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    RunFinished,
    NeedsYou,
}

/// The plaintext of a blob: what the phone decrypts and shows. `target` is the string the phone opens a
/// screen by, `run:<project>` or `session:<id>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub kind: Kind,
    pub project: String,
    pub title: String,
    pub body: String,
    pub target: String,
}

fn project_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The message for a run that has just ended. `tasks` being `None` means the board could not be diffed,
/// which is not the same as nothing moving, so the body does not claim a count.
pub fn run_finished(project_path: &str, summary: Option<&RunSummary>) -> Message {
    let project = project_name(project_path);
    let body = match summary.and_then(|summary| summary.tasks.as_ref()) {
        Some(tasks) => format!(
            "{project}: {} closed, {} waiting for you",
            count(tasks.closed.len(), "task"),
            tasks.parked.len()
        ),
        None => format!("{project}: the run has ended"),
    };
    Message {
        kind: Kind::RunFinished,
        title: "Run finished".into(),
        body,
        target: format!("run:{project_path}"),
        project,
    }
}

/// The message for a session that has started waiting. The body names the agent, and the session's title
/// when it has one.
pub fn needs_you(project_path: &str, session: u64, agent: &str, title: Option<&str>) -> Message {
    needs_you_at(project_path, format!("session:{session}"), agent, title)
}

/// The same message for a terminal (PTY) session. Only the target differs: terminal ids and conversation
/// ids are separate counters that both start at 1, so `session:3` would open somebody else's conversation.
pub fn terminal_needs_you(project_path: &str, terminal: u64, agent: &str) -> Message {
    needs_you_at(project_path, format!("terminal:{terminal}"), agent, None)
}

fn needs_you_at(project_path: &str, target: String, agent: &str, title: Option<&str>) -> Message {
    let project = project_name(project_path);
    let body = match title.filter(|title| !title.is_empty()) {
        Some(title) => format!("{project}: {agent} on \"{title}\""),
        None => format!("{project}: {agent}"),
    };
    Message {
        kind: Kind::NeedsYou,
        title: "An agent is waiting for you".into(),
        body,
        target,
        project,
    }
}

/// Whether this change of state is the start of a wait. One push per transition, not per event: while a
/// session stays in `NeedsYou` — a second permission queued behind the first, a text delta, a journal
/// event of any kind — the state does not change and this is false.
pub fn waiting_began(before: SessionState, after: SessionState) -> bool {
    after == SessionState::NeedsYou && before != SessionState::NeedsYou
}

/// Whether a run report moved the run into its ended state. `was_over` is what the worker held before
/// the report, `None` when it held nothing under that token. A stop with nothing in flight ends a run on
/// the worker's own side before any report, and is the person's own act: it is not announced.
pub fn run_ended(was_over: Option<bool>, is_over: bool) -> bool {
    is_over && was_over == Some(false)
}

pub fn notify_run_finished(ctx: &Context, run: &crate::runs::model::Run) {
    ctx.push(run_finished(&run.project, run.summary.as_ref()));
}

pub fn notify_needs_you(ctx: &Context, session: u64, project: &str, agent: &str, title: Option<&str>) {
    ctx.push(needs_you(project, session, agent, title));
}

pub fn notify_terminal_needs_you(ctx: &Context, terminal: u64, project: &str, agent: &str) {
    ctx.push(terminal_needs_you(project, terminal, agent));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runs::summary::{TaskLine, Tasks};
    use crate::session::model::{state_of, Event, EventKind};

    fn summary(closed: usize, parked: usize) -> RunSummary {
        let line = |n: usize| TaskLine { id: format!("t-{n}"), title: format!("task {n}") };
        RunSummary {
            seconds: 60,
            tasks: Some(Tasks { closed: (0..closed).map(line).collect(), parked: (0..parked).map(line).collect() }),
            report: None,
        }
    }

    #[test]
    fn the_run_body_counts_and_does_not_invent_a_count_it_lacks() {
        assert_eq!(run_finished("/a/b", Some(&summary(1, 0))).body, "b: 1 task closed, 0 waiting for you");
        assert_eq!(run_finished("/a/b", None).body, "b: the run has ended");
        let unknown = RunSummary { seconds: 1, tasks: None, report: None };
        assert_eq!(run_finished("/a/b", Some(&unknown)).body, "b: the run has ended");
    }

    #[test]
    fn a_run_is_announced_once_when_it_becomes_over() {
        assert!(run_ended(Some(false), true));
        assert!(!run_ended(Some(true), true), "a second report of an ended run");
        assert!(!run_ended(Some(false), false));
        assert!(!run_ended(None, true), "a report for a run the worker no longer holds");
    }

    fn ev(seq: u64, kind: EventKind) -> Event {
        Event { seq, at: "2026-10-15T12:00:00Z".into(), kind }
    }

    fn permission(id: &str) -> EventKind {
        EventKind::Permission {
            id: id.into(),
            tool: "Bash".into(),
            detail: "ls".into(),
            options: vec![],
            input: serde_json::Value::Null,
        }
    }

    #[test]
    fn a_wait_makes_one_needs_you_however_many_events_follow_it() {
        let journal = vec![
            ev(1, EventKind::TurnStart { by: crate::session::model::Actor::Person }),
            ev(2, permission("p1")),
            ev(3, EventKind::TextDelta { text: "still thinking".into() }),
            ev(4, permission("p2")),
            ev(5, EventKind::TextDelta { text: "more".into() }),
        ];
        let mut state = SessionState::Starting;
        let mut pushes = 0;
        for upto in 1..=journal.len() {
            let next = state_of(&journal[..upto], true);
            if waiting_began(state, next) {
                pushes += 1;
            }
            state = next;
        }
        assert_eq!(state, SessionState::NeedsYou);
        assert_eq!(pushes, 1, "the second permission and the deltas are the same wait");
    }

    #[test]
    fn a_wait_after_an_answer_is_a_new_one() {
        let journal = vec![
            ev(1, EventKind::TurnStart { by: crate::session::model::Actor::Person }),
            ev(2, permission("p1")),
            ev(3, EventKind::PermissionAnswered {
                id: "p1".into(),
                decision: crate::session::model::Decision::Allow,
                answers: None,
            }),
            ev(4, permission("p2")),
        ];
        let mut state = SessionState::Starting;
        let mut pushes = 0;
        for upto in 1..=journal.len() {
            let next = state_of(&journal[..upto], true);
            if waiting_began(state, next) {
                pushes += 1;
            }
            state = next;
        }
        assert_eq!(pushes, 2);
    }

    #[test]
    fn the_needs_you_message_names_the_project_and_the_agent() {
        let message = needs_you("/work/smetana", 7, "claude", Some("fix the board"));
        assert_eq!(message.kind, Kind::NeedsYou);
        assert_eq!(message.title, "An agent is waiting for you");
        assert_eq!(message.body, "smetana: claude on \"fix the board\"");
        assert_eq!(message.target, "session:7");
    }

    #[test]
    fn a_terminal_session_message_targets_terminal_and_is_otherwise_the_same() {
        let message = terminal_needs_you("/work/smetana", 7, "claude");
        assert_eq!(message.target, "terminal:7");
        let conversation = needs_you("/work/smetana", 7, "claude", None);
        assert_eq!(message.kind, conversation.kind);
        assert_eq!(message.title, conversation.title);
        assert_eq!(message.body, conversation.body);
        assert_eq!(needs_you("/work/smetana", 7, "claude", None).body, "smetana: claude");
    }
}
