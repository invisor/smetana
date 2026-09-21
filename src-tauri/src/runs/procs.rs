//! The process table, and the two signals sent to a process group. The only
//! unsafe code in `runs/`, and deliberately the whole of what the rest of the
//! recovery knows about the operating system — `registry.rs` decides and this
//! answers.
//!
//! Three questions were asked here at first, and a fourth joined them in
//! smetana-kkz2. *What is under this pid right now* (`look`), which is what
//! makes a record's own evidence checkable, *write down what is under this
//! pid* (`snapshot`), which is how that evidence is produced in the first
//! place, and *write down what is under this pid, now that it is itself*
//! (`spawned`), which is the same question asked about a process this app has
//! only just started and which is therefore not yet certain to be the process
//! it was started as. All three answer with the same stamp so that they are
//! comparable by construction rather than by a rule kept in step by hand.
//!
//! The fourth question is smetana-kkz2's: **what has a session left behind
//! that a signal to its process group can no longer reach**, and **what has a
//! whole app instance left behind, dead or alive**. `killpg` reaches
//! whatever a session's own process group holds, which is every ordinary
//! child; it stops reaching anything the moment something inside a session
//! asks for a group of its own — `setsid`, `nohup`, a shell's own `&` job
//! control — because such a child's leader answers to nobody's `killpg` but
//! its own, and once that leader exits the child is reparented under pid 1
//! with no group left that names the session at all. The measurement behind
//! this task: Claude Code's own Bash tool runs every command in a session of
//! its own (`getsid` of the tool's shell equals its own pid), so anything a
//! command backgrounds and leaves running is outside the agent's session from
//! the moment it starts, and a plain group signal was never going to reach
//! it.
//!
//! **There is no one channel for this across three operating systems, so
//! there are three.** `terminal/service.rs` sweeps at four moments —
//! a session ending on its own, a session being removed, the app's own exit,
//! and the app's own start, recovering what a dead previous instance left —
//! without needing to know which of the three answers underneath it; what
//! differs is only the shape of evidence each platform can produce here.
//!
//! **Linux** carries a mark in every agent session's own environment,
//! `SMETANA_SESSION` (`mark`/`parse_mark`/`marked`/`strays`, kept from the
//! first attempt at this task): a variable is inherited by every descendant
//! regardless of its process group and survives a reparent under pid 1 that a
//! `pid_t` alone cannot. Read through `/proc/<pid>/environ`, readable for a
//! process this user owns regardless of Yama's `ptrace_scope` (which gates
//! `PTRACE_ATTACH`, not a plain read of that file).
//!
//! **macOS has no such channel**, and this module's first attempt over this
//! task assumed otherwise. `sysctl(KERN_PROCARGS2)` and `ps -wwE` both hand
//! back a process's environment only to the process asking about *itself* —
//! measured twice, by two different sessions, on macOS 26.5.2 (25F84): this
//! app's own pid answered its full environment, a same-user child spawned a
//! moment earlier answered nothing at all. So macOS answers the same two
//! questions two different ways instead. **What a live session has left
//! behind** (points 1 and 2 in `terminal/service.rs`) is a snapshot the
//! session's own poller keeps of every descendant it has ever seen under its
//! PTY child, walked by parent pid from one `KERN_PROC_ALL` sysctl every two
//! seconds, plus the process group of each one — a descendant that escapes
//! the parent-pid tree by reparenting under pid 1 is still found by the group
//! it once led, because a group headed by one of a session's own descendants
//! can only belong to that session or to a session one of its own
//! descendants started, which is evidence with no private API behind it
//! (`Descendants`, `descendants_by_ppid`). **What a whole app instance has
//! left behind** (points 3 and 4) is the resource *coalition* every process
//! the app starts shares — `proc_pidinfo(pid, 20, …)`, a private call into a
//! 40-byte struct nowhere in the public SDK declares, the same pair `ps`
//! itself stands on. Measured for this task, on the same machine and the same
//! build: the app, every agent it starts and a background `sleep` reparented
//! under pid 1 all carry the identical resource coalition id; the app's own
//! WebKit/AppKit XPC helpers share it too and are excluded by path
//! (`is_xpc_service`); and — the measurement this task's own first step was —
//! **the id survives the death of the coalition's own leader on an orphaned
//! descendant**, confirmed against a genuine `launchd`-owned job (not merely
//! a member of this shell's own inherited coalition) whose leader was
//! `kill -9`ed while a `setsid`-detached child of it, already reparented
//! under pid 1, kept answering the identical coalition id for several seconds
//! afterwards. See `.claude/rules/terminal.md` for the exact commands and
//! output this rests on.
//!
//! **Windows carries neither a mark nor a coalition**, and needs neither: a
//! *Job Object*, created for every agent session right after it spawns and
//! assigned its child, with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` set, is the
//! platform's own answer to exactly this question — every process a job's
//! members go on to start joins the job automatically, and the *last* open
//! handle to it closing (an explicit `terminate`, or this app's own process
//! dying and the kernel closing every handle it held) kills everything the
//! job has ever held. `SessionJob` is `terminal/pty.rs`'s own handle, one per
//! agent session and never for a person's own shell; points 1–3 call
//! `terminate` on it directly and point 4 costs nothing, since a dead
//! previous instance's own handle closing is what already did the killing,
//! before the next instance is even running.
//!
//! `Unknown` — read as "not provably ours", never as "gone" — is the answer
//! every one of these three backends gives when it cannot say: a platform
//! this file has no reader for, a `proc_pidinfo` call answering a size this
//! file does not recognise, a process that has already gone between two
//! reads. Nothing here ever signals a pid it cannot show is ours; the worst
//! outcome of a channel closing under a future OS update is today's
//! behaviour, never a signal to a stranger.
//!
//! The stamp is a process's start time, and it is read per platform because
//! there is no portable way to ask: macOS answers a `proc_bsdinfo` through
//! `proc_pidinfo`, Linux keeps it in `/proc/<pid>/stat` in ticks since boot,
//! which is turned into ticks since the epoch so that a reboot cannot make two
//! different processes carry the same stamp. A platform not on that list
//! answers `Unknown` to everything, and `registry.rs` reads `Unknown` as a
//! reason to touch nothing at all. `libc` already exports every call the
//! Linux and the common halves need; the macOS descendant snapshot and the
//! coalition read are private APIs with no binding anywhere, hand-declared
//! here with the byte offsets they were measured against — see the doc
//! comments on `KernProc` and `ProcPidCoalitionInfo` for exactly what was
//! checked and against which SDK.

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

