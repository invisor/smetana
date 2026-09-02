//! The disk half of the run registry, and the sweep the app runs at start-up.
//!
//! `registry.rs` decides and `procs.rs` answers for the operating system; this
//! is the file, the read-modify-write, and the one place a leftover process is
//! actually signalled. The same split `gitignore.rs` makes between `amend` and
//! `ensure`, in two files rather than one because the OS half needs `unsafe`
//! and the rules need none.
//!
//! **Every write in this module happens on the run worker's own task**, which
//! is what makes the read-modify-write below safe within one app: the worker
//! sweeps before it serves its first request and is the only thing that writes
//! afterwards. Two app instances working in one project are not ordered against
//! each other, and that is accepted rather than papered over — the loss is one
//! record of one run, both instances leave each other's records alone by
//! construction (`registry::note_run` keys by writer as well as token), and the
//! alternative is a lock file with its own staleness rule, which is the very
//! thing `merging` already pays for elsewhere.
//!
//! Nothing here writes to bd, and nothing here is shown in the interface. The
//! sweep is the app finishing its own interrupted shutdown, not a new decision:
//! a dialog listing what was found would greet a person with housekeeping after
//! every crash — and, in development, after every Rust rebuild — and a line in
//! the interface would spend the loudness budget that belongs to a card needing
//! a human. What was killed goes to the log.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use chrono::Utc;
use tokio::time::Instant;

use super::procs;
use super::registry::{self, Batch, Liveness, Orphan, Proc, Record, Registry};

/// How long a signalled agent is given to leave on its own, and how often it is
/// asked. The same numbers `terminal/service.rs` uses on the way out, for the
/// same reason: a hangup is what a CLI is built for, and one that will not go
/// in a second is not going to be talked out of it. Nothing is waiting on this
/// — it runs before the first request the worker serves, and a run cannot start
/// until it is done, which is deliberate: a batch spawned beside an agent this
/// sweep is about to kill would be two agents in one worktree.
const GRACE: Duration = Duration::from_millis(1200);
const POLL: Duration = Duration::from_millis(50);

/// How long a just-started process is given to become itself, and how often it
/// is asked — see `group`, which is the only caller and carries the reasoning.
/// The window measured on this machine was 160–675 µs, so a second is three
/// orders of magnitude of headroom for a slower machine loading a much larger
/// binary, and it is only ever paid in full by a process that never does become
/// distinguishable, which is not a batch anybody is waiting on twice.
const EXEC: Duration = Duration::from_secs(1);
const EXEC_POLL: Duration = Duration::from_millis(1);

/// Names temp files apart — see `write`.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Where a project's registry lives: beside `project.toml`, inside the folder
/// `gitignore.rs` keeps out of the repository.
pub fn path(root: &Path) -> PathBuf {
    root.join(".smetana").join("runs.json")
}

/// This app process, read once. `None` on a platform that cannot answer, and
/// that answer switches the whole feature off rather than degrading it: a
/// record whose writer carries no evidence would be a record nobody could ever
/// show stale, which is worse than no record at all.
fn writer() -> Option<&'static Proc> {
    static WRITER: OnceLock<Option<Proc>> = OnceLock::new();
    WRITER
        .get_or_init(|| {
            let own = procs::own();
            if own.is_none() {
                log::error!(
                    "[runs] this platform cannot read a process start time, so no run registry is kept"
                );
            }
            own
        })
        .as_ref()
}

/// Remember a run for as long as it lives. Called when the run starts.
pub fn note_run(root: &Path, token: u64, target_branch: &str) {
    let Some(writer) = writer() else { return };
    let record = Record {
        token,
        project: root.to_string_lossy().into_owned(),
        target_branch: target_branch.to_owned(),
        started_at: Utc::now().to_rfc3339(),
        writer: writer.clone(),
        batches: Vec::new(),
    };
    update(root, |held| {
        registry::note_run(held, record);
        true
    });
}

/// Remember the batch a run has just started: the actor it claims under and the
/// process group it can be found in.
pub fn note_batch(root: &Path, token: u64, actor: String, group: Option<Proc>) {
    let Some(writer) = writer() else { return };
    update(root, |held| registry::note_batch(held, writer, token, Batch { actor, group }));
}

/// The run's loop task is gone, however it went. The record goes with it unless
/// it still names a process that is running — see `registry::forget_run`, which
/// is where that condition and its reason live.
pub fn forget_run(root: &Path, token: u64) {
    let Some(writer) = writer() else { return };
    update(root, |held| registry::forget_run(held, writer, token, &procs::look));
}

