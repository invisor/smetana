//! Images attached to a task before an agent files it.
//!
//! The same shape as `files/` and `git.rs`, and for the same reason: writing a
//! couple of megabytes holds no state that anything has to guard, so there is
//! no worker and no queue here — pure functions carrying the tests, and thin
//! commands over them. Two of those store an image and two are the settings
//! window's Storage tab, which counts what is here and, on a person's press,
//! sweeps it. This file is the disk; `cleanup.rs` is the deciding, with no
//! filesystem anywhere in it.
//!
//! **Where the bytes go.** Into the app's own data directory (`app_data_dir()`),
//! and the path that reaches the agent is absolute. A file chosen with the
//! picker is copied too, rather than pointed at where it lies: the main case is
//! a screenshot in `~/Downloads`, which a person throws away in a week, and the
//! link in the issue is obliged to outlive that. Writing into the repository
//! instead — `.smetana/attachments/` and a relative path — was considered and
//! refused: it would work in every clone and every worktree, but only for files
//! somebody committed, and committing binaries into another person's tree is
//! not this app's decision to make. The price of the choice is plain: in
//! somebody else's clone, and in CI, the images are not there.
//!
//! **A picture goes into its project's own folder** — `attachments/<key>/…` —
//! and `cleanup::project_key` is the whole of what a key is. That layout is not
//! tidiness: it is the boundary the one deleting thing in this app works
//! inside. A run of the tidy-up reads the active project's tracker and can
//! reach nothing but the active project's folder, so a neighbouring project's
//! pictures are out of the way physically rather than by a careful condition in
//! a loop. Anything already sitting in the root of the store from before that
//! split is out of reach for the same reason and stays there for good: those
//! files belong to no project, so no board can be read to find out whether
//! somebody still wants them.
//!
//! **Nothing here ever deletes on its own.** Taking a thumbnail out of the
//! dialog forgets the path and leaves the file, closing the dialog leaves what
//! was attached, and neither starting the app nor closing it removes anything.
//! The one thing that deletes is `attachments_clean`, which a person starts by
//! pressing a button in the settings window after reading how many files and
//! how many bytes it is about to take — deleting is irreversible, so it is
//! always somebody's decision and never a schedule's.
//!
//! **There is no `resolve_within` here, and its absence is the design rather
//! than an oversight.** `files/fs.rs` confines every path to the project root
//! because everything it touches belongs to the project. Nothing here does: the
//! *source* of `attachment_import` is whatever a person picked in a system
//! dialog or dragged off their desktop, and a folder outside the project is the
//! ordinary case, not the attack. Reading it is exactly what was asked for, by
//! the same person, through the OS's own picker. What is confined is the
//! *destination*: it is always `app_data_dir()/attachments/<key>`, and the file name
//! is not the one that arrived — `stored_name` builds it out of a timestamp and
//! a `slug` that keeps ASCII letters and digits and nothing else, so no name
//! coming in can climb a directory, hide behind a dot or need quoting.

mod cleanup;

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tokio::sync::oneshot;

use crate::tracker::model::{Issue, Snapshot};
use crate::tracker::service::{Request, TrackerHandle};

use cleanup::{project_key, StoredFile, Tally};

/// The ceiling, and deliberately **not** `files::model::MAX_FILE_BYTES`.
///
/// That one is 2 MiB and answers a different question: how much text a
/// `textarea` will open without freezing the window. This one answers how big a
/// screenshot is, and the two have no reason to agree — a full-screen retina
/// PNG, which is the gesture this whole feature exists for, routinely lands
/// between 2 and 8 MB. Reusing the editor's number would have refused the
/// primary case on ordinary input, and tying them together would guarantee that
/// one of the two is wrong whenever the other is right.
///
/// 8 MiB is where a screenshot stops being a screenshot. Past it the payload
/// costs more to carry through the webview than the picture is worth, and the
/// refusal names both numbers so a person can see what they are up against.
pub const MAX_IMAGE_BYTES: u64 = 8 * 1024 * 1024;

/// The directory under `app_data_dir()` everything lands in.
const STORE: &str = "attachments";

/// What a refusal calls something that arrived with no name — a screenshot off
/// the clipboard. One copy, because the pre-check and the real check have to
/// name it the same way or the same file would be refused in two voices.
const PASTED: &str = "the pasted image";