/// What is under a pid this app forked a moment ago, which is not the same
/// question as `snapshot` and cannot be answered by it.
///
/// A process's name is a lagging fact about it, and that is the whole of
/// smetana-6nr0: every `batches[].group.command` in this repository's own
/// `.smetana/runs.json` read `app`, the name of the process that started the
/// agent, against a pid whose start time matched to the microsecond and which
/// `ps` showed as `claude`. The two skills that read the field compare it with
/// what stands under the pid *now* and read a difference as a reused pid, so
/// the recorded lie inverts the lock rule: a live batch reads as dead and a
/// lead mid-merge has its lock broken under it.
///
/// The cause, measured rather than assumed. A child gets its own `p_comm` only
/// at `exec`; between the `fork` and the `exec` it is a copy of the process
/// that forked it and wears **that** process's name — this one's. Ordinarily
/// nobody sees that window, because `std`'s fork path holds the parent on a
/// close-on-exec pipe until the image has been swapped; but `portable-pty`'s
/// `pre_exec` calls `close_random_fds`, which closes every descriptor above 2
/// and so closes that pipe itself, and the parent is released early by
/// construction. Measured on macOS 26.5 against `/bin/sleep` spawned exactly as
/// `terminal/pty.rs` spawns an agent: the first read after `spawn_command`
/// returned gave the test binary's own name in 20 runs out of 20, and the real
/// name arrived 160–675 µs later. The pid is right throughout, and so is the
/// start time — `pbi_start_tvsec` is stamped at the fork — which is why the
/// defect showed up in exactly one field.
///
/// So the wait is for the name to stop being ours, and that is enough because
/// it is the same field, read by the same call, that the answer is made of:
/// there is no second signal to keep in step with it, and no ordering inside
/// the kernel to be right about. What it cannot tell apart is a child that
/// really did exec into a program sharing this one's short name; that answers
/// `Inherited` for ever and the caller gives up, which writes no name at all —
/// the safe direction, since both skills read a missing `group` as "leave the
/// lock alone" and a wrong one as "break it".
///
/// Asked about this very process it answers `Inherited` too, which is why
/// `own()` above does not go through here: the app's own name is the one thing
/// this function exists to refuse.
pub enum Spawned {
    /// A name that is the process's own. The evidence to write down.
    Named(Proc),
    /// Still wearing the name of the process that forked it. Ask again.
    Inherited,
    /// Nothing there to name, and nothing to wait for.
    Nothing,
}

/// The evidence to write down about a process this app has just started. See
/// `Spawned` for why this is not `snapshot`.
pub fn spawned(pid: i32) -> Spawned {
    let Some((started, command)) = info(pid) else { return Spawned::Nothing };
    // Nothing to compare against is not a licence to trust the name: on a
    // platform that cannot answer for this process, it cannot answer for that
    // one either, and `info` returning something here while returning nothing
    // for us would be a machine nobody has met.
    let Some((_, ours)) = info(std::process::id() as i32) else { return Spawned::Nothing };
    if command == ours {
        return Spawned::Inherited;
    }
    Spawned::Named(Proc { pid, started, command })
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

/// The environment key `terminal/pty.rs` puts on every agent session's own
/// environment on every Unix platform alike — never on a person's own shell,
/// see `build_shell_command` there — harmless to set even where nothing reads
/// it back (macOS's own backend does not), and the only name `marked`/`strays`
/// below ever look for.
pub const MARK_KEY: &str = "SMETANA_SESSION";

/// The mark for one session: this app's own pid, the stamp that survives its
/// reuse, and the session's id, joined the same way a `Proc` and an actor
/// name already sit beside each other in `registry.rs`. `None` on a platform
/// `own` cannot answer for — a mark nothing could ever prove dead is not one
/// worth writing at all.
pub fn mark(session: u64) -> Option<String> {
    let me = own()?;
    Some(format!("{}:{}:{session}", me.pid, me.started))
}

/// The three numbers a mark was built from, or `None` for anything that is
/// not exactly that shape — an environment variable of the same name from
/// outside this app, say, which must never be read as evidence about one of
/// ours. Only `is_stray` (Linux) reads this back at runtime — macOS writes
/// the mark for the reason the module header gives but answers points 1
/// through 4 a different way — so a non-Linux build sees no caller of its
/// own; `#[allow(dead_code)]` there rather than gating the function itself,
/// since it stays exercised directly by this file's own tests on every
/// platform.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_mark(value: &str) -> Option<(i32, u64, u64)> {
    let mut parts = value.split(':');
    let pid = parts.next()?.parse().ok()?;
    let started = parts.next()?.parse().ok()?;
    let session = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((pid, started, session))
}

/// Every process on this machine carrying this exact mark, and never this
/// very process — a worker asking about its own mark is not a question this
/// file answers by including the one process that could never be a stray.
///
/// Linux only: see the module header for why macOS answers points 1 and 2 a
/// different way, and why the mark is still written there regardless.
#[cfg(target_os = "linux")]
pub fn marked(value: &str) -> Vec<i32> {
    let me = std::process::id() as i32;
    all_pids()
        .into_iter()
        .filter(|&pid| pid != me)
        .filter(|&pid| environment_mark(pid).as_deref() == Some(value))
        .collect()
}