/// The evidence to write down about a session's process group — the pid the
/// terminal worker holds, paired with the stamp that survives its reuse.
///
/// There is a window between the two reads: if the agent died and the machine
/// handed its pid to something else in that instant, this records a stranger.
/// It is microseconds wide and it fails safe in the only direction that matters
/// at the far end — the stranger's own stamp is what gets written, so the next
/// start compares like with like and signals it only if that very process is
/// still there, which by then means the agent's pid was never reused at all.
///
/// The waiting is the second window, and unlike the first one it is certain
/// rather than unlucky: the terminal worker answers with the pid as soon as the
/// fork is done, and until the exec lands the kernel names that pid after the
/// process that forked it — this app (smetana-6nr0). `procs::spawned` carries
/// what was measured and why the name is what is waited on; this is only the
/// bound. It is asked again rather than slept through once because the wait is
/// normally a single poll, and it is bounded because the alternative to giving
/// up is a run that never starts its batch.
///
/// Giving up writes nothing at all, which is the whole point: a batch with no
/// `group` is one `smetana:merging` and `smetana:running-tasks` leave alone,
/// while a batch under the app's own name is one they break the lock of. The
/// cost of that is one recovery, and the cost of the wrong name is two runs on
/// one branch.
pub async fn group(pid: u32) -> Option<Proc> {
    let deadline = Instant::now() + EXEC;
    loop {
        match procs::spawned(pid as i32) {
            procs::Spawned::Named(proc) => return Some(proc),
            procs::Spawned::Nothing => return None,
            procs::Spawned::Inherited if Instant::now() >= deadline => return None,
            procs::Spawned::Inherited => tokio::time::sleep(EXEC_POLL).await,
        }
    }
}

/// Finish the shutdown a dead app never got to, in every project this one knows
/// about.
///
/// Which projects those are is the settings file's open list plus whatever the
/// app is opening with — a project somebody ran in and then closed is not swept
/// until they open it again, which is the price of the registry being per
/// project rather than one file in `app_config_dir()`. That trade was taken
/// deliberately: a file in the project is one Phase R can find with an ordinary
/// read, where a platform-dependent path in the app's own config directory
/// could only reach a skill by the app telling it, which is a second coupling
/// to keep in step.
pub async fn recover(projects: &[PathBuf]) {
    let mut seen: Vec<&Path> = Vec::new();
    for project in projects {
        if seen.contains(&project.as_path()) {
            continue;
        }
        seen.push(project.as_path());
        recover_one(project).await;
    }
}

async fn recover_one(root: &Path) {
    // The ordinary case, and the reason this is cheap enough to do for every
    // project on every start: no run has ever been started here.
    if !path(root).exists() {
        return;
    }
    let held = match read(root) {
        Held::Read(registry) => registry,
        // The replacement is done here rather than left to whenever a run next
        // starts, so that the line the read just logged is true when it is
        // logged and the `.bak` is not taken again on every launch.
        Held::Damaged => return write(root, &Registry::default()),
        Held::Unreadable => return,
    };
    // What the app is deliberately *not* doing, said out loud: these claims are
    // the next run's Phase R to recover, and the app writes to the tracker
    // nowhere. Somebody reading the log after a crash otherwise has no way to
    // tell "nothing was left behind" from "the tracker half is somebody else's
    // job" — and it is the same conclusion the skill reaches from the same
    // file, which is why it is `registry`'s function and not a second reading
    // of it here.
    let abandoned = registry::abandoned_actors(&held, &procs::look);
    if !abandoned.is_empty() {
        log::info!(
            "[runs] recovery in {}: {} claimed under a run whose app is gone; the next run's Phase R recovers those",
            root.display(),
            abandoned.join(", ")
        );
    }
    let swept = registry::sweep(&held, Utc::now(), &procs::look);
    hang_up(root, &swept.signal).await;
    if swept.changed {
        write(root, &swept.next);
    }
}

/// Signal what a dead run left running, then make sure it went.
///
/// Signal first and kill second, which is what a clean exit does and not an
/// approximation of it: an agent given no warning flushes nothing. Anything the
/// registry does not name is not here and is never touched — the app cannot
/// show it started it.
async fn hang_up(root: &Path, orphans: &[Orphan]) {
    let mut signalled: Vec<&Orphan> = Vec::new();
    for orphan in orphans {
        if procs::hangup_group(orphan.group.pid) {
            log::info!(
                "[runs] recovery in {}: hung up {} (pid {}), left by a run whose app is gone",
                root.display(),
                describe(orphan),
                orphan.group.pid
            );
            signalled.push(orphan);
        }
    }
    if signalled.is_empty() {
        return;
    }
    let deadline = Instant::now() + GRACE;
    while Instant::now() < deadline {
        if signalled.iter().all(|orphan| gone(orphan)) {
            return;
        }
        tokio::time::sleep(POLL).await;
    }
    for orphan in signalled {
        if !gone(orphan) && procs::kill_group(orphan.group.pid) {
            log::warn!(
                "[runs] recovery in {}: killed {} (pid {}), which did not leave on its own",
                root.display(),
                describe(orphan),
                orphan.group.pid
            );
        }
    }
}

