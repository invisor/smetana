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
///
/// **It guards entering `NeedsYou` and not staying there** — the asymmetry is
/// `detect`'s and is explained there.
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
    ///
    /// `None` for a session that runs no agent, which today is the person's own
    /// shell (`SessionWork::Shell`). Layer A still runs over it and is welcome
    /// to: a shell that rings the bell has rung it for the person sitting in
    /// front of it, and nothing in this app acts on a shell's state — it has no
    /// row in the agents panel, and notifications are raised by a run. What
    /// there is no honest answer for is layer B, which is one named harness's
    /// interface being read; a shell has no harness, so the reading is skipped
    /// rather than handed to whichever profile happened to be configured.
    pub profile: Option<&'static dyn Profile>,
    /// The state this session was in when this tick started — `Session::state`
    /// as `reassess` read it, before the `apply` this answer feeds.
    ///
    /// It is the one thing layer B's threshold is asymmetric about, and
    /// `detect` is where that rule is written down. It arrives as a field
    /// rather than being looked up from the session here, which is what keeps
    /// this function pure: the tests below drive it with a state of their own
    /// and no session and no clock anywhere near them.
    pub was: SessionState,
}

pub struct Detected {
    pub state: SessionState,
    pub question: Option<Question>,
}

pub fn detect(input: DetectInput) -> Detected {
    // Layer B: the profile knows exactly what is being asked, so it takes
    // precedence.
    //
    // **`SETTLE` is a condition for entering `NeedsYou`, never for staying in
    // it**, and that asymmetry is the whole of this rule. A person typing an
    // answer redraws the screen on every keystroke, so it never settles while
    // they type: asking layer B only on a settled screen meant the dialog
    // standing right in front of them went unread on those ticks, layer A
    // answered `Running`, and the agent row, both counters and the project tile
    // flickered between yellow and blue at the speed of typing (smetana-4a6).
    // A session that is already `NeedsYou` has read this very frame once on a
    // settled screen, so there is nothing left for the threshold to protect —
    // layer B is asked anyway, and a dialog it can still see keeps the state
    // where it is.
    //
    // What releases it is layer B itself rather than a timer: the moment the
    // person presses Return and the agent wipes the dialog, nothing matches and
    // layer A has the very next tick, with no ceiling to wait out. The price,
    // named in advance: if the profile fails to read one half-drawn frame in
    // the middle of typing, the state dips to `Running` for a single tick
    // (~64 ms). That is an order of magnitude shorter than the flicker it
    // replaces, and deliberately not smoothed away by counting consecutive
    // misses — a counter would be state, and this function has none.
    let holding = input.was == SessionState::NeedsYou;
    if input.still_for >= SETTLE || holding {
        if let Some(question) = input.profile.and_then(|p| p.question(input.screen)) {
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
            profile: Some(crate::agents::resolve("claude").unwrap()),
            // A session that has not been loud before this tick. The tests
            // about holding `NeedsYou` say so for themselves rather than
            // borrowing this default.
            was: SessionState::Running,
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
            profile: Some(crate::agents::resolve("claude").unwrap()),
            was: SessionState::Running,
        });
        assert_eq!(out.state, SessionState::NeedsYou);
        assert!(out.question.expect("there is no question").text.ends_with('?'));
    }

    #[test]
    fn a_dialog_still_being_drawn_is_not_trusted_to_the_profile() {
        // Entering `NeedsYou`, which is the half `SETTLE` still guards: this
        // session has never been loud, so there is no earlier settled reading
        // of this screen behind it and a half-drawn frame is all there is.
        let out = detect(DetectInput {
            bell_pending: false,
            still_for: Duration::from_millis(20),
            screen: dialog(),
            transcript: false,
            profile: Some(crate::agents::resolve("claude").unwrap()),
            was: SessionState::Running,
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
            profile: Some(crate::agents::resolve("claude").unwrap()),
            was: SessionState::Running,
        });
        assert_eq!(out.state, SessionState::NeedsYou);
    }

    /// The session a person opened a shell in: no agent, so no layer B, and
    /// layer A left running because there is nothing about it to switch off.
    #[test]
    fn a_session_with_no_agent_behind_it_still_gets_layer_a() {
        let quiet = detect(DetectInput {
            bell_pending: false,
            still_for: Duration::from_secs(30),
            screen: dialog(),
            transcript: false,
            profile: None,
            was: SessionState::Running,
        });
        // A dialog a profile would have read, on a screen that has no profile
        // to read it: the shell is simply quiet, which is what it looks like.
        assert!(quiet.question.is_none(), "a session with no profile was given a question");
        assert_eq!(quiet.state, SessionState::Idle);

        let rang = detect(DetectInput {
            bell_pending: true,
            still_for: Duration::from_millis(0),
            screen: dialog(),
            transcript: false,
            profile: None,
            was: SessionState::Running,
        });
        // The bell is the person's own `\a`, and it costs nothing to believe.
        assert_eq!(rang.state, SessionState::NeedsYou);
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
            profile: Some(no_layer_b()),
            was: SessionState::Running,
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
        let profile = Some(no_layer_b());
        // A profile with no layer B is asked either way and answers `None`
        // either way, so what this tick was in before it does not reach the
        // answer: these simulations are about layer A alone.
        let was = SessionState::Running;
        let input =
            DetectInput { bell_pending: false, still_for, screen, transcript: batch, profile, was };
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
            profile: Some(no_layer_b()),
            was: SessionState::Running,
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
            profile: Some(crate::agents::resolve("claude").unwrap()),
            was: SessionState::Running,
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

    /// A real permission dialog off claude 2.1, with one row of a half-typed
    /// answer under the options.
    ///
    /// **That row is synthetic and the rest of the screen is not**, which is
    /// the split worth knowing before trusting the tests below. The fixture is
    /// `claude-2.1-permission-edit.txt`, captured under a PTY from a live
    /// session; reaching the frame these tests want live would mean driving a
    /// real agent into a permission prompt, choosing the option that opens a
    /// free-text field and photographing the screen between two keystrokes.
    /// That costs model quota for a row whose only load-bearing property is
    /// that it differs from the one sampled before it — the dialog above it,
    /// which is what layer B actually reads, is the real capture and is
    /// untouched.
    fn dialog_being_answered(typed: &str) -> Vec<String> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/claude-2.1-permission-edit.txt");
        let mut screen: Vec<String> =
            std::fs::read_to_string(path).unwrap().lines().map(str::to_owned).collect();
        // Under the options, where the free-text field is drawn: the question
        // and the numbered block a profile reads both sit above it, which is
        // why typing cannot change what layer B sees.
        screen.push(format!("> {typed}"));
        screen
    }

    /// One detection tick with a profile that does read dialogs, told what the
    /// session was in before it. `tick_as` above cannot serve: it is fixed to
    /// `no_layer_b`, and layer B is the whole subject here.
    fn tick_watched(
        quiet: &mut Quiet,
        at: Instant,
        screen: &[String],
        was: SessionState,
    ) -> Detected {
        let still_for = quiet.still_for(screen, at);
        detect(DetectInput {
            bell_pending: false,
            still_for,
            screen,
            transcript: false,
            profile: Some(crate::agents::resolve("claude").unwrap()),
            was,
        })
    }

    #[test]
    fn typing_an_answer_never_takes_the_session_out_of_needs_you() {
        // The defect (smetana-4a6). Every keystroke changed the picture, so the
        // screen never held still for `SETTLE` and layer B was not asked at
        // all; layer A saw a rung-and-cleared bell well short of `IDLE_AFTER`
        // and answered `Running`. The dialog had not moved — only the input row
        // under it — so the row, the counters and the project tile flickered
        // yellow to blue and back at the speed of somebody typing.
        let mut quiet = Quiet::new();
        let start = Instant::now();
        let mut state = SessionState::Starting;
        let mut at = start;

        // The dialog stands untouched until it settles: this is the entry,
        // which is what `SETTLE` still guards.
        let waiting = dialog_being_answered("");
        while at < start + Duration::from_secs(1) {
            state = tick_watched(&mut quiet, at, &waiting, state).state;
            at += SAMPLE;
        }
        assert_eq!(state, SessionState::NeedsYou, "a settled dialog was not read at all");

        // And now somebody answers it, one character per sample — nothing else
        // about the screen moves.
        let answer = "use the Write tool instead";
        for taken in 1..=answer.len() {
            let screen = dialog_being_answered(&answer[..taken]);
            let out = tick_watched(&mut quiet, at, &screen, state);
            state = out.state;
            assert_eq!(
                state,
                SessionState::NeedsYou,
                "typing {taken} characters of an answer moved the session off needs-you"
            );
            // The question travels with the state on every one of those ticks:
            // `Session::apply` drops it on any other state, so a dip would
            // blank the panel's phrase as well as its colour.
            assert!(out.question.is_some(), "the question was lost while it was being answered");
            at += SAMPLE;
        }
    }

    #[test]
    fn the_dialog_leaving_the_screen_ends_needs_you_on_the_next_tick() {
        // The other half of the rule, and what makes holding safe: the release
        // is layer B failing to match, not a clock. The person presses Return,
        // the agent wipes the dialog and goes back to work, and the very first
        // sample of that screen is `Running` — 64 ms after the change, with
        // `SETTLE` nowhere in it.
        let mut quiet = Quiet::new();
        let start = Instant::now();
        let mut state = SessionState::Starting;
        let mut at = start;
        let waiting = dialog_being_answered("");
        while at < start + Duration::from_secs(1) {
            state = tick_watched(&mut quiet, at, &waiting, state).state;
            at += SAMPLE;
        }
        assert_eq!(state, SessionState::NeedsYou);

        // No numbered block and no cursor on one: an agent working again.
        let working = lines(&["⏺ Update(tabs.js)", "  Updated tabs.js", "✻ Thinking…"]);
        let out = tick_watched(&mut quiet, at, &working, state);
        assert_eq!(out.state, SessionState::Running, "yellow stuck after the dialog had gone");
        assert!(out.question.is_none());
    }
}