/// Every marked process whose own app half no longer names a live Smetana
/// carrying the very stamp it was written under — the shape a killed app
/// leaves behind, and the question `terminal::service`'s start-up sweep asks
/// before touching anything.
///
/// A mark whose app cannot be read at all (`Seen::Unknown`) answers "no"
/// here, the same rule `registry::sweep` keeps for the group it signals: an
/// unreadable answer is never a reason to hang something up.
///
/// Linux only, for the reason `marked` above carries.
#[cfg(target_os = "linux")]
pub fn strays() -> Vec<(i32, String)> {
    let me = std::process::id() as i32;
    all_pids()
        .into_iter()
        .filter(|&pid| pid != me)
        .filter_map(|pid| environment_mark(pid).map(|value| (pid, value)))
        .filter(|(_, value)| is_stray(value))
        .collect()
}

#[cfg(target_os = "linux")]
fn is_stray(value: &str) -> bool {
    let Some((app_pid, app_started, _)) = parse_mark(value) else { return false };
    match look(app_pid) {
        Seen::Running { started } => started != app_started,
        Seen::Gone => true,
        Seen::Unknown => false,
    }
}

/// The soft signal, to one pid rather than to a group — `hangup_group`'s own
/// pair, for a descendant a group can no longer reach. The same two refusals
/// as `signal_group`: nothing at or below 1, and never this app's own pid,
/// whatever a stray environment variable happened to claim.
pub fn hangup_pid(pid: i32) -> bool {
    signal_pid(pid, HANGUP)
}

/// What is left after the grace period. Same guard, same reasoning.
pub fn kill_pid(pid: i32) -> bool {
    signal_pid(pid, KILL)
}

#[cfg(unix)]
fn signal_pid(pid: i32, signal: libc::c_int) -> bool {
    if pid <= 1 || pid == std::process::id() as i32 {
        return false;
    }
    unsafe { libc::kill(pid as libc::pid_t, signal) == 0 }
}

/// Windows has no signal to send here either, for the same reason
/// `signal_group` names.
#[cfg(not(unix))]
fn signal_pid(_pid: i32, _signal: i32) -> bool {
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

/// Every numeric entry of `/proc` — `marked`'s and `strays`' own candidate
/// list, walked fresh on every call rather than cached: this runs a few times
/// a minute at most, on a session ending or the app starting, and a cache
/// would be one more thing to invalidate correctly.
#[cfg(target_os = "linux")]
fn all_pids() -> Vec<i32> {
    let Ok(entries) = std::fs::read_dir("/proc") else { return Vec::new() };
    entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().to_str()?.parse::<i32>().ok())
        .collect()
}

/// This pid's `SMETANA_SESSION`, out of `/proc/<pid>/environ` — NUL-separated
/// `KEY=VALUE` entries, readable for any process this user owns and refused
/// by the kernel for anything it does not, which answers `None` exactly as a
/// process that has already gone does.
#[cfg(target_os = "linux")]
fn environment_mark(pid: i32) -> Option<String> {
    let bytes = std::fs::read(format!("/proc/{pid}/environ")).ok()?;
    let prefix = format!("{MARK_KEY}=");
    bytes
        .split(|&b| b == 0)
        .filter(|entry| !entry.is_empty())
        .find_map(|entry| String::from_utf8_lossy(entry).strip_prefix(&prefix).map(str::to_owned))
}

/* =======================================================================
 * macOS: a session's own accumulated descendants (points 1-2), and the
 * app-wide resource coalition (points 3-4). See the module header for the
 * measurements both of these rest on.
 * ======================================================================= */

/// One row of the machine's whole process table: pid, parent, process group
/// and start stamp together, from one `KERN_PROC_ALL` sysctl rather than a
/// `proc_pidinfo` call per candidate — the shape `snapshot_all` is built to
/// fill in one pass.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KernProc {
    pub pid: i32,
    pub ppid: i32,
    pub pgid: i32,
    /// Microseconds since the epoch, the same unit `info` above answers —
    /// comparable against a `Proc.started` with no conversion.
    pub started: u64,
}

/// `struct kinfo_proc`'s byte layout, read out of the buffer `KERN_PROC_ALL`
/// hands back rather than declared as a `#[repr(C)]` type. Unlike the
/// coalition struct below, `kinfo_proc` genuinely is in the public SDK
/// (`<sys/sysctl.h>`) — but it nests `extern_proc` and `eproc`, both built
/// from kernel-only pointer types (`struct proc *`, `struct vmspace`, a
/// `sigset_t`, …) that `libc` does not bind on this platform, so a faithful
/// `#[repr(C)]` copy would have to declare every one of them correctly just
/// to get the four fields this file actually wants at the right offsets.
///
/// Those four offsets were measured directly against this SDK's own header
/// rather than assumed — `clang`, `offsetof`, the MacOSX SDK shipped with
/// this machine's command line tools, cross-checked against `sysctl(CTL_KERN,
/// KERN_PROC, KERN_PROC_PID, getpid())` returning this very process's own
/// pid, ppid and start time correctly:
///
///   `sizeof(struct kinfo_proc)` = 648, `sizeof(struct extern_proc)` = 296
///   `extern_proc.p_un.p_starttime` (a `struct timeval`) at offset 0
///   `extern_proc.p_pid` at offset 40
///   `struct eproc` (`kp_eproc`) begins at offset 296
///   `eproc.e_ppid` at offset 264 within it (560 absolute)
///   `eproc.e_pgid` at offset 268 within it (564 absolute)
///
/// Every field up to and including these four is POD of a size identical on
/// both of this app's macOS targets (8-byte pointers, 4-byte `int`, 8-byte
/// `long`, both LP64), so the offsets hold on x86_64 as well as the arm64
/// they were measured on — only the one architecture was checked directly.
/// A struct whose *shape* changed under a future SDK is exactly the cost
/// `KINFO_PROC_SIZE` below is checked against: a returned size that no
/// longer matches drops that entry rather than reading the wrong bytes as
/// somebody's pid.
#[cfg(target_os = "macos")]
const KINFO_PROC_SIZE: usize = 648;
#[cfg(target_os = "macos")]
const KP_START_SEC: usize = 0;
#[cfg(target_os = "macos")]
const KP_START_USEC: usize = 8;
#[cfg(target_os = "macos")]
const KP_PID: usize = 40;
#[cfg(target_os = "macos")]
const KP_EPROC: usize = 296;
#[cfg(target_os = "macos")]
const KP_PPID: usize = KP_EPROC + 264;
#[cfg(target_os = "macos")]
const KP_PGID: usize = KP_EPROC + 268;