/// The smallest number of bytes a base64 string of this length can decode to.
///
/// Every 4 characters carry 3 bytes, and at most 2 of the last 3 are padding.
/// The *lower* bound is the one to compare against a ceiling: this check sits
/// in front of the authoritative one in `save_into` and must never turn away a
/// payload that one would have accepted — a base64 length cannot tell a file of
/// exactly the ceiling from one a byte over, and of the two ways to be wrong
/// here, refusing early is the one that costs somebody their screenshot.
fn decoded_at_least(len: usize) -> u64 {
    (len / 4 * 3).saturating_sub(2) as u64
}

/// What the bytes turn out to be. The name a file arrives with is not asked:
/// a pasted screenshot has no name at all, and a `.png` that is really a JPEG
/// would reach the agent labelled wrongly by the one party that could check.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl Format {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Webp => "webp",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }
}

/// The four raster formats every agent that reads an image reads. SVG is
/// deliberately absent: it is a document rather than a picture, no harness
/// takes it as one, and the thumbnail in the dialog would be the only party
/// that ever rendered it.
pub fn sniff(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(Format::Png);
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Some(Format::Jpeg);
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(Format::Gif);
    }
    // RIFF is a container for several things; only one of them is an image, and
    // the tag that says which sits four bytes past the length field.
    if bytes.starts_with(b"RIFF") && bytes.len() >= 12 && &bytes[8..12] == b"WEBP" {
        return Some(Format::Webp);
    }
    None
}

/// How many characters of the original name survive. Long enough to still say
/// what the picture is, short enough that a path in an issue description stays
/// readable on one line.
const STEM_MAX: usize = 40;

/// The part of the original name worth keeping: ASCII letters and digits,
/// every run of anything else folded to a single `-`. The stored name ends up
/// inside a prompt, a shell argument and an issue description, and a name that
/// needs quoting in any of the three is one that will eventually be pasted
/// without them. A name with nothing ASCII in it at all leaves nothing here,
/// and the caller falls back to a name of its own.
fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= STEM_MAX {
            break;
        }
    }
    out.trim_matches('-').to_owned()
}

/// The name a stored image gets: when it arrived, what it was called, and what
/// it actually is. `n` is a collision counter and is only spelled out past
/// zero — two files in the same second is the only way to reach it.
///
/// The extension comes from the sniffed format rather than from the original
/// name, so a screenshot saved as `shot.PNG` and a JPEG somebody renamed both
/// end up labelled with what they are.
pub fn stored_name(original: Option<&str>, format: Format, stamp: &str, n: u32) -> String {
    let stem = original
        .map(|name| slug(Path::new(name).file_stem().unwrap_or_default().to_string_lossy().as_ref()))
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "image".to_owned());
    let suffix = if n == 0 { String::new() } else { format!("-{n}") };
    format!("{stamp}-{stem}{suffix}.{}", format.extension())
}

/// One stored image, as the dialog sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    /// Absolute, and the string that ends up in the issue description.
    pub path: String,
    /// The stored file's own name — what the thumbnail is labelled with.
    pub name: String,
    pub bytes: u64,
    pub mime: String,
    /// The file's bytes, base64. The webview cannot read a file it did not
    /// open — no asset protocol is enabled here, and enabling one would open
    /// the app's data directory to the page for the sake of a thumbnail — so
    /// the bytes travel back with the answer. Both commands answer alike, which
    /// is what leaves the strip one shape to draw.
    pub data: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AttachmentError {
    #[error("{name} is {bytes} bytes; the ceiling is {MAX_IMAGE_BYTES} bytes")]
    TooLarge { name: String, bytes: u64 },
    #[error("{0} is not a PNG, JPEG, GIF or WebP image")]
    NotAnImage(String),
    /// Only the tidy-up ever sends this. Which pictures are still wanted is
    /// read off a project's board, so with no project open the question has no
    /// answer — and answering "nothing to delete" would be a guess dressed as a
    /// result.
    #[error("no project is open, so there is no board to say which images are still in use")]
    NoProject,
    #[error("{0}")]
    Io(String),
}

impl AttachmentError {
    /// The machine-readable half, the same as `FilesError`'s: the message says
    /// what happened in the filesystem's words, and what to put on screen is
    /// decided from this field rather than by reading that text.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::TooLarge { .. } => "tooLarge",
            Self::NotAnImage(_) => "notAnImage",
            Self::NoProject => "noProject",
            Self::Io(_) => "io",
        }
    }
}

