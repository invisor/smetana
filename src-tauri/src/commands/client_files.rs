//! What a client does with files on **its own** disk when the server is not on this machine.
//!
//! With a remote server a path picked in a dialog or dropped on a window names a file the server
//! cannot open (`docs/superpowers/specs/2026-10-25-headless-server-design.md`, "Things that name a path
//! on the client"). Three commands, all of them this machine's and none of them forwarded:
//!
//! - `client_file_read` reads one file here, so the front end can send its bytes through
//!   `attachment_write`. The ceiling is the attachment store's, and the front end checks it first.
//! - `client_open_document` opens a report that is already a file here (a local server) in the
//!   system's browser.
//! - `client_open_fetched` writes the bytes of a report the server sent to a temporary file and opens
//!   that.
//!
//! The two openers take an HTML document and nothing else: they hand a path to whatever the system
//! has registered for it, so a command that opened any path would open an executable as readily.

// The release registers the three commands (the lists that pair them with the server's are held equal
// by tests) but they refuse there, which leaves the helpers behind them unused in that build.
#![cfg_attr(not(feature = "split"), allow(dead_code))]

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::Serialize;
use smetana_core::attachments::MAX_ATTACHMENT_BYTES;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientFile {
    pub name: String,
    pub bytes: u64,
    /// base64.
    pub data: String,
}

/// Reads a file on this machine for upload.
pub fn read_file(path: &Path) -> Result<ClientFile, String> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let meta = std::fs::metadata(path).map_err(|err| format!("{}: {err}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{name} is a folder, not a file"));
    }
    if meta.len() > MAX_ATTACHMENT_BYTES {
        return Err(format!("{name} is {} bytes; the ceiling is {MAX_ATTACHMENT_BYTES} bytes", meta.len()));
    }
    let bytes = std::fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(ClientFile { name, bytes: bytes.len() as u64, data: base64::engine::general_purpose::STANDARD.encode(bytes) })
}

/// What the build without `split` says to the three commands below: they exist only for a client of a
/// server on another machine, and the release has its back end in this process. The commands are
/// registered in both builds (the lists that pair them with the server's are held equal by tests), so
/// the release's copy refuses rather than reading or opening anything.
#[cfg(not(feature = "split"))]
const NOT_A_CLIENT: &str = "This build has no server on another machine, so there is nothing to send or fetch.";

#[tauri::command]
pub async fn client_file_read(path: String) -> Result<ClientFile, String> {
    #[cfg(not(feature = "split"))]
    {
        let _ = path;
        Err(NOT_A_CLIENT.to_owned())
    }
    #[cfg(feature = "split")]
    tokio_blocking(move || read_file(Path::new(&path))).await
}

async fn tokio_blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|err| err.to_string())?
}

/// Whether `name` ends in an extension this command is willing to hand to the system.
pub fn is_html_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".html") || lower.ends_with(".htm")
}

/// A file name that is safe to create: its last segment, ASCII letters, digits and `-_.` only, and
/// an `.html` ending. `None` for a name that is not a document at all.
pub fn temp_file_name(name: &str) -> Option<String> {
    let last = name.rsplit(['/', '\\']).next().unwrap_or(name);
    if !is_html_name(last) {
        return None;
    }
    let cleaned: String = last
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') { ch } else { '-' })
        .collect();
    let cleaned = cleaned.trim_start_matches('.').to_owned();
    is_html_name(&cleaned).then_some(cleaned)
}

const TEMP_PARENT: &str = "smetana-fetched";
/// A fetched file left in the temporary folder this long is removed the next time one is written.
const KEEP: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Removes what earlier calls left behind and is old enough to go.
fn sweep(parent: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else { return };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|modified| modified.elapsed().ok())
            .is_some_and(|age| age > KEEP);
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Writes `bytes` into a new private folder under `parent` and answers the file's path.
pub fn write_fetched(parent: &Path, name: &str, data: &str) -> Result<PathBuf, String> {
    let name = temp_file_name(name).ok_or_else(|| "Only an HTML document can be opened this way.".to_owned())?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.as_bytes())
        .map_err(|err| format!("the document did not decode: {err}"))?;
    std::fs::create_dir_all(parent).map_err(|err| format!("{}: {err}", parent.display()))?;
    sweep(parent);
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let dir = parent.join(format!("{}-{stamp}", std::process::id()));
    create_private_dir(&dir)?;
    let file = dir.join(name);
    std::fs::write(&file, bytes).map_err(|err| format!("{}: {err}", file.display()))?;
    Ok(file)
}

fn create_private_dir(dir: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new().mode(0o700).create(dir).map_err(|err| format!("{}: {err}", dir.display()))
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))
    }
}