#[cfg(target_os = "macos")]
fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_ne_bytes(bytes[offset..offset + 4].try_into().expect("4 bytes"))
}

#[cfg(target_os = "macos")]
fn i64_at(bytes: &[u8], offset: usize) -> i64 {
    i64::from_ne_bytes(bytes[offset..offset + 8].try_into().expect("8 bytes"))
}

/// One `kinfo_proc`-sized chunk, or `None` for a short one — a size that does
/// not match `KINFO_PROC_SIZE` is never partially read.
#[cfg(target_os = "macos")]
fn kern_proc_from(bytes: &[u8]) -> Option<KernProc> {
    if bytes.len() < KINFO_PROC_SIZE {
        return None;
    }
    let sec = i64_at(bytes, KP_START_SEC);
    let usec = i32_at(bytes, KP_START_USEC);
    if sec < 0 {
        // A slot the kernel left unfilled at the tail of an oversized read —
        // see the headroom comment in `snapshot_all`.
        return None;
    }
    let started = (sec as u64).saturating_mul(1_000_000).saturating_add(usec.max(0) as u64);
    Some(KernProc {
        pid: i32_at(bytes, KP_PID),
        ppid: i32_at(bytes, KP_PPID),
        pgid: i32_at(bytes, KP_PGID),
        started,
    })
}

/// One `sysctl(CTL_KERN, KERN_PROC, KERN_PROC_ALL)` call, turned into every
/// process on the machine with its parent, its group and its start stamp —
/// the one call `terminal/service.rs`'s two-second poller makes per tick, and
/// the fresh table points 1, 2 and 3 ask their candidates against. `None` on
/// a read this platform refused, which is `Unknown`'s shape here: a poll
/// that cannot see the table skips its tick rather than acting on a partial
/// one.
#[cfg(target_os = "macos")]
pub fn snapshot_all() -> Option<Vec<KernProc>> {
    let mut mib: [libc::c_int; 3] = [libc::CTL_KERN, libc::KERN_PROC, libc::KERN_PROC_ALL];
    let mut size: libc::size_t = 0;
    let sized = unsafe {
        libc::sysctl(mib.as_mut_ptr(), 3, std::ptr::null_mut(), &mut size, std::ptr::null_mut(), 0)
    };
    if sized != 0 || size == 0 {
        return None;
    }
    // Headroom for a process starting between the sizing call and the read —
    // the same margin `all_pids` on Linux gives `proc_listallpids`, sized
    // more generously here because the struct itself is hundreds of bytes:
    // an undersized buffer costs the newest arrivals, not a failure.
    let mut buf = vec![0u8; size + size / 8 + KINFO_PROC_SIZE * 32];
    let mut out_size = buf.len() as libc::size_t;
    let read = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buf.as_mut_ptr().cast::<libc::c_void>(),
            &mut out_size,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 {
        return None;
    }
    buf.truncate(out_size);
    Some(buf.chunks_exact(KINFO_PROC_SIZE).filter_map(kern_proc_from).collect())
}

/// Every entry transitively parented under `root`, walked by `ppid` — the
/// tree a session's own PTY child heads, at the moment of one snapshot and
/// before whatever it has spawned had any chance to ask for a session of its
/// own. Pure over data already in hand, so a fixture can drive it with no
/// process table at all.
#[cfg(target_os = "macos")]
pub fn descendants_by_ppid(snapshot: &[KernProc], root: i32) -> Vec<KernProc> {
    let mut frontier = vec![root];
    let mut found = Vec::new();
    while let Some(parent) = frontier.pop() {
        for entry in snapshot.iter().filter(|p| p.ppid == parent) {
            found.push(*entry);
            frontier.push(entry.pid);
        }
    }
    found
}

/// What one agent session's own poller has accumulated: every descendant pid
/// (paired with its own start stamp, so a reused pid is never mistaken for
/// the process that once held it) the tree under its PTY child has ever
/// held, and every process group any of them has ever led.
///
/// Points 1 and 2 read this rather than a fresh `descendants_by_ppid` walk,
/// because the very shape this exists for — `setsid`, `nohup`, a shell's own
/// `&` — takes a process *out* of the tree that walk can still find it in the
/// instant its own leader reparents under pid 1: by the time a session is
/// ending, the parent-pid link a fresh walk depends on may already be gone,
/// which is exactly the case a snapshot with no memory of its own would miss
/// (see the "Отвергнуто" entry on tracking parentage with no snapshot in the
/// design). Group membership survives that reparenting because a group led
/// by one of a session's own descendants can only ever belong to that
/// session or to a session one of its own descendants went on to start — no
/// third party could ever join it — so it stands as evidence on its own, with
/// no private API behind it at all.
#[cfg(target_os = "macos")]
#[derive(Clone, Default, Debug)]
pub struct Descendants {
    seen: std::collections::HashSet<(i32, u64)>,
    groups: std::collections::HashSet<i32>,
}

#[cfg(target_os = "macos")]
impl Descendants {
    /// One poll tick: walk `root`'s tree in this fresh snapshot and fold it
    /// into what has been seen so far. Nothing already held is ever removed
    /// — a descendant that reparented under pid 1 between two polls has
    /// already left `descendants_by_ppid`'s reach, and forgetting it here
    /// would undo the one thing this type exists to remember about it.
    pub fn absorb(&mut self, snapshot: &[KernProc], root: i32) {
        for entry in descendants_by_ppid(snapshot, root) {
            self.seen.insert((entry.pid, entry.started));
            self.groups.insert(entry.pgid);
        }
    }

