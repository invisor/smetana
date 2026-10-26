//! Taking over the data a desktop that held its workers in-process had kept, once.
//!
//! The release before the client-server switch kept the device key, the list of paired devices and the
//! attachment store in the app's own data directory. After the switch the local server keeps them in a
//! directory of its own (`--data-dir`, which the shell passes and the server never guesses), so
//! without this a phone paired with the old release would find a server with a different key, and
//! every device would have to be paired again. `smetanad run --inherit-from <dir>` names the old
//! directory; this module reads from it and writes into the server's own.
//!
//! It runs at the start of [`crate::server::Server::start`], after the data directory's lock is held:
//! the one process that can be writing there is this one, so there is no second server to race with.
//!
//! What it takes, and the rules it keeps:
//!
//! - **`remote-key`**, the identity every paired phone trusts. Copied, never regenerated: the server's
//!   node id is then the one the phones already know. Only when the server has no key yet; one that
//!   has its own is never replaced. A source key that does not parse is an error, not a reason to
//!   start with a fresh one — that would unpair every phone without a word.
//! - **`remote-devices.json`**, the paired devices. Copied only together with the key they are bound
//!   to: when the server already holds a different key the list would name devices that cannot
//!   connect, and is left where it is.
//! - **`attachments/`**, the pictures issues name by absolute path. Mirrored, with a hard link where
//!   the file system allows one and a copy where it does not. The files are not moved, because the
//!   old paths are written into issues and agents go on reading them there; anything the server's
//!   store already has is kept.
//!
//! What it does not take, because it is not in this directory: the runs' registry is
//! `.smetana/runs.json` in each project, and the session transcripts are Claude Code's, under
//! `~/.claude/projects`. Neither moves with the server. The server half of `settings.json` is handed
//! over by the shell through `settings_adopt` (`src-tauri/src/client/migrate.rs`), since only the shell
//! knows which file is the device's.
//!
//! **Nothing is removed from the source**, so a failure leaves it exactly as it was and an older
//! release still finds its own data. Every write is a temporary file renamed into place, so an
//! interrupted run leaves no half-written key. The marker [`MARKER`] is written last, after
//! everything else succeeded; until it is there the next start does the work again, and
//! each step skips what is already done. A failure is an error that stops the start: starting with
//! some of the data would be starting as a different server.

use std::fs;
use std::path::Path;

use remote_server::{devices, endpoint};
use serde::Serialize;

/// What says the inheritance is over. Its absence is the only thing that makes the next start try.
pub const MARKER: &str = "inherited.json";

/// What a run of [`inherit`] did, for the log and for the marker.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The source directory.
    pub from: String,
    /// The key was copied from the source (and not found there already or absent from it).
    pub key: bool,
    /// The device list was copied.
    pub devices: bool,
    /// Attachment files that are now in the server's store because of this run.
    pub attachments: u64,
    /// The run had already been done, or there was nothing to do.
    pub skipped: Option<&'static str>,
}

