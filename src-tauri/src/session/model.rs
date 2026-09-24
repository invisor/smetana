//! The vocabulary of a driven session and the pure rules over it. No I/O here:
//! the process lives in `service.rs`, the codec in a driver.
//!
//! Every type here is what the front end sees, so the serde shape is part of
//! the contract rather than an implementation detail.

use std::collections::BTreeMap;

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
///
/// **`TextDelta` is the streamed agent-message shape for both driven
/// harnesses.** Claude Code supplies it from `--include-partial-messages`;
/// Codex's app-server supplies `item/agentMessage/delta`. Their final whole
/// messages become `Text`, which closes the same row authoritatively. Codex
/// drives every intent a person talks to now, the same as Claude Code —
/// `Intent::Run` is the one intent neither harness drives at all
/// (`session::service::drivable`), since nobody is in a run's conversation.
///
/// **`TextDelta` must never survive a re-entry, and that is a property of the
/// wire rather than of anything this crate filters.** Claude Code's own
/// persisted transcript (`~/.claude/projects/.../*.jsonl`, what
/// `session::history` replays after a restart) never contains a `stream_event`
/// record — only the consolidated `assistant`/`user`/`result` records this enum
/// already had before partial messages existed. Verified against the installed
/// CLI (2.1.269): a `claude -p --include-partial-messages` run's own stdout
/// carries `content_block_delta` lines, and the `.jsonl` it writes to disk
/// afterwards carries none. So a resumed session's history is built the same
/// way it always was and a `TextDelta` can only ever reach `journalRows` live,
/// while its stream is still open. `session::history::is_past` filters it out
/// a second time regardless, as a cheap independent guard rather than reliance
/// on that fact alone.
#[derive(Clone, PartialEq, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum EventKind {
    TurnStart { by: Actor },
    UserMessage { text: String, attachments: Vec<String> },
    /// The turn the app opened this session with on the person's behalf. What
    /// travelled to the harness is the whole brief — `Driver::opening`, the
    /// same text the PTY road hands over positionally; what is recorded is the
    /// part of it that is the person's own: the new-task dialog's text and
    /// pictures, or nothing at all for a start nobody typed a word into. The
    /// panel draws `None` as the session row's own caption, which is the
    /// front end's sentence and is deliberately not written here.
    Opening { text: Option<String>, attachments: Vec<String> },
    /// Markdown as the agent wrote it. Rendering is the front end's business.
    Text { text: String },
    /// A completed plain-text reply whose final paragraph asks the person a
    /// question. There is no protocol request to answer, so this deliberately
    /// carries neither an id nor options: the ordinary composer remains the
    /// only response path.
    TextQuestion,
    /// One incremental piece of the reply now being written, in the order it
    /// arrived — never accumulated here. `journal.js` is what stitches a run of
    /// these into a growing row, and the closing `Text` above replaces the
    /// stitched text wholesale with the harness's own authoritative final copy
    /// rather than trusting the concatenation, which costs nothing and is
    /// immune to the two ever drifting.
    ///
    /// Empty text is refused at the emitting end (`claude_driver::one_event`)
    /// for the reason `Text` refuses a blank one, but **never on
    /// `text.trim().is_empty()`** the way `Text` does: a delta that is a single
    /// space or newline between two words is real content, and trimming it
    /// away would silently glue two words together on the wire.
    TextDelta { text: String },
    Reasoning { text: String },
    ToolUse { id: String, name: String, detail: String },
    ToolResult { id: String, ok: bool, summary: String },
    /// The tool call's own arguments, untouched, **for the short list of
    /// tools that need them structured** — today just `AskUserQuestion`,
    /// whose own card (`src/components/conversation/AskUserQuestion.vue`)
    /// reads its `questions` out of this field, since `detail` is
    /// `tool_detail`'s one-line summary and cannot carry four questions each
    /// with its own options. Every other tool's `Permission` carries
    /// `Value::Null` here and keeps drawing from `detail` alone.
    ///
    /// The field itself is generic — on every `Permission` event whatever the
    /// tool — so that a second structured tool needs no second field, only a
    /// second name in the list `session::service::question` gates this on.
    /// It is *not* filled in unconditionally: this event is appended to a
    /// journal that lives for the life of a session and is cloned whole on
    /// every attach (`journal::BUDGET` holds because each event is small,
    /// and a driven session asks on every `Write`, `Edit`, `MultiEdit` and
    /// `Task` — a bound that a universal, unclipped `input` would break by
    /// carrying whole file bodies and whole subagent prompts through it).
    Permission { id: String, tool: String, detail: String, options: Vec<Decision>, input: serde_json::Value },
    /// `answers` is `Some` only for a person's own answer to `AskUserQuestion`
    /// — the text of each question mapped to what was chosen or typed, the
    /// same shape that rides back to the harness as `updatedInput.answers`
    /// (`permission::decision_payload`). `None` for an ordinary allow/deny,
    /// and for a decline to answer: refusing a question is `Decision::Deny`
    /// with no answers, exactly like refusing any other tool.
    PermissionAnswered { id: String, decision: Decision, answers: Option<BTreeMap<String, String>> },
    Result { tokens_in: u64, tokens_out: u64, cost_usd: Option<f64>, ms: u64 },
    /// A terminal turn failure. Kept distinct from `Error`: protocol errors
    /// can be retryable notifications while this one closes `Running`.
    TurnFailed { text: String },
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

