//! The disk: reading the project's directories and files.
//!
//! There is deliberately no worker here. The tracker has one because a bd call
//! costs about two seconds and someone has to own the snapshot; `read_dir`
//! costs milliseconds and holds no state — a queue would be guarding something
//! nobody contends for. The same reason settings have none.
//!
//! That still holds, and the cost it was written about no longer does: since the
//! tree started drawing git-ignored rows muted, `list_dir` spawns git once per
//! listing on top of the `read_dir` — see `mark_git_ignored` below, and the
//! module header in `mod.rs` for what the focus sweep multiplies it by.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::UNIX_EPOCH;

use super::model::{
    ignored_names, looks_binary, mark_ignored, reject_bad_name, reject_traversal, sort_entries,
    Entry, EntryKind, FileText, FilesError, Listing, Stat, BINARY_SNIFF_BYTES, MAX_ENTRIES,
    MAX_FILE_BYTES,
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

/// The same guarantee as `resolve_within`, for a path that does not exist yet.
///
/// It cannot simply call that one: `canonicalize` fails on anything that is not
/// on disk, so the trip would end in `NotFound` *before* the check the whole
/// call is there for ever ran, and every refusal would arrive under the wrong
/// name. So the parent — which does exist — is the thing canonicalized, and
/// what is joined to it is a name rather than a path: `reject_bad_name` is what
/// makes that join safe, since a `..` or a separator in the name would walk out
/// of the directory the check was just made about.
///
/// The last refusal is the one that keeps this honest about what it is for.
/// Something already sitting at the resulting path is an ordinary outcome —
/// somebody typed a name that is taken — and not a reason to overwrite it.
/// `symlink_metadata` rather than `exists`: a symlink pointing nowhere answers
/// "no" to the second and still takes the name.
pub fn resolve_new_within(root: &Path, dir: &str, name: &str) -> Result<PathBuf, FilesError> {
    reject_bad_name(name)?;
    let parent = resolve_within(root, dir)?;
    if !parent.is_dir() {
        return Err(FilesError::NotAFile(dir.to_owned()));
    }
    let full = parent.join(name);
    if full.symlink_metadata().is_ok() {
        return Err(FilesError::AlreadyExists(child_path(dir, name)));
    }
    Ok(full)
}

/// A new empty file. `create_new` rather than `create`: the existence check in
/// `resolve_new_within` and this call are two moments, and between them somebody
/// else's agent may write the same name — at which point truncating their file
/// to nothing is the one outcome this verb must never have.
pub fn create_file(root: &Path, dir: &str, name: &str) -> Result<String, FilesError> {
    let full = resolve_new_within(root, dir, name)?;
    let rel = child_path(dir, name);
    match fs::File::options().write(true).create_new(true).open(&full) {
        Ok(_) => Ok(rel),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(FilesError::AlreadyExists(rel))
        }
        Err(err) => Err(io_error(&rel, &err)),
    }
}

/// A new directory, one level deep — `create_dir` and not `create_dir_all`, for
/// the reason a name is not a path: nothing here can ask for two levels, so a
/// call that would silently make the intermediate ones is answering a question
/// nobody asked.
pub fn create_dir(root: &Path, dir: &str, name: &str) -> Result<String, FilesError> {
    let full = resolve_new_within(root, dir, name)?;
    let rel = child_path(dir, name);
    match fs::create_dir(&full) {
        Ok(()) => Ok(rel),
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
            Err(FilesError::AlreadyExists(rel))
        }
        Err(err) => Err(io_error(&rel, &err)),
    }
}

