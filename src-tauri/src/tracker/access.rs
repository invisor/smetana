//! Whether the folder can be read at all — and the one repair for when it
//! cannot.
//!
//! This exists because two different troubles used to arrive as one state. A
//! `bd list` that fails because the tracker was built by an older bd, and a
//! `bd list` that fails because the operating system will not let this app open
//! the folder, both reached the screen as `error` — "bd is failing … most often
//! the tracker was made by an older bd", with a button that runs a database
//! migration. The second of those is not about the tracker at all, and sending
//! somebody into a migration over a permission is the whole of smetana-8lq.
//!
//! The fact is taken from the filesystem rather than from bd's words. bd's
//! wording is bd's, it moves between releases, and a recognizer built out of a
//! grep over prose is the failure mode `Bd::repair` already records for
//! `bd migrate` — while `ErrorKind::PermissionDenied` on the folder is the same
//! fact the file tree already reports with its own sentence
//! (`files::fs::io_error`), which is why the two now agree on screen.
//!
//! The macOS half. A grant is stored per bundle identifier **and** per code
//! requirement, and there are two ordinary ways for this app to end up refused
//! with no prompt in sight:
//!
//! - somebody once answered "Don't Allow", and macOS asks only when there is no
//!   stored decision, so no dialog will ever appear again;
//! - the grant was given to an ad-hoc signed copy, whose code requirement is a
//!   cdhash that changes with every build, so an in-place update silently
//!   invalidated whatever had been granted before it. smetana-fkt ended that
//!   for every release signed with a Developer ID, whose requirement is the
//!   team and does not move between builds — but a grant already lost that way
//!   stays lost, and this is the only thing that gets it back. Which releases
//!   were ad-hoc signed is in RELEASING.md; it is not a fact this file has any
//!   way of checking, so it is not repeated here.
//!
//! Both are undone the same way — `tccutil reset <service> <identifier>` and a
//! restart, after which macOS has no stored decision and asks again. That is
//! what [`reset`] does, and it is the whole of the repair.
//!
//! **The repair is offered only where macOS will actually ask again**, and that
//! is the sharpest decision in this file. TCC has one service per protected
//! place — Desktop, Documents, Downloads, a mounted volume — and prompts for
//! each; everything else on the disk is governed by Full Disk Access, which
//! macOS **never prompts for at all**. So running `tccutil reset
//! SystemPolicyAllFiles` would clear a grant and leave no way to ask for it
//! back: the person has to find System Settings themselves, which is precisely
//! what the button was supposed to save them.
//!
//! It is worse than useless there, because a folder outside the four is refused
//! for reasons that have nothing to do with TCC — an ordinary unix mode, a
//! volume that went away — and nothing in a path tells those apart from a stale
//! Full Disk Access entry. The button would then wipe a working grant the
//! person did give, restart, and come back to the same refusal. So
//! [`tcc_service`] answers `None` there, [`AccessRepair`] says
//! `full-disk-access`, and the notice names System Settings instead of
//! promising a prompt.
//!
//! **A folder has two spellings and both are asked**, which is [`service_for`]
//! and the last decision in this file. A path can be a symlink *out* of a
//! protected folder or *into* one, and both are ordinary rather than exotic:
//! with iCloud "Desktop & Documents Folders" switched on — a checkbox a great
//! many people have ticked — `~/Desktop` is itself a link into
//! `~/Library/Mobile Documents/…`, while a checked-out project can just as
//! easily be a link the other way. So the literal path is asked first and the
//! canonicalized one second, and the first promptable service either spelling
//! names is the answer.
//!
//! Choosing one spelling cannot cover both, because the two cases are
//! symmetric. Asking both is safe for one specific reason, and it is the same
//! asymmetry the paragraph above rests on: **every service either spelling can
//! name is one macOS will prompt for again.** `SystemPolicyAllFiles` is not in
//! the answer space at all, so the union of two promptable answers is still
//! promptable, and a wrong hit costs exactly one dialog and nothing that cannot
//! be taken back.
//!
//! It follows that [`repair_for`] and [`reset`] must resolve a folder the same
//! way, and the way they do it is by both calling [`service_for`] rather than
//! each doing its own. They disagreed once, and the cost was the shape of the
//! defect this whole module exists to remove: the notice promising a prompt
//! over a button that then refused, and — the other way round — the notice
//! sending somebody to grant Full Disk Access for a folder a Desktop dialog
//! would have covered. `repair_for_agrees_with_what_reset_would_do` is that
//! invariant, written down.

