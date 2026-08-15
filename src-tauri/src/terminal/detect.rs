//! Session state detection. A pure function of the screen snapshot, whether a
//! bell rang and the timings — scheduling and storage live in service.rs.
//!
//! Layer A is agent-independent and has nothing in it to break. Layer B
//! (each profile's own `question`) reads someone else's interface and is
//! therefore fragile; it is layered on top and, when it fails to match,
//! silently leaves layer A in place.
//!
//! **Quiet is measured on the screen, not on the byte stream**, and that is
//! the whole of what `Quiet` below exists for. An agent that is waiting can
//! still be talking: Claude Code 2.1 repaints an open permission dialog about
//! every 0.61 s for as long as it stands there, and while quiet meant "no
//! bytes arrived" every one of those chunks restarted the clock — so a session
//! waiting on a human read as `Running` for as long as it waited, and
//! `IDLE_AFTER` was simply unreachable. A repaint that draws the same text
//! changes nothing a person could act on, so what gets timed is the picture
//! they see.
//!
//! The rule cuts the other way too, deliberately: a session whose screen holds
//! still for `IDLE_AFTER` is called idle even while bytes pour in. That is the
//! honest reading — the app knows exactly what somebody watching the terminal
//! would know — and it is cheap to be wrong about. `Idle` reaches the front end
//! as the `ready` status, whose loudness is `live`, the same as `running`: the
//! whole visible cost of a false `Idle` is the dot beside the row turning from
//! the spinning `loader-circle` into `circle-dashed`. Nothing dims, nothing
//! shouts, and nothing else in the app acts on the state at all; the first byte
//! that changes the screen puts it back to `Running`. The two states that cost
//! something to get wrong are untouched: `NeedsYou` still comes only from a
//! bell or from a profile's own match, never from silence of any kind.
//!
//! That two-way cut is a rule about a screen a harness draws for a person, and
//! there is exactly one kind of session it does not hold for: one whose screen
//! is a rendered transcript of a machine-format stream (`transcript` below).
//! Such a harness emits bytes only when a tool call begins or ends, so a
//! five-minute `cargo test` leaves the picture untouched for five minutes while
//! the agent works flat out — and stillness, which on a TUI means the agent has
//! stopped, means nothing of the sort here. **That is the mechanism this rule
//! assumes, read off the stream's own event types rather than watched on a live
//! batch under this build** — the same standing the smetana-8h7 fix above has.
//! So layer A does not offer such a session `Idle` at all: what is left to it
//! is `Running`, with the bell and layer B untouched above it, so `NeedsYou` is
//! still reachable from either (smetana-07o). The threshold, the fingerprint
//! and the way quiet is measured do not move for anybody else.
//!
//! An agent that has genuinely finished still reaches `Idle` at three seconds,
//! give or take a sample — not to the millisecond it did before, and the
//! difference goes both ways. Earlier, because the last bytes a CLI writes are
//! often invisible ones (showing the cursor again, resetting the window title),
//! and the old clock counted those while this one does not. Later, because
//! `since` is stamped when the worker next looks rather than when the screen
//! actually changed, so it can lag by up to one detection interval
//! (`REASSESS_EVERY` × `FLUSH`, ~64 ms today). The two clocks are close, not
//! equivalent — anyone lengthening that interval is lengthening this error with
//! it.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use super::model::{Question, SessionState};
use crate::agents::Profile;

/// The screen has looked the same for this long — treat the agent as idle.
pub const IDLE_AFTER: Duration = Duration::from_secs(3);
/// How long the screen must hold still before a profile trusts it: a dialog
/// is not drawn instantly, and a half-drawn frame would match a truncated
/// question.
pub const SETTLE: Duration = Duration::from_millis(150);

/// How long the screen has looked exactly as it does now — layer A's clock,
/// and the only state it keeps between ticks. One per session, owned by the
/// worker, fed the screen on every detection tick.
///
/// `now` arrives as an argument rather than being read here, which is what
/// keeps this pure and its tests free of sleeping.
#[derive(Default)]
pub struct Quiet {
    /// A hash rather than the lines themselves. This runs for every live
    /// session on every detection tick, and keeping the previous screen would
    /// mean copying a few kilobytes per session per tick to compare against.
    /// A collision would read a busy session as idle for as long as the two
    /// screens stayed identical — one dot drawn dashed instead of spinning.
    seen: Option<u64>,
    /// When the screen last showed something new. `None` until the first
    /// screen is seen at all.
    since: Option<Instant>,
}

