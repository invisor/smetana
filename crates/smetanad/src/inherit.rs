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
//!   has its own is never replaced. What happens when the key cannot be taken is below.
//! - **`remote-devices.json`**, the paired devices. Merged into the server's list (a device the list
//!   already holds is kept as it is) only together with the key they are bound to: when the server
//!   holds a different key the list would name devices that cannot connect, and is left where it is.
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
//! interrupted run leaves no half-written key.
//!
//! **A failure never stops the server.** The app cannot reach a server that will not start, and it
//! would try again for ever; a person with a working release and one damaged file would lose the whole
//! app over it. So each step that fails is a line in [`Report::problems`] and the start goes on:
//!
//! - a key that does not parse was useless to the old release too (it ran without phones), so the
//!   server makes a key of its own and the list bound to the broken one is left where it is;
//! - a key that could not be *read* or *written* this time may be next time, so the report says so
//!   ([`Report::key_pending`]) and the server serves this run under a key it does not keep
//!   (`Server::start`): writing one would take the place of the key the phones trust for good;
//! - an attachment that could not be mirrored is skipped and the rest are not.
//!
//! The marker [`MARKER`] is written last, and **only when nothing failed**; until it is there each
//! start does the work again, and each step skips what is already done.
//!
//! It also reads which relay the old release paired its phones through (`remote.relayUrl` in the old
//! `settings.json`), into [`Report::relay`] and the marker. The server binds the public endpoint at
//! start when an inherited phone is on the list, and at that moment its own `settings.json` does not
//! exist yet (the shell hands the server half over only once it is connected), so without this a
//! person with a relay of their own would be served on the default one until the next restart.

use std::fs;
use std::path::Path;

use remote_server::{devices, endpoint};
use serde::Serialize;
use serde_json::Value;

/// What says the inheritance is over. Its absence is the only thing that makes the next start try.
pub const MARKER: &str = "inherited.json";

/// What a run of [`inherit`] did, for the log and for the marker.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The source directory.
    pub from: String,
    /// The key was copied from the source (and not found there already or absent from it).
    pub key: bool,
    /// The source has a key that could not be taken this time and might be next time: the server must
    /// not make and keep a key of its own now.
    pub key_pending: bool,
    /// Devices from the source list that are now on the server's.
    pub devices: u64,
    /// Attachment files that are now in the server's store because of this run.
    pub attachments: u64,
    /// The relay the old release's settings named, when it had a settings file.
    pub relay: Option<String>,
    /// Every step that failed, in words. Not empty means no marker, and the next start tries again.
    pub problems: Vec<String>,
    /// The run had already been done, or there was nothing to do.
    pub skipped: Option<&'static str>,
}