/// The whole of the `session:state` payload.
///
/// A type rather than a `json!` literal at the emit, and the third field is why
/// it became one. `id` and `state` were two words nobody could misspell twice;
/// `conversation` is the name a row in the agents panel is keyed by, is read in
/// `stores/conversation.js` off this very object, and nothing mechanical joins
/// the two sides — so it is written down here, once, where the store's contract
/// is the rest of this file.
///
/// **The conversation id travels on every state change rather than once at the
/// start**, because a state event is the one thing a window is told about a
/// session it has not attached to. It is the same value for the life of the
/// session and repeating it costs a UUID per change; the alternative is a
/// window that missed one message and never learns the name of the row it is
/// drawing.
#[derive(Clone, Debug, serde::Serialize)]
pub struct StateChange {
    pub id: SessionId,
    pub state: SessionState,
    /// The id this session's transcript is named after and its record in
    /// `.smetana/agents.json` is keyed by, or `None` for a session that has
    /// neither: a fork, whose new transcript Claude Code names itself, and a
    /// machine that would not give the random bytes.
    pub conversation: Option<String>,
    /// The automatic title the agents panel names this row by, or `None`
    /// while the session has none. Travels on every state change for the
    /// reason `conversation` does: a window that missed one event must still
    /// learn the name of the row it is drawing.
    pub title: Option<String>,
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
    /// A directory a resumed session was asked to open in that is not a folder
    /// inside the project: outside the root, gone from disk, or a file.
    ///
    /// **Its own variant rather than a `Spawn`, and that is about what a person
    /// reads.** A `Spawn` carries its own text to the front end unchanged
    /// (`ERRORS.spawn` is the identity), which is right for a sentence written
    /// for whoever fixes things and wrong for this one: a removed worktree is
    /// the ordinary outcome here, not a fault, and the store has words for it.
    /// `stores/conversation.js` keys `badCwd` on this tag and answers with the
    /// same sentence `stores/terminals.js` answers `TerminalError::BadCwd`
    /// with, which is what makes one refusal read the same on either road.
    ///
    /// The payload is the path, unused by that sentence and kept for the log.
    #[error("that folder cannot be a working directory: {0}")]
    BadCwd(String),
    /// This road never took the intent at all: the harness has no codec for
    /// it (`driver_for` answered `None`), or the intent is `Run`, which no
    /// person is ever in the conversation of. Nothing was spawned and nothing
    /// failed — the front door asked a cheap question and Rust is the one
    /// that actually knows.
    ///
    /// **Its own variant and not a `Spawn`, and the distinction is the whole
    /// of acceptance criterion 1 of smetana-gb7f.4.** `startAgent` in
    /// `DesktopApp.vue` reads this tag to decide whether falling through to
    /// `createSession` — the PTY road — is the front door being wrong about a
    /// capability, or an app-server this app *did* try to drive failing to
    /// start or answering with a protocol error. Only this tag may fall
    /// through in silence; every other kind — `Spawn` above included, which
    /// now means "the attempt was made and it failed" — stops here; the
    /// person keeps their reason, and the caller's draft is untouched, since
    /// the dialog that holds it only closes once `startAgent` says the
    /// session actually started.
    ///
    /// The two `Spawn`s `commands.rs::ask` produces — "the session worker is
    /// not running" and "the session worker did not answer" — are on the
    /// stopping side too, even though they are arguably "never attempted" in
    /// the same sense as this variant: they mean the worker itself could not
    /// be reached, which says nothing about whether a Codex app-server would
    /// have driven the intent, and a PTY spawned from a worker that cannot
    /// answer an `oneshot` channel is not a retry worth making either. Noted
    /// here so the next reader auditing this split does not re-derive it.
    #[error("{0}")]
    NotDriven(String),
}