impl Quiet {
    pub fn new() -> Self {
        Self::default()
    }

    /// Take in the screen as it is now and answer how long it has looked that
    /// way. The first screen ever seen counts as a change — there was nothing
    /// before it to be still against — and so answers zero.
    ///
    /// Note what this compares and what it does not: **the plain text of the
    /// visible rows, and nothing else**. `Screen::lines` comes from
    /// `vt100::Screen::rows`, which writes out characters only — no colour, no
    /// bold or reverse, no cursor. So a repaint that changes only an attribute,
    /// or moves the cursor over unchanged text, is stillness here.
    ///
    /// **That exclusion is deliberate, and widening it would bring smetana-8h7
    /// straight back.** A CLI agent waiting on a person redraws its dialog to
    /// keep the highlight under the selected option alive — a colour repaint of
    /// identical text, which is exactly the case this whole file was rewritten
    /// to see through. Feeding attributes into the fingerprint would make every
    /// such wait look like work again, and the symptom is silence: a session
    /// needing a human that reads as busy, with nothing anywhere to say so. If
    /// some agent's spinner is animated purely in colour, the price of getting
    /// that one wrong is a dashed dot instead of a spinning one; the price of
    /// the other mistake is the feature.
    pub fn still_for(&mut self, screen: &[String], now: Instant) -> Duration {
        let print = fingerprint(screen);
        if self.seen != Some(print) {
            self.seen = Some(print);
            self.since = Some(now);
        }
        now.saturating_duration_since(self.since.unwrap_or(now))
    }

    /// Start the count again without waiting for the screen to say so: the app
    /// has just written into the session itself, and stillness that predates
    /// our own input is not an answer to it. Only the clock is reset — the
    /// screen we remember stays, so the next tick still compares against what
    /// was actually there.
    pub fn restart(&mut self, now: Instant) {
        self.since = Some(now);
    }
}

fn fingerprint(screen: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    screen.hash(&mut hasher);
    hasher.finish()
}

pub struct DetectInput<'a> {
    /// A bell rang and hasn't been cleared yet. It's cleared by a human
    /// writing into the session — from the keyboard or a button — by a view
    /// attaching to it, which is a human looking at what rang, and by the
    /// process exiting.
    pub bell_pending: bool,
    /// How long the screen has held still — `Quiet::still_for`, not the gap
    /// since the last byte. See the note at the top of this file.
    pub still_for: Duration,
    pub screen: &'a [String],
    /// This session's screen is a rendered transcript of a machine-format
    /// stream rather than a live TUI — `Live::transcript` in service.rs, set
    /// for a run's batch and for nothing else.
    ///
    /// It is a fact about where the picture comes from, not a policy about
    /// runs, and that is why it is this and not `agents::is_batch`: Codex runs
    /// its batches interactively, has no translator, and a still screen of its
    /// means exactly what a person's session's does. What is true of the
    /// sessions this names is that the harness emits bytes only when a tool
    /// call begins or ends, so the screen holds still for the whole of a long
    /// call — three seconds of that is not a stopped agent, it is one halfway
    /// through `npm install`.
    ///
    /// What makes this the same question as "this harness runs
    /// non-interactively" is not structural but held by a test:
    /// `a_translator_is_only_ever_installed_over_a_stream_that_was_asked_for`
    /// in `agents/mod.rs` asserts across `IDS` that a profile answering
    /// `transcript` is exactly one answering `batch_args`. The case that would
    /// break the equivalence is the one `Profile::transcript`'s own doc invites
    /// — a harness printing readable progress by itself, which would keep the
    /// default of no translator, ask for `batch_args` all the same, have this
    /// very defect and read `false` here. That test refuses such a harness
    /// today; whoever relaxes it in order to ship one has to come back to this
    /// field.
    ///
    /// So layer A does not offer such a session `Idle` at all, and the cost is
    /// named: a batch wedged dead, printing nothing ever again, reads as
    /// `Running` until its process exits rather than going quiet after
    /// `IDLE_AFTER`. Nothing is lost by that — a run waits for its batch on the
    /// exit code and never on this state, and the only readers of a batch's
    /// `Idle` were the dot in the agent list and `configFreshness`, which is
    /// the pair this exists to stop lying to. The bell and layer B are
    /// deliberately untouched here, so neither loud reading is disabled along
    /// with the quiet one.
    pub transcript: bool,
    /// Which agent this session runs — layer B is that agent's own dialog
    /// reader, not a hardcoded one.
    pub profile: &'static dyn Profile,
}