/// The system trash, and deliberately not `remove_file`. A deletion somebody
/// can undo from their own file manager is a smaller promise than a permanent
/// one, and this is a tree of somebody's sources. A directory goes with
/// everything under it — that is what a trash is.
///
/// It takes `resolve_new_within`'s shape rather than `resolve_within`'s, and
/// the reason is the whole of this function. `resolve_within` canonicalizes
/// every component, the last one included, which is right for reading a file
/// and wrong for destroying one:
///
///   - a link is drawn in the tree as an ordinary row (`list_dir` asks
///     `file_type`, which does not follow one), so deleting
///     `node_modules/.bin/vite` would take the package's real script — a file
///     nobody named and a loss nobody can connect to what they clicked;
///   - and a link pointing at the project's own root resolves to the root,
///     passing every string guard there is, at which point the app throws away
///     the folder it is looking at. So does the literal `.`, which is the same
///     directory by another spelling.
///
/// So the **parent** is what gets canonicalized and checked for containment,
/// the last component is checked as a name — `reject_bad_name` refuses `.`,
/// `..`, a separator, a drive prefix and the empty string, which is every
/// spelling of "not a child of that folder" — and what is joined back on is
/// handed over as it stands. A link goes as a link, and the root cannot be
/// named at all.
pub fn move_to_trash(root: &Path, rel: &str) -> Result<(), FilesError> {
    move_to_trash_with(root, rel, platform_trash)
}

/// The platform's own trash. On macOS this is **not** the crate's default, and
/// the difference is worth the `cfg`.
///
/// `trash::delete` there is `DeleteMethod::Finder`: an `osascript` subprocess
/// driving Finder over Apple Events. Three costs, and none of them is
/// theoretical. It cannot delete a symbolic link at all — Finder exits 0,
/// prints nothing and leaves the link where it was, which in this very
/// repository is every row under `node_modules/.bin`. It needs an Apple Events
/// grant, which a signed and hardened bundle may only ask for with an
/// `NSAppleEventsUsageDescription` in its Info.plist — `tauri.conf.json`
/// declares no macOS bundle block at all, so the first delete in a shipped
/// build is a prompt nobody wrote the words for, or a denial. And it is slow
/// enough to measure in seconds against milliseconds.
///
/// `NsFileManager` is `trashItemAtURL`: no subprocess, no permission, removes a
/// link as a link. What it costs is Finder's "Put Back" on some systems (a
/// macOS bug the crate documents) — the entry is still in the Trash and still
/// comes out of it by dragging, which is what "restored by the platform's
/// ordinary means" means here.
///
/// Everywhere else the default is the only method there is.
#[cfg(target_os = "macos")]
fn platform_trash(full: &Path) -> Result<(), String> {
    use trash::macos::{DeleteMethod, TrashContextExtMacos};
    let mut context = trash::TrashContext::default();
    context.set_delete_method(DeleteMethod::NsFileManager);
    context.delete(full).map_err(|err| err.to_string())
}

#[cfg(not(target_os = "macos"))]
fn platform_trash(full: &Path) -> Result<(), String> {
    trash::delete(full).map_err(|err| err.to_string())
}