/// How long an automatic title may be, in characters. A row is one line and
/// clips with an ellipsis; this only keeps a pasted page out of the record and
/// off the wire on every state change.
pub const TITLE_CHARS: usize = 120;

/// The automatic title a session opens with: the first thing the person said,
/// on one line, cut to [`TITLE_CHARS`]. `None` when they said nothing, so the
/// row keeps its intent caption rather than going blank.
pub fn first_words(text: &str) -> Option<String> {
    let line = crate::sessions::model::one_line(text);
    let cut = match line.char_indices().nth(TITLE_CHARS) {
        Some((at, _)) => line[..at].trim_end().to_owned(),
        None => line,
    };
    (!cut.is_empty()).then_some(cut)
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
    let mut text_question = false;
    let mut seen_anything = false;
    for event in events {
        seen_anything = true;
        match &event.kind {
            EventKind::TurnStart { .. } => {
                open_turn = true;
                text_question = false;
            }
            EventKind::Result { .. } => open_turn = false,
            EventKind::TurnFailed { .. } => open_turn = false,
            EventKind::Permission { id, .. } => pending.push(id),
            // By id rather than by count: an answer that arrives out of order
            // must settle its own question and leave the others standing.
            EventKind::PermissionAnswered { id, .. } => pending.retain(|open| open != id),
            EventKind::TextQuestion => text_question = true,
            _ => {}
        }
    }
    if !child_alive {
        // A turn still open when the process went is the honest reading of a
        // crash; a turn closed is an ordinary end.
        return if open_turn { SessionState::Failed } else { SessionState::Exited };
    }
    // A question outranks a turn in flight: the agent is not working, it waits.
    if !pending.is_empty() || text_question {
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

/// Does a completed plain-text reply leave the person a question to answer?
///
/// Paragraphs are separated by blank or whitespace-only lines. Only the last
/// non-empty paragraph matters, and an ASCII question mark anywhere in it is
/// sufficient: a sentence after the question is still part of the same ask.
pub fn text_waits_for_reply(text: &str) -> bool {
    let mut paragraph = String::new();
    let mut last = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !paragraph.trim().is_empty() {
                last = std::mem::take(&mut paragraph);
            }
        } else {
            if !paragraph.is_empty() {
                paragraph.push('\n');
            }
            paragraph.push_str(line);
        }
    }
    if !paragraph.trim().is_empty() {
        last = paragraph;
    }
    last.contains('?')
}

