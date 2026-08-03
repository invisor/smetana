//! Session state detection. A pure function of the screen snapshot, whether a
//! bell rang and the timings — scheduling and storage live in service.rs.
//!
//! Layer A is agent-independent and has nothing in it to break. Layer B
//! (profiles.rs) reads someone else's interface and is therefore fragile; it
//! is layered on top and, when it fails to match, silently leaves layer A in
//! place.

use std::time::Duration;

use super::model::{Question, SessionState};

/// No output for this long — treat the agent as idle.
pub const IDLE_AFTER: Duration = Duration::from_secs(3);
/// How long the screen must hold still before a profile trusts it: a dialog
/// is not drawn instantly, and a half-drawn frame would match a truncated
/// question.
pub const SETTLE: Duration = Duration::from_millis(150);

pub struct DetectInput<'a> {
    /// A bell rang and hasn't been cleared yet. It's cleared by a human
    /// writing into the session — from the keyboard or a button — and by the
    /// process exiting.
    pub bell_pending: bool,
    pub quiet_for: Duration,
    pub screen: &'a [String],
    pub alive: bool,
}

pub struct Detected {
    pub state: SessionState,
    pub question: Option<Question>,
}

pub fn detect(input: DetectInput) -> Detected {
    let state = if input.bell_pending {
        SessionState::NeedsYou
    } else if input.quiet_for >= IDLE_AFTER {
        SessionState::Idle
    } else {
        SessionState::Running
    };
    Detected { state, question: None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn input(bell: bool, quiet_ms: u64, screen: &[&str]) -> DetectInput<'static> {
        // The screen is leaked on purpose: the test is short-lived but the
        // reference needs to be 'static.
        let lines: Vec<String> = screen.iter().map(|s| (*s).to_owned()).collect();
        DetectInput {
            bell_pending: bell,
            quiet_for: Duration::from_millis(quiet_ms),
            screen: Box::leak(lines.into_boxed_slice()),
            alive: true,
        }
    }

    #[test]
    fn пока_сыплет_вывод_это_работа() {
        assert_eq!(detect(input(false, 100, &["building..."])).state, SessionState::Running);
    }

    #[test]
    fn затих_надолго_это_простой() {
        assert_eq!(detect(input(false, 5_000, &["$ "])).state, SessionState::Idle);
    }

    #[test]
    fn простой_тихий_а_не_громкий() {
        // An agent that has finished and an agent that is waiting for an
        // answer look identical from outside — both simply stop producing
        // output. Shouting on every pause would make loudness unreadable
        // within a week.
        assert_ne!(detect(input(false, 60_000, &["$ "])).state, SessionState::NeedsYou);
    }

    #[test]
    fn звонок_громкий_даже_посреди_вывода() {
        assert_eq!(detect(input(true, 10, &["working"])).state, SessionState::NeedsYou);
    }

    #[test]
    fn звонок_громкий_и_в_простое() {
        assert_eq!(detect(input(true, 9_000, &["waiting"])).state, SessionState::NeedsYou);
    }

    #[test]
    fn слой_a_вопроса_не_знает() {
        assert!(detect(input(true, 10, &["working"])).question.is_none());
    }
}