use std::path::Path;

use super::model::{Health, HealthState, TrackerError};

/// Reading this path is refused, as opposed to failing some other way.
///
/// `read_dir` rather than a metadata call, because metadata is not what is
/// being refused: on macOS a `stat` of a folder inside `~/Desktop` answers
/// perfectly well for an app with no Desktop grant, and it is opening the
/// directory that comes back `EPERM`. That asymmetry is why the reported build
/// got as far as `error` at all — `project::has_tracker` is an `is_dir`, it
/// said yes, and only bd found out the truth.
///
/// The first entry is read as well as the handle opened. It costs one syscall
/// and covers the shape where the refusal arrives on the read rather than on
/// the open; a directory that is genuinely empty yields `None` there, which is
/// not a refusal and is not treated as one.
///
/// Anything else — the path is not there, it is a file, the disk is gone — is
/// deliberately **not** a refusal. This function answers one question, and a
/// state that swallowed every I/O error would put "the system is refusing this
/// folder" under a folder somebody had simply deleted.
fn denied(path: &Path) -> bool {
    match std::fs::read_dir(path) {
        Err(err) => err.kind() == std::io::ErrorKind::PermissionDenied,
        Ok(mut entries) => matches!(
            entries.next(),
            Some(Err(err)) if err.kind() == std::io::ErrorKind::PermissionDenied
        ),
    }
}

/// The project folder is being refused, as a sentence for the screen — or
/// `None`, which is the ordinary answer.
///
/// The folder **and** its `.beads`, because the two are refused independently:
/// a project directory the app may read whose `.beads` sits behind a mode
/// nobody meant to set is the same trouble from a person's side, and it is bd
/// that would report it. The path that was actually refused is what the message
/// names, since "this folder" is ambiguous the moment there are two of them.
///
/// The message says what is true and nothing more. It must not mention bd's
/// version or the tracker's data: claiming either is precisely the defect this
/// module was written to remove.
pub fn refusal(dir: &Path) -> Option<String> {
    [dir.to_path_buf(), dir.join(".beads")]
        .into_iter()
        .find(|path| denied(path))
        .map(|path| format!("no permission to read {}", path.display()))
}

/// What a failed bd call in this folder actually amounts to.
///
/// The one place the two states are told apart, and it is a function rather
/// than three lines inside `HealthReporter::failed` so that it can be tested:
/// the reporter holds an `AppHandle` and nothing in a unit test has one.
///
/// The filesystem is asked only when bd has already failed. A folder is not
/// probed on the happy path — a successful `bd list` is proof the folder could
/// be read, and a `read_dir` per sweep would be a syscall bought for nothing.
pub fn health_for_failure(dir: Option<&Path>, error: &TrackerError) -> Health {
    if let Some(message) = dir.and_then(refusal) {
        return Health { state: HealthState::FolderRefused, message: Some(message) };
    }
    Health { state: HealthState::Error, message: Some(error.to_string()) }
}

/// Whether this build can offer the one-press repair.
///
/// `tccutil` is macOS's and has no counterpart anywhere else. A refused folder
/// is still a refused folder on Linux and on Windows — the state is reported
/// there exactly the same way — and the front end draws the notice without a
/// button, with copy that says what to do by hand instead.
pub const fn reset_supported() -> bool {
    cfg!(target_os = "macos")
}

