//! The ordered event log of one session. This is what `terminal::ring` is for a
//! terminal — the thing a window that has just opened is caught up from — with
//! events in place of bytes.

use std::collections::HashSet;

use super::model::{Event, EventKind};

/// How many events a session keeps. A long night of tool calls is thousands of
/// them and each is small; the ceiling is here so that a session left running
/// for a week cannot grow without bound.
///
/// **That "each is small" is no longer true of every event, and `append`'s own
/// `TextDelta` collapse below is what keeps it true in practice.** One
/// substantial streamed reply is several hundred `TextDelta`s where it used to
/// be a single `Text` (smetana-6we6); left uncollapsed, this budget would be
/// spent by five to fifteen ordinary turns rather than the "long night" or
/// "week" this comment has always promised — a promise that was never
/// re-measured against the granularity the wire actually carries once it
/// changed. It holds again only because a closed reply's deltas do not stay in
/// the journal once it closes.
pub const BUDGET: usize = 4000;

pub struct Journal {
    events: Vec<Event>,
    next: u64,
    /// The highest `seq` trimming has actually taken. Recorded rather than
    /// inferred from the oldest event still held, because trimming pins an
    /// unanswered `Permission` at the front and drops what follows it: the
    /// journal is then not a contiguous suffix, and its first event's `seq`
    /// says nothing about where the gap ends. Inferring it meant `since` could
    /// never report a gap again once a question was pinned.
    trimmed_through: u64,
}

impl Journal {
    pub fn new() -> Self {
        // Numbering opens at 1 so that 0 can mean "nothing seen yet" — a front
        // end that has never attached asks from 0 and must be given everything.
        Self { events: Vec::new(), next: 1, trimmed_through: 0 }
    }

    pub fn append(&mut self, kind: EventKind, at: String) -> Event {
        let event = Event { seq: self.next, at, kind };
        self.next += 1;
        self.events.push(event.clone());
        // The `TextDelta`s a closing `Text` supersedes do not go on living in
        // the journal beside it. Without this, one substantial streamed reply
        // is several hundred events where it used to be one, and `BUDGET`
        // arrives in five to fifteen turns rather than the week this file's
        // own comment on it promises — see that comment for the reasoning in
        // full.
        //
        // **`trimmed_through` is deliberately untouched.** That field is
        // `since`'s floor: below it, a cursor is told to re-snapshot rather
        // than trust a vec with a hole in it. Moving it here would claim the
        // same thing `trim` claims — "everything below this line is gone for
        // a reason a caller must recover from" — which is false of a delta a
        // live client already received in full. `since(seq)` only ever
        // filters by `event.seq > seq`; it does not ask whether a particular
        // `seq` is present, so a delta's absence from `self.events` is
        // invisible to a cursor holder who long since moved past it, and a
        // fresh attacher's `snapshot()` simply never sees deltas a reply has
        // already closed over.
        //
        // A stream can only ever have one reply's worth of deltas held at
        // once, by construction: the very last time a `Text` was appended, it
        // ran this same retain and cleared every delta that came before it.
        // So this always removes exactly the run just superseded, never more.
        if matches!(&event.kind, EventKind::Text { .. }) {
            self.events.retain(|held| !matches!(held.kind, EventKind::TextDelta { .. }));
        }
        self.trim();
        event
    }

    /// Everything after `seq`, or `None` when `seq` is older than what is still
    /// held. `None` is not an error: it is what tells the front end to take a
    /// fresh snapshot instead of drawing a conversation with a hole in it.
    pub fn since(&self, seq: u64) -> Option<Vec<Event>> {
        // Everything above `trimmed_through` is still here, so a caller holding
        // exactly that number — or anything later — can be caught up, and the
        // newest `seq` is always one of those. Below it something is missing,
        // whether or not the journal's first event happens to be older still.
        //
        // A `seq` past the tip lands here as `Some(vec![])` rather than `None`,
        // and that is deliberate: a journal lives exactly as long as the
        // session id it belongs to, so no client can be holding a cursor minted
        // by another one, and the only way to be ahead of the tip is to have
        // asked twice with nothing appended in between.
        if seq < self.trimmed_through {
            return None;
        }
        Some(self.events.iter().filter(|event| event.seq > seq).cloned().collect())
    }

    pub fn snapshot(&self) -> (Vec<Event>, u64) {
        (self.events.clone(), self.next - 1)
    }

    pub fn events(&self) -> &[Event] {
        &self.events
    }