    /// The pids to reap right now: a fresh snapshot's entries whose (pid,
    /// stamp) this session has itself seen, or whose group this session has
    /// itself seen led by one of its own descendants — minus whatever
    /// `exclude` names, which is this app's own pid and the pid of every
    /// session still alive (`terminal/service.rs`'s own exclusion list).
    /// Pure over the fresh snapshot and the two sets accumulated so far, so
    /// the whole rule is testable with no process table beyond one fixture.
    pub fn candidates(&self, snapshot: &[KernProc], exclude: &[i32]) -> Vec<i32> {
        snapshot
            .iter()
            .filter(|p| !exclude.contains(&p.pid))
            .filter(|p| self.seen.contains(&(p.pid, p.started)) || self.groups.contains(&p.pgid))
            .map(|p| p.pid)
            .collect()
    }

    /// Whether this session's own poller has ever seen anything at all —
    /// `terminal/service.rs`'s guard for whether it is worth asking `procs`
    /// for a fresh snapshot on a session that never spawned a thing.
    pub fn is_empty(&self) -> bool {
        self.seen.is_empty() && self.groups.is_empty()
    }
}

/// macOS's private `proc_pidinfo` flavour for a process's resource
/// coalition — 20, undocumented anywhere in the public SDK.
#[cfg(target_os = "macos")]
const PROC_PIDCOALITIONINFO: libc::c_int = 20;

/// The 40-byte struct that flavour hands back, declared by hand because
/// nowhere in the public SDK does: three coalition ids (index 0 is the
/// resource/jetsam coalition this file cares about; the app, every agent it
/// starts and anything reparented under pid 1 after leaving one of them all
/// carry the identical value there — see the module header), a requested
/// role that was measured as 0 for every member on this machine and cannot
/// tell one apart from another, and three reserved words. **This struct is
/// private and unstable by nature — any answer whose size does not match it
/// exactly is read by `interpret_coalition` as `Unknown`, and that is what
/// makes an SDK that changes its shape degrade this file's whole macOS
/// coalition sweep to doing nothing, rather than reading a stranger's bytes
/// as a resource id.**
#[cfg(target_os = "macos")]
#[repr(C)]
struct ProcPidCoalitionInfo {
    coalition_id: [u64; 3],
    requested_role: u32,
    reserved1: u32,
    reserved2: u32,
    reserved3: u32,
}

/// What this file could establish about a pid's coalition.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coalition {
    Known(u64),
    /// The call failed, the pid is gone, or the kernel answered a size this
    /// file does not recognise. Never read as evidence of anything, the same
    /// rule every other `Unknown` in this module keeps.
    Unknown,
}

/// The pure half of `coalition`: given how many bytes actually came back
/// against how many were expected, and the resource id that was in the
/// buffer regardless, what does this file conclude? Split out so the
/// "unexpected size" case — the one this struct's whole safety rests on —
/// can be driven by a test with no `proc_pidinfo` call at all.
#[cfg(target_os = "macos")]
fn interpret_coalition(read: libc::c_int, expected: usize, resource_id: u64) -> Coalition {
    if read == expected as libc::c_int {
        Coalition::Known(resource_id)
    } else {
        Coalition::Unknown
    }
}

/// This pid's resource coalition, or `Unknown` if it cannot be read — gone,
/// belonging to another user, or a kernel that answered the wrong size.
#[cfg(target_os = "macos")]
pub fn coalition(pid: i32) -> Coalition {
    let mut info =
        ProcPidCoalitionInfo { coalition_id: [0; 3], requested_role: 0, reserved1: 0, reserved2: 0, reserved3: 0 };
    let size = std::mem::size_of::<ProcPidCoalitionInfo>() as libc::c_int;
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            PROC_PIDCOALITIONINFO,
            0,
            (&mut info as *mut ProcPidCoalitionInfo).cast::<libc::c_void>(),
            size,
        )
    };
    interpret_coalition(read, size as usize, info.coalition_id[0])
}

/// This app's own resource coalition — every agent it starts, and anything
/// they leave behind, shares it.
#[cfg(target_os = "macos")]
pub fn own_coalition() -> Coalition {
    coalition(std::process::id() as i32)
}

/// The path of the executable behind a pid, through `proc_pidpath` — the same
/// call `ps` itself is built on, public and unprivileged for a process this
/// user owns.
#[cfg(target_os = "macos")]
pub fn exe_path(pid: i32) -> Option<String> {
    let mut buf = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    let read =
        unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr().cast::<libc::c_void>(), buf.len() as u32) };
    if read <= 0 {
        return None;
    }
    buf.truncate(read as usize);
    Some(String::from_utf8_lossy(&buf).into_owned())
}

/// This app's own XPC helpers — WebKit's GPU/Networking/WebContent
/// processes, the open/save panel service, QuickLook, and so on — share its
/// coalition and end up with `ppid == 1` once their own launching service
/// exits, which is indistinguishable from an orphan by coalition and parent
/// alone. Measured for this task: every one of them runs out of a bundle
/// under `.../XPCServices/…` (this app's own plugins) or its own `*.xpc/`
/// bundle (the system frameworks'), so a path check is what tells them apart
/// — without it, this app's own coalition sweep would kill its own webview.
#[cfg(target_os = "macos")]
pub fn is_xpc_service(path: &str) -> bool {
    path.contains("/XPCServices/") || path.contains(".xpc/")
}

/// Candidates for the macOS coalition sweep (points 3 and 4): every process
/// on the machine sharing `coalition`, reparented under pid 1 — `ppid == 1`
/// is what proves nobody but init stands over it any more, and coalition
/// membership is what proves it is this app's own leaving and not some
/// stranger's — excluding this app's own pid, every XPC helper, and anything
/// `exclude` names (a still-live session's own pid, at point 3; nothing, at
/// point 4, where a dead instance's own sessions are gone by definition).
#[cfg(target_os = "macos")]
pub fn coalition_candidates(snapshot: &[KernProc], coalition: u64, exclude: &[i32]) -> Vec<i32> {
    let me = std::process::id() as i32;
    snapshot
        .iter()
        .filter(|p| p.ppid == 1 && p.pid != me && !exclude.contains(&p.pid))
        .filter(|p| matches!(self::coalition(p.pid), Coalition::Known(id) if id == coalition))
        .filter(|p| !exe_path(p.pid).is_some_and(|path| is_xpc_service(&path)))
        .map(|p| p.pid)
        .collect()
}