/// Takes over what `from` holds, into `into`. Idempotent: a second call, or one after the marker is
/// written, changes nothing.
pub fn inherit(from: &Path, into: &Path) -> Result<Report, String> {
    let mut report = Report { from: from.display().to_string(), ..Report::default() };
    if into.join(MARKER).exists() {
        report.skipped = Some("already done");
        return Ok(report);
    }
    let same = match (from.canonicalize(), into.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if same {
        // The server's directory *is* the old one: there is nothing to take from itself.
        report.skipped = Some("the source is the server's own directory");
        return Ok(report);
    }
    if !from.is_dir() {
        // A fresh install: no old release left anything. The marker is still written, so that a
        // directory that appears later is not mistaken for an old release's.
        report.skipped = Some("no source directory");
        write_marker(into, &report)?;
        return Ok(report);
    }

    let source_key = from.join(endpoint::KEY_FILE);
    let own_key = into.join(endpoint::KEY_FILE);
    if source_key.is_file() && !own_key.exists() {
        let text = fs::read_to_string(&source_key).map_err(|err| format!("{}: {err}", source_key.display()))?;
        // Parsed before anything is written: a damaged key is the one thing this must not paper over.
        let temp = into.join(format!("{}.inheriting", endpoint::KEY_FILE));
        let _ = fs::remove_file(&temp);
        validate_key(&text, &source_key)?;
        endpoint::write_private(&temp, text.trim().as_bytes())?;
        fs::rename(&temp, &own_key).map_err(|err| {
            let _ = fs::remove_file(&temp);
            format!("{}: {err}", own_key.display())
        })?;
        report.key = true;
    }

    let source_devices = from.join(devices::FILE_NAME);
    let own_devices = into.join(devices::FILE_NAME);
    if source_devices.is_file() && !own_devices.exists() && same_key(&source_key, &own_key) {
        let bytes = fs::read(&source_devices).map_err(|err| format!("{}: {err}", source_devices.display()))?;
        devices::write_atomic(&own_devices, &bytes)?;
        report.devices = true;
    }

    let source_store = from.join("attachments");
    if source_store.is_dir() {
        report.attachments = mirror(&source_store, &into.join("attachments"))?;
    }

    write_marker(into, &report)?;
    Ok(report)
}

fn validate_key(text: &str, path: &Path) -> Result<(), String> {
    let bytes = data_encoding::HEXLOWER
        .decode(text.trim().as_bytes())
        .map_err(|err| format!("{}: {err}", path.display()))?;
    if bytes.len() != 32 {
        return Err(format!("{}: not a 32 byte key", path.display()));
    }
    Ok(())
}

/// Both files exist and say the same thing: the list belongs to this server's identity.
fn same_key(source: &Path, own: &Path) -> bool {
    match (fs::read_to_string(source), fs::read_to_string(own)) {
        (Ok(a), Ok(b)) => a.trim() == b.trim(),
        _ => false,
    }
}

/// Every regular file under `from`, into the same relative place under `to`, unless something is
/// already there. Symbolic links are not followed or copied. Returns how many files were added.
fn mirror(from: &Path, to: &Path) -> Result<u64, String> {
    fs::create_dir_all(to).map_err(|err| format!("{}: {err}", to.display()))?;
    let mut added = 0;
    for entry in fs::read_dir(from).map_err(|err| format!("{}: {err}", from.display()))? {
        let entry = entry.map_err(|err| format!("{}: {err}", from.display()))?;
        let kind = entry.file_type().map_err(|err| format!("{}: {err}", entry.path().display()))?;
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            added += mirror(&entry.path(), &target)?;
        } else if kind.is_file() && !target.exists() {
            // Linked when possible, which costs no space, and copied otherwise. Written under a
            // temporary name and renamed, so an interrupted copy is never taken for a whole file.
            if fs::hard_link(entry.path(), &target).is_err() {
                let temp = to.join(format!(".{}.inheriting", entry.file_name().to_string_lossy()));
                fs::copy(entry.path(), &temp).map_err(|err| format!("{}: {err}", entry.path().display()))?;
                fs::rename(&temp, &target).map_err(|err| {
                    let _ = fs::remove_file(&temp);
                    format!("{}: {err}", target.display())
                })?;
            }
            added += 1;
        }
    }
    Ok(added)
}

