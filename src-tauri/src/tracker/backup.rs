//! The copy taken before the tracker is migrated.
//!
//! A migration is the only irreversible thing this app does to somebody else's
//! `.beads`, and the Repair button deliberately has no confirmation dialog in
//! front of it — the copy is what buys that. So the copy is a precondition
//! rather than a courtesy: `service.rs` refuses to migrate when this fails, and
//! nothing anywhere removes what it leaves behind.
//!
//! Everything here is `std::fs`. No `cp` is spawned, which is a rule rather
//! than a preference in this repository (`AGENTS.md`): `cp` may be aliased to
//! `-i` on somebody's machine, and a copy waiting for an answer nobody can give
//! would hang the worker for the rest of the process's life.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

use super::model::TrackerError;

/// The tracker's own directory, the one being copied.
pub const BEADS: &str = ".beads";

/// What the copy is called: `.beads.backup-<UTC>`, beside the original.
///
/// A pure function of the moment, so the name is a test rather than a thing
/// somebody reads off a screenshot. UTC and not the local zone, and no colons
/// in it: a name is sorted by whoever lists the folder, two machines in two
/// zones must not disagree about which copy is newer, and `12:30:00` is not a
/// filename on Windows at all.
///
/// Beside `.beads` rather than inside it: bd opens the tracker directory, and a
/// second database nested in the first is a thing it might read.
pub fn backup_name(now: DateTime<Utc>) -> String {
    format!("{BEADS}.backup-{}", now.format("%Y%m%dT%H%M%SZ"))
}

/// The suffix a copy wears while it is being taken. See `copy_beads` for why
/// there is one at all.
const PARTIAL: &str = ".partial";

/// What one walk has already entered, and the one place it must never go.
///
/// Both exist for the same reason and neither can be dropped: symbolic links
/// are **followed**, so the source is a graph rather than a tree and a walk
/// over it has to remember where it has been.
struct Walk {
    /// The copy's own root, canonicalised. Never descended into — a link
    /// pointing at an ancestor of the destination would otherwise have the walk
    /// discovering the copy it is in the middle of making and copying that
    /// too, which grows a new real directory on every pass and never ends. The
    /// `seen` set alone does not stop it, because each of those directories is
    /// genuinely one this walk has not met before.
    stop: PathBuf,
    /// Every source directory already copied, canonicalised. A second route to
    /// one is skipped rather than copied again: the subtree is already in the
    /// copy under whichever name got there first, so this de-duplicates and
    /// never omits.
    seen: HashSet<PathBuf>,
}

/// Copy a directory tree, creating what is missing on the way.
///
/// **Symbolic links are followed, both to files and to directories, and the
/// walk is bounded by `Walk` instead.** Following them is what "copy the
/// tracker" has to mean. The case that decides it is a directory link somebody
/// put inside `.beads` themselves — a store, or part of one, kept on another
/// volume — because then everything the database *is* sits on the far side of
/// that link, and a copy that stepped over it would hold none of it.
///
/// Note which link that is: it is **the person's, not bd's**. Nothing bd writes
/// into `.beads` is a directory link, and reasoning from that was the mistake
/// this comment replaces — it is true, and it is not the case that bites.
///
/// The two alternatives were both rejected, and one of them was in this file
/// for a commit. **Skipping directory links** terminates and copies the wrong
/// thing: that relocated store is then absent from the copy, `copy_beads` still
/// answers `Ok`, and the app tells somebody their backup was taken by name and
/// then migrates a database the backup does not contain. That is the same
/// "looks complete and is not" property the `.partial` rename exists to
/// prevent, with an affirmative claim on top. **Refusing outright** on a
/// directory link would at least be honest, but it refuses the repair to
/// exactly the people whose setup is legitimate, and a repair is what they came
/// for. Following with a visited set loses neither.
///
/// A **broken** link is passed over. There are no bytes behind it, bd cannot
/// read it either, and there is nothing a copy could carry. Anything that is
/// neither a file nor a directory is passed over for the same reason — a socket
/// left in a database directory is not data, and refusing the whole copy over
/// one would refuse the repair.
fn copy_tree(walk: &mut Walk, from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let path = entry.path();
        let target = to.join(entry.file_name());
        // `metadata` follows a link, which is the whole intent here; the error
        // is a broken one, and it is the only thing skipped silently.
        let Ok(meta) = fs::metadata(&path) else { continue };
        if meta.is_dir() {
            let real = fs::canonicalize(&path)?;
            if real == walk.stop || !walk.seen.insert(real) {
                continue;
            }
            copy_tree(walk, &path, &target)?;
        } else if meta.is_file() {
            fs::copy(&path, &target)?;
        }
    }
    Ok(())
}