pub struct Detected {
    pub state: SessionState,
    pub question: Option<Question>,
}

pub fn detect(input: DetectInput) -> Detected {
    // Layer B: the profile knows exactly what is being asked, so it takes
    // precedence. Trusted only once the screen has settled — see SETTLE.
    if input.still_for >= SETTLE {
        if let Some(question) = input.profile.question(input.screen) {
            return Detected { state: SessionState::NeedsYou, question: Some(question) };
        }
    }

    // Layer A: agent-independent, nothing in it to break.
    let state = if input.bell_pending {
        SessionState::NeedsYou
    } else if input.still_for >= IDLE_AFTER && !input.transcript {
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

    /// The worker samples the screen every fourth flush tick — see
    /// `REASSESS_EVERY` in service.rs. The simulations below step by the same
    /// amount, so what they measure is what the app measures.
    const SAMPLE: Duration = Duration::from_millis(64);

    fn lines(screen: &[&str]) -> Vec<String> {
        screen.iter().map(|s| (*s).to_owned()).collect()
    }

    fn input(bell: bool, still_ms: u64, screen: &[&str]) -> DetectInput<'static> {
        // The screen is leaked on purpose: the test is short-lived but the
        // reference needs to be 'static.
        DetectInput {
            bell_pending: bell,
            still_for: Duration::from_millis(still_ms),
            screen: Box::leak(lines(screen).into_boxed_slice()),
            transcript: false,
            profile: crate::agents::resolve("claude").unwrap(),
        }
    }

    #[test]
    fn while_the_screen_keeps_changing_it_is_work() {
        assert_eq!(detect(input(false, 100, &["building..."])).state, SessionState::Running);
    }

    #[test]
    fn a_screen_that_has_held_still_a_long_time_is_idle() {
        assert_eq!(detect(input(false, 5_000, &["$ "])).state, SessionState::Idle);
    }

    #[test]
    fn idle_is_quiet_not_loud() {
        // An agent that has finished and an agent that is waiting for an
        // answer look identical from outside — both simply stop producing
        // output. Shouting on every pause would make loudness unreadable
        // within a week.
        assert_ne!(detect(input(false, 60_000, &["$ "])).state, SessionState::NeedsYou);
    }

    #[test]
    fn a_bell_is_loud_even_in_the_middle_of_output() {
        assert_eq!(detect(input(true, 10, &["working"])).state, SessionState::NeedsYou);
    }

    #[test]
    fn a_bell_is_loud_when_idle_too() {
        assert_eq!(detect(input(true, 9_000, &["waiting"])).state, SessionState::NeedsYou);
    }

    #[test]
    fn layer_a_knows_nothing_of_the_question() {
        assert!(detect(input(true, 10, &["working"])).question.is_none());
    }

    fn dialog() -> &'static [String] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/claude-permission-dialog.txt");
        let lines: Vec<String> = std::fs::read_to_string(path).unwrap().lines().map(str::to_owned).collect();
        Box::leak(lines.into_boxed_slice())
    }

    #[test]
    fn a_settled_dialog_is_a_question_with_text() {
        let out = detect(DetectInput {
            bell_pending: false,
            still_for: Duration::from_millis(500),
            screen: dialog(),
            transcript: false,
            profile: crate::agents::resolve("claude").unwrap(),
        });
        assert_eq!(out.state, SessionState::NeedsYou);
        assert!(out.question.expect("there is no question").text.ends_with('?'));
    }

    #[test]
    fn a_dialog_still_being_drawn_is_not_trusted_to_the_profile() {
        let out = detect(DetectInput {
            bell_pending: false,
            still_for: Duration::from_millis(20),
            screen: dialog(),
            transcript: false,
            profile: crate::agents::resolve("claude").unwrap(),
        });
        assert!(out.question.is_none(), "the profile believed a half-drawn screen");
        assert_eq!(out.state, SessionState::Running);
    }

    #[test]
    fn the_profile_is_louder_than_idle() {
        let out = detect(DetectInput {
            bell_pending: false,
            still_for: Duration::from_secs(30),
            screen: dialog(),
            transcript: false,
            profile: crate::agents::resolve("claude").unwrap(),
        });
        assert_eq!(out.state, SessionState::NeedsYou);
    }

    #[test]
    fn a_profile_with_no_layer_b_sees_no_question_in_a_dialog_it_could_read() {
        // Same settled, Claude-shaped screen as `a_settled_dialog_is_a_question_with_text`,
        // read by a profile that has no `question` of its own — this proves
        // `detect` actually consults `input.profile` rather than ignoring it.
        let out = detect(DetectInput {
            bell_pending: false,
            still_for: Duration::from_millis(500),
            screen: dialog(),
            transcript: false,
            profile: no_layer_b(),
        });
        assert!(out.question.is_none(), "a profile with no layer B was given one");
    }

    /// A profile with no `question` of its own, which is the trait's default
    /// and the honest answer for any harness whose dialog nobody has taught the
    /// app to read.
    ///
    /// It is written here rather than borrowed from `agents::` because every
    /// shipped profile now matches dialogs. The tests below are about layer A
    /// alone, and running them through a profile that matches would mean they
    /// passed for layer B's reasons rather than their own — and worse, that
    /// they went on passing only for as long as that profile happened to miss
    /// their screens. Borrowing `codex` was true when it was written and stopped
    /// being true the day that profile learnt to read its own dialog
    /// (smetana-603); nothing failed, which is exactly the problem.
    struct NoLayerB;

    impl Profile for NoLayerB {
        fn id(&self) -> &'static str {
            "no-layer-b"
        }

        fn binary(&self) -> &'static str {
            "no-layer-b"
        }

        fn delivery(&self) -> crate::agents::SkillDelivery {
            crate::agents::SkillDelivery::Inline
        }

        fn command(&self, _launch: &crate::agents::Launch) -> portable_pty::CommandBuilder {
            portable_pty::CommandBuilder::new(self.binary())
        }

        // `question` is deliberately left at the trait's default of `None`.
        // That absence is the whole of what this profile is for.
    }

    fn no_layer_b() -> &'static dyn Profile {
        &NoLayerB
    }

    /// One detection tick, as the worker performs it: sample the screen, ask
    /// how long it has looked that way, read a state off the answer. The
    /// simulations below drive this and nothing else, so the arithmetic they
    /// exercise is the arithmetic in `reassess`.
    fn tick(quiet: &mut Quiet, at: Instant, screen: &[String]) -> SessionState {
        tick_as(quiet, at, screen, false)
    }

    /// The same tick, told where the picture comes from. `transcript` is what
    /// service.rs fills from `Live::transcript`, so the two simulations below
    /// differ in exactly the one fact the rule turns on and in nothing else.
    fn tick_as(quiet: &mut Quiet, at: Instant, screen: &[String], batch: bool) -> SessionState {
        let still_for = quiet.still_for(screen, at);
        let profile = no_layer_b();
        let input =
            DetectInput { bell_pending: false, still_for, screen, transcript: batch, profile };
        detect(input).state
    }

    #[test]
    fn the_first_screen_a_session_shows_is_not_stillness() {
        // There was nothing before it to be still against, so the count starts
        // here rather than reaching back to the session's birth.
        let mut quiet = Quiet::new();
        assert_eq!(quiet.still_for(&lines(&["$ "]), Instant::now()), Duration::ZERO);
    }

    #[test]
    fn a_screen_redrawn_identically_holds_still() {
        let mut quiet = Quiet::new();
        let at = Instant::now();
        let screen = lines(&["Do you want to proceed?", "1. Yes", "2. No"]);
        quiet.still_for(&screen, at);
        assert_eq!(
            quiet.still_for(&screen, at + Duration::from_secs(2)),
            Duration::from_secs(2),
            "a redraw of the same text restarted the count"
        );
    }

    #[test]
    fn one_character_moving_is_a_change() {
        let mut quiet = Quiet::new();
        let at = Instant::now();
        quiet.still_for(&lines(&["⠋ Thinking"]), at);
        assert_eq!(
            quiet.still_for(&lines(&["⠙ Thinking"]), at + Duration::from_secs(2)),
            Duration::ZERO,
            "a spinner turning was mistaken for a still screen"
        );
    }

    #[test]
    fn a_dialog_that_repaints_itself_forever_still_goes_quiet() {
        // The measured shape of the bug: Claude Code 2.1 emits a ~40 byte
        // chunk about every 0.61 s while a permission prompt stands open, and
        // redraws the same text with it. Under the old rule — quiet meaning no
        // bytes — the count never reached `IDLE_AFTER`, so a session waiting
        // for a human read as busy for as long as it waited.
        let mut quiet = Quiet::new();
        let start = Instant::now();
        let screen = lines(&["Do you want to proceed?", "1. Yes", "2. No"]);
        let mut state = SessionState::Starting;
        let mut at = start;
        // Five seconds of the worker's own sampling rate, and a repaint
        // arriving between samples changes nothing this can see.
        while at < start + Duration::from_secs(5) {
            state = tick(&mut quiet, at, &screen);
            at += SAMPLE;
        }
        assert_eq!(state, SessionState::Idle, "a repainting dialog still counted as work");
    }

    #[test]
    fn an_unread_permission_dialog_stops_reading_as_work() {
        // The same thing against a real captured screen, read by a profile
        // with no layer B — which is the case this task is about: when the
        // profile fails to recognise its own dialog, layer A must at least
        // stop claiming the agent is busy.
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/claude-2.1-permission-bash.txt");
        let screen: Vec<String> =
            std::fs::read_to_string(path).unwrap().lines().map(str::to_owned).collect();
        let mut quiet = Quiet::new();
        let start = Instant::now();
        let mut state = SessionState::Starting;
        let mut at = start;
        while at < start + Duration::from_secs(5) {
            state = tick(&mut quiet, at, &screen);
            at += SAMPLE;
        }
        assert_eq!(state, SessionState::Idle);
        // And it is still not loud: nothing here guesses that a still screen
        // is a screen with a question on it. Loudness is the bell's or the
        // profile's to raise.
        assert_ne!(state, SessionState::NeedsYou);
    }

    #[test]
    fn a_working_agent_is_never_called_idle() {
        // The regression this rule had to be written not to cause. An agent
        // that is actually working repaints something that differs — a
        // spinner, a counter, a line of output — and every one of those
        // restarts the count, however long the run lasts.
        let mut quiet = Quiet::new();
        let start = Instant::now();
        let mut at = start;
        let mut step = 0u32;
        while at < start + Duration::from_secs(30) {
            let line = format!("Building… {step} files");
            assert_eq!(
                tick(&mut quiet, at, &lines(&[line.as_str()])),
                SessionState::Running,
                "a busy agent was called idle after {:?}",
                at - start
            );
            at += SAMPLE;
            step += 1;
        }
    }

    /// The whole of what a batch's pane holds while a tool call runs: the row
    /// `agents::claude::transcript_line` renders for the `tool_use` block that
    /// started the call, and then nothing at all until it returns.
    ///
    /// Rendered here rather than transcribed, so the fixture cannot drift from
    /// what a batch actually shows — this *is* that renderer's output. Note
    /// what is therefore absent and could not be present: the stream carries no
    /// event for a call in progress, so there is no second row saying it is
    /// running, and none of the interactive TUI's glyphs reach a pane
    /// `transcript_line` composes.
    fn mid_tool_call() -> Vec<String> {
        let pane = crate::agents::claude::transcript_line(
            r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"cargo test --manifest-path src-tauri/Cargo.toml"}}]}}"#,
        );
        // A typo in that JSON would render nothing, and an empty screen holds
        // still just as well — the simulations below would pass having measured
        // a pane no agent ever produced. How many rows it renders is
        // `claude.rs`'s own business and no stake of theirs: any still,
        // non-empty screen serves them equally.
        assert!(!pane.is_empty(), "the batch's pane rendered no row for a tool call");
        pane
    }

    #[test]
    fn a_batch_between_two_stream_events_is_never_called_idle() {
        // The defect (smetana-07o). A batch runs its harness in print mode, so
        // between one tool call starting and the next event arriving it emits
        // nothing at all and the screen holds still for the whole call. Five
        // minutes of `cargo test` is five minutes of stillness under an agent
        // working flat out.
        let mut quiet = Quiet::new();
        let start = Instant::now();
        let screen = mid_tool_call();
        let mut at = start;
        while at < start + Duration::from_secs(300) {
            assert_eq!(
                tick_as(&mut quiet, at, &screen, true),
                SessionState::Running,
                "a working batch was called idle after {:?}",
                at - start
            );
            at += SAMPLE;
        }
    }

    #[test]
    fn the_same_still_screen_goes_idle_for_a_session_a_person_started() {
        // The counterpart, and what makes the test above about the flag rather
        // than about the screen: nothing else has moved. An ordinary session
        // whose harness draws for a person still reaches `Idle` at
        // `IDLE_AFTER`, off this very screen.
        let mut quiet = Quiet::new();
        let start = Instant::now();
        let screen = mid_tool_call();
        let mut state = SessionState::Starting;
        let mut at = start;
        while at < start + Duration::from_secs(5) {
            state = tick_as(&mut quiet, at, &screen, false);
            at += SAMPLE;
        }
        assert_eq!(state, SessionState::Idle, "the idle rule moved for everybody else");
    }

    #[test]
    fn a_bell_is_still_loud_for_a_batch() {
        // The idle rule is the whole of what this flag switches off. A bell is
        // the fail-safe layer A keeps whatever the screen is doing, and losing
        // it here would be silent — a batch that rang for a person would sit
        // reading as work.
        let screen = mid_tool_call();
        let out = detect(DetectInput {
            bell_pending: true,
            still_for: Duration::from_secs(600),
            screen: &screen,
            transcript: true,
            profile: no_layer_b(),
        });
        assert_eq!(out.state, SessionState::NeedsYou);
    }

    #[test]
    fn layer_b_is_still_loud_for_a_batch() {
        // The other fail-safe. A profile that reads a dialog on the screen is
        // believed for a batch exactly as for anybody else — the flag is read
        // after layer B has had its say, not instead of it.
        let out = detect(DetectInput {
            bell_pending: false,
            still_for: Duration::from_secs(600),
            screen: dialog(),
            transcript: true,
            profile: crate::agents::resolve("claude").unwrap(),
        });
        assert_eq!(out.state, SessionState::NeedsYou);
        assert!(out.question.is_some(), "a batch's dialog was read as no question");
    }

    #[test]
    fn an_agent_that_has_finished_goes_idle_at_the_same_three_seconds() {
        // The other half of the promise: nothing about an ordinary session's
        // road to idle has moved. For an agent that has genuinely stopped, the
        // last byte and the last change to the screen are the same moment, so
        // the threshold is measured from the same instant it always was.
        let mut quiet = Quiet::new();
        let start = Instant::now();
        let mut at = start;
        for step in 0..10u32 {
            let line = format!("step {step}");
            tick(&mut quiet, at, &lines(&[line.as_str()]));
            at += SAMPLE;
        }
        let last_change = at - SAMPLE;
        let final_screen = lines(&["step 9"]);
        assert_eq!(
            tick(&mut quiet, last_change + Duration::from_millis(2_900), &final_screen),
            SessionState::Running,
            "idle arrived early"
        );
        assert_eq!(
            tick(&mut quiet, last_change + IDLE_AFTER, &final_screen),
            SessionState::Idle,
            "idle did not arrive at IDLE_AFTER"
        );
    }

    #[test]
    fn our_own_input_restarts_the_count() {
        // `terminal_run_capture` writes into a session and then waits. The
        // screen has not answered yet, and stillness measured from before that
        // write would report the session idle while it is being driven.
        let mut quiet = Quiet::new();
        let at = Instant::now();
        let screen = lines(&["$ "]);
        quiet.still_for(&screen, at);
        let wrote_at = at + Duration::from_secs(60);
        quiet.restart(wrote_at);
        assert_eq!(quiet.still_for(&screen, wrote_at), Duration::ZERO);
        assert_eq!(
            quiet.still_for(&screen, wrote_at + Duration::from_secs(1)),
            Duration::from_secs(1),
            "the restart threw away the screen as well as the clock"
        );
    }
}