fn open(path: &Path) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|err| err.to_string())
}

/// Opens an HTML document that is a file on this machine.
#[tauri::command]
pub async fn client_open_document(path: String) -> Result<(), String> {
    #[cfg(not(feature = "split"))]
    {
        let _ = path;
        return Err(NOT_A_CLIENT.to_owned());
    }
    #[cfg(feature = "split")]
    open_document(&path)
}

fn open_document(path: &str) -> Result<(), String> {
    let path = PathBuf::from(path);
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    if !is_html_name(&name) {
        return Err("Only an HTML document can be opened this way.".to_owned());
    }
    if !path.is_file() {
        return Err(format!("{} is not a file on this machine.", path.display()));
    }
    open(&path)
}

/// Opens a document the server sent: written to a file under the app's cache folder first.
#[tauri::command]
pub async fn client_open_fetched(app: tauri::AppHandle, name: String, data: String) -> Result<(), String> {
    #[cfg(not(feature = "split"))]
    {
        let _ = (app, name, data);
        Err(NOT_A_CLIENT.to_owned())
    }
    #[cfg(feature = "split")]
    {
        use tauri::Manager;
        // The app's own cache folder, which belongs to this user: a shared /tmp would let another
        // account on the machine read, or plant, a report.
        let parent = app.path().app_cache_dir().map_err(|err| err.to_string())?.join(TEMP_PARENT);
        let file = tokio_blocking(move || write_fetched(&parent, &name, &data)).await?;
        open(&file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_html_names_are_documents() {
        assert!(is_html_name("run.html") && is_html_name("RUN.HTM"));
        for name in ["run.exe", "run.html.sh", "run", "", "app.app", "script.command"] {
            assert!(!is_html_name(name), "{name}");
        }
    }

    #[test]
    fn a_temporary_name_is_one_plain_html_segment() {
        assert_eq!(temp_file_name("run-2026-10-26.html").as_deref(), Some("run-2026-10-26.html"));
        assert_eq!(temp_file_name("/srv/x/../run report.html").as_deref(), Some("run-report.html"));
        assert_eq!(temp_file_name("C:\\a\\b.htm").as_deref(), Some("b.htm"));
        assert_eq!(temp_file_name("../../etc/passwd"), None);
        assert_eq!(temp_file_name("evil.sh"), None);
        assert_eq!(temp_file_name(".html"), None);
        assert_eq!(temp_file_name(""), None);
    }

    #[test]
    fn fetched_bytes_land_in_a_private_folder_of_their_own() {
        let tmp = tempfile::tempdir().unwrap();
        let file = write_fetched(&tmp.path().join("parent"), "r.html", "PGh0bWw+").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "<html>");
        assert!(file.starts_with(tmp.path()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(file.parent().unwrap()).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700);
        }
        // A second document does not overwrite the first.
        let other = write_fetched(&tmp.path().join("parent"), "r.html", "PGI+").unwrap();
        assert_ne!(file, other);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "<html>");
    }

    #[test]
    fn something_that_is_not_a_document_or_not_base64_is_refused_and_writes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let parent = tmp.path().join("parent");
        assert!(write_fetched(&parent, "run.sh", "aGk=").is_err());
        assert!(write_fetched(&parent, "run.html", "!!not base64!!").is_err());
        let left = std::fs::read_dir(&parent).map(|d| d.count()).unwrap_or(0);
        assert_eq!(left, 0);
    }

    #[test]
    fn a_file_is_read_whole_and_a_folder_or_a_huge_file_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("shot.png");
        std::fs::write(&file, b"png").unwrap();
        let read = read_file(&file).unwrap();
        assert_eq!((read.name.as_str(), read.bytes, read.data.as_str()), ("shot.png", 3, "cG5n"));
        assert!(read_file(tmp.path()).unwrap_err().contains("folder"));
        let big = tmp.path().join("big.bin");
        std::fs::write(&big, vec![0u8; MAX_ATTACHMENT_BYTES as usize + 1]).unwrap();
        assert!(read_file(&big).unwrap_err().contains("ceiling"));
        assert!(read_file(&tmp.path().join("missing")).is_err());
    }

    /// The release's copies read and open nothing: a webview of the release must not gain a way to
    /// read a file of this machine that it did not have.
    #[cfg(not(feature = "split"))]
    #[test]
    fn the_release_refuses_all_three() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("secret.html");
        std::fs::write(&file, "x").unwrap();
        let path = file.to_string_lossy().into_owned();
        tauri::async_runtime::block_on(async {
            assert!(client_file_read(path.clone()).await.is_err());
            assert!(client_open_document(path).await.is_err());
        });
    }
}
