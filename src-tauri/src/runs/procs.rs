//! The process table, and the two signals sent to a process group. The only
//! unsafe code in `runs/`, and deliberately the whole of what the rest of the
//! recovery knows about the operating system — `registry.rs` decides and this
//! answers.
//!
//! Two questions are asked here and no others. *What is under this pid right
//! now* (`look`), which is what makes a record's own evidence checkable, and
//! *write down what is under this pid* (`snapshot`), which is how that evidence
//! is produced in the first place. Both answer with the same stamp so that the
//! two are comparable by construction rather than by a rule kept in step by
//! hand.
//!
//! The stamp is a process's start time, and it is read per platform because
//! there is no portable way to ask: macOS answers a `proc_bsdinfo` through
//! `proc_pidinfo`, Linux keeps it in `/proc/<pid>/stat` in ticks since boot,
//! which is turned into ticks since the epoch so that a reboot cannot make two
//! different processes carry the same stamp. A platform not on that list
//! answers `Unknown` to everything, and `registry.rs` reads `Unknown` as a
//! reason to touch nothing at all. That is why no crate was added for this:
//! `libc` is already here for the two `killpg` calls the app makes, the two
//! readers are twenty lines apiece, and a dependency in this tree costs a
//! `Cargo.lock` that two branches then have to merge.

use crate::runs::registry::{Proc, Seen};

/// What is under this pid right now.
pub fn look(pid: i32) -> Seen {
    match reachable(pid) {
        Reachable::No => Seen::Gone,
        Reachable::Unsure => Seen::Unknown,
        Reachable::Yes => match info(pid) {
            Some((started, _)) => Seen::Running { started },
            // The pid answered a moment ago and will not answer for its own
            // details: it left in between, or the kernel would not say. Either
            // way this is not a process anything here may act on.
            None => Seen::Unknown,
        },
    }
}

/// The evidence to write down about a process that is running now. `None` when
/// this platform cannot say, which is what stops a record claiming a liveness
/// it could never prove.
pub fn snapshot(pid: i32) -> Option<Proc> {
    let (started, command) = info(pid)?;
    Some(Proc { pid, started, command })
}

/// This process, for the `writer` on every record it goes on to write.
pub fn own() -> Option<Proc> {
    snapshot(std::process::id() as i32)
}

/// The soft signal, to the process *group* — the same one `terminal/pty.rs`
/// sends on the way out and for the same reason: the agent is a session leader,
/// so whatever it started is in its group, and a signal to the child alone
/// would leave those behind as the orphans this exists to clear.
///
/// The pid guard is the last of three: `registry::sweep` refuses a group at or
/// below 1 before it ever gets here, and this refuses it again, because
/// `killpg(0)` would signal the app's own group and everything the app has
/// started with it.
pub fn hangup_group(pid: i32) -> bool {
    signal_group(pid, HANGUP)
}

/// What is left after the grace period. Same guard, same reasoning.
pub fn kill_group(pid: i32) -> bool {
    signal_group(pid, KILL)
}

#[cfg(unix)]
const HANGUP: libc::c_int = libc::SIGHUP;
#[cfg(unix)]
const KILL: libc::c_int = libc::SIGKILL;
#[cfg(not(unix))]
const HANGUP: i32 = 0;
#[cfg(not(unix))]
const KILL: i32 = 0;

#[cfg(unix)]
fn signal_group(pid: i32, signal: libc::c_int) -> bool {
    if pid <= 1 {
        return false;
    }
    unsafe { libc::killpg(pid as libc::pid_t, signal) == 0 }
}

/// Windows has no signal to send here, and saying so is what keeps the caller
/// from waiting out a grace period that could not have helped anyone — the same
/// answer `Pty::hangup` gives there.
#[cfg(not(unix))]
fn signal_group(_pid: i32, _signal: i32) -> bool {
    false
}

enum Reachable {
    Yes,
    No,
    Unsure,
}

/// Whether anything holds this pid, asked separately from its details because
/// the two failures mean opposite things. `ESRCH` is nothing there, which is
/// evidence; `EPERM` is a process belonging to somebody else, which is a pid
/// this app cannot have started and must not read as gone either.
#[cfg(unix)]
fn reachable(pid: i32) -> Reachable {
    if pid <= 0 {
        // `kill(0, …)` and `kill(-1, …)` are broadcasts rather than questions.
        return Reachable::Unsure;
    }
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return Reachable::Yes;
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(code) if code == libc::ESRCH => Reachable::No,
        _ => Reachable::Unsure,
    }
}

#[cfg(not(unix))]
fn reachable(_pid: i32) -> Reachable {
    Reachable::Unsure
}

