//! How many file descriptors this process — and therefore every agent it
//! starts — is allowed to hold.
//!
//! A bundled app is launched by launchd, and launchd's per-process default is
//! `maxfiles 256 unlimited` (`launchctl limit maxfiles`): a soft limit of 256
//! against a hard limit of infinity. A terminal window on the same machine
//! starts at a million, because the login shell raises the soft limit on its
//! way up; nothing does that for us. `terminal::pty` starts every agent as a
//! child of this process, and a child inherits both limits, so the 256 is what
//! reaches the agent. Node lifts what it can on its own and lands on 2560,
//! which is where the failure is finally printed — by claude, into the pane a
//! person is watching, as advice they cannot follow:
//!
//! ```text
//! error: An unknown error occurred, possibly due to low max file descriptors
//! Current limit: 2560
//! To fix this, try running: ulimit -n 2147483646
//! ```
//!
//! There is no shell between us and the agent to type that into. The place the
//! limit can still be moved is here, before anything is spawned, which is why
//! this runs as the first statement of `run()` rather than from the setup hook:
//! a limit raised after a child exists does that child no good at all.
//!
//! Only the soft limit is touched. The hard limit is left exactly as found, in
//! both directions: raising it needs privilege we do not have, and lowering it
//! is one-way for the life of the process — a `ulimit -n 256` that quietly sets
//! both is how a shell locks itself out of ever raising the number again.
//!
//! ## Why the hard limit is not the answer
//!
//! The obvious `soft = hard` is wrong twice over, once per platform, and the
//! two failures look nothing alike.
//!
//! On macOS the hard limit is `RLIM_INFINITY`, which is not a number of
//! descriptors anybody gets. What happens to a process that asks for it depends
//! on the release: older macOS refuses the call outright with `EINVAL`, leaving
//! the soft limit at 256 and the bug unfixed, while Darwin 25.5 accepts it
//! (measured: `ulimit -Sn unlimited` succeeds in `/bin/sh`) and hands back a
//! soft limit the kernel will not honour, because descriptors are capped per
//! process by `kern.maxfilesperproc` whatever the rlimit says — 122880 on the
//! machine in the bug report. One path leaves the limit too low and the other
//! makes it a lie, which is why the sysctl is read at all: it is the one number
//! here that is both accepted and true, and where it comes to less than what we
//! ask for, it is what we ask for instead.
//!
//! On Linux the hard limit is a real number and the call would succeed, which
//! is the more insidious half: under systemd it is commonly 524288 or 1048576,
//! so `soft = hard` would hand every agent a million and call it a fix. systemd
//! keeps the *soft* default low on purpose. A program that sizes an array by
//! `getrlimit(RLIMIT_NOFILE)`, or closes descriptors by looping to it, pays for
//! the whole range whether or not anything is open, and select-based code stops
//! working once a descriptor number passes `FD_SETSIZE`.
//!
//! So neither limit is asked for verbatim. This module asks for a number it
//! wants, and the limits it reads only ever reduce that number.

use std::sync::OnceLock;

/// What a usable descriptor limit is, for us: the ceilings below only ever
/// clamp this, never raise it. The bug needs a number above the 2560 an agent
/// refused to work at, not the largest number the machine will part with —
/// 65536 is twenty-five times that, comfortably past anything an agent and the
/// tools it spawns will open at once, and small enough that a program sizing
/// itself by `getrlimit` (see the header) costs nothing. It is also the value
/// production configurations converge on for the same reason.
///
/// The `cfg_attr` here and on the two below is the whole of the Windows story:
/// the constants and the decision are still compiled and still tested there,
/// they simply have no caller, because `raise()` on a platform with no
/// `setrlimit` has an empty body.
#[cfg_attr(not(unix), allow(dead_code))]
const WANTED: u64 = 65_536;