impl Serialize for AttachmentError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("AttachmentError", 2)?;
        st.serialize_field("kind", self.kind())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}

/// When this arrived, to the second. Local time rather than UTC: the number is
/// read by a person looking at a list of file names, next to a screenshot they
/// took a minute ago.
fn stamp() -> String {
    chrono::Local::now().format("%Y%m%d-%H%M%S").to_string()
}

/// The whole store: every project's folder lives under this one.
fn store_root(app: &AppHandle) -> Result<PathBuf, AttachmentError> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|err| AttachmentError::Io(format!("no app data directory: {err}")))?
        .join(STORE))
}

/// Somewhere to put it: the active project's own folder. Created on the way,
/// since the very first attachment in a project arrives before it exists.
///
/// A picture attached while no project is open, or while the tracker worker
/// cannot say which one it is, lands in the root of the store instead. That is
/// the honest place for it rather than a fallback: the root is exactly the part
/// of the store no cleanup can reach, and a file whose project is unknown must
/// not be filed under a project that would later decide nothing refers to it.
fn store_dir(app: &AppHandle, project: Option<&Path>) -> Result<PathBuf, AttachmentError> {
    let root = store_root(app)?;
    let dir = match project {
        Some(project) => root.join(project_key(project)),
        None => root,
    };
    std::fs::create_dir_all(&dir)
        .map_err(|err| AttachmentError::Io(format!("{}: {err}", dir.display())))?;
    Ok(dir)
}

/// The folder the tracker worker is looking at, and the board it holds, as one
/// answer.
///
/// One request rather than two, and that is the point of it: the cleanup reads
/// a project's tracker to decide what may go out of that project's folder, and
/// two questions answered a moment apart could name two different projects
/// across a switch. Asking the worker rather than taking a path from the front
/// end is the same decision `runs::commands::target_branches` records — a
/// project threaded down through the webview is a second copy of a fact, and
/// the two get out of step in exactly the window that matters.
async fn current(app: &AppHandle) -> Result<(Option<PathBuf>, Snapshot), AttachmentError> {
    let handle = app
        .try_state::<TrackerHandle>()
        .ok_or_else(|| AttachmentError::Io("the tracker worker is not running".to_owned()))?;
    let (tx, rx) = oneshot::channel();
    handle
        .0
        .send(Request::Current(tx))
        .await
        .map_err(|_| AttachmentError::Io("the tracker worker is not running".to_owned()))?;
    rx.await
        .map_err(|_| AttachmentError::Io("the tracker worker did not answer".to_owned()))
}

/// Which project a picture being attached right now belongs to. A worker that
/// cannot answer is not a reason to refuse somebody's screenshot — the file
/// goes to the root of the store, which is the one place nothing ever deletes.
async fn current_project(app: &AppHandle) -> Option<PathBuf> {
    match current(app).await {
        Ok((project, _)) => project,
        Err(err) => {
            log::warn!("attachments: no project for this image ({err}); storing it in the root");
            None
        }
    }
}

/// The files sitting directly in one directory of the store.
///
/// Directly, and nothing below: the store is two levels deep by construction,
/// and a walk that descended would give the deleting code paths it did not
/// build. `file_type` follows no symlink, so a link somebody dropped in here is
/// neither counted as bytes it does not own nor offered up to be deleted.
fn list(dir: &Path) -> Vec<StoredFile> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        // A project that has never had an attachment has no folder, and that is
        // an empty list rather than a failure — the same reading `git.rs` gives
        // a folder outside git.
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        if !entry.file_type().map(|kind| kind.is_file()).unwrap_or(false) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let name = entry.file_name().to_string_lossy().into_owned();
        files.push(StoredFile {
            path: dir.join(&name).to_string_lossy().into_owned(),
            name,
            bytes: meta.len(),
        });
    }
    files
}

/// What the whole store weighs: the root's own leftovers plus every project's
/// folder. This is the number the settings window calls the storage, and it
/// deliberately covers more than the button can reach — a person asking how big
/// this has got is asking about the directory, not about their share of it.
fn weigh(root: &Path) -> Tally {
    let mut total = Tally::default();
    for file in list(root) {
        total.add(file.bytes);
    }
    let Ok(entries) = std::fs::read_dir(root) else { return total };
    for entry in entries.flatten() {
        if entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
            for file in list(&entry.path()) {
                total.add(file.bytes);
            }
        }
    }
    total
}