/// Which TCC service governs this folder — when one macOS will **ask** about
/// governs it at all.
///
/// Pure, and given the home directory rather than reading it, because this is
/// the piece worth a test: naming the wrong service resets a grant the person
/// never gave for a folder they were not asking about.
///
/// `Path::starts_with` compares whole components, so `~/Desktopping` is not
/// under `~/Desktop` — a string prefix test would have said it was. Nothing is
/// canonicalized here, which is what keeps this pure: resolving a path is
/// [`service_for`]'s and nowhere else's, and the module header says why there
/// is exactly one place that does it.
///
/// `None` is the important answer and not a failure: it means no promptable
/// service covers this folder, so there is nothing here a reset could usefully
/// clear. The module header carries the whole of why that is a refusal rather
/// than a fall back to `SystemPolicyAllFiles`.
///
/// One limit left inside the `Some`: a mounted volume can be removable, a disk
/// image or a network share, and only the last has a service of its own
/// (`SystemPolicyNetworkVolumes`). Nothing in the path says which, so `/Volumes`
/// answers with the common case — and unlike Full Disk Access, both of those are
/// grants macOS will prompt for again, so a wrong guess costs one dialog rather
/// than a grant that cannot be asked for.
///
/// Only [`reset`] and [`repair_for`] call it, so on a platform without `tccutil`
/// it is dead code — and it stays compiled and stays tested there anyway rather
/// than being cfg'd away. This is the piece a mistake is expensive in, the test
/// that guards it is pure, and a rule only one platform's build type-checks is a
/// rule that rots on the other two.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn tcc_service(dir: &Path, home: Option<&Path>) -> Option<&'static str> {
    if let Some(home) = home {
        for (folder, service) in [
            ("Desktop", "SystemPolicyDesktopFolder"),
            ("Documents", "SystemPolicyDocumentsFolder"),
            ("Downloads", "SystemPolicyDownloadsFolder"),
        ] {
            if dir.starts_with(home.join(folder)) {
                return Some(service);
            }
        }
    }
    if dir.starts_with("/Volumes") {
        return Some("SystemPolicyRemovableVolume");
    }
    None
}

/// What can be done about a refused folder, from here, right now.
///
/// Three answers rather than a boolean, because the two that offer no button
/// offer different advice and one of them is the whole of finding 3 on this
/// task: a folder governed by Full Disk Access has a repair, it is simply not
/// one this app may press on somebody's behalf, and a sentence that said only
/// "grant this app access" would leave them looking for a dialog that is never
/// going to appear.
///
/// It is a question about the folder and not about the build, which is why the
/// front end asks it again on a project switch: `~/Desktop/a` and `~/code/b` get
/// different answers in the same launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccessRepair {
    /// A promptable service governs it: `tccutil reset` and a restart, and macOS
    /// asks again. The one case with a button.
    Reset,
    /// macOS, but Full Disk Access governs it — which is never prompted for. The
    /// notice names System Settings; nothing is pressed here.
    FullDiskAccess,
    /// No `tccutil` on this platform at all. The refusal is still reported and
    /// still drawn; the sentence has to carry the whole of what to do.
    Unavailable,
}

/// A path as the filesystem would spell it, or as it stands when it cannot be
/// read.
///
/// The fallback is not a nicety: `canonicalize` opens the path, and being unable
/// to open the path is the whole condition being repaired. Falling back to the
/// literal spelling loses nothing, because [`service_for`] asks that spelling
/// first anyway.
fn resolved(path: &Path) -> std::path::PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}

/// Are these two spellings the same folder?
///
/// The literal comparison first, so the common case costs no syscall at all,
/// and [`resolved`] behind it for the case that made this a rule rather than a
/// `==`: a run carries the folder it was started with while the tracker worker
/// carries the folder it was handed, and a symlink, or `/tmp` against
/// `/private/tmp`, makes those two spellings of one project. Reading them as
/// two projects sends a run's board read to a stranger's board, which is the
/// whole of what this predicate exists to stop.
///
/// It is [`resolved`] rather than a second `canonicalize` of its own for the
/// reason [`service_for`] uses that one: a path that cannot be opened still has
/// to answer, and the literal spelling is what it answers with. A run's root
/// may be gone from under it — a folder somebody moved mid-run — and a
/// predicate that panicked or answered "not the same folder as itself" there
/// would turn a missing folder into a write aimed somewhere else.
pub(super) fn same_dir(a: &Path, b: &Path) -> bool {
    a == b || resolved(a) == resolved(b)
}

/// Which promptable service governs this folder, under **either** spelling of
/// its path.
///
/// The one place a folder is resolved, and the reason it is one place is that
/// [`repair_for`] and [`reset`] have to agree: the first decides whether a
/// button is drawn and which sentence goes above it, the second decides what
/// `tccutil` is handed, and a folder that answered differently to the two would
/// draw a button that refuses itself, or withhold one while telling a person to
/// go and grant Full Disk Access instead. The module header carries why both
/// spellings are asked and why asking both is safe.
///
/// `home` is resolved alongside `dir` rather than left as it came, so the two
/// sides of every `starts_with` are spelled the same way. A home directory is a
/// symlink far less often than a project is, but `/tmp` against `/private/tmp`
/// is enough to make the point on this very platform.
///
/// Unlike [`tcc_service`] this touches the disk, which is why the pure half is
/// still a function of its own: the mapping is what the tests are about, and it
/// stays reachable without a filesystem to arrange.
pub fn service_for(dir: &Path, home: Option<&Path>) -> Option<&'static str> {
    if let Some(service) = tcc_service(dir, home) {
        return Some(service);
    }
    let home = home.map(resolved);
    tcc_service(&resolved(dir), home.as_deref())
}