/// Copy `<dir>/.beads` to `<dir>/.beads.backup-<UTC>` and answer with the
/// absolute path of the copy.
///
/// A folder with no `.beads` is refused here rather than copied as an empty
/// directory: an empty copy would look like a successful backup and would then
/// let a migration run against a tracker that does not exist.
///
/// **The walk writes into `<name>.partial` and the copy earns its real name by
/// a rename at the end.** Without that, an `io::Error` half-way through leaves
/// a directory named exactly like a finished backup, holding an arbitrary
/// subset of a Dolt database, with nothing to say so — and this copy is the
/// entire safety argument for a button that asks no confirmation. It is reached
/// for only after a migration has already gone wrong, when there is nothing
/// left to compare it against, so "looks complete and is not" is worse than
/// nothing at all. A rename inside one directory is atomic on every platform
/// this ships to, so the final name never exists over a partial copy.
///
/// Debris is not removed on failure, deliberately: `.partial` says what it is,
/// and a repair that deleted things after failing would be the one behaviour
/// this whole file exists to avoid.
pub fn copy_beads(dir: &Path, now: DateTime<Utc>) -> Result<PathBuf, TrackerError> {
    let source = dir.join(BEADS);
    if !source.is_dir() {
        return Err(TrackerError::Backup(format!("no {BEADS} directory in {}", dir.display())));
    }
    let name = backup_name(now);
    let target = dir.join(&name);
    let partial = dir.join(format!("{name}{PARTIAL}"));
    // The name is second-granularity, so either of these existing means a
    // second repair inside the same second — or the debris of one that failed.
    // Writing into it either way would mix two copies into one directory.
    for taken in [&target, &partial] {
        if taken.exists() {
            return Err(TrackerError::Backup(format!("{} is already there", taken.display())));
        }
    }
    // The destination has to exist before it can be canonicalised, and it has
    // to be canonicalised before the walk starts, because the walk's one job
    // besides copying is to never descend into it.
    let mut walk = {
        let start = || -> std::io::Result<Walk> {
            fs::create_dir_all(&partial)?;
            Ok(Walk {
                stop: fs::canonicalize(&partial)?,
                // The tracker's own directory counts as entered from the
                // outset, so a link inside it pointing back at it is a repeat
                // like any other.
                seen: HashSet::from([fs::canonicalize(&source)?]),
            })
        };
        start().map_err(|e| {
            TrackerError::Backup(format!("could not make {}: {e}", partial.display()))
        })?
    };
    copy_tree(&mut walk, &source, &partial).map_err(|e| {
        TrackerError::Backup(format!(
            "could not copy {} to {}: {e}",
            source.display(),
            partial.display()
        ))
    })?;
    fs::rename(&partial, &target).map_err(|e| {
        TrackerError::Backup(format!(
            "the copy at {} could not be given its name: {e}",
            partial.display()
        ))
    })?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("smetana-backup-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("the temporary directory is made");
        dir
    }

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("the parent is made");
        }
        fs::write(path, body).expect("the file is written");
    }

    /// The name is what somebody reads in their own file manager months later,
    /// so it is pinned to the character: the tracker's own name, the word
    /// backup, and one sortable UTC stamp with nothing in it a filesystem
    /// refuses.
    #[test]
    fn the_copy_is_named_after_the_moment_it_was_taken() {
        let now = DateTime::parse_from_rfc3339("2026-08-25T12:30:00Z")
            .expect("parses")
            .with_timezone(&Utc);
        assert_eq!(backup_name(now), ".beads.backup-20260825T123000Z");
    }

    #[test]
    fn the_copy_carries_the_nested_files_too() {
        let dir = temp_dir("nested");
        write(&dir.join(".beads/config.json"), "{}");
        write(&dir.join(".beads/embeddeddolt/.dolt/noms/chunk"), "bytes");

        let now = Utc::now();
        let copy = copy_beads(&dir, now).expect("the copy is taken");

        assert_eq!(copy, dir.join(backup_name(now)));
        assert_eq!(fs::read_to_string(copy.join("config.json")).unwrap(), "{}");
        assert_eq!(
            fs::read_to_string(copy.join("embeddeddolt/.dolt/noms/chunk")).unwrap(),
            "bytes"
        );
        // The original is a copy and not a move: bd has to keep reading it.
        assert!(dir.join(".beads/config.json").is_file());
    }

    /// The copy earns its name by a rename, so nothing under the final name is
    /// ever half-written. This checks the finished state rather than the middle
    /// of the walk, which is what a caller can actually observe: the real name
    /// is there, no `.partial` is left beside it, and the rename did not copy
    /// a second time.
    #[test]
    fn the_copy_takes_its_real_name_only_when_the_walk_has_finished() {
        let dir = temp_dir("partial");
        write(&dir.join(".beads/config.json"), "{}");

        let now = Utc::now();
        let copy = copy_beads(&dir, now).expect("the copy is taken");

        assert!(copy.join("config.json").is_file());
        assert!(!dir.join(format!("{}.partial", backup_name(now))).exists(), "no debris beside it");
        let left: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(left.len(), 2, "the tracker and one copy, and nothing else: {left:?}");
    }

    /// The case that decides how directory links are treated: somebody moved
    /// their Dolt store off the boot disk. Everything the database *is* sits on
    /// the far side of that link, so a copy that passed over it would answer
    /// `Ok` while holding none of the data — and the app would then say the
    /// backup was taken and migrate.
    #[cfg(unix)]
    #[test]
    fn a_relocated_store_is_copied_for_what_it_holds() {
        let dir = temp_dir("relocated");
        let elsewhere = temp_dir("relocated-volume");
        write(&elsewhere.join("noms/chunk"), "the database");
        fs::create_dir_all(dir.join(".beads")).expect("the tracker directory is made");
        write(&dir.join(".beads/config.json"), "{}");
        std::os::unix::fs::symlink(&elsewhere, dir.join(".beads/embeddeddolt"))
            .expect("the store is relocated");

        let copy = copy_beads(&dir, Utc::now()).expect("the copy is taken");

        assert_eq!(fs::read_to_string(copy.join("embeddeddolt/noms/chunk")).unwrap(), "the database");
        assert_eq!(fs::read_to_string(copy.join("config.json")).unwrap(), "{}");
        let _ = fs::remove_dir_all(&elsewhere);
    }

    /// A directory link pointing at an ancestor is the shape that makes this
    /// walk a graph rather than a tree, and an unbounded walk does not fail —
    /// it fills the disk and then overflows the stack, which in Rust aborts the
    /// process. So the test that matters is that the call *returns at all*.
    ///
    /// It is also the shape the `seen` set alone does not stop: following the
    /// link reaches the directory the copy is being written into, and copying
    /// that grows a new real directory on every pass, each one genuinely
    /// unvisited. `Walk::stop` is what ends it.
    #[cfg(unix)]
    #[test]
    fn a_directory_link_pointing_upwards_does_not_send_the_walk_round_forever() {
        let dir = temp_dir("cycle");
        write(&dir.join(".beads/config.json"), "{}");
        std::os::unix::fs::symlink(&dir, dir.join(".beads/up")).expect("the link is made");

        let copy = copy_beads(&dir, Utc::now()).expect("the copy is taken");

        assert!(copy.join("config.json").is_file(), "the real files are still copied");
        // The link is followed, so the folder it names is there — holding what
        // the walk was allowed to take from it, which is neither the tracker it
        // has already copied nor the copy it is making.
        assert!(copy.join("up").is_dir(), "the link was followed rather than passed over");
        assert!(!copy.join("up/.beads").exists(), "the tracker is not taken a second time");
        let inner: Vec<String> = fs::read_dir(copy.join("up"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            inner.iter().all(|name| !name.starts_with(".beads.backup-")),
            "the copy must not contain itself: {inner:?}"
        );
    }

    /// A link that points nowhere has no bytes behind it, so there is nothing a
    /// copy could carry and nothing bd could read either.
    #[cfg(unix)]
    #[test]
    fn a_broken_link_is_passed_over_rather_than_failing_the_copy() {
        let dir = temp_dir("broken");
        write(&dir.join(".beads/config.json"), "{}");
        std::os::unix::fs::symlink(dir.join("nowhere"), dir.join(".beads/dangling"))
            .expect("the link is made");

        let copy = copy_beads(&dir, Utc::now()).expect("the copy is taken");

        assert!(copy.join("config.json").is_file());
        assert!(!copy.join("dangling").exists());
    }

    /// The other half of "links are followed": a link to a file is copied for
    /// its bytes and lands in the copy as a real file, not as a link back into
    /// the original — which is the directory about to be migrated.
    #[cfg(unix)]
    #[test]
    fn a_file_link_is_copied_for_what_it_points_at() {
        let dir = temp_dir("filelink");
        write(&dir.join("outside.txt"), "bytes");
        fs::create_dir_all(dir.join(".beads")).expect("the tracker directory is made");
        std::os::unix::fs::symlink(dir.join("outside.txt"), dir.join(".beads/linked.txt"))
            .expect("the link is made");

        let copy = copy_beads(&dir, Utc::now()).expect("the copy is taken");

        assert_eq!(fs::read_to_string(copy.join("linked.txt")).unwrap(), "bytes");
        assert!(!fs::symlink_metadata(copy.join("linked.txt")).unwrap().file_type().is_symlink());
    }

    /// The refusal that keeps a migration from running against nothing. An
    /// empty directory here would be a backup that succeeded, and the caller
    /// would go on to migrate.
    #[test]
    fn a_folder_without_a_tracker_is_refused_rather_than_copied_empty() {
        let dir = temp_dir("bare");
        let err = copy_beads(&dir, Utc::now()).expect_err("there is nothing to copy");
        assert!(err.to_string().contains("nothing was migrated"), "{err}");
        // And nothing at all was left behind under that name.
        let left: Vec<_> = fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert!(left.is_empty(), "{left:?}");
    }
}