/// Split one project's folder into what stays and what may go.
fn survey_dir(dir: &Path, issues: &[Issue]) -> (Tally, Tally) {
    let files = list(dir);
    let doomed = cleanup::removable(&files, issues);
    let mut removable = Tally::default();
    for file in &doomed {
        removable.add(file.bytes);
    }
    let mut kept = Tally::default();
    for file in &files {
        if !doomed.iter().any(|other| other.name == file.name) {
            kept.add(file.bytes);
        }
    }
    (kept, removable)
}

/// Delete what no unfinished task refers to, and say what actually went.
///
/// Every path handed to `remove_file` is this directory joined with a name that
/// came out of this directory's own `read_dir`, checked once more by
/// `plain_name` on the way past. Nothing else is ever built, no subdirectory is
/// entered and no string from the front end reaches this function at all, which
/// is what makes "the button cannot touch another project" a property of the
/// code rather than of a condition somebody has to keep right.
fn sweep(dir: &Path, issues: &[Issue]) -> (Tally, u64) {
    let files = list(dir);
    let mut removed = Tally::default();
    let mut failed = 0;
    for file in cleanup::removable(&files, issues) {
        if !cleanup::plain_name(&file.name) {
            log::warn!("attachments: refusing to delete {:?}, which is not a plain name", file.name);
            failed += 1;
            continue;
        }
        match std::fs::remove_file(dir.join(&file.name)) {
            Ok(()) => removed.add(file.bytes),
            Err(err) => {
                log::warn!("attachments: could not delete {}: {err}", file.path);
                failed += 1;
            }
        }
    }
    (removed, failed)
}

/// What the settings window draws before anybody presses anything.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Survey {
    /// Everything under `attachments/`, whoever it belongs to — the storage.
    pub store: Tally,
    /// The active project, absolute, or `None` when no project is open. The
    /// window names it, because the button reaches this project and no other.
    pub project: Option<String>,
    /// This project's files an unfinished task still refers to.
    pub kept: Tally,
    /// This project's files nothing unfinished refers to — what the button
    /// would take, counted before it is pressed, which is the whole reason this
    /// command exists apart from the one that deletes.
    pub removable: Tally,
}

/// What a press actually did, and where that leaves the store.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cleaned {
    pub removed: Tally,
    /// Files the rule chose and the disk would not give up — a permission, a
    /// file already gone. Reported rather than swallowed: a person who was told
    /// seven and sees six wants to know which half of that is true.
    pub failed: u64,
    /// The same numbers the window opened on, recomputed after the deleting, so
    /// one press updates the screen without a second round trip through a
    /// worker that may be two seconds into a bd call.
    pub survey: Survey,
}

/// Read the store and the board, and say what could go.
///
/// A read, so it answers with a project of `None` rather than refusing when
/// there is nothing open: the section still has a size to show, and the button
/// under it has a reason to be disabled that a person can read.
#[tauri::command]
pub async fn attachments_survey(app: AppHandle) -> Result<Survey, AttachmentError> {
    let root = store_root(&app)?;
    let (project, snapshot) = current(&app).await?;
    let (kept, removable) = match &project {
        Some(project) => survey_dir(&root.join(project_key(project)), &snapshot.issues),
        None => (Tally::default(), Tally::default()),
    };
    Ok(Survey {
        store: weigh(&root),
        project: project.map(|path| path.to_string_lossy().into_owned()),
        kept,
        removable,
    })
}

/// The one thing in this app that deletes somebody's pictures, and it happens
/// only here, only when a person presses the button, and only inside the active
/// project's own folder.
#[tauri::command]
pub async fn attachments_clean(app: AppHandle) -> Result<Cleaned, AttachmentError> {
    let root = store_root(&app)?;
    let (project, snapshot) = current(&app).await?;
    // Not "delete nothing and say it worked": with no project there is no
    // tracker to ask what is still wanted, so there is no honest answer here at
    // all — and a write that looked like it happened is the one thing this app
    // refuses everywhere.
    let Some(project) = project else { return Err(AttachmentError::NoProject) };

    let dir = root.join(project_key(&project));
    let (removed, failed) = sweep(&dir, &snapshot.issues);
    let (kept, removable) = survey_dir(&dir, &snapshot.issues);
    Ok(Cleaned {
        removed,
        failed,
        survey: Survey {
            store: weigh(&root),
            project: Some(project.to_string_lossy().into_owned()),
            kept,
            removable,
        },
    })
}