/// The body of `move_to_trash` with the deletion itself swappable, the same
/// seam and for the same reason as `read_text_reading_with`: what has to be
/// tested here is *which path* is handed over, and the real answer to that
/// question would be an entry in whoever runs the tests' own Recycle Bin.
fn move_to_trash_with(
    root: &Path,
    rel: &str,
    delete: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), FilesError> {
    // `/` alone, and that is not an oversight about Windows. Every path the
    // tree produces is `child_path`'s, which uses `/` on every platform and
    // says why; a backslash in one of these strings is therefore part of a
    // **file name**, and `reject_traversal` lets such a name through, so
    // `files_list` lists it and `files_read` opens it. Splitting on it would
    // read `a\b.txt` as a folder and a file and delete something nobody named.
    // Left in the tail, it is what `reject_bad_name` refuses — a refusal
    // instead of the wrong file.
    let (dir, name) = match rel.rfind('/') {
        Some(at) => (&rel[..at], &rel[at + 1..]),
        None => ("", rel),
    };
    reject_bad_name(name)?;
    let parent = resolve_within(root, dir)?;
    let full = parent.join(name);
    // `symlink_metadata` and not `exists`, which answers "no" for a link
    // pointing nowhere — and a broken link is a thing somebody wants gone.
    if full.symlink_metadata().is_err() {
        return Err(FilesError::NotFound(rel.to_owned()));
    }
    delete(&full).map_err(|err| FilesError::Io(format!("{rel}: {err}")))?;
    // And then it is checked, which is not belt and braces. A platform trash
    // that answers `Ok` and leaves the entry where it was is a thing that
    // happens — `DeleteMethod::Finder` does exactly that for a symbolic link,
    // which is why `platform_trash` above does not use it — and the folder is
    // re-read the moment this returns, so the row would simply still be there
    // with nothing on screen to say why. A silent no-op is the one outcome
    // worse than a toast, and this is the net under every platform, including
    // whichever one grows the behaviour next.
    if full.symlink_metadata().is_ok() {
        return Err(FilesError::Io(format!(
            "{rel}: the system trash reported success and the entry is still there"
        )));
    }
    Ok(())
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
            ignored: false,
        });
    }

    sort_entries(&mut entries);
    let truncated = entries.len().saturating_sub(MAX_ENTRIES);
    entries.truncate(MAX_ENTRIES);
    mark_git_ignored(&full, &mut entries);

    Ok(Listing { dir: rel.to_owned(), entries, truncated })
}

