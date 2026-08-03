//! The disk: reading the project's directories and files.
//!
//! There is deliberately no worker here. The tracker has one because a bd call
//! costs about two seconds and someone has to own the snapshot; `read_dir`
//! costs milliseconds and holds no state — a queue would be guarding something
//! nobody contends for. The same reason settings have none.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use super::model::{
    looks_binary, reject_traversal, sort_entries, Entry, EntryKind, FileText, FilesError, Listing,
    Stat, BINARY_SNIFF_BYTES, MAX_ENTRIES, MAX_FILE_BYTES,
};

/// An I/O error in terms the front end understands.
fn io_error(path: &str, err: &std::io::Error) -> FilesError {
    match err.kind() {
        std::io::ErrorKind::NotFound => FilesError::NotFound(path.to_owned()),
        std::io::ErrorKind::PermissionDenied => FilesError::Denied(path.to_owned()),
        _ => FilesError::Io(format!("{path}: {err}")),
    }
}

/// Milliseconds since the epoch. A file dated before 1970 is not our case, but
/// there is no reason to panic on it either: a zero is more honest than a panic.
pub fn mtime_of(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// An absolute path inside the root — or a refusal.
///
/// Two lines of defence. The first, `reject_traversal`, is free and catches `..`
/// and absolute paths. The second is `canonicalize`: it unwinds symlinks, and
/// without it a link inside the project pointing outward would open anything on
/// the disk. `root` is canonicalized too: otherwise on macOS `/var/...` against
/// `/private/var/...` would never match.
///
/// To be honest about the purpose: this is a trap for our own mistakes and odd
/// names, not a barrier against an attacker — `root` comes from the front end,
/// and the front end is ours.
pub fn resolve_within(root: &Path, rel: &str) -> Result<PathBuf, FilesError> {
    reject_traversal(rel)?;
    let root = root.canonicalize().map_err(|err| io_error(&root.to_string_lossy(), &err))?;
    let joined = if rel.is_empty() { root.clone() } else { root.join(rel) };
    let full = joined.canonicalize().map_err(|err| io_error(rel, &err))?;
    if !full.starts_with(&root) {
        return Err(FilesError::Outside(rel.to_owned()));
    }
    Ok(full)
}

/// An entry's path relative to the root, always with `/`. On Windows `read_dir`
/// returns a backslash, and the very same string serves as the key in settings
/// and in the tree map — it must not diverge.
fn child_path(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_owned()
    } else {
        format!("{dir}/{name}")
    }
}

pub fn list_dir(root: &Path, rel: &str) -> Result<Listing, FilesError> {
    let full = resolve_within(root, rel)?;
    if !full.is_dir() {
        return Err(FilesError::NotAFile(rel.to_owned()));
    }
    let reader = fs::read_dir(&full).map_err(|err| io_error(rel, &err))?;

    let mut entries = Vec::new();
    for item in reader {
        // An entry that vanished between `read_dir` and `next` is no reason to
        // drop the whole directory: skip it and keep reading.
        let Ok(item) = item else { continue };
        let name = item.file_name().to_string_lossy().into_owned();
        if super::model::skip_in_tree(&name) {
            continue;
        }
        // `file_type` does not follow symlinks — a directory here is what is a
        // directory in itself; `resolve_within` unwinds the link when it is
        // clicked, and refuses there if it leads outside.
        let Ok(kind) = item.file_type() else { continue };
        entries.push(Entry {
            path: child_path(rel, &name),
            name,
            kind: if kind.is_dir() { EntryKind::Dir } else { EntryKind::File },
        });
    }

    sort_entries(&mut entries);
    let truncated = entries.len().saturating_sub(MAX_ENTRIES);
    entries.truncate(MAX_ENTRIES);

    Ok(Listing { dir: rel.to_owned(), entries, truncated })
}

pub fn read_text(root: &Path, rel: &str) -> Result<FileText, FilesError> {
    read_text_reading_with(root, rel, |full| fs::read(full))
}

