//! Keeping the machine awake while a run is going.
//!
//! A run is the app driving itself for hours, usually at night and usually with
//! nobody in the room. Nothing in it touches the keyboard or the mouse, so as
//! far as the operating system can tell the machine is idle: the idle timer
//! runs to its end and suspends the agent sessions, the PTYs and the loop task
//! waiting on them all at once. What the person finds in the morning is a run
//! that stopped at whatever minute the machine gave up, with no ending, no
//! report and no reason on the bar. The cost is largest in the case the runs
//! subsystem was built for — a run paused on a spent allowance (`usage.rs`)
//! holds no session and prints nothing for hours *by design*, which is exactly
//! the shape an idle timer reads as an empty room.
//!
//! **The promise is "the machine does not fall asleep by itself while a run is
//! going", not "the machine cannot sleep".** The difference is the bug report
//! that arrives otherwise, so it is written down in both places, here and in
//! `.claude/rules/runs.md`:
//!
//! - on macOS, closing a laptop's lid suspends the machine whatever assertions
//!   are held; no API changes that, only mains power with an external display;
//! - sleep a person chooses from a menu goes through;
//! - a forced sleep on a critically low battery goes through.
//!
//! The system is held and the display is not (`display(false)`, `idle(true)`,
//! `sleep(true)`). Nothing in a run depends on anything being drawn — the
//! browser live check (`browser.rs`) drives Playwright or the Chrome extension,
//! and neither needs a lit screen — so holding the backlight on all night would
//! be battery and a bright room bought for nothing.
//!
//! Taking the hold sits behind [`Power`] so that the counting rule below, which
//! is the part that can be wrong, is tested without asking the operating system
//! for anything.

/// Where a hold comes from. One implementation talks to the machine
/// ([`System`]) and one counts calls for the tests; nothing else needs to
/// exist.
///
/// Releasing has no method: a hold is released by being dropped, which is what
/// makes the worker task's own ending — including a panic unwinding through it
/// — release too.
pub trait Power {
    /// The hold itself, opaque to everything here.
    type Hold;

    /// Ask the machine to stay awake. The error is already a sentence for the
    /// log: there is nothing else to do with it, since a failure here never
    /// blocks a run.
    fn take(&self) -> Result<Self::Hold, String>;
}

/// What the keeper is holding, and the only thing it remembers.
///
/// Three states rather than an `Option`, because "asked and was refused" has to
/// be told apart from "nothing is held": both mean no assertion, and only one
/// of them is worth asking about again on the next pass of the worker loop.
enum Held<H> {
    /// No run is live. The next rise above zero asks.
    Nothing,
    /// A run is live and the machine has been asked.
    Hold(H),
    /// A run is live, the machine was asked on the rise above zero and said no
    /// — no logind on this Linux, a platform the crate cannot serve, an error
    /// from the OS. Not asked again until the count falls back to zero: a retry
    /// on every pass of the worker loop would write the same line to the log
    /// for eight hours.
    Refused,
}

/// One hold for the whole app, taken while any run is live anywhere.
///
/// The count is **derived, never stored**: `sync` is handed `active.len()` from
/// the run worker's map on every pass, and this struct keeps no count and no
/// flag of its own — see the rule this leans on in `service.rs`, where the map
/// has exactly two mutation points and the removal is guaranteed for every
/// ending by the `Ending` drop guard. Two halves of one fact drift, and the
/// drift here would be silent in both directions: a machine held awake for a
/// week by a flag nobody cleared, or a run that quietly lost its hold and
/// stopped at three in the morning.
pub struct Keeper<P: Power> {
    power: P,
    held: Held<P::Hold>,
}

impl<P: Power> Keeper<P> {
    pub fn new(power: P) -> Self {
        Self { power, held: Held::Nothing }
    }