fn describe(orphan: &Orphan) -> String {
    format!("{} working as {}", orphan.group.command, orphan.actor)
}

/// Has this group's leader gone? A pid that answers with a different stamp has
/// gone too — what is under it now is somebody else, and the wait is over
/// either way.
fn gone(orphan: &Orphan) -> bool {
    registry::liveness(&orphan.group, procs::look(orphan.group.pid)) != Liveness::Alive
}

/// Read, change, write — and write nothing when nothing changed, so that a
/// no-op cannot lose a record another instance wrote between the two.
fn update(root: &Path, change: impl FnOnce(&mut Registry) -> bool) {
    let (mut held, replacing) = match read(root) {
        Held::Read(registry) => (registry, false),
        // A copy is already beside it; this write is the replacement, and it
        // goes out whether or not the change itself asked for one.
        Held::Damaged => (Registry::default(), true),
        Held::Unreadable => return,
    };
    if change(&mut held) || replacing {
        write(root, &held);
    }
}

/// What came back from the file, in the three states that need different
/// answers — the same three `settings/file.rs` keeps apart, and for the reason
/// it records: overwriting a file nobody could read would destroy it sight
/// unseen. Here what would be destroyed is Phase R's evidence and the pids of
/// processes that are still running.
enum Held {
    /// The file as it stands, or an empty registry where there is no file yet.
    Read(Registry),
    /// Damaged, or written by a newer app. A copy is in `runs.json.bak` and the
    /// caller replaces what is left — leaving it instead would be a project in
    /// which no run can ever be recorded again, and a registry with no readers
    /// is not evidence of anything.
    ///
    /// **The too-new half of that is accepted rather than overlooked**, and
    /// replacing at every start rather than at the next run widened it: an
    /// older app now wipes a newer one's live records when it launches. It is
    /// the same answer `settings/file.rs` gives a too-new file, the cost is
    /// bounded by two app versions being run against one project at once, and
    /// this module already accepts that two instances there are unordered.
    /// Distinguishing the two would mean a second staleness rule for a file
    /// whose whole design is that liveness is proved by the record.
    Damaged,
    /// There is a file and it could not be read at all: no permissions, a
    /// directory in its place, a failing disk. Nothing may be written over it,
    /// and there is nothing to copy either — `fs::copy` would fail the same
    /// way the read did.
    Unreadable,
}

fn read(root: &Path) -> Held {
    let path = path(root);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Held::Read(Registry::default())
        }
        Err(err) => {
            log::warn!(
                "[runs] {} could not be read ({err}); nothing will be written over it",
                path.display()
            );
            return Held::Unreadable;
        }
    };
    match registry::parse(&text) {
        Some(registry) => Held::Read(registry),
        None => {
            let backup = path.with_extension("json.bak");
            if let Err(err) = std::fs::copy(&path, &backup) {
                log::warn!("[runs] could not save a copy to {}: {err}", backup.display());
            }
            log::warn!(
                "[runs] {} could not be read; a copy is in {} and it is being replaced",
                path.display(),
                backup.display()
            );
            Held::Damaged
        }
    }
}

/// Atomic, the same way `settings/file.rs` writes: a neighbour first, flushed,
/// then renamed. A break halfway through would otherwise leave half a JSON, and
/// the next start would read a file it has to throw away — taking with it the
/// evidence that a run was ever going.
fn write(root: &Path, held: &Registry) {
    let path = path(root);
    let text = match registry::render(held) {
        Ok(text) => text,
        Err(err) => {
            log::warn!("[runs] could not write the run registry: {err}");
            return;
        }
    };
    let Some(dir) = path.parent() else { return };
    if let Err(err) = std::fs::create_dir_all(dir) {
        log::warn!("[runs] could not create {}: {err}", dir.display());
        return;
    }
    // The counter beside the pid, copied from `settings/file.rs` along with its
    // reason: two overlapping writes would otherwise share one name, and the
    // first would rename what the second had not finished writing. Every write
    // here is a synchronous call on the run worker's single task today, so the
    // overlap cannot happen — but the guard costs one atomic and the reason for
    // dropping it would have to be rediscovered by whoever first calls this
    // from somewhere else.
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!("runs.{}.{n}.tmp", std::process::id()));
    if let Err(err) = write_all(&temp, &text) {
        log::warn!("[runs] could not write {}: {err}", temp.display());
        let _ = std::fs::remove_file(&temp);
        return;
    }
    if let Err(err) = std::fs::rename(&temp, &path) {
        log::warn!("[runs] could not write {}: {err}", path.display());
        let _ = std::fs::remove_file(&temp);
    }
}