/// The pessimistic ceiling, used only when the first attempt is refused. That
/// happens when nothing on the machine would say what the real cap is and the
/// number we picked overshot it, which in practice means an older macOS whose
/// sysctl could not be read. 10240 is that platform's historical `OPEN_MAX`,
/// the value `ulimit -n` has been willing to accept across every release of it,
/// and forty times launchd's default of 256: less than we wanted, and far more
/// than the limit that produced the bug.
#[cfg_attr(not(unix), allow(dead_code))]
const FALLBACK_CEILING: u64 = 10_240;

/// The pure decision, and the only part of this module a test can reach.
///
/// `hard` and `ceiling` are both `Option` for the same reason, and the reason
/// is not "an error": `None` for the hard limit means `RLIM_INFINITY` — a
/// permission to ask for anything rather than a quantity — and `None` for the
/// ceiling means this platform exports no per-process cap worth reading, which
/// is every Unix that is not macOS. `None` coming back means the soft limit is
/// already at least as good as anything we would ask for, and nothing should be
/// called at all.
///
/// The last line is the one that matters most, and it is the reason this
/// function can only ever be an improvement: a proposal at or below `soft` is
/// dropped rather than made. Everything above it is a ceiling being applied, so
/// a machine whose hard limit or per-process cap sits *below* a soft limit
/// somebody already raised — a terminal-launched build, at a million, against a
/// sysctl of 122880 — falls out here as `None` instead of as a downgrade. That
/// downgrade is the one irreversible thing this module could do to a running
/// process, since the descriptors it would drop are gone for good.
#[cfg_attr(not(unix), allow(dead_code))]
fn target_soft(soft: u64, hard: Option<u64>, ceiling: Option<u64>) -> Option<u64> {
    let mut want = WANTED;
    // A finite hard limit is the kernel's own statement of what this process may
    // have, and asking past it is an error rather than a negotiation.
    if let Some(hard) = hard {
        want = want.min(hard);
    }
    // On macOS the two are independent, and the smaller of them is what
    // actually opens files.
    if let Some(ceiling) = ceiling {
        want = want.min(ceiling);
    }
    (want > soft).then_some(want)
}

/// What `raise` had to complain about, kept until there is a log to say it to.
///
/// `raise` runs as the first statement of `run()` — before the Tauri builder,
/// so before the log plugin exists — and a `log::warn!` made there reaches no
/// logger and is dropped where it stands. Writing to stderr instead is what
/// this module used to do, and stderr is what a bundled `.app` has none of, so
/// the one line that says an agent is about to be started with a limit it
/// cannot work at went nowhere in exactly the build that matters. The line is
/// held here instead and `lib.rs` asks for it once the plugin is registered.
///
/// One string and not a list: `raise` complains at most once per call, and it
/// is called once.
static COMPLAINT: OnceLock<String> = OnceLock::new();

/// Say whatever `raise` had to say. Called from `lib.rs`'s setup hook, right
/// after the log plugin is installed; a no-op when the limit was raised without
/// argument, which is the ordinary case.
pub fn report() {
    if let Some(complaint) = COMPLAINT.get() {
        log::warn!("{complaint}");
    }
}

/// Raise this process's soft `RLIMIT_NOFILE` to a usable number — `WANTED`, or
/// as much of it as the machine allows. Never panics, never blocks and never
/// reports upwards: the worst outcome is a line in the log and an app that
/// comes up with the limit it was given, which is where it was before this
/// module existed.
#[cfg(unix)]
pub fn raise() {
    let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        complain(format!(
            "[rlimit] could not read RLIMIT_NOFILE: {}",
            std::io::Error::last_os_error()
        ));
        return;
    }
    let soft = limit.rlim_cur as u64;
    let hard = (limit.rlim_max != libc::RLIM_INFINITY).then_some(limit.rlim_max as u64);
    let Some(want) = target_soft(soft, hard, per_process_ceiling()) else {
        return;
    };
    if set_soft(want, limit.rlim_max) {
        return;
    }
    let refused = std::io::Error::last_os_error();
    // There is a cap here that nothing on this machine would tell us about.
    // Ask again pretending we know nothing at all, and only when that comes to
    // a genuinely smaller request than the one just refused — otherwise the
    // retry is the same call twice and the log line would be a lie.
    match target_soft(soft, hard, Some(FALLBACK_CEILING)) {
        Some(retry) if retry < want && set_soft(retry, limit.rlim_max) => {
            complain(format!(
                "[rlimit] {want} descriptors was refused ({refused}); settled for {retry}"
            ));
        }
        _ => {
            complain(format!(
                "[rlimit] could not raise the descriptor limit from {soft} to {want}: {refused}; \
                 an agent started from here may refuse to run"
            ));
        }
    }
}