    /// How many runs are live anywhere, from the worker's map. Called on every
    /// pass rather than at the two edges, so that no ending has to remember to
    /// release and no new stop reason has to be added to a list.
    ///
    /// "Anywhere" and not "in this project": the app holds several projects and
    /// a project holds several runs (smetana-5hf). The hold is taken when the
    /// first run starts anywhere and released when the last one ends.
    pub fn sync(&mut self, active: usize) {
        if active == 0 {
            // Dropping it *is* the release; nothing else happens here.
            self.held = Held::Nothing;
            return;
        }
        // Already held, or asked and refused on this stretch above zero.
        if !matches!(self.held, Held::Nothing) {
            return;
        }
        self.held = match self.power.take() {
            Ok(hold) => Held::Hold(hold),
            Err(err) => {
                // `usage.rs`'s rule — an unreadable answer never blocks a run —
                // rather than `browser.rs`'s, and the difference between the
                // two is who is reading. `browser.rs` is loud because somebody
                // is looking at a toggle; here there is nothing on screen at
                // all, and refusing to start would trade a night's work for a
                // power assertion.
                log::warn!("could not keep the machine awake: {err}; the run goes ahead");
                Held::Refused
            }
        };
    }
}

/// The machine, through `keepawake`: IOKit on macOS, `SetThreadExecutionState`
/// on Windows, logind and the screensaver interface over D-Bus on Linux.
pub struct System;

/// What `pmset -g assertions` and `systemd-inhibit --list` show against the
/// assertion, so that somebody reading either can tell what put it there.
const REASON: &str = "Smetana is carrying out a run";

impl Power for System {
    type Hold = keepawake::KeepAwake;

    fn take(&self) -> Result<Self::Hold, String> {
        keepawake::Builder::default()
            // The system only. See the module's note: nothing in a run depends
            // on the screen being lit.
            .display(false)
            .idle(true)
            .sleep(true)
            .reason(REASON)
            .app_name("Smetana")
            .app_reverse_domain("com.invisor.smetana")
            .create()
            .map_err(|err| err.to_string())
    }
}

/// The keeper the run worker owns.
pub fn system() -> Keeper<System> {
    Keeper::new(System)
}