fn write_all(temp: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = std::fs::File::create(temp)?;
    file.write_all(text.as_bytes())?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_root() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!("smetana-registry-{}-{n}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the test's project");
        root
    }

    /// The round trip through the disk: while a run is live the file names it,
    /// and its ending takes the record away once nothing it named is still
    /// running. The other half of that condition — an ending that leaves a
    /// session alive at its prompt — is `registry`'s to pin, since it is a rule
    /// and not a file operation.
    #[test]
    fn a_live_run_is_named_in_the_file_and_an_ending_removes_it() {
        let root = temp_root();
        if writer().is_none() {
            eprintln!("this platform keeps no registry; nothing to check");
            return;
        }

        note_run(&root, 3, "develop");
        // A group that is provably gone, because no kernel hands out
        // `i32::MAX`: this test is about the disk, and a batch that is still
        // running is deliberately a record that stays.
        let dead = Proc { pid: i32::MAX, started: 1, command: "claude".into() };
        note_batch(&root, 3, "smetana-run-7".into(), Some(dead));

        let text = std::fs::read_to_string(path(&root)).expect("the file exists");
        let held = registry::parse(&text).expect("and reads");
        assert_eq!(held.runs.len(), 1);
        assert_eq!(held.runs[0].token, 3);
        assert_eq!(held.runs[0].target_branch, "develop");
        assert_eq!(held.runs[0].project, root.to_string_lossy());
        assert_eq!(held.runs[0].batches[0].actor, "smetana-run-7");
        assert_eq!(
            held.runs[0].writer.pid,
            std::process::id() as i32,
            "the record says which process wrote it"
        );

        forget_run(&root, 3);
        let text = std::fs::read_to_string(path(&root)).expect("the file is still there");
        assert!(registry::parse(&text).expect("and reads").runs.is_empty());
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The sweep against a live writer — this very test process — is the
    /// closest a test gets to the second start: nothing may be signalled and
    /// the record has to survive it whole.
    #[tokio::test]
    async fn a_sweep_leaves_a_live_writers_record_alone() {
        let root = temp_root();
        if writer().is_none() {
            eprintln!("this platform keeps no registry; nothing to check");
            return;
        }
        note_run(&root, 1, "main");
        let before = std::fs::read_to_string(path(&root)).expect("written");

        recover(&[root.clone(), root.clone()]).await;

        assert_eq!(std::fs::read_to_string(path(&root)).expect("still there"), before);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The sweep against real processes, which is as close as a test gets to
    /// the thing itself: the app cannot be `kill -9`ed from inside its own test
    /// binary, so the dead writer is stated in the file rather than produced —
    /// everything after that is the code that runs at start-up, against a
    /// process group that is genuinely there.
    ///
    /// Two properties in one test because they are one situation: what the
    /// registry names goes, together with what it spawned, and what the
    /// registry does not name is not touched. Splitting them would mean
    /// spawning the same little tree twice to check the halves separately.
    #[cfg(unix)]
    #[tokio::test]
    async fn a_dead_runs_processes_go_and_a_stranger_is_left_alone() {
        use std::io::BufRead;
        use std::os::unix::process::{CommandExt, ExitStatusExt};
        use std::process::{Command, Stdio};

        /// A process group of the shape an agent leaves behind: a leader of its
        /// own session — which is what `portable-pty` gives a session, and what
        /// makes a group-wide signal reach the work rather than the agent alone
        /// — with a child of its own inside it.
        fn detached(script: &str) -> std::process::Child {
            let mut command = Command::new("/bin/sh");
            command.arg("-c").arg(script).stdout(Stdio::piped());
            unsafe {
                command.pre_exec(|| match libc::setsid() {
                    -1 => Err(std::io::Error::last_os_error()),
                    _ => Ok(()),
                });
            }
            command.spawn().expect("a shell to stand in for an agent")
        }

        let root = temp_root();
        let mut orphan = detached("sleep 30 & echo $! ; wait");
        let mut stranger = detached("sleep 30");
        let spawned: i32 = {
            let out = orphan.stdout.take().expect("the pid comes back on stdout");
            let mut line = String::new();
            std::io::BufReader::new(out).read_line(&mut line).expect("read the pid");
            line.trim().parse().expect("a pid")
        };
        let leader = orphan.id() as i32;
        let bystander = stranger.id() as i32;
        let group = procs::snapshot(leader).expect("this platform can read a start time");

        // The file a run killed with the app would have left: a writer that is
        // provably gone — no kernel hands out `i32::MAX` — naming a group that
        // is provably still there.
        write(
            &root,
            &Registry {
                version: registry::VERSION,
                runs: vec![Record {
                    token: 1,
                    project: root.to_string_lossy().into_owned(),
                    target_branch: "main".into(),
                    started_at: Utc::now().to_rfc3339(),
                    writer: Proc { pid: i32::MAX, started: 1, command: "smetana".into() },
                    batches: vec![Batch { actor: "smetana-run-1".into(), group: Some(group) }],
                }],
            },
        );

        recover(&[root.clone()]).await;

        let ended = orphan.wait().expect("the agent is reaped");
        assert_eq!(
            ended.signal(),
            Some(libc::SIGHUP),
            "the same hangup a clean exit sends, not a kill"
        );
        // What the agent started goes with it, which is the whole reason the
        // signal is group-wide. It is nobody's child now, so it leaves the
        // table when init reaps it rather than the instant it dies.
        let deadline = Instant::now() + Duration::from_secs(5);
        while procs::look(spawned) != registry::Seen::Gone && Instant::now() < deadline {
            tokio::time::sleep(POLL).await;
        }
        assert_eq!(procs::look(spawned), registry::Seen::Gone, "what the agent spawned went too");

        assert!(
            matches!(procs::look(bystander), registry::Seen::Running { .. }),
            "a process the registry never named is not the app's to signal"
        );
        let Held::Read(held) = read(&root) else { panic!("the file is still readable") };
        assert_eq!(held.runs.len(), 1, "the record stays: its actors are Phase R's evidence");

        unsafe { libc::killpg(bystander, libc::SIGKILL) };
        let _ = stranger.wait();
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_damaged_file_is_kept_as_a_copy_and_replaced_where_it_stood() {
        let root = temp_root();
        std::fs::create_dir_all(root.join(".smetana")).expect("setup");
        std::fs::write(path(&root), "{not json").expect("setup");

        assert!(matches!(read(&root), Held::Damaged));
        assert_eq!(
            std::fs::read_to_string(root.join(".smetana").join("runs.json.bak")).expect("a copy"),
            "{not json"
        );

        // The replacement is the caller's, and both callers do it — otherwise
        // the log line saying so is untrue and the copy is retaken on every
        // launch until a run happens to start.
        recover(&[root.clone()]).await;
        assert!(
            matches!(read(&root), Held::Read(registry) if registry == Registry::default()),
            "the sweep leaves a file this app can read"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The refusal `settings/file.rs` already makes for the same reason:
    /// overwriting a file nobody could read destroys it sight unseen, and here
    /// what it holds is Phase R's evidence and the pids of live orphans.
    #[cfg(unix)]
    #[test]
    fn a_file_that_cannot_be_read_is_never_written_over() {
        use std::os::unix::fs::PermissionsExt;

        if unsafe { libc::geteuid() } == 0 {
            // Root reads whatever it likes, so the setup below would not hold.
            eprintln!("running as root; the unreadable case cannot be set up");
            return;
        }
        let root = temp_root();
        std::fs::create_dir_all(root.join(".smetana")).expect("setup");
        let path = path(&root);
        std::fs::write(&path, "the bytes that must survive").expect("setup");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).expect("setup");

        assert!(matches!(read(&root), Held::Unreadable));
        note_run(&root, 1, "main");
        assert!(!root.join(".smetana").join("runs.json.bak").exists(), "there was nothing to copy");

        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("cleanup");
        assert_eq!(
            std::fs::read_to_string(&path).expect("read it back"),
            "the bytes that must survive"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn a_project_that_has_never_held_a_run_is_not_written_to() {
        // The sweep runs for every project the app opens with, so the case with
        // no file at all has to cost nothing and leave nothing behind.
        let root = temp_root();
        recover(&[root.clone()]).await;
        assert!(!root.join(".smetana").exists(), "nothing is created by looking");
        let _ = std::fs::remove_dir_all(&root);
    }
}
