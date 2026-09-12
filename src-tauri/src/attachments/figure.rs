//! `image_read`: the bytes behind an illustration in an agent's prose.
//!
//! Beside `attachment_reopen`, which this deliberately mirrors, and for a
//! different question. That command reads a file this app itself wrote, and
//! confines the read to `store_root()` because everything it may legitimately
//! be handed came out of the store to begin with. This one reads a file an
//! *agent* wrote, wherever the agent put it — the session's own worktree, the
//! project root, or a place on the machine with no relation to either — and
//! there is deliberately no `resolve_within` here either, for the same reason
//! `mod.rs`'s own header gives for `attachment_import`: the source belongs to
//! whoever wrote the prose, not to this app, and reading it is exactly what a
//! `![…](src)` node in that prose asked for. What the render rule refuses is
//! narrower and answered before this file ever runs — `figureSource.js`'s own
//! gate keeps a `javascript:` scheme and a remote address out of the markup at
//! all, and `is_network_path` below is this file's own half of the one gate
//! left for a command to enforce on its own account: a network share is not
//! "this machine's disk" whichever door asked for it.
//!
//! **The record is `Attachment`, unchanged.** A path read this way answers
//! with the same `{ path, name, bytes, mime, data }` shape `attachment_reopen`
//! already answers with, so `stores/attachments.js`'s `record()` turns either
//! into the same `{ path, name, bytes, url }` a component draws, and
//! `ImageWindow.vue` does not have to know which of the two commands supplied
//! what it is showing.

use std::path::{Path, PathBuf};

use base64::Engine;

use super::{sniff, Attachment, AttachmentError, MAX_IMAGE_BYTES};

/// Whether `src` opens a network share rather than naming a file on this
/// machine — the UNC form (`\\host\share\…`) and its forward-slash twin
/// (`//host/…`), refused the same way `figureSource.js`'s own gate refuses a
/// protocol-relative `<img src>` before it ever reaches a renderer: a
/// browser's URL resolver treats `\` exactly like `/`, so the two are one
/// hole and not two. Checked on the leading two characters alone, ahead of
/// any join with a base directory — `Path::is_absolute` says nothing about
/// this on its own, since `\\host\p.png` *is* an absolute path on Windows,
/// and this check runs the same way on every platform, string only, so a
/// test does not need a second OS to prove it holds.
///
/// Trimmed first, matching `figureSource.js`'s own `isNetworkPath` exactly —
/// `markdown.js` captures a source as `(\S+?)` on both sides of the wire, so
/// leading or trailing whitespace does not reach either gate from a real
/// `![…](src)` node today, but this function's own doc otherwise claims "one
/// string check on each side of the boundary rather than trusting the other
/// one to have run", and that claim was false for a caller that skipped the
/// front-end gate and handed this one `" \\\\host\\p.png"` untrimmed.
pub fn is_network_path(src: &str) -> bool {
    let mut chars = src.trim().chars();
    let first = chars.next();
    let second = chars.next();
    matches!((first, second), (Some(a), Some(b)) if is_slashy(a) && is_slashy(b))
}

fn is_slashy(ch: char) -> bool {
    ch == '/' || ch == '\\'
}

/// Where `src` actually points: itself, if it already names an absolute
/// location, or joined onto `base` otherwise — the session's own cwd, or the
/// project root with no session behind the reply (`Markdown.vue`'s own
/// header carries the rest of that rule; this file only resolves what it is
/// handed). `figureSource.js`'s gate has already refused a scheme and a
/// network path by the time this runs, so what is left is always meant as a
/// filesystem path — one this app resolves itself, since the webview's own
/// origin has nothing to do with the disk `image_read` reads from.
fn resolve(base: &str, src: &str) -> PathBuf {
    let path = Path::new(src);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        Path::new(base).join(path)
    }
}