/// The rule above, as one call. `home` is passed in for the same reason
/// [`tcc_service`] takes it — so the whole of the decision is reachable from a
/// test without a particular machine's home directory in it.
pub fn repair_for(dir: &Path, home: Option<&Path>) -> AccessRepair {
    if !reset_supported() {
        return AccessRepair::Unavailable;
    }
    match service_for(dir, home) {
        Some(_) => AccessRepair::Reset,
        None => AccessRepair::FullDiskAccess,
    }
}

/// The home directory, for [`tcc_service`]. `HOME` rather than a crate, the
/// same way `agents::library` and `runs::browser` already read it. Dead
/// everywhere but macOS, for the reason given above it.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from).filter(|p| !p.as_os_str().is_empty())
}

/// Forget the stored decision for this folder, so that macOS asks again.
///
/// Blocking: it spawns a process and waits for it. The caller puts it on a
/// blocking thread — the tracker worker is the one task answering every other
/// command.
///
/// `identifier` is passed in rather than written here. The bundle identifier
/// already exists in `tauri.conf.json` and is repeated once in `runs::awake`,
/// and a third literal copy is a string that goes stale in silence: the caller
/// reads it from the running app with `app.config().identifier`, so this cannot
/// name an app other than the one asking.
///
/// The absolute path rather than the bare name. An app launched from Finder
/// inherits `launchd`'s environment and not a login shell's — the same fact
/// `terminal::pty` and `shell_env` are built around — and a `PATH` this app did
/// not set is not a thing to look a repair up in.
///
/// The service comes from [`service_for`] and **not** from a resolution of this
/// function's own. That is the invariant this module was corrected for: the
/// read that draws the notice asks the same question through the same call, so
/// the button and the sentence above it cannot come to disagree about which
/// folder this is or what can be done to it.
///
/// The refusal on `None` is the second lock on the same door. The front end is
/// already told not to offer the button ([`repair_for`]), and this refuses
/// anyway, because the cost of getting here by some other route is a Full Disk
/// Access grant cleared with no way to ask for it back.
#[cfg(target_os = "macos")]
pub fn reset(dir: &Path, identifier: &str) -> Result<(), String> {
    let Some(service) = service_for(dir, home().as_deref()) else {
        return Err(
            "no permission macOS will ask about again governs this folder, so there is \
             nothing here to reset"
                .to_string(),
        );
    };
    let out = std::process::Command::new("/usr/bin/tccutil")
        .args(["reset", service, identifier])
        .output()
        .map_err(|err| format!("could not run tccutil: {err}"))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let said = said.trim();
        let said = if said.is_empty() { "it said nothing" } else { said };
        return Err(format!("tccutil reset {service} exited unsuccessfully: {said}"));
    }
    Ok(())
}