/// Write the bytes into `dir` and say what was written.
///
/// The refusals come before the disk is touched at all: an oversized file and
/// something that is not an image are both ordinary things for a person to
/// hand over, and neither leaves anything behind.
pub fn save_into(
    dir: &Path,
    original: Option<&str>,
    bytes: Vec<u8>,
    stamp: &str,
) -> Result<Attachment, AttachmentError> {
    let label = original.unwrap_or(PASTED).to_owned();
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(AttachmentError::TooLarge { name: label, bytes: bytes.len() as u64 });
    }
    let Some(format) = sniff(&bytes) else {
        return Err(AttachmentError::NotAnImage(label));
    };

    // A name nothing else in the directory has. Two attachments inside one
    // second is the only way past the first turn of this loop, and the
    // alternative — trusting the timestamp alone — silently overwrites the
    // first of the two, which is a person's file.
    let mut n = 0;
    let (name, full) = loop {
        let name = stored_name(original, format, stamp, n);
        let full = dir.join(&name);
        if !full.exists() {
            break (name, full);
        }
        n += 1;
    };

    std::fs::write(&full, &bytes)
        .map_err(|err| AttachmentError::Io(format!("{}: {err}", full.display())))?;

    Ok(Attachment {
        path: full.to_string_lossy().into_owned(),
        name,
        bytes: bytes.len() as u64,
        mime: format.mime().to_owned(),
        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
    })
}

/// A file already on disk — what the picker and a drop on the window produce.
///
/// The size is read from the metadata first, so a video somebody dropped by
/// mistake is refused without being read into memory — and, since the project's
/// folder is asked of the tracker worker, without queueing behind a bd call for
/// something that was never going to be stored.
#[tauri::command]
pub async fn attachment_import(app: AppHandle, path: String) -> Result<Attachment, AttachmentError> {
    let source = PathBuf::from(&path);
    let name = source.file_name().map(|n| n.to_string_lossy().into_owned());
    let meta = std::fs::metadata(&source)
        .map_err(|err| AttachmentError::Io(format!("{path}: {err}")))?;
    // A folder is an ordinary thing to drop on a window by mistake, and reading
    // one answers with the operating system's own wording about directories.
    // The refusal a person can act on is the one about images.
    if !meta.is_file() {
        return Err(AttachmentError::NotAnImage(name.unwrap_or(path)));
    }
    if meta.len() > MAX_IMAGE_BYTES {
        return Err(AttachmentError::TooLarge {
            name: name.unwrap_or(path),
            bytes: meta.len(),
        });
    }
    let bytes = std::fs::read(&source).map_err(|err| AttachmentError::Io(format!("{path}: {err}")))?;
    let dir = store_dir(&app, current_project(&app).await.as_deref())?;
    save_into(&dir, name.as_deref(), bytes, &stamp())
}