/// A file an agent wrote into its own prose, read by path rather than out of
/// the attachment store. `base` is the directory a relative `src` resolves
/// from; `src` itself is taken as-is when it already names an absolute
/// location, exactly the way `markdown.js`'s own image node carries it —
/// unmodified — out of the parser.
#[tauri::command]
pub async fn image_read(base: String, src: String) -> Result<Attachment, AttachmentError> {
    if is_network_path(&src) {
        return Err(AttachmentError::NetworkPath(src));
    }
    let resolved = resolve(&base, &src);
    let label = resolved.display().to_string();
    let meta = std::fs::metadata(&resolved).map_err(|err| AttachmentError::Io(format!("{label}: {err}")))?;
    // A folder is an ordinary thing for a written-by-hand path to name by
    // mistake, and reading one answers with the operating system's own
    // wording about directories; the refusal a person can act on is the one
    // about pictures, the same choice `attachment_import` makes.
    if !meta.is_file() {
        return Err(AttachmentError::NotAnImage(label));
    }
    if meta.len() > MAX_IMAGE_BYTES {
        return Err(AttachmentError::TooLarge { name: label, bytes: meta.len() });
    }
    let bytes = std::fs::read(&resolved).map_err(|err| AttachmentError::Io(format!("{label}: {err}")))?;
    let Some(format) = sniff(&bytes) else {
        return Err(AttachmentError::NotAnImage(label));
    };
    let name = resolved
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or(label);
    Ok(Attachment {
        path: resolved.to_string_lossy().into_owned(),
        name,
        bytes: bytes.len() as u64,
        mime: format.mime().to_owned(),
        data: base64::engine::general_purpose::STANDARD.encode(&bytes),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("smetana-figure-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create the temp directory");
        dir
    }

    #[test]
    fn a_unc_path_is_refused_before_it_ever_touches_the_disk() {
        assert!(is_network_path("\\\\host\\p.png"));
        assert!(is_network_path("//host/p.png"));
        // Mixed on purpose: a browser's own resolver does not care which of
        // the two supplied either half of the leading pair.
        assert!(is_network_path("/\\host\\p.png"));
        assert!(is_network_path("\\/host/p.png"));
    }

    #[test]
    fn a_unc_path_is_still_caught_with_leading_or_trailing_whitespace() {
        // The front-end gate trims before testing; this one has to as well,
        // or a caller that reached `image_read` without going through
        // `figureSource.js` first — the one this doc comment claims cannot
        // happen — could still slip a UNC path past it on a single space.
        assert!(is_network_path(" \\\\host\\p.png"));
        assert!(is_network_path("\\\\host\\p.png\t"));
        assert!(is_network_path("  //host/p.png  "));
    }

    #[test]
    fn an_ordinary_path_is_not_a_network_path() {
        assert!(!is_network_path("./a.png"));
        assert!(!is_network_path("/Users/ada/fig.png"));
        assert!(!is_network_path("C:\\Users\\ada\\fig.png"));
    }

    #[tokio::test]
    async fn a_network_path_is_refused_by_the_command_itself() {
        let err = image_read("/anywhere".into(), "\\\\host\\p.png".into()).await;
        assert!(matches!(err, Err(AttachmentError::NetworkPath(_))), "{err:?}");
    }

    #[tokio::test]
    async fn a_relative_source_resolves_against_the_base() {
        let dir = scratch("relative");
        std::fs::write(dir.join("fig.png"), PNG).unwrap();

        let record = image_read(dir.to_string_lossy().into_owned(), "./fig.png".into())
            .await
            .expect("a picture beside the base is read");

        assert_eq!(record.name, "fig.png");
        assert!(Path::new(&record.path).is_absolute(), "{}", record.path);
        assert_eq!(record.mime, "image/png");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn an_absolute_source_ignores_the_base_entirely() {
        let dir = scratch("absolute");
        let elsewhere = scratch("absolute-elsewhere");
        std::fs::write(elsewhere.join("fig.png"), PNG).unwrap();
        let absolute = elsewhere.join("fig.png").to_string_lossy().into_owned();

        let record = image_read(dir.to_string_lossy().into_owned(), absolute.clone())
            .await
            .expect("an absolute path is read as-is");

        assert_eq!(record.path, absolute);

        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&elsewhere);
    }

    #[tokio::test]
    async fn a_missing_file_is_refused_with_its_path_in_the_message() {
        let dir = scratch("missing");

        let err = image_read(dir.to_string_lossy().into_owned(), "nope.png".into()).await;

        match err {
            Err(AttachmentError::Io(message)) => assert!(message.contains("nope.png"), "{message}"),
            other => panic!("expected Io, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_file_that_is_not_a_picture_is_refused_by_its_bytes() {
        let dir = scratch("not-an-image");
        std::fs::write(dir.join("notes.txt"), b"just text").unwrap();

        let err = image_read(dir.to_string_lossy().into_owned(), "notes.txt".into()).await;

        assert!(matches!(err, Err(AttachmentError::NotAnImage(_))), "{err:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn an_oversized_file_is_refused_from_its_metadata_alone() {
        let dir = scratch("too-large");
        let mut huge = PNG.to_vec();
        huge.resize(MAX_IMAGE_BYTES as usize + 1, 0);
        std::fs::write(dir.join("huge.png"), &huge).unwrap();

        let err = image_read(dir.to_string_lossy().into_owned(), "huge.png".into()).await;

        assert!(matches!(err, Err(AttachmentError::TooLarge { .. })), "{err:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn a_directory_is_refused_as_not_a_picture_rather_than_read() {
        let dir = scratch("a-directory");
        std::fs::create_dir(dir.join("sub")).unwrap();

        let err = image_read(dir.to_string_lossy().into_owned(), "sub".into()).await;

        assert!(matches!(err, Err(AttachmentError::NotAnImage(_))), "{err:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