/// When the process under this pid started, and what it is called.
///
/// The stamp is microseconds since the epoch: `proc_bsdinfo` carries the two
/// halves separately and combining them is what makes two processes started in
/// the same second distinguishable.
#[cfg(target_os = "macos")]
fn info(pid: i32) -> Option<(u64, String)> {
    let mut bsd: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
    let read = unsafe {
        libc::proc_pidinfo(
            pid as libc::c_int,
            libc::PROC_PIDTBSDINFO,
            0,
            (&mut bsd as *mut libc::proc_bsdinfo).cast::<libc::c_void>(),
            size,
        )
    };
    // A short answer is not a partially useful one: the call returns the number
    // of bytes it filled, and anything but the whole struct means it failed.
    if read != size {
        return None;
    }
    let started = bsd.pbi_start_tvsec.saturating_mul(1_000_000).saturating_add(bsd.pbi_start_tvusec);
    Some((started, c_name(&bsd.pbi_comm)))
}

/// `pbi_comm` is a fixed 16 bytes and is not promised a terminator, so the name
/// is taken up to the first NUL or to the end, whichever comes first.
#[cfg(target_os = "macos")]
fn c_name(raw: &[libc::c_char]) -> String {
    let bytes: Vec<u8> = raw.iter().take_while(|byte| **byte != 0).map(|byte| *byte as u8).collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Linux keeps it in `/proc/<pid>/stat`: field 22 is the start time in clock
/// ticks since boot, and `btime` in `/proc/stat` is when boot was. Added
/// together they are ticks since the epoch, which is what stops a reboot
/// letting two different processes carry the same stamp.
///
/// The parse starts at the last `)` rather than splitting the whole line:
/// field 2 is the command in parentheses and a command may hold spaces and
/// parentheses of its own, which is the classic way to misread this file.
#[cfg(target_os = "linux")]
fn info(pid: i32) -> Option<(u64, String)> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let open = stat.find('(')?;
    let close = stat.rfind(')')?;
    let command = stat.get(open + 1..close)?.to_owned();
    // Field 3 is the first one after the name, so field 22 is the twentieth
    // token of what is left.
    let ticks: u64 = stat.get(close + 1..)?.split_whitespace().nth(19)?.parse().ok()?;
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    let hz = if hz > 0 { hz as u64 } else { 100 };
    let boot = boot_time()?;
    Some((boot.saturating_mul(hz).saturating_add(ticks), command))
}

/// Seconds since the epoch at which this machine booted.
#[cfg(target_os = "linux")]
fn boot_time() -> Option<u64> {
    std::fs::read_to_string("/proc/stat").ok()?.lines().find_map(|line| {
        line.strip_prefix("btime ").and_then(|value| value.trim().parse().ok())
    })
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn info(_pid: i32) -> Option<(u64, String)> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The one thing worth asserting about the OS half: it can answer for a
    /// process it is standing inside, and the answer is stable — a stamp that
    /// changed between two reads of the same process would make every
    /// comparison in `registry.rs` false and every record read as dead.
    #[test]
    fn this_process_can_be_looked_up_and_answers_the_same_thing_twice() {
        let Some(mine) = own() else {
            // A platform with no reader is a supported outcome, not a failure —
            // it answers `Unknown` everywhere and the recovery does nothing.
            eprintln!("this platform cannot read a process start time; nothing to check");
            return;
        };
        assert_eq!(mine.pid, std::process::id() as i32);
        assert!(!mine.command.is_empty(), "a process has a name");
        assert_eq!(look(mine.pid), Seen::Running { started: mine.started });
    }

    #[test]
    fn a_pid_nobody_holds_reads_as_gone() {
        // Above any pid a kernel hands out, so this is deterministic rather
        // than a guess at a free number.
        assert_eq!(look(i32::MAX), Seen::Gone);
    }

    /// The claim the whole registry rests on, against the real process table
    /// rather than a stated one: evidence written down while a process lives
    /// reads as dead once it is gone, with nobody asked and nothing but the
    /// record and the kernel involved.
    #[cfg(unix)]
    #[test]
    fn evidence_written_down_while_a_process_lived_reads_as_dead_afterwards() {
        use crate::runs::registry::{liveness, Liveness};

        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("exit 0")
            .spawn()
            .expect("a process to write down");
        let recorded = snapshot(child.id() as i32).expect("read while it is alive");
        assert_eq!(liveness(&recorded, look(recorded.pid)), Liveness::Alive);

        // Reaped, because a zombie is still a process to the kernel and this is
        // its parent — the case the sweep never meets, since the processes it
        // looks at belonged to an app that is gone and init has long since
        // cleared up after them.
        child.wait().expect("reaped");

        assert_eq!(
            liveness(&recorded, look(recorded.pid)),
            Liveness::Dead,
            "and if that pid has already been handed out again, the stamp says so"
        );
    }

    #[test]
    fn nothing_signals_the_apps_own_process_group() {
        // The guard `registry::sweep` also keeps, kept twice on purpose: this
        // is the call that would take the app and every agent down with it.
        for pid in [-1, 0, 1] {
            assert!(!hangup_group(pid));
            assert!(!kill_group(pid));
        }
    }
}