fn write_marker(into: &Path, report: &Report) -> Result<(), String> {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    let body = serde_json::json!({ "version": 1, "at": at, "report": report });
    let text = serde_json::to_vec_pretty(&body).map_err(|err| err.to_string())?;
    devices::write_atomic(&into.join(MARKER), &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";

    fn old_release(dir: &Path) {
        fs::write(dir.join(endpoint::KEY_FILE), KEY).unwrap();
        fs::write(
            dir.join(devices::FILE_NAME),
            r#"{"version":1,"devices":[{"key":"81a3","name":"Pixel","platform":"android","push_token":null,"push_key":"59","paired_at":1,"last_contact":2}]}"#,
        )
        .unwrap();
        fs::create_dir_all(dir.join("attachments/smetana-abc")).unwrap();
        fs::write(dir.join("attachments/smetana-abc/shot.png"), b"png").unwrap();
        fs::write(dir.join("attachments/loose.png"), b"loose").unwrap();
        fs::write(dir.join("settings.json"), "{}").unwrap();
        fs::write(dir.join(".window-state.json"), "{}").unwrap();
    }

    fn dirs() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let root = tempfile::tempdir().unwrap();
        let old = root.path().join("old");
        let new = root.path().join("old/server");
        fs::create_dir_all(&new).unwrap();
        (root, old, new)
    }

    #[test]
    fn the_key_the_phones_trust_becomes_the_servers_and_the_node_id_does_not_change() {
        let (_root, old, new) = dirs();
        old_release(&old);
        let before = endpoint::load_or_create_key(&old.join(endpoint::KEY_FILE)).unwrap().public();
        let report = inherit(&old, &new).unwrap();
        assert!(report.key && report.devices);
        let after = endpoint::load_or_create_key(&new.join(endpoint::KEY_FILE)).unwrap().public();
        assert_eq!(before, after, "the server must be the node the paired phones already know");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(new.join(endpoint::KEY_FILE)).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "nobody else may read the key");
        }
    }

    #[test]
    fn paired_devices_survive_with_their_kind() {
        let (_root, old, new) = dirs();
        old_release(&old);
        inherit(&old, &new).unwrap();
        let list = devices::Devices::load(new.join(devices::FILE_NAME)).all();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Pixel");
        assert_eq!(list[0].kind, remote_protocol::DeviceKind::Phone, "an entry with no kind is a phone");
        assert_eq!(list[0].last_contact, Some(2));
    }

    #[test]
    fn attachments_are_in_both_places_and_nothing_is_taken_from_the_old_one() {
        let (_root, old, new) = dirs();
        old_release(&old);
        let report = inherit(&old, &new).unwrap();
        assert_eq!(report.attachments, 2);
        assert_eq!(fs::read(new.join("attachments/smetana-abc/shot.png")).unwrap(), b"png");
        assert_eq!(fs::read(new.join("attachments/loose.png")).unwrap(), b"loose");
        for kept in ["attachments/smetana-abc/shot.png", "remote-key", "remote-devices.json", "settings.json", ".window-state.json"] {
            assert!(old.join(kept).exists(), "{kept} stays where the old release left it");
        }
        assert!(!new.join("settings.json").exists(), "the settings halves are the shell's to hand over");
        assert!(!new.join(".window-state.json").exists(), "the device's own files are not the server's");
    }

    #[test]
    fn it_is_done_once_and_a_second_call_changes_nothing() {
        let (_root, old, new) = dirs();
        old_release(&old);
        inherit(&old, &new).unwrap();
        assert!(new.join(MARKER).exists());
        // Something the server did since: a new attachment, and a new key in the source that must not win.
        fs::write(new.join("attachments/later.png"), b"later").unwrap();
        fs::write(old.join(endpoint::KEY_FILE), "ff".repeat(32)).unwrap();
        let again = inherit(&old, &new).unwrap();
        assert_eq!(again.skipped, Some("already done"));
        assert_eq!(fs::read_to_string(new.join(endpoint::KEY_FILE)).unwrap(), KEY);
        assert!(new.join("attachments/later.png").exists());
    }

    #[test]
    fn an_interrupted_run_is_finished_by_the_next_without_redoing_or_losing_anything() {
        let (_root, old, new) = dirs();
        old_release(&old);
        // The state after a crash between the key and the rest: a key in place, no marker.
        fs::write(new.join(endpoint::KEY_FILE), KEY).unwrap();
        fs::create_dir_all(new.join("attachments/smetana-abc")).unwrap();
        fs::write(new.join("attachments/smetana-abc/shot.png"), b"already here").unwrap();
        let report = inherit(&old, &new).unwrap();
        assert!(!report.key, "a key that is there is not copied again");
        assert!(report.devices, "the list follows the key it belongs to");
        assert_eq!(report.attachments, 1, "only the file that was missing");
        assert_eq!(fs::read(new.join("attachments/smetana-abc/shot.png")).unwrap(), b"already here");
        assert!(new.join(MARKER).exists());
    }

    #[test]
    fn a_server_with_its_own_key_keeps_it_and_does_not_take_a_list_bound_to_another() {
        let (_root, old, new) = dirs();
        old_release(&old);
        let own = "ab".repeat(32);
        fs::write(new.join(endpoint::KEY_FILE), &own).unwrap();
        let report = inherit(&old, &new).unwrap();
        assert!(!report.key && !report.devices);
        assert_eq!(fs::read_to_string(new.join(endpoint::KEY_FILE)).unwrap(), own);
        assert!(!new.join(devices::FILE_NAME).exists());
    }

    #[test]
    fn a_damaged_key_stops_the_start_instead_of_unpairing_every_phone() {
        let (_root, old, new) = dirs();
        old_release(&old);
        fs::write(old.join(endpoint::KEY_FILE), "nonsense").unwrap();
        assert!(inherit(&old, &new).is_err());
        assert!(!new.join(endpoint::KEY_FILE).exists(), "no fresh key is made in its place");
        assert!(!new.join(MARKER).exists(), "and the next start tries again");
    }

    #[test]
    fn a_machine_with_no_old_release_has_nothing_to_take_and_is_marked_so() {
        let (root, _old, new) = dirs();
        let report = inherit(&root.path().join("nowhere"), &new).unwrap();
        assert_eq!(report.skipped, Some("no source directory"));
        assert!(new.join(MARKER).exists());
        assert!(!new.join(endpoint::KEY_FILE).exists());
    }

    #[test]
    fn a_source_that_is_the_servers_own_directory_is_left_alone() {
        let (_root, _old, new) = dirs();
        let report = inherit(&new, &new).unwrap();
        assert_eq!(report.skipped, Some("the source is the server's own directory"));
        assert!(!new.join(MARKER).exists());
    }
}
