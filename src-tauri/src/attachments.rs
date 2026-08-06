//! Images attached to a task before an agent files it.
//!
//! The same shape as `files/` and `git.rs`, and for the same reason: writing a
//! couple of megabytes holds no state that anything has to guard, so there is
//! no worker and no queue here — pure functions carrying the tests, and two
//! thin commands over them.
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
//! Nothing here ever deletes. Taking a thumbnail out of the dialog forgets the
//! path and leaves the file; tidying the store is deliberately outside this
//! work, so the directory grows.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Manager};

/// The ceiling, and deliberately not a number of its own: it is the one the
/// editor already refuses to open a file above. Two answers to "how big is too
/// big" in one app would be two numbers to keep in step.
pub use crate::files::model::MAX_FILE_BYTES as MAX_IMAGE_BYTES;

/// The directory under `app_data_dir()` everything lands in.
const STORE: &str = "attachments";

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

/// Somewhere to put it. Created on the way, since the very first attachment on
/// a machine arrives before the directory exists.
fn store_dir(app: &AppHandle) -> Result<PathBuf, AttachmentError> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|err| AttachmentError::Io(format!("no app data directory: {err}")))?
        .join(STORE);
    std::fs::create_dir_all(&dir)
        .map_err(|err| AttachmentError::Io(format!("{}: {err}", dir.display())))?;
    Ok(dir)
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
    let label = original.unwrap_or("the pasted image").to_owned();
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
/// mistake is refused without being read into memory.
#[tauri::command]
pub async fn attachment_import(app: AppHandle, path: String) -> Result<Attachment, AttachmentError> {
    let dir = store_dir(&app)?;
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
    save_into(&dir, name.as_deref(), bytes, &stamp())
}

/// Bytes the webview is holding — a paste, and nothing else can produce them:
/// the clipboard exists in the page and nowhere this process can reach it.
///
/// base64 rather than a `Vec<u8>`, for the reason `terminal/service.rs` records
/// on the way out: a byte array crosses this boundary as a JSON array of
/// numbers, which for a screenshot is several times the size of the picture.
#[tauri::command]
pub async fn attachment_write(
    app: AppHandle,
    name: Option<String>,
    data: String,
) -> Result<Attachment, AttachmentError> {
    let dir = store_dir(&app)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|err| AttachmentError::Io(format!("the pasted image did not decode: {err}")))?;
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

    #[test]
    fn every_refusal_travels_as_a_kind_and_a_readable_line() {
        let too_large = AttachmentError::TooLarge { name: "huge.png".into(), bytes: 9_000_000 };
        let json = serde_json::to_value(&too_large).unwrap();
        assert_eq!(json["kind"], "tooLarge");
        let message = json["message"].as_str().unwrap();
        assert!(message.contains("huge.png"), "{message}");
        assert!(message.contains(&MAX_IMAGE_BYTES.to_string()), "the ceiling is the actionable half: {message}");

        assert_eq!(AttachmentError::NotAnImage("a.txt".into()).kind(), "notAnImage");
        assert_eq!(AttachmentError::Io("x".into()).kind(), "io");
    }
}