/// Windows has no `setrlimit` and no equivalent to inherit — a handle count is
/// not a rationed thing there — so the whole subject is absent rather than
/// unsupported, and the call site stays free of a `cfg`.
#[cfg(not(unix))]
pub fn raise() {}

/// Put a complaint where `report` will find it. The second one in a process
/// would be dropped, and there is no second one to drop.
#[cfg_attr(not(unix), allow(dead_code))]
fn complain(line: String) {
    let _ = COMPLAINT.set(line);
}

/// Set the soft limit, carrying the hard limit through untouched. `true` when
/// the kernel took it.
#[cfg(unix)]
fn set_soft(want: u64, hard: libc::rlim_t) -> bool {
    let limit = libc::rlimit { rlim_cur: want as libc::rlim_t, rlim_max: hard };
    unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) == 0 }
}

/// `kern.maxfilesperproc`: the number of descriptors macOS will let one process
/// hold, which is a separate thing from the rlimit and can be the smaller of
/// the two. Read rather than hardcoded because it is tunable and has moved a
/// long way between releases — 10240 on the machines the old `ulimit` advice
/// was written for, 122880 on the one in the bug report. Only the low end of
/// that range binds anything, since above `WANTED` the answer changes nothing;
/// it is exactly the old machines, where asking for more would be refused
/// outright, that this call is for.
#[cfg(target_os = "macos")]
fn per_process_ceiling() -> Option<u64> {
    let mut value: libc::c_int = 0;
    let mut size = std::mem::size_of::<libc::c_int>();
    let read = unsafe {
        libc::sysctlbyname(
            c"kern.maxfilesperproc".as_ptr(),
            (&mut value as *mut libc::c_int).cast::<libc::c_void>(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    // A negative or zero answer is not a ceiling, and neither is a short read;
    // any of them means we know nothing, which the decision above handles.
    (read == 0 && size == std::mem::size_of::<libc::c_int>() && value > 0).then(|| value as u64)
}

/// Everywhere else there is no second number worth reading. Linux does have a
/// ceiling on what `setrlimit` will accept — `fs.nr_open` — but it is not a cap
/// on descriptors the way `kern.maxfilesperproc` is, and the hard limit is
/// already at or under it on any machine we would run on, so the hard limit is
/// what we go on.
#[cfg(all(unix, not(target_os = "macos")))]
fn per_process_ceiling() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launchd_default_is_raised_to_what_we_want() {
        // The shipped case: 256 against an infinite hard limit, under the
        // sysctl this was measured against. Neither number on the machine is
        // asked for — the cap is far above what we want, so what we want wins.
        assert_eq!(target_soft(256, None, Some(122_880)), Some(WANTED));
    }

    #[test]
    fn an_infinite_hard_limit_is_never_asked_for_verbatim() {
        // The naive `soft = hard`, which is either refused or untrue.
        assert_eq!(target_soft(256, None, None), Some(WANTED));
    }

    #[test]
    fn a_finite_hard_limit_is_the_ceiling() {
        assert_eq!(target_soft(256, Some(4_096), Some(122_880)), Some(4_096));
    }

    #[test]
    fn a_generous_hard_limit_is_not_taken_verbatim_either() {
        // systemd's, which is real and would be granted: the point of the
        // clamp is that being allowed a million is not a reason to take one.
        assert_eq!(target_soft(1_024, Some(1_048_576), None), Some(WANTED));
    }

    #[test]
    fn a_per_process_cap_below_what_we_want_binds() {
        // An older macOS, where asking for `WANTED` would be refused outright.
        assert_eq!(target_soft(256, None, Some(10_240)), Some(10_240));
    }

    #[test]
    fn a_high_soft_limit_is_left_alone() {
        // A terminal-launched build inherits a login shell's million, which is
        // above everything here: every one of these would be a downgrade.
        assert_eq!(target_soft(1_048_576, None, Some(122_880)), None);
        assert_eq!(target_soft(1_048_576, Some(1_048_576), None), None);
    }

    #[test]
    fn a_soft_limit_already_at_the_ceiling_is_not_reset() {
        assert_eq!(target_soft(WANTED, None, Some(122_880)), None);
        assert_eq!(target_soft(4_096, Some(4_096), None), None);
    }

    #[test]
    fn a_hard_limit_below_the_soft_one_lowers_nothing() {
        // Not a state a process reaches on its own, and reachable all the same:
        // `setrlimit` may set the pair in one call, and a parent that lowered
        // the hard limit without touching the soft one hands this down. The
        // answer is to leave it alone — descriptors given up here are given up
        // for the life of the process.
        assert_eq!(target_soft(4_096, Some(1_024), None), None);
    }

    #[test]
    fn a_per_process_cap_below_the_soft_limit_lowers_nothing() {
        // The same shape from the other ceiling: a sysctl that has been tuned
        // down under a process whose soft limit was raised before it was.
        assert_eq!(target_soft(8_192, Some(65_536), Some(4_096)), None);
    }

    #[test]
    fn the_retry_is_smaller_than_the_first_attempt_or_absent() {
        // What `raise` asks for after a refusal, in the two shapes it matters:
        // a machine that told us nothing and refused what we picked, and one
        // whose soft limit is already above the fallback, where there is
        // nothing smaller left to try.
        assert!(FALLBACK_CEILING < WANTED, "the retry must be a smaller request");
        assert_eq!(target_soft(256, None, Some(FALLBACK_CEILING)), Some(FALLBACK_CEILING));
        assert_eq!(target_soft(WANTED, None, Some(FALLBACK_CEILING)), None);
    }

    /// The impure half, end to end, and the one test here that changes the
    /// process it runs in — which is safe in a way a limit going the other way
    /// would not be: every other test in this binary can only gain by it. It is
    /// worth having because the failure this module exists to fix lives
    /// entirely in the parts a pure test cannot reach, the sysctl and the call
    /// itself. Run it under `ulimit -Sn 256` to see it do the work; on a
    /// terminal-launched suite the soft limit is already a million and what it
    /// checks is that nothing was taken away.
    #[cfg(unix)]
    #[test]
    fn raising_leaves_nothing_further_to_ask_for() {
        let before = current();
        raise();
        let after = current();
        assert!(
            after.0 >= before.0,
            "the soft limit went down, from {} to {}",
            before.0,
            after.0
        );
        assert_eq!(
            target_soft(after.0, after.1, per_process_ceiling()),
            None,
            "there was still headroom after raising: soft {}",
            after.0
        );
    }

    /// The soft limit and the hard one, in the shape `target_soft` reads them.
    #[cfg(unix)]
    fn current() -> (u64, Option<u64>) {
        let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) }, 0);
        (limit.rlim_cur as u64, (limit.rlim_max != libc::RLIM_INFINITY).then_some(limit.rlim_max as u64))
    }

    /// Not a test of the decision but of the machine, in the spirit of
    /// `shell_env`'s login-shell probe: on any Unix box this call answers, and
    /// the answer is a plausible number of descriptors. Where it stops being
    /// true, the fallback above is what a person would be left with.
    #[cfg(unix)]
    #[test]
    fn this_machine_reports_a_descriptor_limit() {
        let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
        assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) }, 0);
        assert!(limit.rlim_cur >= 256, "a soft limit of {} is below the POSIX minimum", limit.rlim_cur);
        // macOS answers with its sysctl; everywhere else `None` is correct.
        if let Some(ceiling) = per_process_ceiling() {
            assert!(ceiling >= 256, "kern.maxfilesperproc came back as {ceiling}");
        }
    }
}