/// The body of `read_text` with the byte read swappable.
///
/// The substitution exists for exactly one test, and there is no other way to
/// write it: the "mtime first, bytes second" order is visible only to someone
/// who manages to rewrite the file precisely between those two steps, and
/// nothing but a race can get a test into that gap. The closure *is* that gap.
fn read_text_reading_with(
    root: &Path,
    rel: &str,
    read_bytes: impl FnOnce(&Path) -> std::io::Result<Vec<u8>>,
) -> Result<FileText, FilesError> {
    let full = resolve_within(root, rel)?;
    let meta = fs::metadata(&full).map_err(|err| io_error(rel, &err))?;
    if !meta.is_file() {
        return Err(FilesError::NotAFile(rel.to_owned()));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(FilesError::TooLarge { path: rel.to_owned(), bytes: meta.len() });
    }
    // The mtime is taken BEFORE the bytes are read, and never taken a second time.
    //
    // Content and mtime cannot be read atomically, and the file may be rewritten
    // between the two calls — so the choice is not between "right" and "wrong"
    // but between two ways of being wrong:
    //
    //   mtime before the read — new content leaves for the front end with an old
    //                           mtime, and the next write asks the person with a
    //                           `Stale` refusal;
    //   mtime after           — old content leaves with a new mtime, the next
    //                           write passes the check and silently erases
    //                           somebody else's edit.
    //
    // We are obliged to err towards a false refusal: it costs one question,
    // while a silent overwrite costs somebody's work. The `expected_mtime` check
    // in `write_text` exists for exactly this, and the "mtime after" order would
    // rob it of its meaning.
    let mtime = mtime_of(&meta);

    let bytes = read_bytes(&full).map_err(|err| io_error(rel, &err))?;
    if looks_binary(&bytes[..bytes.len().min(BINARY_SNIFF_BYTES)]) {
        return Err(FilesError::Binary(rel.to_owned()));
    }
    let text = String::from_utf8(bytes).map_err(|_| FilesError::NotUtf8(rel.to_owned()))?;

    Ok(FileText { path: rel.to_owned(), text, mtime })
}

/// Timestamps in a batch. There are no refusals here: "the file is gone" is a
/// tab's state, not a command failure, and the whole sweep must not be dropped
/// because of it.
pub fn stat_many(root: &Path, rels: &[String]) -> Vec<Stat> {
    rels.iter()
        .map(|rel| {
            let mtime = resolve_within(root, rel)
                .ok()
                .and_then(|full| fs::metadata(full).ok())
                .map(|meta| mtime_of(&meta));
            Stat { path: rel.clone(), mtime }
        })
        .collect()
}

/// A counter for temp files. Together with the pid it gives a name no other
/// entry has — neither in this process nor in a neighbouring one. The same
/// trick as in `settings/file.rs`.
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_path(path: &Path) -> PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()))
}