/// Is this session holding this question open — a `Permission` with that id and
/// no `PermissionAnswered` for it since?
///
/// The narrow half of `state_of`'s fold, and it exists because a question id
/// alone identifies nothing: they are minted from one counter for the whole
/// app, and the permission listener looks them up in one map across every
/// session. The journal is the only place that knows which session was asked,
/// so this is the check that has to stand between an answer off the wire and
/// both the journal and that listener.
///
/// Reads forward for the reason `state_of` does, and agrees with it by
/// construction: a question this answers `false` for is one `state_of` does not
/// count as pending either.
pub fn is_open_question(events: &[Event], question: &str) -> bool {
    let mut open = false;
    for event in events {
        match &event.kind {
            EventKind::Permission { id, .. } if id == question => open = true,
            EventKind::PermissionAnswered { id, .. } if id == question => open = false,
            _ => {}
        }
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_words_collapse_whitespace_and_stop_at_the_budget() {
        assert_eq!(first_words("  Rename\n\nthe   rows ").as_deref(), Some("Rename the rows"));
        let long = "слово ".repeat(40);
        let title = first_words(&long).expect("a long draft still titles");
        assert_eq!(title.chars().count(), TITLE_CHARS - 1, "cut on a character boundary, trailing space trimmed");
    }

    #[test]
    fn nothing_said_is_no_title() {
        assert_eq!(first_words(""), None);
        assert_eq!(first_words(" \n\t "), None);
    }

    fn ev(seq: u64, kind: EventKind) -> Event {
        Event { seq, at: "2026-09-10T12:00:00Z".into(), kind }
    }

    fn permission(id: &str) -> EventKind {
        EventKind::Permission {
            id: id.into(),
            tool: "Bash".into(),
            detail: "rm -rf /tmp/x".into(),
            options: vec![Decision::Allow, Decision::Deny],
            input: serde_json::json!({ "command": "rm -rf /tmp/x" }),
        }
    }

    fn answered(id: &str, decision: Decision) -> EventKind {
        EventKind::PermissionAnswered { id: id.into(), decision, answers: None }
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
            ev(3, answered("p1", Decision::Allow)),
        ];
        assert_eq!(state_of(&events, true), SessionState::Running);
    }

    #[test]
    fn an_answer_carrying_chosen_labels_settles_the_question_like_any_other() {
        // `AskUserQuestion`'s own answer carries `answers` on top of the plain
        // `Decision`, and the fold's `..` pattern must not care: settling a
        // question is about the id, never about what rode along with it.
        let mut answers = BTreeMap::new();
        answers.insert("Which approach?".to_string(), "Rewrite the migration".to_string());
        let events = vec![
            ev(1, permission("p1")),
            ev(2, EventKind::PermissionAnswered { id: "p1".into(), decision: Decision::Allow, answers: Some(answers) }),
        ];
        assert!(!is_open_question(&events, "p1"));
        assert_eq!(state_of(&events, true), SessionState::Ready);
    }

    #[test]
    fn two_questions_are_settled_one_at_a_time() {
        // Answering the first leaves the second standing. Matching by id rather
        // than by count is what makes an out-of-order answer harmless.
        let events = vec![
            ev(1, permission("p1")),
            ev(2, permission("p2")),
            ev(3, answered("p2", Decision::Deny)),
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
    fn a_completed_text_question_waits_until_the_next_turn_starts() {
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Person }),
            ev(2, EventKind::Result { tokens_in: 1, tokens_out: 1, cost_usd: None, ms: 1 }),
            ev(3, EventKind::TextQuestion),
        ];
        assert_eq!(state_of(&events, true), SessionState::NeedsYou);

        let mut answered = events;
        answered.push(ev(4, EventKind::TurnStart { by: Actor::Person }));
        assert_eq!(state_of(&answered, true), SessionState::Running);
    }

    #[test]
    fn text_questions_read_only_the_last_non_empty_paragraph() {
        assert!(text_waits_for_reply(
            "Do you confirm the document? After that I will make the plan and create the task."
        ));
        assert!(!text_waits_for_reply("Do you confirm the document?\n\nI will make the plan."));
        assert!(!text_waits_for_reply("The plan is ready.\n \t\n"));
    }

    #[test]
    fn a_terminal_turn_failure_closes_running_but_an_error_does_not() {
        let open = vec![ev(1, EventKind::TurnStart { by: Actor::Agent })];
        let retryable = vec![open[0].clone(), ev(2, EventKind::Error { text: "retrying".into() })];
        let failed = vec![open[0].clone(), ev(2, EventKind::TurnFailed { text: "failed".into() })];
        assert_eq!(state_of(&retryable, true), SessionState::Running);
        assert_eq!(state_of(&failed, true), SessionState::Ready);
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

    #[test]
    fn a_question_this_journal_never_held_is_not_one_to_answer() {
        // The one that matters: ids are minted from a single counter for the
        // whole app, so `q1` exists in some session — just not this one.
        // Without this check an answer aimed at the wrong session would release
        // the other one's tool call and leave its journal asking for good.
        let events = vec![ev(1, EventKind::TurnStart { by: Actor::Agent })];
        assert!(!is_open_question(&events, "q1"));
        assert!(!is_open_question(&[], "q1"));
    }

    #[test]
    fn an_opening_turn_reads_as_running_until_the_harness_answers() {
        let events = vec![
            ev(1, EventKind::TurnStart { by: Actor::Person }),
            ev(2, EventKind::Opening { text: Some("hello".into()), attachments: vec![] }),
        ];
        assert_eq!(state_of(&events, true), SessionState::Running);
    }

    #[test]
    fn an_opening_turn_is_on_the_wire_as_its_own_kind() {
        let json = serde_json::to_value(EventKind::Opening { text: None, attachments: vec![] }).unwrap();
        assert_eq!(json["kind"], "opening");
        assert!(json["text"].is_null());
        assert_eq!(json["attachments"], serde_json::json!([]));
    }

    #[test]
    fn a_text_question_is_on_the_wire_without_a_structured_request() {
        let json = serde_json::to_value(EventKind::TextQuestion).unwrap();
        assert_eq!(json, serde_json::json!({ "kind": "text-question" }));
    }

    #[test]
    fn a_question_that_is_standing_is_one_to_answer_exactly_once() {
        let asked = vec![ev(1, permission("q1"))];
        assert!(is_open_question(&asked, "q1"));
        let mut settled = asked.clone();
        settled.push(ev(2, answered("q1", Decision::Allow)));
        assert!(!is_open_question(&settled, "q1"), "a second answer has nothing to settle");
        assert!(!is_open_question(&settled, "q2"), "and it settled only its own question");
    }
}