/* =======================================================================
 * Windows: a Job Object per agent session, in place of a mark or a
 * coalition. See the module header for why this backend needs neither.
 * ======================================================================= */

/// The job assigned to one agent session's own child, with
/// `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` set — `terminal/pty.rs`'s own answer
/// to points 1 through 3, and to point 4 for free.
///
/// `HANDLE` is `Send` in every way this app uses it: the Win32 API is
/// documented as safe to call on any thread with a handle that was itself
/// obtained validly, and this one crosses from the worker's own tokio task
/// (where the session was spawned) to nowhere else at all — it lives inside
/// the one `Live` entry that owns the session, exactly where the process
/// handle `portable-pty` gives back already lives.
#[cfg(windows)]
pub struct SessionJob(windows::Win32::Foundation::HANDLE);

#[cfg(windows)]
unsafe impl Send for SessionJob {}

#[cfg(windows)]
impl SessionJob {
    /// A new, unnamed job with `KILL_ON_JOB_CLOSE` set. `None` on any step
    /// failing, which is `Unknown`'s Windows shape: a session that could not
    /// be given a job is left exactly as this feature would leave it if it
    /// did not exist, rather than half-wired to a job nothing will ever be
    /// assigned to.
    pub fn new() -> Option<Self> {
        use windows::Win32::System::JobObjects::{
            JobObjectExtendedLimitInformation, SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows::Win32::System::JobObjects::CreateJobObjectW;
        use windows::core::PCWSTR;

        let handle = unsafe { CreateJobObjectW(None, PCWSTR::null()) }.ok()?;
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        let set = unsafe {
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                (&info as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast::<core::ffi::c_void>(),
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if set.is_err() {
            unsafe { let _ = windows::Win32::Foundation::CloseHandle(handle); }
            return None;
        }
        Some(Self(handle))
    }

    /// Puts this session's own child into the job. The window between
    /// `spawn_command` returning and this call landing is milliseconds, and
    /// a grandchild the child spawns inside that window is not yet a member
    /// of anything — see `.claude/rules/terminal.md` for the size of that
    /// window measured against the harnesses this app actually starts, and
    /// why it is accepted rather than closed by serialising every session's
    /// start behind it.
    pub fn assign(&self, pid: u32) -> bool {
        use windows::Win32::System::JobObjects::AssignProcessToJobObject;
        use windows::Win32::System::Threading::{OpenProcess, PROCESS_SET_QUOTA, PROCESS_TERMINATE};

        let Ok(process) = (unsafe { OpenProcess(PROCESS_SET_QUOTA | PROCESS_TERMINATE, false, pid) })
        else {
            return false;
        };
        let assigned = unsafe { AssignProcessToJobObject(self.0, process) }.is_ok();
        unsafe { let _ = windows::Win32::Foundation::CloseHandle(process); }
        assigned
    }

    /// Points 1 through 3: everything this job has ever held, gone at once.
    /// Windows has no SIGHUP and no grace period to wait out — see the
    /// module header — so this is the whole of the signal, on the spot.
    pub fn terminate(&self) -> bool {
        use windows::Win32::System::JobObjects::TerminateJobObject;
        unsafe { TerminateJobObject(self.0, 1) }.is_ok()
    }

    /// Test-only: reads the limit flags back off the job object itself,
    /// through `QueryInformationJobObject` — the read half of what `new`
    /// above writes with `SetInformationJobObject` — so a test can check the
    /// flag actually landed on the kernel object rather than trusting the
    /// write call's own `Ok`. `pub(crate)` rather than private because the
    /// one reader is `terminal::pty`'s own test module and not this one;
    /// `#[cfg(test)]` because nothing this app ships ever needs to ask a job
    /// what it already knows it asked for.
    #[cfg(test)]
    pub(crate) fn limit_flags(&self) -> Option<windows::Win32::System::JobObjects::JOB_OBJECT_LIMIT> {
        use windows::Win32::System::JobObjects::{
            JobObjectExtendedLimitInformation, QueryInformationJobObject,
            JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        };
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        let size = std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32;
        let read = unsafe {
            QueryInformationJobObject(
                Some(self.0),
                JobObjectExtendedLimitInformation,
                (&mut info as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast::<core::ffi::c_void>(),
                size,
                None,
            )
        };
        read.is_ok().then_some(info.BasicLimitInformation.LimitFlags)
    }
}

/// Point 4 costs this app nothing to write: `KILL_ON_JOB_CLOSE` means the
/// *kernel itself* kills a job the moment its last open handle closes, and a
/// process dying — however it dies — closes every handle it held. So a dead
/// previous instance's own jobs are already gone by the time the next
/// instance is running, and there is nothing here for a start-up sweep to
/// find.
#[cfg(windows)]
impl Drop for SessionJob {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
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

    /// The half of smetana-6nr0 that can be asserted without a race: a name
    /// this process cannot tell apart from its own is refused outright. Asked
    /// about this very process, which is the extreme case of that, `spawned`
    /// must never hand back a `Proc` — every recorded `group.command` in the
    /// file that produced the bug was exactly this name.
    #[test]
    fn a_name_this_process_cannot_tell_from_its_own_is_never_handed_back() {
        let Some(mine) = own() else {
            eprintln!("this platform cannot read a process name; nothing to check");
            return;
        };
        assert!(
            matches!(spawned(mine.pid), Spawned::Inherited),
            "the app's own name is the one answer this must refuse"
        );
    }

    /// The defect itself, against the spawn path the app actually uses: a
    /// process started here is written down under **its** name and never under
    /// this test binary's, which is what `pbi_comm` answers until the exec
    /// lands. A test asserting only that the name is non-empty passes on the
    /// broken version, since `app` is a perfectly good non-empty string.
    ///
    /// `portable-pty` rather than a plain `Command`, because the window is that
    /// crate's doing — `close_random_fds` in its `pre_exec` closes the pipe
    /// `std` would otherwise hold the parent on until the exec (see `Spawned`),
    /// and a plain spawn shows no window at all.
    #[cfg(unix)]
    #[test]
    fn a_process_started_here_is_written_down_under_its_own_name() {
        use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

        let Some(mine) = own() else {
            eprintln!("this platform cannot read a process name; nothing to check");
            return;
        };
        let system = NativePtySystem::default();
        let pair = system
            .openpty(PtySize { rows: 24, cols: 80, pixel_width: 0, pixel_height: 0 })
            .expect("a pty to start a process in");
        let mut command = CommandBuilder::new("/bin/sleep");
        command.arg("30");
        let mut child = pair.slave.spawn_command(command).expect("a process to write down");
        // The same drop `Pty::start` makes, and for the same reason.
        drop(pair.slave);
        let pid = child.process_id().expect("a pid") as i32;

        // The caller's bounded wait, in miniature: the answer is asked for
        // until it is the process's own, and the deadline is what stops a
        // failure hanging the suite.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let named = loop {
            match spawned(pid) {
                Spawned::Named(proc) => break Some(proc),
                Spawned::Nothing => break None,
                Spawned::Inherited if std::time::Instant::now() >= deadline => break None,
                Spawned::Inherited => std::thread::sleep(std::time::Duration::from_millis(1)),
            }
        };

        let _ = child.kill();
        let _ = child.wait();

        let named = named.expect("the process is named within the deadline");
        assert_eq!(named.pid, pid);
        assert_eq!(named.command, "sleep", "the process's own name, not the one that forked it");
        assert_ne!(named.command, mine.command);
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

    /* ---- the SMETANA_SESSION mark, added in smetana-kkz2 ------------------ */

    #[test]
    fn a_mark_parses_back_into_the_three_numbers_it_was_built_from() {
        assert_eq!(parse_mark("123:456:7"), Some((123, 456, 7)));
        assert_eq!(parse_mark(""), None, "not three parts at all");
        assert_eq!(parse_mark("123:456"), None, "too few parts");
        assert_eq!(parse_mark("123:456:7:8"), None, "too many parts");
        assert_eq!(parse_mark("x:456:7"), None, "the first part is not a number");
        assert_eq!(parse_mark("123:456:x"), None, "the last part is not a number");
    }

    #[test]
    fn a_mark_round_trips_through_the_app_that_wrote_it() {
        let Some(mine) = own() else {
            eprintln!("this platform cannot read a process start time; nothing to check");
            return;
        };
        let value = mark(42).expect("a platform that can answer `own` can build a mark");
        assert_eq!(parse_mark(&value), Some((mine.pid, mine.started, 42)));
    }

    #[test]
    fn nothing_signals_a_pid_this_app_may_not_reap() {
        // The same four the tracker's design carries: nothing at or below 1,
        // and never this very process, whatever a stray environment variable
        // elsewhere on the machine happened to claim.
        let me = std::process::id() as i32;
        for pid in [-1, 0, 1, me] {
            assert!(!hangup_pid(pid), "{pid}");
            assert!(!kill_pid(pid), "{pid}");
        }
    }

    /// A child in a process group of its own — the shape `setsid`, `nohup`
    /// and a shell's `&` all leave, and exactly the one `killpg` can no
    /// longer reach once its own leader has gone. This is the smallest such
    /// shape a test may safely make: a plain `/bin/sleep`, in nobody's group
    /// but its own, carrying the one mark this test wrote.
    #[cfg(target_os = "linux")]
    fn marked_child(mark: &str) -> std::process::Child {
        use std::os::unix::process::CommandExt;
        std::process::Command::new("/bin/sleep")
            .arg("30")
            .env(MARK_KEY, mark)
            .process_group(0)
            .spawn()
            .expect("a marked child to find")
    }

    /// Polls a condition rather than sleeping a fixed amount: the mark is set
    /// at exec, and reading it back out through the kernel is not promised to
    /// be instant the moment `spawn` returns.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn wait_for(mut condition: impl FnMut() -> bool) -> bool {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !condition() {
            if std::time::Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        true
    }

    /// Every test here signals only a child it started itself: this reads the
    /// process table but never acts on what it finds, which is the guard
    /// `hangup_pid`/`kill_pid` and their tests keep separately.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_marked_child_in_its_own_group_is_found_by_its_mark_and_lost_with_it() {
        let value = mark(999_001).expect("this platform can build a mark");
        let mut child = marked_child(&value);
        let pid = child.id() as i32;

        assert!(wait_for(|| marked(&value) == vec![pid]));
        assert!(!marked(&value).contains(&(std::process::id() as i32)), "never this test's own pid");

        let _ = child.kill();
        let _ = child.wait();

        assert!(wait_for(|| marked(&value).is_empty()), "the child is still found once it is gone");
    }

    /// The other half of the same mark: whether its app half is provably
    /// dead is `strays`' own question, and it has to answer "no" for an app
    /// that is plainly still running before it can be trusted to answer "yes"
    /// for one that is not.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_marked_child_is_a_stray_only_once_its_app_half_is_provably_dead() {
        // This test process is the app half here, and it is running — so a
        // child marked under it must never be read as a stray.
        let live = mark(999_002).expect("this platform can build a mark");
        let mut alive = marked_child(&live);
        assert!(wait_for(|| marked(&live).contains(&(alive.id() as i32))));
        assert!(
            !strays().iter().any(|(_, value)| value == &live),
            "the app this mark names is still running"
        );
        let _ = alive.kill();
        let _ = alive.wait();

        // No kernel hands out `i32::MAX`, so this app half is provably gone
        // from the moment the child starts — the shape a killed app's own
        // session leaves behind.
        let dead = format!("{}:{}:{}", i32::MAX, 1, 999_003);
        let mut orphan = marked_child(&dead);
        let pid = orphan.id() as i32;
        assert!(
            wait_for(|| strays().iter().any(|(found, value)| *found == pid && value == &dead)),
            "a mark whose app half is provably gone must be found as a stray"
        );

        let _ = orphan.kill();
        let _ = orphan.wait();
    }

    /* ---- macOS: the descendant snapshot and the coalition, added in
     * smetana-kkz2's second attempt ---------------------------------------- */

    /// `setsid()` then `exec /bin/sleep`, the shape Claude Code's own Bash
    /// tool leaves behind and exactly the shape `Descendants::candidates`
    /// exists to find by process group once the parent-pid link to it is
    /// gone. A tiny helper binary rather than a `setsid` *command*: macOS
    /// carries no such command at all (see `.claude/rules/terminal.md`'s
    /// reproduction steps), only the syscall.
    #[cfg(target_os = "macos")]
    fn escaping_child() -> std::process::Command {
        // Built once per test process into a temp file, since there is no
        // `setsid` binary on this platform to shell out to and Rust has no
        // portable way to ask a spawned child to call the syscall on its own
        // behalf before it execs.
        static BIN: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
        let bin = BIN.get_or_init(|| {
            let src = r#"
                #include <unistd.h>
                int main(void) {
                    if (setsid() == -1) { return 1; }
                    execl("/bin/sleep", "sleep", "30", (char*)0);
                    return 1;
                }
            "#;
            let dir = std::env::temp_dir().join(format!("smetana-kkz2-escaper-{}", std::process::id()));
            std::fs::create_dir_all(&dir).expect("a scratch directory");
            let c = dir.join("escaper.c");
            let bin = dir.join("escaper");
            std::fs::write(&c, src).expect("write the helper's source");
            let status = std::process::Command::new("cc")
                .arg("-O0")
                .arg("-o")
                .arg(&bin)
                .arg(&c)
                .status()
                .expect("a C compiler to build the helper with");
            assert!(status.success(), "the escaping-child helper must build");
            bin
        });
        std::process::Command::new(bin)
    }

    /// The shape of the acceptance criterion word for word: a child spawned
    /// in its own process group is visible in a fresh snapshot by its `ppid`
    /// while its intermediate shell is still alive; once that shell has
    /// exited — reparenting the child under pid 1 and severing the `ppid`
    /// chain a fresh walk depends on — it is found instead by the pgid the
    /// poller already folded in while the chain still held; and once the
    /// child itself is gone, neither finds it.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_macos_descendant_is_found_by_ppid_then_by_pgid_then_not_at_all() {
        let mut shell = escaping_child();
        let mut child = shell.spawn().expect("a process to track");
        let shell_pid = child.id() as i32;

        // The escaper's own pid, not the shell's: it `exec`s in place, so
        // this pid answers for the running `sleep` throughout.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut descendants = Descendants::default();
        loop {
            let snapshot = snapshot_all().expect("this test's own platform can snapshot");
            let found = descendants_by_ppid(&snapshot, std::process::id() as i32);
            // The escaper reparents itself under nobody — it *is* the direct
            // child here, found by walking from this test process.
            if found.iter().any(|p| p.pid == shell_pid) {
                descendants.absorb(&snapshot, std::process::id() as i32);
                break;
            }
            assert!(std::time::Instant::now() < deadline, "the child never appeared in a snapshot");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(!descendants.is_empty(), "the poll folded the child in");

        // `setsid` takes the child out of nobody's reach here — it was
        // already this test process's own direct child — but it is what the
        // real defect's shape rests on once it is orphaned: give it a
        // moment, then confirm it is still found, now purely by the pgid the
        // absorb above already captured, on a *fresh* snapshot exactly as
        // `terminal/service.rs` takes one at a session's own ending.
        std::thread::sleep(std::time::Duration::from_millis(50));
        let fresh = snapshot_all().expect("a fresh snapshot");
        let candidates = descendants.candidates(&fresh, &[std::process::id() as i32]);
        assert!(candidates.contains(&shell_pid), "found by the group it leads: {candidates:?}");

        let _ = child.kill();
        let _ = child.wait();
        assert!(wait_for(|| look(shell_pid) == Seen::Gone), "the child is reaped");
        let fresh = snapshot_all().expect("a fresh snapshot");
        assert!(
            !descendants.candidates(&fresh, &[]).contains(&shell_pid),
            "gone from the fresh table, gone from the candidates"
        );

        // The scratch directory `escaping_child` built its helper binary
        // into — same formula, since the `OnceLock` inside that function
        // keeps no path a caller can ask for. Removed rather than left for
        // the OS's own temp cleanup: this suite runs `cc` again on every
        // invocation, and a directory a day never emptied is exactly the
        // kind of leftover this whole task exists to stop leaving.
        let scratch = std::env::temp_dir().join(format!("smetana-kkz2-escaper-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&scratch);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn an_unexpected_coalition_read_size_answers_unknown() {
        assert_eq!(interpret_coalition(40, 40, 99), Coalition::Known(99));
        assert_eq!(interpret_coalition(-1, 40, 99), Coalition::Unknown, "the call itself failed");
        assert_eq!(interpret_coalition(0, 40, 99), Coalition::Unknown, "nothing was read");
        assert_eq!(
            interpret_coalition(24, 40, 99),
            Coalition::Unknown,
            "a short read — an SDK whose struct shrank under this file"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn this_process_can_read_its_own_coalition() {
        // The measurement the design rests on, checked live rather than only
        // in the rule file: this process answers a coalition id at all, and
        // asking twice answers the same one.
        let Coalition::Known(id) = own_coalition() else {
            panic!("this platform answered proc_pidinfo(20) for its own pid; Unknown means the struct \
                    no longer matches and the whole macOS coalition sweep is inert");
        };
        assert_eq!(own_coalition(), Coalition::Known(id), "stable across two reads");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn a_bundled_xpc_helpers_path_is_excluded_and_an_ordinary_binarys_is_not() {
        assert!(is_xpc_service("/Applications/Smetana.app/Contents/XPCServices/Foo.xpc/Contents/MacOS/Foo"));
        assert!(is_xpc_service(
            "/System/Library/Frameworks/WebKit.framework/Versions/A/XPCServices/com.apple.WebKit.WebContent.xpc/Contents/MacOS/com.apple.WebKit.WebContent"
        ));
        assert!(!is_xpc_service("/bin/sleep"));
        assert!(!is_xpc_service("/usr/local/bin/claude"));
    }
}