/// The counting rule, over a `Power` that touches nothing: the four edges, and
/// what a refusal does and does not change.
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    /// What the double hands out. Releasing is dropping, the same as the real
    /// one, so the count of releases is kept by this rather than by `Fake`.
    struct FakeHold {
        released: Rc<Cell<u32>>,
    }

    impl Drop for FakeHold {
        fn drop(&mut self) {
            self.released.set(self.released.get() + 1);
        }
    }

    #[derive(Default)]
    struct Fake {
        asked: Rc<Cell<u32>>,
        released: Rc<Cell<u32>>,
        refuse: Rc<Cell<bool>>,
    }

    impl Power for Fake {
        type Hold = FakeHold;

        fn take(&self) -> Result<Self::Hold, String> {
            self.asked.set(self.asked.get() + 1);
            if self.refuse.get() {
                return Err("no logind on this machine".into());
            }
            Ok(FakeHold { released: self.released.clone() })
        }
    }

    /// A keeper over a double, with the three counters kept to hand.
    fn keeper() -> (Keeper<Fake>, Rc<Cell<u32>>, Rc<Cell<u32>>, Rc<Cell<bool>>) {
        let power = Fake::default();
        let (asked, released, refuse) =
            (power.asked.clone(), power.released.clone(), power.refuse.clone());
        (Keeper::new(power), asked, released, refuse)
    }

    #[test]
    fn nothing_is_taken_while_no_run_is_live() {
        let (mut keeper, asked, _released, _refuse) = keeper();
        keeper.sync(0);
        keeper.sync(0);
        assert_eq!(asked.get(), 0, "an app sitting idle asks the machine for nothing");
    }

    #[test]
    fn the_first_run_anywhere_takes_the_hold() {
        let (mut keeper, asked, released, _refuse) = keeper();
        keeper.sync(0);
        keeper.sync(1);
        assert_eq!(asked.get(), 1);
        assert_eq!(released.get(), 0);
    }

    #[test]
    fn a_second_run_beside_the_first_does_not_take_another() {
        // One hold for the app, not one per run: two runs in two projects, or
        // two scopes of one project, are still one assertion.
        let (mut keeper, asked, released, _refuse) = keeper();
        keeper.sync(1);
        keeper.sync(2);
        assert_eq!(asked.get(), 1, "the hold is the app's, not the run's");
        assert_eq!(released.get(), 0);
    }

    #[test]
    fn the_hold_survives_every_pass_while_a_run_stays_live() {
        // The worker calls `sync` on every pass of its loop, which is once per
        // request and once per report — hundreds of times over a run.
        let (mut keeper, asked, released, _refuse) = keeper();
        for _ in 0..50 {
            keeper.sync(1);
        }
        assert_eq!(asked.get(), 1);
        assert_eq!(released.get(), 0, "and a paused run keeps it for the whole pause");
    }

    #[test]
    fn the_first_of_two_runs_ending_does_not_release() {
        let (mut keeper, asked, released, _refuse) = keeper();
        keeper.sync(2);
        keeper.sync(1);
        assert_eq!(released.get(), 0, "one run is still live, so the machine stays awake");
        assert_eq!(asked.get(), 1, "and nothing is taken a second time either");
    }

    #[test]
    fn the_last_run_ending_releases_the_hold() {
        let (mut keeper, _asked, released, _refuse) = keeper();
        keeper.sync(2);
        keeper.sync(1);
        keeper.sync(0);
        assert_eq!(released.get(), 1);
    }

    #[test]
    fn a_run_ending_any_way_at_all_releases_because_the_count_is_the_map() {
        // There is no list of stop reasons here to keep in step: an empty
        // queue, a stop from the bar during the preflight, a crash and a panic
        // unwinding through the loop task all reach this as the same fall to
        // zero, because the map's `Ending` guard takes the entry out for all of
        // them.
        let (mut keeper, asked, released, _refuse) = keeper();
        keeper.sync(1);
        keeper.sync(0);
        assert_eq!(released.get(), 1);
        keeper.sync(1);
        assert_eq!(asked.get(), 2, "and the next run takes a fresh one");
        assert_eq!(released.get(), 1);
    }

    #[test]
    fn dropping_the_keeper_releases_the_hold() {
        // The worker task's own ending, which is the app quitting.
        let (mut keeper, _asked, released, _refuse) = keeper();
        keeper.sync(1);
        drop(keeper);
        assert_eq!(released.get(), 1);
    }

    #[test]
    fn a_refused_hold_does_not_stop_the_run_or_the_counting() {
        let (mut keeper, asked, released, refuse) = keeper();
        refuse.set(true);
        keeper.sync(1);
        assert_eq!(asked.get(), 1, "asked once and told no");
        // The counting carries on regardless: the point of the assertion is
        // that a failure here is a line in the log and nothing else.
        keeper.sync(2);
        keeper.sync(1);
        keeper.sync(0);
        assert_eq!(released.get(), 0, "there was never anything to release");
    }

    #[test]
    fn a_refusal_is_not_retried_until_the_count_returns_to_zero() {
        // Retrying on every pass would fill the log with one line for eight
        // hours; not retrying at all would mean one refusal at midnight cost
        // every later run of the session its hold.
        let (mut keeper, asked, released, refuse) = keeper();
        refuse.set(true);
        keeper.sync(1);
        for _ in 0..20 {
            keeper.sync(1);
            keeper.sync(2);
        }
        assert_eq!(asked.get(), 1, "one line in the log, not twenty");

        keeper.sync(0);
        refuse.set(false);
        keeper.sync(1);
        assert_eq!(asked.get(), 2, "the next rise above zero tries again");
        keeper.sync(0);
        assert_eq!(released.get(), 1, "and that one was a real hold");
    }

    #[test]
    fn the_hold_the_worker_holds_is_send() {
        // It lives in the run worker's tokio task and is held across every
        // `await` in the loop, so a hold that was not `Send` on the platform
        // being built would not compile in `service.rs` at all. Checked here so
        // the failure names the reason.
        fn assert_send<T: Send>() {}
        assert_send::<Keeper<System>>();
    }
}