    /// Drop from the front, and never a question nobody has answered.
    ///
    /// `state_of` folds over whatever the journal holds, so trimming an open
    /// `Permission` away would leave the session `needs-you` with nothing on
    /// screen to answer and no way back to `running`. An answered one carries
    /// no such weight and goes with the rest.
    fn trim(&mut self) {
        if self.events.len() <= BUDGET {
            return;
        }
        let answered: HashSet<String> = self
            .events
            .iter()
            .filter_map(|event| match &event.kind {
                EventKind::PermissionAnswered { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        let mut over = self.events.len() - BUDGET;
        let mut trimmed_through = self.trimmed_through;
        self.events.retain(|event| {
            if over == 0 {
                return true;
            }
            if let EventKind::Permission { id, .. } = &event.kind {
                if !answered.contains(id) {
                    return true;
                }
            }
            over -= 1;
            trimmed_through = trimmed_through.max(event.seq);
            false
        });
        self.trimmed_through = trimmed_through;
    }
}

impl Default for Journal {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::model::{Actor, Decision};

    fn text(body: &str) -> EventKind {
        EventKind::Text { text: body.into() }
    }

    fn delta(piece: &str) -> EventKind {
        EventKind::TextDelta { text: piece.into() }
    }

    fn at() -> String {
        "2026-09-10T12:00:00Z".into()
    }

    fn permission(id: &str) -> EventKind {
        EventKind::Permission {
            id: id.into(),
            tool: "Bash".into(),
            detail: "ls".into(),
            options: vec![],
        }
    }

    #[test]
    fn seq_starts_at_one_so_that_zero_can_mean_nothing_seen_yet() {
        // A front end that has never attached asks from 0, and must be given
        // everything rather than everything-but-the-first.
        let mut journal = Journal::new();
        assert_eq!(journal.append(text("a"), at()).seq, 1);
        assert_eq!(journal.since(0).expect("nothing was trimmed").len(), 1);
    }

    #[test]
    fn since_hands_back_only_what_came_after_the_number_it_was_given() {
        let mut journal = Journal::new();
        journal.append(text("a"), at());
        journal.append(text("b"), at());
        journal.append(text("c"), at());
        let tail = journal.since(1).expect("nothing was trimmed");
        assert_eq!(tail.len(), 2);
        assert_eq!(tail[0].seq, 2);
    }

    #[test]
    fn a_seq_that_was_trimmed_away_answers_none_rather_than_a_hole() {
        // None is what makes the front end re-attach. Handing back a partial
        // tail would leave it drawing a conversation with a silent gap in it.
        let mut journal = Journal::new();
        for n in 0..BUDGET + 10 {
            journal.append(text(&n.to_string()), at());
        }
        assert!(journal.since(1).is_none(), "seq 1 is long gone");
        assert!(journal.since(journal.snapshot().1).is_some(), "the newest is always reachable");
    }

    #[test]
    fn a_pinned_question_does_not_blind_since_to_everything_trimmed_behind_it() {
        // The two safety rules meet here, and apart they each look right: the
        // question is kept at the front, everything after it goes, and the
        // journal stops being a contiguous suffix. A floor read off the oldest
        // event still held is then that pinned `seq` — 1 — and no cursor on
        // earth is below it, so `since` would answer `Some` over a hole four
        // thousand events wide and the conversation would lose them silently.
        let mut journal = Journal::new();
        journal.append(permission("p1"), at());
        for n in 0..BUDGET * 2 {
            journal.append(text(&n.to_string()), at());
        }
        assert!(
            journal.since(3000).is_none(),
            "a cursor inside the gap is told to re-snapshot, not handed the far side of it"
        );
        assert!(
            journal.since(journal.snapshot().1).is_some(),
            "the newest is still always reachable"
        );
    }

    #[test]
    fn a_cursor_at_the_far_end_of_the_number_line_is_answered_rather_than_overflowing() {
        // The floor is compared against, never added to. An implementation
        // asking whether `seq + 1` is below it panics here in a debug build and
        // wraps to zero in a release one.
        let mut journal = Journal::new();
        journal.append(text("a"), at());
        assert_eq!(journal.since(u64::MAX).expect("nothing was trimmed").len(), 0);
    }

    #[test]
    fn trimming_never_takes_a_question_still_waiting_for_an_answer() {
        // Losing it would leave the session stuck in needs-you with nothing on
        // screen to answer, since `state_of` folds over what the journal holds.
        let mut journal = Journal::new();
        journal.append(permission("p1"), at());
        for n in 0..BUDGET + 10 {
            journal.append(text(&n.to_string()), at());
        }
        assert!(
            journal
                .events()
                .iter()
                .any(|e| matches!(&e.kind, EventKind::Permission { id, .. } if id == "p1")),
            "the unanswered question survived the trim"
        );
    }

    #[test]
    fn a_question_that_has_been_answered_is_ordinary_and_may_be_trimmed() {
        let mut journal = Journal::new();
        journal.append(permission("p1"), at());
        journal.append(
            EventKind::PermissionAnswered { id: "p1".into(), decision: Decision::Allow },
            at(),
        );
        for n in 0..BUDGET + 10 {
            journal.append(text(&n.to_string()), at());
        }
        assert!(journal.events().len() <= BUDGET + 2, "answered questions do not accumulate");
    }

    #[test]
    fn the_snapshot_says_which_seq_to_continue_from() {
        let mut journal = Journal::new();
        journal.append(text("a"), at());
        journal.append(text("b"), at());
        let (events, seq) = journal.snapshot();
        assert_eq!(seq, 2);
        assert_eq!(events.last().expect("two were appended").seq, 2);
        assert!(journal.since(seq).expect("nothing trimmed").is_empty());
    }

    #[test]
    fn an_empty_journal_snapshots_to_nothing_at_zero() {
        let (events, seq) = Journal::new().snapshot();
        assert!(events.is_empty());
        assert_eq!(seq, 0);
    }

    #[test]
    fn the_turn_start_of_an_agent_is_kept_the_way_a_person_message_is() {
        // Both are structure rather than chatter, and a journal trimmed to
        // paragraphs alone would draw prose with no turn boundaries in it.
        let mut journal = Journal::new();
        journal.append(EventKind::TurnStart { by: Actor::Person }, at());
        assert_eq!(journal.events().len(), 1);
    }

    /// The blocking finding smetana-6we6's review came back with: a
    /// substantial streamed reply is several hundred `TextDelta`s where it
    /// used to be one `Text`, and `BUDGET` would arrive in five to fifteen
    /// turns rather than the week `BUDGET`'s own comment promises. The closing
    /// `Text` has to take its deltas with it.
    #[test]
    fn a_closing_text_drops_the_deltas_it_superseded() {
        let mut journal = Journal::new();
        journal.append(EventKind::TurnStart { by: Actor::Agent }, at());
        journal.append(delta("The identity"), at());
        journal.append(delta(" is read once"), at());
        journal.append(text("The identity is read once."), at());

        let kinds: Vec<&EventKind> = journal.events().iter().map(|event| &event.kind).collect();
        assert_eq!(
            kinds,
            vec![
                &EventKind::TurnStart { by: Actor::Agent },
                &EventKind::Text { text: "The identity is read once.".into() }
            ],
            "the deltas must not go on living beside the reply that closed over them"
        );
    }

    /// A reply that streamed for a long time can still be several hundred
    /// deltas wide the instant before it closes — this is the shape that
    /// blew the budget open, pinned directly rather than only through its
    /// consequence above.
    #[test]
    fn many_deltas_collapse_to_one_event_on_close() {
        let mut journal = Journal::new();
        for n in 0..500 {
            journal.append(delta(&n.to_string()), at());
        }
        assert_eq!(journal.events().len(), 500);
        journal.append(text("the whole reply"), at());
        assert_eq!(journal.events().len(), 1);
    }

    /// Only the run just superseded goes — an earlier, already-closed reply's
    /// words stay exactly as they were, and a *second* stream's deltas are
    /// left standing until their own close, not swept early by someone else's.
    #[test]
    fn closing_one_stream_never_touches_an_earlier_reply_or_a_later_one() {
        let mut journal = Journal::new();
        journal.append(text("the first reply"), at());
        journal.append(delta("the second"), at());
        journal.append(text("the second reply"), at());
        journal.append(delta("a third, still open"), at());

        let kinds: Vec<&EventKind> = journal.events().iter().map(|event| &event.kind).collect();
        assert_eq!(
            kinds,
            vec![
                &EventKind::Text { text: "the first reply".into() },
                &EventKind::Text { text: "the second reply".into() },
                &EventKind::TextDelta { text: "a third, still open".into() }
            ]
        );
    }

    /// `trimmed_through` is deliberately untouched by the collapse: it is
    /// `since`'s floor, and moving it would tell a cursor holder to
    /// re-snapshot over a gap that cost it nothing, since it already had the
    /// deltas live before they were dropped.
    #[test]
    fn dropping_superseded_deltas_does_not_move_the_since_floor() {
        let mut journal = Journal::new();
        journal.append(delta("a"), at());
        journal.append(delta("b"), at());
        let (_, before_close) = journal.snapshot();
        journal.append(text("ab"), at());

        assert!(
            journal.since(before_close).is_some(),
            "a cursor that watched the stream live is not told to re-snapshot"
        );
        assert_eq!(journal.since(before_close).expect("checked above").len(), 1);
    }

    /// A fresh attacher after the close sees the consolidated reply and none
    /// of the deltas it was built from — the live-journal half of the
    /// acceptance criterion the transcript-replay half already covered.
    #[test]
    fn a_snapshot_taken_after_the_close_holds_no_deltas_at_all() {
        let mut journal = Journal::new();
        journal.append(EventKind::TurnStart { by: Actor::Agent }, at());
        for piece in ["The ", "identity ", "is read once."] {
            journal.append(delta(piece), at());
        }
        journal.append(text("The identity is read once."), at());

        let (events, _) = journal.snapshot();
        assert!(
            !events.iter().any(|event| matches!(event.kind, EventKind::TextDelta { .. })),
            "a fresh attacher must never see a delta the reply it belongs to has already closed"
        );
    }
}

