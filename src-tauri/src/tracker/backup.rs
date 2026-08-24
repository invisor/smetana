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

/// Copy a directory tree, creating what is missing on the way.
///
/// **A symbolic link to a file is followed; a symbolic link to a directory is
/// not.** The split is deliberate and the second half of it is what makes this
/// walk terminate. Following a file link is the plain reading of "copy the
/// tracker" — what is wanted is the bytes bd would read, and a link recreated
/// into a folder that is about to be migrated would point back at the original.
/// A *directory* link has no such argument and one pointing at an ancestor
/// would recurse until the disk filled and the stack blew, which in Rust is an
/// abort rather than a panic: the whole app would vanish, with no message, on a
/// button press, over somebody's data. Nothing bd writes into `.beads` is a
/// directory link, so skipping them costs nothing real and removes the only
/// cycle this walk can have. The real tree below is finite, so no depth bound
/// is needed on top.
///
/// Anything that is neither a file nor a directory is skipped — a socket left
/// in a database directory is not data, and refusing the whole copy over one
/// would refuse the repair.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let path = entry.path();
        // `symlink_metadata` answers about the link itself, which is the only
        // way to tell one from what it points at; `metadata` follows it.
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            // A link to a file is copied for its bytes. A link to a directory,
            // and a broken one, are both passed over — see above.
            if fs::metadata(&path).is_ok_and(|m| m.is_file()) {
                fs::copy(&path, &target)?;
            }
            continue;
        }
        let meta = entry.metadata()?;
        if meta.is_dir() {
            copy_tree(&path, &target)?;
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
    copy_tree(&source, &partial).map_err(|e| {
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

    /// A directory symlink pointing at an ancestor is the one shape that made
    /// this walk unbounded, and an unbounded one does not fail — it fills the
    /// disk and then overflows the stack, which in Rust aborts the process. So
    /// the test that matters is that the call *returns at all*.
    #[cfg(unix)]
    #[test]
    fn a_directory_link_pointing_upwards_does_not_send_the_walk_round_forever() {
        let dir = temp_dir("cycle");
        write(&dir.join(".beads/config.json"), "{}");
        std::os::unix::fs::symlink(&dir, dir.join(".beads/up")).expect("the link is made");

        let copy = copy_beads(&dir, Utc::now()).expect("the copy is taken");

        assert!(copy.join("config.json").is_file(), "the real files are still copied");
        assert!(!copy.join("up").exists(), "the directory link is passed over");
    }

    /// The other half of that split: a link to a *file* is followed for its
    /// bytes, because what is wanted is what bd would read.
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
        // And it is a real file in the copy, not a link back into the original,
        // which is about to be migrated.
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