/// Ask git which of these entries it ignores, and mark them. One call per
/// listing, and the only process this module spawns.
///
/// **After the truncation above and deliberately not before it.** The ceiling
/// exists so that one click on `node_modules` cannot wedge the render, and
/// asking git about forty thousand names that are on their way to being thrown
/// away would spend the whole of that saving.
///
/// **The working directory is the folder being listed** — the absolute path
/// `resolve_within` has already vouched for. That is the entire multi-repository
/// story: git walks up from there and finds whichever repository owns this
/// folder, so a nested repository, a worktree, and a project holding several
/// repositories side by side are all served right, and there is not a line here
/// that knows which folder belongs to which. No `repos::discover`, no
/// `project.toml`.
///
/// **Inheritance costs nothing**, which is why there is no flag to pass down:
/// git answers for `.bin` inside an ignored `node_modules` on its own, so every
/// listing answers for itself and expanding a folder deep inside an ignored one
/// works with no state carried anywhere.
///
/// **git and not a matcher of our own.** The rules are more numerous than they
/// look and this repository's own `.gitignore` shows nearly all of them in
/// twenty lines: a re-inclusion under an excluded parent, where the outcome
/// turns on the order of two lines; a pattern anchored to the repository root
/// rather than matching a folder of that name at any depth; an ignore file at
/// every level of the tree, plus `.git/info/exclude`, plus whatever a person
/// keeps in a global `core.excludesFile`. A second implementation of all that
/// would agree with git for a week and drift quietly afterwards, and the drift
/// would surface as a row in the wrong colour with nothing to point at. git also
/// gives one thing away free: `check-ignore` consults the index, so a file that
/// matches a pattern and is tracked anyway — added with `git add -f` — is
/// reported as **not** ignored and stays at full strength. That is what VS Code
/// does and what a person expects, and it is not a property of the ignore files
/// at all.
///
/// **Nothing here ever reaches a person.** `git check-ignore` exits 1 when
/// nothing matched and 128 when the folder is in no repository at all, and both
/// mean the same thing to the tree: no row is drawn muted. The first is what
/// `git_maybe_fed`'s `absent` argument is for; the second arrives as an `Err`
/// carrying git's own stderr, and so do no git on the machine and a read that
/// hit `READ_CEILING`. Every one of them is swallowed here and written to
/// stderr — a folder outside git is a perfectly ordinary state, and a toast
/// saying so is noise about something that is not broken, which is the standing
/// `git.rs` already takes for the branch in the scope bar. The worst outcome of
/// any failure here is a tree that looks exactly as it looks today.
fn mark_git_ignored(dir: &Path, entries: &mut [Entry]) {
    // Not a shortcut: `git check-ignore --stdin` given nothing at all exits 128
    // with "no path specified", which is a refusal on a directory that is
    // perfectly readable and simply empty.
    if entries.is_empty() {
        return;
    }
    let mut question = String::new();
    for entry in entries.iter() {
        question.push_str(&entry.name);
        question.push('\0');
    }
    let asked = crate::vcs::run::git_maybe_fed(
        dir,
        &["check-ignore", "-z", "--stdin"],
        1,
        question.as_bytes(),
    );
    match asked {
        Ok(Some(answer)) => mark_ignored(entries, &ignored_names(&answer)),
        // Exit 1: git was asked and nothing matched.
        Ok(None) => {}
        Err(err) => {
            eprintln!("[files] could not ask git about {}: {err}", dir.display());
        }
    }
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

    #[test]
    fn a_path_that_does_not_exist_yet_still_resolves_inside_the_root() {
        let root = scratch("new-within");
        fs::create_dir_all(root.join("src")).unwrap();

        let full = resolve_new_within(&root, "src", "main.rs").unwrap();

        assert_eq!(full, root.join("src").join("main.rs"));
        assert!(!full.exists(), "resolving must not create anything");
        assert_eq!(
            resolve_new_within(&root, "", "notes.md").unwrap(),
            root.join("notes.md"),
            "the empty directory is the root itself, as it is everywhere else here"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_name_carrying_a_separator_is_refused_rather_than_split() {
        let root = scratch("new-separator");

        // Making `a/b.js` in one keystroke is what VS Code does and is
        // deliberately out of scope: a name is one level of intent.
        assert!(matches!(
            resolve_new_within(&root, "", "a/b.js"),
            Err(FilesError::BadName(_))
        ));
        assert!(matches!(
            resolve_new_within(&root, "", "..\\escape.txt"),
            Err(FilesError::BadName(_))
        ));
        assert!(
            !root.join("a").exists(),
            "a refused name must leave nothing behind, not even the directory it named"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_two_dot_names_and_the_empty_one_are_refused() {
        let root = scratch("new-dots");

        for name in [".", "..", "", "   "] {
            assert!(
                matches!(resolve_new_within(&root, "", name), Err(FilesError::BadName(_))),
                "{name:?} names nothing that can be made"
            );
        }
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_name_already_taken_is_an_ordinary_refusal_and_not_an_overwrite() {
        let root = scratch("new-taken");
        fs::write(root.join("a.txt"), "somebody's work\n").unwrap();
        fs::create_dir_all(root.join("src")).unwrap();

        assert!(matches!(
            resolve_new_within(&root, "", "a.txt"),
            Err(FilesError::AlreadyExists(_))
        ));
        assert!(
            matches!(resolve_new_within(&root, "", "src"), Err(FilesError::AlreadyExists(_))),
            "a directory takes the name as surely as a file does"
        );
        assert_eq!(
            fs::read_to_string(root.join("a.txt")).unwrap(),
            "somebody's work\n",
            "the file that was already there must not be touched"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_parent_outside_the_root_is_refused_before_the_name_is_ever_joined() {
        let root = scratch("new-outside");

        assert!(matches!(
            resolve_new_within(&root, "../elsewhere", "a.txt"),
            Err(FilesError::Outside(_))
        ));
        assert!(
            matches!(resolve_new_within(&root, "nope", "a.txt"), Err(FilesError::NotFound(_))),
            "a parent that is not there is missing, not outside"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[cfg(unix)]
    #[test]
    fn a_parent_reached_through_a_symlink_out_of_the_root_is_refused_too() {
        let root = scratch("new-escape");
        let outside = scratch("new-escape-target");
        // `reject_traversal` is powerless here: the path holds neither ".." nor
        // a root, and the target exists — so only `canonicalize` can see it.
        std::os::unix::fs::symlink(&outside, root.join("link")).unwrap();

        assert!(matches!(
            resolve_new_within(&root, "link", "planted.txt"),
            Err(FilesError::Outside(_))
        ));
        assert!(!outside.join("planted.txt").exists());
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    #[test]
    fn a_new_file_is_empty_and_answers_with_its_path_from_the_root() {
        let root = scratch("create-file");
        fs::create_dir_all(root.join("src")).unwrap();

        let rel = create_file(&root, "src", "main.rs").unwrap();

        assert_eq!(rel, "src/main.rs", "the front end opens a tab on exactly this string");
        assert_eq!(fs::read_to_string(root.join("src/main.rs")).unwrap(), "");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_new_file_never_truncates_one_that_is_already_there() {
        let root = scratch("create-file-taken");
        fs::write(root.join("a.txt"), "somebody's work\n").unwrap();

        let err = create_file(&root, "", "a.txt");

        assert!(matches!(err, Err(FilesError::AlreadyExists(_))), "{err:?}");
        assert_eq!(fs::read_to_string(root.join("a.txt")).unwrap(), "somebody's work\n");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_new_directory_is_one_level_deep_and_no_more() {
        let root = scratch("create-dir");

        let rel = create_dir(&root, "", "docs").unwrap();

        assert_eq!(rel, "docs");
        assert!(root.join("docs").is_dir());
        assert!(
            matches!(create_dir(&root, "", "a/b"), Err(FilesError::BadName(_))),
            "create_dir_all would have made two of them for a question nobody asked"
        );
        assert!(!root.join("a").exists());
        let _ = fs::remove_dir_all(&root);
    }

    /// The project's own root, by every spelling of it there is. The empty
    /// string is the one the tree uses; `.` is the one any caller of
    /// `files_trash` can type and the one `resolve_within` resolves to exactly
    /// the same directory; and a link inside the project pointing at the
    /// project is the one no string test could catch at all.
    ///
    /// Checked through the seam rather than against a real trash, which is what
    /// makes "nothing was handed over" an assertion instead of a hope.
    #[test]
    fn the_project_root_is_not_something_this_offers_to_throw_away() {
        let root = scratch("trash-root");
        fs::write(root.join("a.txt"), "x").unwrap();

        for spelling in ["", ".", "./", "a/.."] {
            let mut asked: Vec<PathBuf> = Vec::new();
            let err = move_to_trash_with(&root, spelling, |path| {
                asked.push(path.to_owned());
                Ok(())
            });
            assert!(err.is_err(), "{spelling:?} names the project itself");
            assert!(asked.is_empty(), "{spelling:?} reached the trash: {asked:?}");
        }
        assert!(root.join("a.txt").exists(), "nothing may have been touched");
        let _ = fs::remove_dir_all(&root);
    }

    /// The defect this shape exists for. `resolve_within` canonicalizes the
    /// whole path, the last component included, so the link is already unwound
    /// by the time anything is deleted — and `list_dir` draws a link as an
    /// ordinary row, so the row somebody right-clicks says nothing about it.
    /// Deleting `node_modules/.bin/vite` would take the package's real script,
    /// which is a file nobody named and a loss nobody can connect to the click.
    #[cfg(unix)]
    #[test]
    fn a_link_is_deleted_as_a_link_and_never_as_what_it_points_at() {
        let root = scratch("trash-link");
        let outside = scratch("trash-link-target");
        fs::write(outside.join("real.txt"), "somebody's file").unwrap();
        std::os::unix::fs::symlink(outside.join("real.txt"), root.join("link.txt")).unwrap();

        let mut asked: Vec<PathBuf> = Vec::new();
        move_to_trash_with(&root, "link.txt", |path| {
            asked.push(path.to_owned());
            // What a working trash does, so the check that the entry really
            // went is satisfied and this test is about the path alone.
            assert!(
                path.symlink_metadata().unwrap().file_type().is_symlink(),
                "an unwound path would have been the target's, and nothing here would say so"
            );
            fs::remove_file(path).map_err(|err| err.to_string())
        })
        .unwrap();

        assert_eq!(asked, vec![root.join("link.txt")], "the path named is the path handed over");
        assert_eq!(
            fs::read_to_string(outside.join("real.txt")).unwrap(),
            "somebody's file",
            "what the link pointed at is a file nobody named and must not have moved"
        );
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
    }

    /// A link pointing at the project's own root. Every string guard passes this
    /// one — no `..`, not absolute, not empty — and a resolver that
    /// canonicalizes the last component answers with the root itself, so before
    /// this shape the whole project went into the trash from inside the app
    /// looking at it.
    ///
    /// Through the seam, like its neighbours: what this is about is which path
    /// is handed over, and the real answer to that question is an entry in
    /// whoever runs the suite's own Trash.
    #[cfg(unix)]
    #[test]
    fn a_link_pointing_at_the_project_hands_over_the_link_and_not_the_project() {
        let root = scratch("trash-selflink");
        fs::write(root.join("keep.txt"), "somebody's work").unwrap();
        std::os::unix::fs::symlink(&root, root.join("selflink")).unwrap();

        let mut asked: Vec<PathBuf> = Vec::new();
        move_to_trash_with(&root, "selflink", |path| {
            asked.push(path.to_owned());
            fs::remove_file(path).map_err(|err| err.to_string())
        })
        .unwrap();

        assert_eq!(asked, vec![root.join("selflink")]);
        assert!(root.is_dir(), "the project itself must still be there");
        assert_eq!(fs::read_to_string(root.join("keep.txt")).unwrap(), "somebody's work");
        let _ = fs::remove_dir_all(&root);
    }

    /// A name carrying a backslash, which on unix is a name and not a path.
    /// `reject_traversal` lets one through, so `files_list` lists such a row and
    /// `files_read` opens it — and a delete that split on the backslash would
    /// read it as a folder and a file, and take `a/b.txt`, which is a file
    /// nobody named and which is plainly still on screen.
    ///
    /// Splitting on `/` alone leaves the backslash in the tail, where
    /// `reject_bad_name` refuses it. So the row cannot be deleted from this
    /// menu at all, and that is the trade: a refusal a person can see beats a
    /// deletion they cannot. Nothing else in the app can make such a name, and
    /// on Windows it could not be one.
    #[cfg(unix)]
    #[test]
    fn a_backslash_in_a_name_is_a_name_and_never_a_second_folder() {
        let root = scratch("trash-backslash");
        fs::create_dir_all(root.join("a")).unwrap();
        fs::write(root.join("a/b.txt"), "somebody else's file").unwrap();
        fs::write(root.join("a\\b.txt"), "the row that was clicked").unwrap();

        let mut asked: Vec<PathBuf> = Vec::new();
        let outcome = move_to_trash_with(&root, "a\\b.txt", |path| {
            asked.push(path.to_owned());
            fs::remove_file(path).map_err(|err| err.to_string())
        });

        assert!(matches!(outcome, Err(FilesError::BadName(_))), "{outcome:?}");
        assert!(asked.is_empty(), "nothing may have been handed over: {asked:?}");
        assert_eq!(
            fs::read_to_string(root.join("a/b.txt")).unwrap(),
            "somebody else's file",
            "the file in the folder of that name must not have been touched"
        );
        assert!(root.join("a\\b.txt").exists(), "and neither may the row that was clicked");
        let _ = fs::remove_dir_all(&root);
    }

    /// The one test that goes to the real trash, and it does not run: an entry
    /// in the Trash of whoever ran `cargo test` is not something a test may
    /// leave behind, and on macOS the answer also depends on which delete
    /// method is set, which is the thing worth checking by hand after touching
    /// `platform_trash`.
    ///
    ///     cargo test --manifest-path src-tauri/Cargo.toml -- --ignored a_link_really_goes
    ///
    /// It should pass, take milliseconds, and spawn nothing. Under the crate's
    /// macOS default it fails on the last assertion, having run `osascript`.
    #[cfg(unix)]
    #[test]
    #[ignore = "goes to the real system trash; run it by hand after changing platform_trash"]
    fn a_link_really_goes_and_what_it_pointed_at_really_stays() {
        let root = scratch("trash-real-link");
        fs::write(root.join("real.txt"), "somebody's file").unwrap();
        std::os::unix::fs::symlink(root.join("real.txt"), root.join("link.txt")).unwrap();

        move_to_trash(&root, "link.txt").expect("a link is an ordinary thing to delete");

        assert_eq!(
            fs::read_to_string(root.join("real.txt")).unwrap(),
            "somebody's file",
            "what the link pointed at is a file nobody named"
        );
        assert!(
            root.join("link.txt").symlink_metadata().is_err(),
            "the link itself is what was asked for and what should have gone"
        );
        let _ = fs::remove_dir_all(&root);
    }

    /// The check behind that promise, without waiting on a real trash. A
    /// platform that reports success and leaves the entry where it was must not
    /// reach the front end as a deletion — the folder is re-read straight
    /// afterwards and the row would simply still be there, with nothing on
    /// screen to say why.
    #[test]
    fn a_deletion_that_did_not_happen_is_not_reported_as_one() {
        let root = scratch("trash-lied");
        fs::write(root.join("a.txt"), "x").unwrap();

        let outcome = move_to_trash_with(&root, "a.txt", |_| Ok(()));

        assert!(matches!(outcome, Err(FilesError::Io(_))), "{outcome:?}");
        assert!(root.join("a.txt").exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_that_is_not_there_is_missing_rather_than_a_trash_failure() {
        let root = scratch("trash-missing");

        let outcome = move_to_trash_with(&root, "gone.txt", |_| Ok(()));

        assert!(matches!(outcome, Err(FilesError::NotFound(_))), "{outcome:?}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_path_outside_the_root_is_not_trashed() {
        let root = scratch("trash-outside");
        let outside = scratch("trash-outside-target");
        fs::write(outside.join("secret.txt"), "not yours to delete").unwrap();

        assert!(matches!(move_to_trash(&root, "../secret.txt"), Err(FilesError::Outside(_))));
        assert!(matches!(move_to_trash(&root, "/etc/hosts"), Err(FilesError::Outside(_))));
        assert!(outside.join("secret.txt").exists());
        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_dir_all(&outside);
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

    /// A repository holding every case the greying is about, made with **real
    /// git**. Nothing else can answer these: a re-inclusion under an excluded
    /// parent turns on the order of two lines, and a file that matches a
    /// pattern and is tracked anyway is a fact about the index rather than
    /// about any ignore file. The whole point of asking git is that these are
    /// not ours to reimplement, so the tests have to run against it.
    fn ignoring_repository(name: &str) -> PathBuf {
        let root = scratch(name);
        let git = |args: &[&str]| {
            crate::vcs::run::git_write(&root, args).expect("git answered");
        };
        git(&["init", "--quiet"]);
        git(&["config", "user.email", "test@example.com"]);
        git(&["config", "user.name", "Test"]);

        fs::create_dir_all(root.join("node_modules/.bin")).unwrap();
        fs::create_dir_all(root.join("src")).unwrap();
        fs::create_dir_all(root.join(".claude/rules")).unwrap();
        fs::create_dir_all(root.join(".claude/settings")).unwrap();
        fs::write(root.join("node_modules/.bin/vite"), "x").unwrap();
        fs::write(root.join("src/main.js"), "x").unwrap();
        fs::write(root.join(".claude/rules/a.md"), "x").unwrap();
        fs::write(root.join(".claude/settings/b.json"), "x").unwrap();
        fs::write(root.join("package.json"), "x").unwrap();
        fs::write(root.join("keep.log"), "x").unwrap();
        fs::write(
            root.join(".gitignore"),
            "node_modules/\n*.log\n.claude/*\n!.claude/rules/\n",
        )
        .unwrap();
        // Tracked in spite of matching `*.log`, which is the whole of the
        // `git add -f` case: it must come back at full strength.
        git(&["add", "-f", "keep.log"]);
        root
    }

    fn muted(listing: &Listing) -> Vec<&str> {
        listing.entries.iter().filter(|e| e.ignored).map(|e| e.name.as_str()).collect()
    }

    #[test]
    fn a_listing_carries_what_git_ignores_and_leaves_the_rest_alone() {
        let root = ignoring_repository("ignored-root");

        let listing = list_dir(&root, "").unwrap();

        assert_eq!(muted(&listing), vec!["node_modules"]);
        let full: Vec<&str> =
            listing.entries.iter().filter(|e| !e.ignored).map(|e| e.name.as_str()).collect();
        assert!(full.contains(&"src"));
        assert!(full.contains(&"package.json"));
        assert!(
            full.contains(&"keep.log"),
            "a file matching a pattern but tracked anyway is not ignored — git consults the index"
        );

        let _ = fs::remove_dir_all(&root);
    }

    /// `.claude/*` followed by `!.claude/rules/`: the outcome turns on the
    /// order of those two lines, and it comes out right here for no reason
    /// other than that git is the one answering.
    #[test]
    fn a_re_inclusion_under_an_excluded_parent_comes_out_right() {
        let root = ignoring_repository("ignored-reinclusion");

        let listing = list_dir(&root, ".claude").unwrap();

        assert_eq!(muted(&listing), vec!["settings"]);
        let rules = listing.entries.iter().find(|e| e.name == "rules").expect("rules is listed");
        assert!(!rules.ignored, "the re-inclusion puts it back at full strength");

        let _ = fs::remove_dir_all(&root);
    }

    /// No flag is carried down the tree, and none has to be: git reports a name
    /// inside an ignored folder as ignored in its own right, so expanding one
    /// works with no state anywhere.
    #[test]
    fn every_listing_answers_for_itself_inside_an_ignored_folder() {
        let root = ignoring_repository("ignored-inside");

        let listing = list_dir(&root, "node_modules").unwrap();

        assert_eq!(muted(&listing), vec![".bin"]);

        let _ = fs::remove_dir_all(&root);
    }

    /// git exits 128 here — the folder is in no repository at all — and the
    /// listing comes back whole with nothing marked and no error of any kind.
    /// This is the ordinary state of a project somebody has not put under git,
    /// and it is an answer rather than a failure.
    #[test]
    fn a_folder_in_no_repository_lists_with_nothing_muted_and_no_refusal() {
        let root = scratch("ignored-no-repo");
        fs::create_dir_all(root.join("node_modules")).unwrap();
        fs::write(root.join("app.js"), "x").unwrap();
        fs::write(root.join("notes.log"), "x").unwrap();

        let listing = list_dir(&root, "").expect("a folder outside git still lists");

        assert_eq!(listing.entries.len(), 3);
        assert!(listing.entries.iter().all(|e| !e.ignored), "there is no .gitignore above it");

        let _ = fs::remove_dir_all(&root);
    }

    /// An empty directory is the one shape that must not reach git at all:
    /// `check-ignore --stdin` given nothing exits 128 with "no path specified",
    /// which would be a refusal on a folder that is perfectly readable.
    #[test]
    fn an_empty_directory_inside_a_repository_is_never_asked_about() {
        let root = ignoring_repository("ignored-empty");
        fs::create_dir_all(root.join("src/empty")).unwrap();

        let listing = list_dir(&root, "src/empty").expect("an empty folder lists");

        assert!(listing.entries.is_empty());

        let _ = fs::remove_dir_all(&root);
    }
}