/// Takes over what `from` holds, into `into`. Idempotent: a second call, or one after the marker is
/// written, changes nothing. Never fails; what went wrong is in the report.
pub fn inherit(from: &Path, into: &Path) -> Report {
    let mut report = Report { from: from.display().to_string(), ..Report::default() };
    if into.join(MARKER).exists() {
        report.skipped = Some("already done");
        return report;
    }
    let same = match (from.canonicalize(), into.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if same {
        // The server's directory *is* the old one: there is nothing to take from itself.
        report.skipped = Some("the source is the server's own directory");
        return report;
    }
    if !from.is_dir() {
        // A fresh install: no old release left anything. The marker is still written, so that a
        // directory that appears later is not mistaken for an old release's.
        report.skipped = Some("no source directory");
        if let Err(err) = write_marker(into, &report) {
            report.problems.push(err);
        }
        return report;
    }

    let old_settings = from.join("settings.json");
    if old_settings.is_file() {
        report.relay = Some(smetana_core::settings::file::load(&old_settings).0.remote.relay_url);
    }

    let source_key = from.join(endpoint::KEY_FILE);
    let own_key = into.join(endpoint::KEY_FILE);
    if source_key.is_file() && !own_key.exists() {
        match take_key(&source_key, &own_key) {
            Ok(()) => report.key = true,
            Err(KeyProblem::Damaged(why)) => report.problems.push(format!(
                "{why}; the old release could not use this key either, so the server makes its own and phones paired with the old one pair again"
            )),
            Err(KeyProblem::Unavailable(why)) => {
                report.key_pending = true;
                report.problems.push(format!("{why}; the next start tries again"));
            }
        }
    }

    let source_devices = from.join(devices::FILE_NAME);
    let own_devices = into.join(devices::FILE_NAME);
    if source_devices.is_file() && same_key(&source_key, &own_key) {
        match merge_devices(&source_devices, &own_devices) {
            Ok(added) => report.devices = added,
            Err(err) => report.problems.push(err),
        }
    }

    let source_store = from.join("attachments");
    if source_store.is_dir() {
        report.attachments = mirror(&source_store, &into.join("attachments"), &mut report.problems);
    }

    if report.problems.is_empty() {
        if let Err(err) = write_marker(into, &report) {
            report.problems.push(err);
        }
    }
    report
}

/// The relay an earlier, finished inheritance read from the old release's settings: for the starts
/// after the one that did the work, until the server has a `settings.json` of its own.
pub fn relay_hint(into: &Path) -> Option<String> {
    let text = fs::read_to_string(into.join(MARKER)).ok()?;
    let marker: Value = serde_json::from_str(&text).ok()?;
    marker["report"]["relay"].as_str().map(str::to_owned)
}

enum KeyProblem {
    /// The bytes are there and are not a key.
    Damaged(String),
    /// The key could not be read or written this time.
    Unavailable(String),
}

fn take_key(source: &Path, own: &Path) -> Result<(), KeyProblem> {
    let text = fs::read_to_string(source).map_err(|err| KeyProblem::Unavailable(format!("{}: {err}", source.display())))?;
    // Parsed before anything is written: a damaged key is the one thing this must not paper over.
    validate_key(&text, source).map_err(KeyProblem::Damaged)?;
    let temp = own.with_file_name(format!("{}.inheriting", endpoint::KEY_FILE));
    let _ = fs::remove_file(&temp);
    endpoint::write_private(&temp, text.trim().as_bytes()).map_err(KeyProblem::Unavailable)?;
    fs::rename(&temp, own).map_err(|err| {
        let _ = fs::remove_file(&temp);
        KeyProblem::Unavailable(format!("{}: {err}", own.display()))
    })
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

/// Adds every device of the source list that the server's list does not hold (by key) to it, and keeps
/// what the server's list already has: a run that served under a key it did not keep may have paired
/// this machine's own client before the inherited phones could arrive. Answers how many were added.
fn merge_devices(source: &Path, own: &Path) -> Result<u64, String> {
    let read = |path: &Path| -> Result<Value, String> {
        let text = fs::read_to_string(path).map_err(|err| format!("{}: {err}", path.display()))?;
        serde_json::from_str(&text).map_err(|err| format!("{}: {err}", path.display()))
    };
    let theirs = read(source)?;
    let Some(incoming) = theirs["devices"].as_array() else {
        return Err(format!("{}: no device list in it", source.display()));
    };
    let mut ours = if own.exists() { read(own)? } else { serde_json::json!({ "version": theirs["version"], "devices": [] }) };
    let Some(list) = ours["devices"].as_array_mut() else {
        return Err(format!("{}: no device list in it", own.display()));
    };
    let mut added = 0;
    for device in incoming {
        if !list.iter().any(|known| known["key"] == device["key"]) {
            list.push(device.clone());
            added += 1;
        }
    }
    if added > 0 {
        let text = serde_json::to_vec_pretty(&ours).map_err(|err| err.to_string())?;
        devices::write_atomic(own, &text)?;
    }
    Ok(added)
}

/// Every regular file under `from`, into the same relative place under `to`, unless something is
/// already there. Symbolic links are not followed or copied. A file or folder that cannot be taken is
/// a line in `problems` and the rest go on. Returns how many files were added.
fn mirror(from: &Path, to: &Path, problems: &mut Vec<String>) -> u64 {
    if let Err(err) = fs::create_dir_all(to) {
        problems.push(format!("{}: {err}", to.display()));
        return 0;
    }
    let entries = match fs::read_dir(from) {
        Ok(entries) => entries,
        Err(err) => {
            problems.push(format!("{}: {err}", from.display()));
            return 0;
        }
    };
    let mut added = 0;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                problems.push(format!("{}: {err}", from.display()));
                continue;
            }
        };
        let kind = match entry.file_type() {
            Ok(kind) => kind,
            Err(err) => {
                problems.push(format!("{}: {err}", entry.path().display()));
                continue;
            }
        };
        let target = to.join(entry.file_name());
        if kind.is_dir() {
            added += mirror(&entry.path(), &target, problems);
        } else if kind.is_file() && !target.exists() {
            match link_or_copy(&entry.path(), to, &target) {
                Ok(()) => added += 1,
                Err(err) => problems.push(err),
            }
        }
    }
    added
}