/// The same signature where there is no `tccutil`. The command exists on every
/// platform so that the handler list is one list; it refuses here, and the
/// front end never offers the button in the first place because
/// [`reset_supported`] is false.
#[cfg(not(target_os = "macos"))]
pub fn reset(_dir: &Path, _identifier: &str) -> Result<(), String> {
    Err("resetting a folder permission is macOS only".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// A directory of its own per test; the pid keeps parallel runs apart.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    /// Take every permission off a directory, and say whether that achieved
    /// anything: running as root ignores the mode, and a test that did not
    /// check would pass while proving nothing.
    ///
    /// A skip says so out loud, the way `runs::recovery` does for the same
    /// case. A test that returns silently under root is one that has been
    /// passing vacuously for however long nobody looked.
    #[cfg(unix)]
    fn make_unreadable(dir: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o000)).expect("drop the permissions");
        if fs::read_dir(dir).is_ok() {
            eprintln!("running as root; a folder that cannot be read cannot be set up");
            return false;
        }
        true
    }

    #[cfg(unix)]
    fn make_readable(dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o755)).expect("restore the mode");
    }

    #[cfg(unix)]
    #[test]
    fn a_folder_that_cannot_be_read_turns_a_bd_failure_into_a_refusal() {
        let root = scratch("refused-folder");
        if !make_unreadable(&root) {
            // The mode does not apply (root), so there is nothing to test.
            make_readable(&root);
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let failure = TrackerError::Command {
            command: "list --all -n 0 --json".into(),
            code: 1,
            stderr: "failed to open store".into(),
        };
        let health = health_for_failure(Some(&root), &failure);

        // Restore the mode before the assertions: a failing assertion panics,
        // and the cleanup below would then never run.
        make_readable(&root);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(health.state, HealthState::FolderRefused);
        let message = health.message.unwrap_or_default();
        assert!(message.contains("no permission"), "{message}");
        assert!(
            !message.contains("failed to open store"),
            "the refusal must not quote bd about a folder bd was never able to read: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_beads_directory_that_cannot_be_read_is_the_same_refusal() {
        let root = scratch("refused-beads");
        let beads = root.join(".beads");
        fs::create_dir_all(&beads).unwrap();
        if !make_unreadable(&beads) {
            make_readable(&beads);
            let _ = fs::remove_dir_all(&root);
            return;
        }

        let refused = refusal(&root);

        make_readable(&beads);
        let _ = fs::remove_dir_all(&root);

        let message = refused.expect("an unreadable .beads is a refusal too");
        assert!(message.contains(".beads"), "the message names what was refused: {message}");
    }

    #[test]
    fn a_readable_folder_leaves_a_bd_failure_as_an_error() {
        let root = scratch("readable-folder");
        let failure = TrackerError::Command {
            command: "list --all -n 0 --json".into(),
            code: 1,
            stderr: "schema version 41 is older than 53".into(),
        };

        let health = health_for_failure(Some(&root), &failure);
        let _ = fs::remove_dir_all(&root);

        assert_eq!(health.state, HealthState::Error);
        assert!(
            health.message.unwrap_or_default().contains("schema version 41"),
            "an ordinary bd failure still reaches the screen in bd's own words"
        );
    }

    #[test]
    fn a_failure_with_no_project_behind_it_is_an_error() {
        // check_version runs before any folder is open, and there is nothing to
        // ask the filesystem about.
        let health = health_for_failure(None, &TrackerError::Empty);
        assert_eq!(health.state, HealthState::Error);
    }

    #[test]
    fn a_folder_that_is_not_there_is_not_a_refusal() {
        let missing = std::env::temp_dir().join("smetana-no-such-folder-at-all");
        let _ = std::fs::remove_dir_all(&missing);
        assert!(refusal(&missing).is_none(), "a missing folder is not a refused one");
    }

    #[test]
    fn the_three_home_folders_have_services_of_their_own() {
        let home = PathBuf::from("/Users/you");
        assert_eq!(
            tcc_service(Path::new("/Users/you/Desktop/Projects/smetana"), Some(&home)),
            Some("SystemPolicyDesktopFolder")
        );
        assert_eq!(
            tcc_service(Path::new("/Users/you/Documents/work"), Some(&home)),
            Some("SystemPolicyDocumentsFolder")
        );
        assert_eq!(
            tcc_service(Path::new("/Users/you/Downloads/x"), Some(&home)),
            Some("SystemPolicyDownloadsFolder")
        );
        // The folder itself, not only something inside it.
        assert_eq!(
            tcc_service(Path::new("/Users/you/Desktop"), Some(&home)),
            Some("SystemPolicyDesktopFolder")
        );
    }

    #[test]
    fn a_name_that_merely_starts_the_same_is_not_the_desktop() {
        // The one a string prefix test gets wrong, and getting it wrong resets a
        // grant nobody asked about.
        let home = PathBuf::from("/Users/you");
        assert_eq!(tcc_service(Path::new("/Users/you/Desktopping/smetana"), Some(&home)), None);
    }

    #[test]
    fn a_mounted_volume_has_one_and_an_ordinary_folder_has_none() {
        let home = PathBuf::from("/Users/you");
        assert_eq!(
            tcc_service(Path::new("/Volumes/Work/smetana"), Some(&home)),
            Some("SystemPolicyRemovableVolume")
        );
        assert_eq!(tcc_service(Path::new("/Users/you/Projects/smetana"), Some(&home)), None);
        assert_eq!(tcc_service(Path::new("/opt/smetana"), Some(&home)), None);
    }

    /// The whole of finding 3 on this task, as an assertion: nothing anywhere
    /// may answer `SystemPolicyAllFiles`. Clearing Full Disk Access is the one
    /// thing this button could do that macOS will never let the person undo
    /// with a dialog, and every folder outside the four is refused for reasons
    /// that have nothing to do with TCC.
    #[test]
    fn full_disk_access_is_never_named_as_something_to_reset() {
        let home = PathBuf::from("/Users/you");
        for dir in [
            "/Users/you/Projects/smetana",
            "/Users/you/Library/Mobile Documents/smetana",
            "/opt/smetana",
            "/",
        ] {
            assert_eq!(
                tcc_service(Path::new(dir), Some(&home)),
                None,
                "{dir} must not be answered with a service, since only AllFiles governs it"
            );
            assert_eq!(
                repair_for(Path::new(dir), Some(&home)),
                if reset_supported() {
                    AccessRepair::FullDiskAccess
                } else {
                    AccessRepair::Unavailable
                },
                "{dir}"
            );
        }
    }

    #[test]
    fn a_promptable_folder_is_the_one_case_with_a_button() {
        let home = PathBuf::from("/Users/you");
        let expected =
            if reset_supported() { AccessRepair::Reset } else { AccessRepair::Unavailable };
        assert_eq!(repair_for(Path::new("/Users/you/Desktop/Projects/x"), Some(&home)), expected);
        assert_eq!(repair_for(Path::new("/Volumes/Work/x"), Some(&home)), expected);
    }

    /// The three answers are what the front end switches on, so their spelling
    /// on the wire is a contract rather than an enum's business.
    #[test]
    fn the_three_answers_travel_as_kebab_case() {
        assert_eq!(serde_json::to_string(&AccessRepair::Reset).unwrap(), "\"reset\"");
        assert_eq!(
            serde_json::to_string(&AccessRepair::FullDiskAccess).unwrap(),
            "\"full-disk-access\""
        );
        assert_eq!(serde_json::to_string(&AccessRepair::Unavailable).unwrap(), "\"unavailable\"");
    }

    #[test]
    fn with_no_home_only_the_absolute_answers_are_left() {
        // A process with no HOME is odd but not impossible, and a folder under a
        // home directory nobody could name is not any particular one of the
        // three.
        assert_eq!(tcc_service(Path::new("/Users/you/Desktop/x"), None), None);
        assert_eq!(
            tcc_service(Path::new("/Volumes/Work"), None),
            Some("SystemPolicyRemovableVolume")
        );
    }

    /// A project that is a link **into** a protected folder — the case that
    /// started this: `~/code/smetana` pointing at `~/Desktop/Projects/smetana`.
    /// The literal spelling names nothing, the resolved one names the Desktop,
    /// and `service_for` is what asks both.
    #[cfg(unix)]
    #[test]
    fn a_link_into_the_desktop_is_answered_by_the_desktop() {
        let root = scratch("link-into-desktop");
        let real = root.join("Desktop").join("Projects").join("smetana");
        fs::create_dir_all(&real).unwrap();
        let link = root.join("code");
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&real, &link).expect("make the link");

        // `root` stands in for a home directory, so `<root>/Desktop` is the
        // protected folder and `<root>/code` is the link nobody would guess is
        // under it.
        assert_eq!(
            tcc_service(&link, Some(&root)),
            None,
            "the literal spelling alone finds nothing, which is why one is not enough"
        );
        assert_eq!(
            service_for(&link, Some(&root)),
            Some("SystemPolicyDesktopFolder"),
            "asked both ways, it is exactly the folder this task was reported about"
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// The other direction, and the one a canonicalize-only answer gets wrong:
    /// iCloud "Desktop & Documents Folders" makes `~/Desktop` itself a link into
    /// `~/Library/Mobile Documents/…`, so the literal spelling is the only one
    /// that names the Desktop. A single spelling cannot serve both tests; the
    /// union serves both, and every service it can name is promptable.
    #[cfg(unix)]
    #[test]
    fn a_desktop_that_is_itself_a_link_is_still_the_desktop() {
        let root = scratch("icloud-desktop");
        let cloud = root.join("Library").join("Mobile Documents").join("Desktop");
        let project = cloud.join("Projects").join("smetana");
        fs::create_dir_all(&project).unwrap();
        let desktop = root.join("Desktop");
        let _ = fs::remove_file(&desktop);
        std::os::unix::fs::symlink(&cloud, &desktop).expect("make the link");

        let through_desktop = desktop.join("Projects").join("smetana");
        assert_eq!(
            tcc_service(&resolved(&through_desktop), Some(&resolved(&root))),
            None,
            "resolved alone lands in Mobile Documents, which no promptable service governs"
        );
        assert_eq!(
            service_for(&through_desktop, Some(&root)),
            Some("SystemPolicyDesktopFolder"),
            "asked literally first, it is the Desktop — which is what macOS asks about"
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// **The invariant.** The read that draws the notice and the write that runs
    /// `tccutil` must answer the same about one folder. They did not once, and
    /// both directions of the disagreement were the defect this module exists to
    /// remove: a button that refuses itself under a sentence promising a prompt,
    /// and a sentence sending somebody to grant Full Disk Access for a folder a
    /// Desktop dialog would have covered.
    ///
    /// `reset` cannot be called here — it would spawn `tccutil` against the real
    /// machine — so what is asserted is the question it asks, which is the same
    /// call it makes.
    #[cfg(unix)]
    #[test]
    fn repair_for_agrees_with_what_reset_would_do() {
        let root = scratch("read-and-write-agree");
        let real = root.join("Desktop").join("Projects").join("smetana");
        let plain = root.join("elsewhere");
        fs::create_dir_all(&real).unwrap();
        fs::create_dir_all(&plain).unwrap();
        let link = root.join("code");
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&real, &link).expect("make the link");

        for dir in [link.clone(), real.clone(), plain.clone()] {
            // What `reset` looks up before it runs anything.
            let would_reset = service_for(&dir, Some(&root)).is_some();
            // What the notice is drawn from.
            let drawn = repair_for(&dir, Some(&root));
            let offers_button = drawn == AccessRepair::Reset;
            if reset_supported() {
                assert_eq!(
                    offers_button,
                    would_reset,
                    "{} draws {drawn:?} while the reset would {}",
                    dir.display(),
                    if would_reset { "have run" } else { "have refused" }
                );
            } else {
                assert_eq!(drawn, AccessRepair::Unavailable, "{}", dir.display());
            }
        }

        let _ = fs::remove_dir_all(&root);
    }

    /// Two spellings of one folder are one folder, and two folders are two.
    ///
    /// The whole reason `same_dir` is not `==`: a run is started against the
    /// folder the project list holds, the tracker worker is handed the folder
    /// the app window opened, and a symlink anywhere between them makes those
    /// two spellings of the same project. Reading them as different projects
    /// is what sends a run's board read — and its parking writes — to a
    /// stranger's board.
    #[test]
    fn two_spellings_of_one_folder_are_the_same_folder() {
        let root = scratch("same-dir");
        let real = root.join("project");
        fs::create_dir_all(&real).expect("create the project folder");
        let other = root.join("other");
        fs::create_dir_all(&other).expect("create the second folder");

        assert!(same_dir(&real, &real), "a folder is itself");
        assert!(!same_dir(&real, &other), "two folders are two folders");

        #[cfg(unix)]
        {
            let link = root.join("link");
            let _ = fs::remove_file(&link);
            std::os::unix::fs::symlink(&real, &link).expect("symlink the project folder");
            assert!(same_dir(&link, &real), "a symlink and its target are one folder");
            assert!(same_dir(&real, &link), "and the same asked the other way round");
            assert!(!same_dir(&link, &other), "the link is still not the other folder");
        }

        let _ = fs::remove_dir_all(&root);
    }

    /// A path that cannot be opened is an ordinary answer, not a panic.
    ///
    /// `resolved` opens the path, and a run's root may be gone from under it —
    /// a folder somebody moved or deleted while the run was going. The literal
    /// spelling is then all there is, and it still has to answer: itself for
    /// itself, and not for somebody else.
    #[test]
    fn a_folder_that_cannot_be_opened_falls_back_to_its_spelling() {
        let missing = Path::new("/no/such/folder/anywhere");
        assert!(same_dir(missing, missing), "a path names itself whether it exists or not");
        assert!(
            !same_dir(missing, Path::new("/no/such/folder/either")),
            "two paths that both fail to resolve are still two paths"
        );
    }
}