/// Bytes the webview is holding — a paste, and nothing else can produce them:
/// the clipboard exists in the page and nowhere this process can reach it.
///
/// base64 rather than a `Vec<u8>`, for the reason `terminal/service.rs` records
/// on the way out: a byte array crosses this boundary as a JSON array of
/// numbers, which for a screenshot is several times the size of the picture.
///
/// The size is judged from the string's own length before any of it is decoded,
/// which is this route's version of the `metadata().len()` read in
/// `attachment_import` — and it matters more here, not less: a paste is the
/// gesture most likely to be oversized, and decoding first would allocate the
/// whole picture a second time only to throw it away.
#[tauri::command]
pub async fn attachment_write(
    app: AppHandle,
    name: Option<String>,
    data: String,
) -> Result<Attachment, AttachmentError> {
    if decoded_at_least(data.len()) > MAX_IMAGE_BYTES {
        return Err(AttachmentError::TooLarge {
            name: name.unwrap_or_else(|| PASTED.to_owned()),
            // The upper bound, which is what the person actually handed over to
            // within two bytes; the lower bound is for deciding, not for
            // reporting a size back to somebody.
            bytes: (data.len() / 4 * 3) as u64,
        });
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|err| AttachmentError::Io(format!("{PASTED} did not decode: {err}")))?;
    let dir = store_dir(&app, current_project(&app).await.as_deref())?;
    save_into(&dir, name.as_deref(), bytes, &stamp())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    const JPEG: &[u8] = b"\xff\xd8\xff\xe0\x00\x10JFIF";

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-attachments-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    #[test]
    fn the_four_formats_are_recognised_by_their_own_first_bytes() {
        assert_eq!(sniff(PNG), Some(Format::Png));
        assert_eq!(sniff(JPEG), Some(Format::Jpeg));
        assert_eq!(sniff(b"GIF89a\x01\x00"), Some(Format::Gif));
        assert_eq!(sniff(b"RIFF\x24\x00\x00\x00WEBPVP8 "), Some(Format::Webp));
    }

    #[test]
    fn anything_else_is_not_an_image() {
        assert_eq!(sniff(b""), None);
        assert_eq!(sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"), None);
        assert_eq!(sniff(b"%PDF-1.7"), None);
        // A RIFF container that holds sound rather than a picture: the four
        // bytes past the length are the whole difference, and a check that
        // stopped at "RIFF" would take a .wav for a screenshot.
        assert_eq!(sniff(b"RIFF\x24\x00\x00\x00WAVEfmt "), None);
    }

    #[test]
    fn the_stored_name_says_when_what_and_which_kind() {
        let name = stored_name(Some("Design mock.png"), Format::Png, "20260806-121314", 0);
        assert_eq!(name, "20260806-121314-Design-mock.png");
    }

    #[test]
    fn the_extension_is_what_the_bytes_are_not_what_the_name_claimed() {
        // A JPEG somebody renamed. The agent is told the truth, and so is
        // anything that opens the file by its path later.
        let name = stored_name(Some("shot.png"), Format::Jpeg, "20260806-121314", 0);
        assert!(name.ends_with(".jpg"), "{name}");
    }

    #[test]
    fn a_paste_has_no_name_and_gets_one() {
        assert_eq!(
            stored_name(None, Format::Png, "20260806-121314", 0),
            "20260806-121314-image.png"
        );
        // And so does a name that survives sanitising as nothing at all.
        assert_eq!(
            stored_name(Some("???.png"), Format::Png, "20260806-121314", 0),
            "20260806-121314-image.png"
        );
    }

    #[test]
    fn a_name_that_would_need_quoting_never_reaches_the_disk() {
        // This string ends up in a prompt, in an argument and in an issue
        // description; the one that eventually gets pasted without quotes is
        // the reason none of it survives.
        let name = stored_name(Some("my shot; rm -rf.png"), Format::Png, "20260806-121314", 0);
        assert_eq!(name, "20260806-121314-my-shot-rm-rf.png");
    }

    #[test]
    fn only_the_file_s_own_name_is_kept_never_the_directories_above_it() {
        // What arrives is an absolute path from a picker or a drop, and the
        // stored name has no business carrying somebody's home directory.
        let name = stored_name(Some("/Users/you/Downloads/shot.png"), Format::Png, "20260806-121314", 0);
        assert_eq!(name, "20260806-121314-shot.png");
    }

    #[test]
    fn a_name_with_no_ascii_in_it_falls_back_rather_than_producing_dashes() {
        // Any script would do; this one reads "screenshot" in Japanese. Nothing
        // survives the sanitiser, and a row of dashes is not a name.
        assert_eq!(
            stored_name(Some("スクリーンショット.png"), Format::Png, "20260806-121314", 0),
            "20260806-121314-image.png"
        );
    }

    #[test]
    fn a_collision_is_counted_rather_than_overwritten() {
        assert_eq!(
            stored_name(Some("a.png"), Format::Png, "20260806-121314", 2),
            "20260806-121314-a-2.png"
        );
    }

    #[test]
    fn a_saved_image_lands_on_disk_and_comes_back_with_its_own_bytes() {
        let dir = scratch("save");

        let saved = save_into(&dir, Some("mock.png"), PNG.to_vec(), "20260806-121314")
            .expect("a PNG under the ceiling is saved");

        assert_eq!(saved.name, "20260806-121314-mock.png");
        assert_eq!(saved.mime, "image/png");
        assert_eq!(saved.bytes, PNG.len() as u64);
        assert_eq!(std::fs::read(&saved.path).unwrap(), PNG);
        assert!(Path::new(&saved.path).is_absolute(), "the issue carries this path: {}", saved.path);
        assert_eq!(
            base64::engine::general_purpose::STANDARD.decode(saved.data.as_bytes()).unwrap(),
            PNG,
            "the thumbnail has to be drawn from what was actually stored"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn two_images_in_the_same_second_both_survive() {
        let dir = scratch("collide");

        let first = save_into(&dir, Some("a.png"), PNG.to_vec(), "20260806-121314").unwrap();
        let second = save_into(&dir, Some("a.png"), JPEG.to_vec(), "20260806-121314").unwrap();

        assert_ne!(first.path, second.path);
        assert_eq!(std::fs::read(&first.path).unwrap(), PNG, "the first must not be overwritten");
        assert_eq!(std::fs::read(&second.path).unwrap(), JPEG);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_oversized_image_is_refused_and_leaves_nothing_behind() {
        let dir = scratch("too-large");
        let mut huge = PNG.to_vec();
        huge.resize(MAX_IMAGE_BYTES as usize + 1, 0);

        let err = save_into(&dir, Some("huge.png"), huge, "20260806-121314");

        assert!(matches!(err, Err(AttachmentError::TooLarge { .. })), "{err:?}");
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0, "a refusal writes nothing");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn something_that_is_not_an_image_is_refused_by_name() {
        let dir = scratch("not-an-image");

        let err = save_into(&dir, Some("notes.txt"), b"just text".to_vec(), "20260806-121314");

        match err {
            Err(AttachmentError::NotAnImage(name)) => assert_eq!(name, "notes.txt"),
            other => panic!("expected NotAnImage, got {other:?}"),
        }
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The base64 length of a file of exactly `bytes`, the way the webview
    /// produces it: 4 characters per 3 bytes, rounded up, padded.
    fn encoded_len(bytes: usize) -> usize {
        bytes.div_ceil(3) * 4
    }

    #[test]
    fn an_oversized_paste_is_refused_from_the_strings_length_alone() {
        // The point is what does *not* happen: nothing is decoded, so a 40 MB
        // paste never becomes 40 MB of bytes beside the 53 MB of text already
        // holding it. `attachment_import` reads `metadata().len()` for exactly
        // the same reason.
        let huge = MAX_IMAGE_BYTES as usize * 4;
        assert!(decoded_at_least(encoded_len(huge)) > MAX_IMAGE_BYTES);
    }

    #[test]
    fn the_cheap_refusal_never_turns_away_what_the_real_one_would_keep() {
        // A base64 length cannot tell a file of exactly the ceiling from one a
        // byte over it, so this check is deliberately the looser of the two:
        // everything up to and including the ceiling passes here and meets the
        // authoritative check in `save_into`.
        for bytes in [0, 1, 1024, MAX_IMAGE_BYTES as usize - 1, MAX_IMAGE_BYTES as usize] {
            assert!(
                decoded_at_least(encoded_len(bytes)) <= MAX_IMAGE_BYTES,
                "{bytes} bytes would have been refused before anything looked at them"
            );
        }
    }

    #[test]
    fn the_ceiling_is_this_module_s_own_and_not_the_editor_s() {
        // A screenshot is not a source file, and 2 MiB — what a textarea will
        // open — refuses the very gesture this feature exists for. If these two
        // are ever equal again it is because somebody wired them together.
        assert_ne!(MAX_IMAGE_BYTES, crate::files::model::MAX_FILE_BYTES);
        assert!(MAX_IMAGE_BYTES >= 8 * 1024 * 1024, "a retina screenshot reaches 8 MB");
    }

    #[test]
    fn every_refusal_travels_as_a_kind_and_a_readable_line() {
        let too_large = AttachmentError::TooLarge { name: "huge.png".into(), bytes: 9_000_000 };
        let json = serde_json::to_value(&too_large).unwrap();
        assert_eq!(json["kind"], "tooLarge");
        let message = json["message"].as_str().unwrap();
        assert!(message.contains("huge.png"), "{message}");
        assert!(message.contains(&MAX_IMAGE_BYTES.to_string()), "the ceiling is the actionable half: {message}");

        assert_eq!(AttachmentError::NotAnImage("a.txt".into()).kind(), "notAnImage");
        assert_eq!(AttachmentError::NoProject.kind(), "noProject");
        assert_eq!(AttachmentError::Io("x".into()).kind(), "io");
    }

    /// A store with one file in the root and one in each of two projects'
    /// folders — the shape the button has to be safe in.
    fn store_with_three(name: &str) -> (PathBuf, PathBuf, PathBuf) {
        let root = scratch(name);
        let mine = root.join(project_key(Path::new("/projects/mine")));
        let theirs = root.join(project_key(Path::new("/projects/theirs")));
        std::fs::create_dir_all(&mine).unwrap();
        std::fs::create_dir_all(&theirs).unwrap();
        std::fs::write(root.join("20240101-000000-old.png"), PNG).unwrap();
        std::fs::write(mine.join("20260806-121314-mine.png"), PNG).unwrap();
        std::fs::write(theirs.join("20260806-121314-theirs.png"), JPEG).unwrap();
        (root, mine, theirs)
    }

    fn issue(status: &str, description: &str) -> crate::tracker::model::Issue {
        crate::tracker::model::Issue {
            id: "smetana-x".into(),
            status: status.into(),
            description: Some(description.into()),
            ..Default::default()
        }
    }

    #[test]
    fn a_sweep_reaches_nothing_but_the_folder_it_was_given() {
        // The whole of the confinement: the neighbouring project and the files
        // left in the root from before the store was split by project are out
        // of reach because nothing ever names them, not because a condition
        // spares them.
        let (root, mine, theirs) = store_with_three("sweep-confined");

        let (removed, failed) = sweep(&mine, &[]);

        assert_eq!(removed.files, 1, "the folder it was given is swept");
        assert_eq!(failed, 0);
        assert!(root.join("20240101-000000-old.png").exists(), "the root is never touched");
        assert!(theirs.join("20260806-121314-theirs.png").exists(), "another project is never touched");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_sweep_keeps_what_an_unfinished_task_names_and_takes_the_rest() {
        let root = scratch("sweep-rule");
        let dir = root.join("project");
        std::fs::create_dir_all(&dir).unwrap();
        let wanted = dir.join("20260806-121314-wanted.png");
        let closed = dir.join("20260806-121315-closed.png");
        let orphan = dir.join("20260806-121316-orphan.png");
        for path in [&wanted, &closed, &orphan] {
            std::fs::write(path, PNG).unwrap();
        }
        let issues = [
            issue("open", &format!("still needed: {}", wanted.display())),
            issue("closed", &format!("done with: {}", closed.display())),
        ];

        let (removed, failed) = sweep(&dir, &issues);

        assert_eq!(failed, 0);
        assert_eq!(removed.files, 2);
        assert_eq!(removed.bytes, PNG.len() as u64 * 2);
        assert!(wanted.exists(), "an unfinished task still refers to it");
        assert!(!closed.exists(), "only a closed task referred to it");
        assert!(!orphan.exists(), "nothing referred to it at all");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_store_weighs_the_root_and_every_projects_folder_alike() {
        // The number in the settings window is about the directory, not about
        // the share of it the button can reach — a person asking how big this
        // has got is not asking about their current project.
        let (root, _, _) = store_with_three("weigh");

        let total = weigh(&root);

        assert_eq!(total.files, 3);
        assert_eq!(total.bytes, PNG.len() as u64 * 2 + JPEG.len() as u64);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_survey_counts_what_stays_apart_from_what_would_go() {
        let root = scratch("survey");
        let dir = root.join("project");
        std::fs::create_dir_all(&dir).unwrap();
        let wanted = dir.join("20260806-121314-wanted.png");
        std::fs::write(&wanted, PNG).unwrap();
        std::fs::write(dir.join("20260806-121315-orphan.png"), JPEG).unwrap();

        let (kept, removable) = survey_dir(&dir, &[issue("open", &wanted.display().to_string())]);

        assert_eq!(kept, Tally { files: 1, bytes: PNG.len() as u64 });
        assert_eq!(removable, Tally { files: 1, bytes: JPEG.len() as u64 });
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_project_that_has_never_had_an_attachment_reads_as_empty_rather_than_failing() {
        let root = scratch("no-folder");

        assert_eq!(survey_dir(&root.join("never-used"), &[]), (Tally::default(), Tally::default()));
        assert_eq!(sweep(&root.join("never-used"), &[]), (Tally::default(), 0));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_directory_inside_a_projects_folder_is_neither_counted_nor_deleted() {
        // Nothing here ever makes one, so it can only be somebody's own doing —
        // and a walk that descended would hand `remove_file` paths this module
        // did not build.
        let root = scratch("nested");
        let dir = root.join("project");
        std::fs::create_dir_all(dir.join("keep-me")).unwrap();
        std::fs::write(dir.join("keep-me/inside.png"), PNG).unwrap();

        assert_eq!(sweep(&dir, &[]), (Tally::default(), 0));
        assert!(dir.join("keep-me/inside.png").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