/// Linked when possible, which costs no space, and copied otherwise. Written under a temporary name
/// and renamed, so an interrupted copy is never taken for a whole file.
fn link_or_copy(source: &Path, dir: &Path, target: &Path) -> Result<(), String> {
    if fs::hard_link(source, target).is_ok() {
        return Ok(());
    }
    let name = target.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let temp = dir.join(format!(".{name}.inheriting"));
    fs::copy(source, &temp).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("{}: {err}", source.display())
    })?;
    fs::rename(&temp, target).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("{}: {err}", target.display())
    })
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
        fs::write(dir.join("settings.json"), r#"{"version":1,"remote":{"relayUrl":"https://relay.example.org"}}"#).unwrap();
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
        let report = inherit(&old, &new);
        assert!(report.key && report.devices == 1, "{report:?}");
        assert!(report.problems.is_empty() && !report.key_pending);
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
        inherit(&old, &new);
        let list = devices::Devices::load(new.join(devices::FILE_NAME)).all();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Pixel");
        assert_eq!(list[0].kind, remote_protocol::DeviceKind::Phone, "an entry with no kind is a phone");
        assert_eq!(list[0].last_contact, Some(2));
    }

    #[test]
    fn the_relay_the_old_release_paired_through_is_read_and_kept_in_the_marker() {
        let (_root, old, new) = dirs();
        old_release(&old);
        let report = inherit(&old, &new);
        assert_eq!(report.relay.as_deref(), Some("https://relay.example.org"));
        assert_eq!(relay_hint(&new).as_deref(), Some("https://relay.example.org"));
    }

    #[test]
    fn attachments_are_in_both_places_and_nothing_is_taken_from_the_old_one() {
        let (_root, old, new) = dirs();
        old_release(&old);
        let report = inherit(&old, &new);
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
        inherit(&old, &new);
        assert!(new.join(MARKER).exists());
        // Something the server did since: a new attachment, and a new key in the source that must not win.
        fs::write(new.join("attachments/later.png"), b"later").unwrap();
        fs::write(old.join(endpoint::KEY_FILE), "ff".repeat(32)).unwrap();
        let again = inherit(&old, &new);
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
        let report = inherit(&old, &new);
        assert!(!report.key, "a key that is there is not copied again");
        assert_eq!(report.devices, 1, "the list follows the key it belongs to");
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
        let report = inherit(&old, &new);
        assert!(!report.key && report.devices == 0);
        assert_eq!(fs::read_to_string(new.join(endpoint::KEY_FILE)).unwrap(), own);
        assert!(!new.join(devices::FILE_NAME).exists());
    }

    #[test]
    fn a_damaged_key_is_a_problem_and_not_a_stop_and_the_rest_is_still_taken() {
        let (_root, old, new) = dirs();
        old_release(&old);
        fs::write(old.join(endpoint::KEY_FILE), "nonsense").unwrap();
        let report = inherit(&old, &new);
        assert!(!report.key && !report.key_pending, "nothing to wait for: the bytes are not a key");
        assert_eq!(report.problems.len(), 1, "{report:?}");
        assert!(!new.join(endpoint::KEY_FILE).exists(), "inheriting makes no key; the server makes its own");
        assert!(!new.join(devices::FILE_NAME).exists(), "a list bound to a broken key is not taken");
        assert_eq!(report.attachments, 2, "the pictures are taken all the same");
        assert!(!new.join(MARKER).exists(), "a failed step leaves the next start to look again");
    }

    #[cfg(unix)]
    #[test]
    fn a_key_that_cannot_be_read_now_is_left_for_the_next_start_and_the_phones_follow_it() {
        use std::os::unix::fs::PermissionsExt;
        let (_root, old, new) = dirs();
        old_release(&old);
        let key = old.join(endpoint::KEY_FILE);
        fs::set_permissions(&key, fs::Permissions::from_mode(0o000)).unwrap();
        if fs::read(&key).is_ok() {
            // Run as a user that reads through permissions (root): nothing to see here.
            return;
        }
        let report = inherit(&old, &new);
        assert!(report.key_pending && !report.key, "{report:?}");
        assert!(!new.join(endpoint::KEY_FILE).exists());
        assert!(!new.join(MARKER).exists());

        // The run in between paired this machine's own client into the server's list.
        fs::write(
            new.join(devices::FILE_NAME),
            r#"{"version":1,"devices":[{"key":"77aa","name":"smetanad (this machine)","platform":"desktop","push_token":null,"push_key":"00","paired_at":5,"last_contact":null,"kind":"desktop","local":true}]}"#,
        )
        .unwrap();
        fs::set_permissions(&key, fs::Permissions::from_mode(0o600)).unwrap();
        let later = inherit(&old, &new);
        assert!(later.key && later.problems.is_empty(), "{later:?}");
        let names: Vec<String> = devices::Devices::load(new.join(devices::FILE_NAME)).all().into_iter().map(|d| d.name).collect();
        assert!(names.contains(&"Pixel".to_owned()), "the phone arrives: {names:?}");
        assert!(names.contains(&"smetanad (this machine)".to_owned()), "and the local client stays: {names:?}");
        assert!(new.join(MARKER).exists());
    }

    #[cfg(unix)]
    #[test]
    fn an_attachment_that_cannot_be_mirrored_is_skipped_and_the_rest_are_taken() {
        use std::os::unix::fs::PermissionsExt;
        let (_root, old, new) = dirs();
        old_release(&old);
        let locked = old.join("attachments/smetana-abc");
        fs::write(old.join("attachments/second.png"), b"second").unwrap();
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let readable = fs::read_dir(&locked).is_ok();
        let report = inherit(&old, &new);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        if readable {
            return;
        }
        assert!(report.key && report.devices == 1, "the key and the phones do not wait on a picture");
        assert_eq!(report.attachments, 2, "loose.png and second.png");
        assert!(!report.problems.is_empty());
        assert!(!new.join(MARKER).exists());
        // Once the folder can be read, the next start takes what was missing and finishes.
        let later = inherit(&old, &new);
        assert_eq!(later.attachments, 1);
        assert!(later.problems.is_empty() && new.join(MARKER).exists(), "{later:?}");
    }

    #[test]
    fn a_machine_with_no_old_release_has_nothing_to_take_and_is_marked_so() {
        let (root, _old, new) = dirs();
        let report = inherit(&root.path().join("nowhere"), &new);
        assert_eq!(report.skipped, Some("no source directory"));
        assert!(new.join(MARKER).exists());
        assert!(!new.join(endpoint::KEY_FILE).exists());
        assert_eq!(relay_hint(&new), None);
    }

    #[test]
    fn a_source_that_is_the_servers_own_directory_is_left_alone() {
        let (_root, _old, new) = dirs();
        let report = inherit(&new, &new);
        assert_eq!(report.skipped, Some("the source is the server's own directory"));
        assert!(!new.join(MARKER).exists());
    }
}
