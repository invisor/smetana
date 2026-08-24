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

/// Copy a directory tree, creating what is missing on the way.
///
/// Symbolic links are followed rather than recreated, which is the plain
/// reading of "copy the tracker": what is wanted is the bytes bd would read,
/// and a link recreated into a folder that is about to be migrated would point
/// at the original. Anything that is neither a file nor a directory is skipped
/// — a socket left in a database directory is not data, and refusing the whole
/// copy over one would refuse the repair.
fn copy_tree(from: &Path, to: &Path) -> std::io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        // `metadata` and not `file_type`: the latter answers about the link
        // itself, and this follows links deliberately.
        let meta = fs::metadata(entry.path())?;
        if meta.is_dir() {
            copy_tree(&entry.path(), &target)?;
        } else if meta.is_file() {
            fs::copy(entry.path(), &target)?;
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
pub fn copy_beads(dir: &Path, now: DateTime<Utc>) -> Result<PathBuf, TrackerError> {
    let source = dir.join(BEADS);
    if !source.is_dir() {
        return Err(TrackerError::Backup(format!("no {BEADS} directory in {}", dir.display())));
    }
    let target = dir.join(backup_name(now));
    if target.exists() {
        return Err(TrackerError::Backup(format!("{} is already there", target.display())));
    }
    copy_tree(&source, &target).map_err(|e| {
        TrackerError::Backup(format!("could not copy {} to {}: {e}", source.display(), target.display()))
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