/// Writing a project file.
///
/// The `expected_mtime` check is the one thing all this fuss is for: without it
/// Cmd+S on a tab opened an hour ago would silently erase an agent's work. A
/// mismatch means a refusal and zero changes on disk.
///
/// The rest is as in `settings/file.rs`: a temp file next to it, `sync_all`,
/// `rename`. Plus one thing that is not needed there: carrying the original's
/// permissions over. `rename` replaces the file wholesale, and without this an
/// executable script would stop running after being saved.
pub fn write_text(
    root: &Path,
    rel: &str,
    text: &str,
    expected_mtime: i64,
) -> Result<i64, FilesError> {
    let full = resolve_within(root, rel)?;
    let meta = fs::metadata(&full).map_err(|err| io_error(rel, &err))?;
    if !meta.is_file() {
        return Err(FilesError::NotAFile(rel.to_owned()));
    }
    if mtime_of(&meta) != expected_mtime {
        return Err(FilesError::Stale(rel.to_owned()));
    }

    let temp = temp_path(&full);
    let written = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(text.as_bytes())?;
        // Without this a power loss could make the rename durable but not what
        // is in the file.
        file.sync_all()
    })();
    if let Err(err) = written {
        let _ = fs::remove_file(&temp);
        return Err(FilesError::Io(format!("{}: {err}", temp.display())));
    }
    if let Err(err) = fs::set_permissions(&temp, meta.permissions()) {
        let _ = fs::remove_file(&temp);
        return Err(FilesError::Io(format!("{}: {err}", temp.display())));
    }
    if let Err(err) = fs::rename(&temp, &full) {
        let _ = fs::remove_file(&temp);
        return Err(io_error(rel, &err));
    }

    let meta = fs::metadata(&full).map_err(|err| io_error(rel, &err))?;
    Ok(mtime_of(&meta))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// An explicit timestamp instead of a pause. On some filesystems the `mtime`
    /// resolution is coarser than the gap between two consecutive writes, and
    /// the "the mtime changed" test is falsely green without this. A `sleep`
    /// would do the same, but it would slow the whole run down and still depend
    /// on the resolution; a timestamp we set depends on neither.
    fn set_mtime(path: &Path, secs: u64) {
        let file = fs::File::options().write(true).open(path).expect("open the file to set its mtime");
        file.set_modified(UNIX_EPOCH + std::time::Duration::from_secs(secs))
            .expect("set the timestamp");
    }

    /// A directory of its own per test: the name carries the pid, so parallel
    /// runs do not get in each other's way. The same trick as in `project.rs`.
    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-files-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create the temp directory");
        // The canonical path: on macOS /var is a symlink to /private/var, and
        // without this the root and the resolved path would never match.
        dir.canonicalize().expect("canonicalize the temp directory")
    }

    #[test]
    fn a_directory_reads_sorted_and_without_git() {
        let root = scratch("listing");
        fs::create_dir_all(root.join(".git")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("README.md"), "x").unwrap();
        fs::write(root.join("app.js"), "x").unwrap();

        let listing = list_dir(&root, "").unwrap();

        let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["src", "app.js", "README.md"]);
        assert_eq!(listing.truncated, 0);
        assert_eq!(listing.entries[0].kind, EntryKind::Dir);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_nested_directory_gives_paths_from_the_root_with_slashes() {
        let root = scratch("nested");
        fs::create_dir_all(root.join("src/components")).unwrap();
        fs::write(root.join("src/App.vue"), "x").unwrap();

        let listing = list_dir(&root, "src").unwrap();

        assert_eq!(listing.dir, "src");
        assert_eq!(listing.entries[0].path, "src/components");
        assert_eq!(listing.entries[1].path, "src/App.vue");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn an_overlong_directory_is_truncated_and_says_by_how_much() {
        let root = scratch("truncate");
        for i in 0..MAX_ENTRIES + 7 {
            fs::write(root.join(format!("f{i:05}.txt")), "x").unwrap();
        }

        let listing = list_dir(&root, "").unwrap();

        assert_eq!(listing.entries.len(), MAX_ENTRIES);
        assert_eq!(listing.truncated, 7, "silent truncation would read as \"there are no more files\"");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn text_is_read_together_with_its_timestamp() {
        let root = scratch("read");
        fs::write(root.join("a.txt"), "hello\n").unwrap();

        let file = read_text(&root, "a.txt").unwrap();

        assert_eq!(file.path, "a.txt");
        assert_eq!(file.text, "hello\n");
        assert!(file.mtime > 0);
        let _ = fs::remove_dir_all(&root);
    }

    /// Pins the order from `read_text`: the mtime is taken from the file before
    /// the bytes are read, and that is the one that leaves. Move the mtime read
    /// back after the byte read and the check in `write_text` stops protecting
    /// anything: content of one version leaves with the mtime of another, and
    /// the next write passes silently.
    #[test]
    fn someone_elses_edit_after_a_read_refuses_the_write() {
        let root = scratch("read-then-clobber");
        let path = root.join("a.txt");
        fs::write(&path, "the agent's work\n").unwrap();
        set_mtime(&path, 1_700_000_000);

        let file = read_text(&root, "a.txt").unwrap();

        assert_eq!(file.text, "the agent's work\n");
        assert_eq!(file.mtime, 1_700_000_000_000, "the mtime of the file that was read is the one that leaves");

        // This is what an agent rewriting the file while the tab was open looks like.
        fs::write(&path, "the agent's new work\n").unwrap();
        set_mtime(&path, 1_700_000_060);

        let err = write_text(&root, "a.txt", "my edits\n", file.mtime);

        assert!(
            matches!(err, Err(FilesError::Stale(_))),
            "a write with the mtime from read_text has to refuse rather than clobber: {err:?}"
        );
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "the agent's new work\n",
            "on a refusal nothing on disk must change"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// The same case, caught at its actual moment: the file is rewritten
    /// precisely between taking the mtime and reading the bytes. An mtime taken
    /// BEFORE the read leaves stale — and the next write refuses with `Stale`,
    /// that is, asks the person. An mtime taken AFTER would leave fresh, the
    /// check in `write_text` would pass, and the agent's work would vanish
    /// silently. This test fails on exactly that reordering.
    #[test]
    fn the_mtime_is_taken_before_the_read_and_so_never_outruns_the_content() {
        let root = scratch("mtime-before-read");
        let path = root.join("a.txt");
        fs::write(&path, "mine\n").unwrap();
        set_mtime(&path, 1_700_000_000);

        let file = read_text_reading_with(&root, "a.txt", |full| {
            let bytes = fs::read(full)?;
            // The agent rewrote the file while we were reading its bytes.
            fs::write(full, "the agent's work\n")?;
            set_mtime(full, 1_700_000_060);
            Ok(bytes)
        })
        .unwrap();

        assert_eq!(file.text, "mine\n");
        assert_eq!(
            file.mtime, 1_700_000_000_000,
            "the mtime of the version that was read has to leave, not the one that landed afterwards"
        );

        let err = write_text(&root, "a.txt", "my edits\n", file.mtime);

        assert!(
            matches!(err, Err(FilesError::Stale(_))),
            "err towards a false refusal, not a silent overwrite: {err:?}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "the agent's work\n");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_binary_and_an_oversized_file_are_not_read() {
        let root = scratch("refuse");
        fs::write(root.join("a.bin"), [0x4d, 0x5a, 0x00, 0x90]).unwrap();
        fs::write(root.join("big.txt"), vec![b'x'; (MAX_FILE_BYTES + 1) as usize]).unwrap();
        fs::write(root.join("bad.txt"), [0xff, 0xfe, 0x41]).unwrap();

        assert!(matches!(read_text(&root, "a.bin"), Err(FilesError::Binary(_))));
        assert!(matches!(read_text(&root, "big.txt"), Err(FilesError::TooLarge { .. })));
        assert!(matches!(read_text(&root, "bad.txt"), Err(FilesError::NotUtf8(_))));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_file_and_a_directory_in_its_place_are_told_apart() {
        let root = scratch("missing");
        fs::create_dir_all(root.join("src")).unwrap();

        assert!(matches!(read_text(&root, "nope.txt"), Err(FilesError::NotFound(_))));
        assert!(matches!(read_text(&root, "src"), Err(FilesError::NotAFile(_))));
        let _ = fs::remove_dir_all(&root);
    }

    // Symlinks are created differently per platform, and on non-unix this test
    // simply does not exist — same as its neighbours below. There used to be a
    // `#[cfg(not(unix))] return;` in the middle of the body here, which left the
    // whole remainder of the test unreachable for the compiler.
    #[cfg(unix)]
    #[test]
    fn a_symlink_leading_outside_does_not_pass_though_the_path_looks_innocent() {
        let root = scratch("escape");
        let outside = scratch("escape-target");
        fs::write(outside.join("secret.txt"), "not for reading").unwrap();
        // `reject_traversal` is powerless here: the path holds neither ".." nor a root.
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

        assert!(matches!(read_text(&root, "link/secret.txt"), Err(FilesError::Outside(_))));
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn timestamps_come_in_a_batch_and_a_vanished_file_is_visible() {
        let root = scratch("stat");
        fs::write(root.join("a.txt"), "x").unwrap();

        let stats = stat_many(&root, &["a.txt".to_string(), "gone.txt".to_string()]);

        assert_eq!(stats.len(), 2);
        assert!(stats[0].mtime.is_some());
        assert_eq!(stats[1].mtime, None, "a vanished file is a state, not an error");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_write_returns_the_new_mtime_and_changes_the_file() {
        let root = scratch("write");
        fs::write(root.join("a.txt"), "before\n").unwrap();
        let before = read_text(&root, "a.txt").unwrap();

        let after = write_text(&root, "a.txt", "after\n", before.mtime).unwrap();

        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "after\n");
        assert!(after >= before.mtime);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn someone_elses_write_is_not_clobbered() {
        let root = scratch("stale");
        fs::write(root.join("a.txt"), "mine\n").unwrap();
        let mine = read_text(&root, "a.txt").unwrap();

        // This is what an agent rewriting the file while the tab was open looks like.
        let err = write_text(&root, "a.txt", "my edits\n", mine.mtime - 1);

        assert!(matches!(err, Err(FilesError::Stale(_))));
        assert_eq!(
            fs::read_to_string(root.join("a.txt")).unwrap(),
            "mine\n",
            "on a refusal nothing on disk must change"
        );

        // A check that the Stale refusal happened BEFORE the temp file was created.
        // If anyone moves the mtime check past File::create, this test fails.
        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "the Stale refusal must happen BEFORE the temp file is created: {leftovers:?}");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_write_outside_the_root_is_rejected() {
        let root = scratch("write-outside");
        assert!(matches!(
            write_text(&root, "../evil.txt", "x", 0),
            Err(FilesError::Outside(_))
        ));
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn an_executable_files_permissions_survive_a_write() {
        use std::os::unix::fs::PermissionsExt;
        let root = scratch("perms");
        let path = root.join("run.sh");
        fs::write(&path, "#!/bin/sh\necho before\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        let before = read_text(&root, "run.sh").unwrap();

        write_text(&root, "run.sh", "#!/bin/sh\necho after\n", before.mtime).unwrap();

        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755, "rename would have swapped the mode for the temp file's");
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn no_temp_file_is_left_behind() {
        let root = scratch("no-litter");
        fs::write(root.join("a.txt"), "x\n").unwrap();
        let before = read_text(&root, "a.txt").unwrap();

        write_text(&root, "a.txt", "y\n", before.mtime).unwrap();

        let leftovers: Vec<_> = fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files were left behind: {leftovers:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn missing_permissions_block_a_write_into_the_directory() {
        use std::os::unix::fs::PermissionsExt;

        let root = scratch("deny-write");
        let subdir = root.join("sub");
        fs::create_dir_all(&subdir).unwrap();
        fs::write(subdir.join("a.txt"), "x\n").unwrap();

        let before = read_text(&root, "sub/a.txt").unwrap();

        // Check that the permissions really do block the operation.
        // Running as root ignores them and the test would pass silently.
        let test_file = subdir.join(".test");
        if fs::write(&test_file, "test").is_ok() {
            let _ = fs::remove_file(&test_file);
            let _ = fs::remove_dir_all(&root);
            return; // Permissions do not apply under root, so there is nothing to test.
        }

        // Take the write permission off the subdirectory.
        fs::set_permissions(&subdir, fs::Permissions::from_mode(0o555)).unwrap();

        let err = write_text(&root, "sub/a.txt", "y\n", before.mtime);

        // Restore the permissions IMMEDIATELY, before the cleanup — otherwise
        // remove_dir_all cannot delete the directory.
        fs::set_permissions(&subdir, fs::Permissions::from_mode(0o755)).unwrap();

        // Check the refusal — write_text must return an error when it lacks
        // write permission.
        assert!(err.is_err(), "write_text must return an error when it lacks write permission");

        // The original file must not have changed.
        assert_eq!(
            fs::read_to_string(subdir.join("a.txt")).unwrap(),
            "x\n",
            "on a refusal the original file must be left untouched"
        );

        let _ = fs::remove_dir_all(&root);
    }
}
