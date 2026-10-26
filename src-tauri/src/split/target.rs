//! Which server this device uses: the one on this machine, or one it was paired with by a ticket.
//!
//! A **device** setting, and kept in a file of its own beside the shell's other files rather than in
//! `settings.json`: the shell reads it before any connection exists, and `settings.json`'s server
//! half is a thing that lives *on* a server. The file holds where a server is and nothing secret -
//! the ticket's secret is spent at pairing - and an unreadable file means the local server, which is
//! the answer for a person who never chose.
//!
//! Choosing another server changes nothing about the one that was left: a local server that is
//! running keeps running, and while the choice is a remote one the shell does not start it again
//! (`link::attempt`).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use smetanad::client::RemoteRecord;

pub const FILE_NAME: &str = "server-target.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Target {
    /// The `smetanad` this app starts and keeps on this machine.
    Local,
    /// A server somewhere else, found through the relay its ticket named.
    Remote { node: String, relay: String },
}

impl Target {
    pub fn is_local(&self) -> bool {
        matches!(self, Target::Local)
    }

    /// What tells one server from another: `local`, or the remote server's key.
    pub fn id(&self) -> String {
        match self {
            Target::Local => "local".to_owned(),
            Target::Remote { node, .. } => node.clone(),
        }
    }

    pub fn remote(record: RemoteRecord) -> Target {
        Target::Remote { node: record.node, relay: record.relay }
    }
}

pub fn path(identity_dir: &Path) -> PathBuf {
    identity_dir.join(FILE_NAME)
}

/// The saved choice, or the local server when there is none or it cannot be read.
pub fn load(identity_dir: &Path) -> Target {
    let file = path(identity_dir);
    match std::fs::read_to_string(&file) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|err| {
            log::warn!("split: {} is not readable, using the local server: {err}", file.display());
            Target::Local
        }),
        Err(_) => Target::Local,
    }
}

/// Saves the choice, atomically: a half-written file would read as "local" and a person who chose a
/// server would be moved back without a word.
pub fn save(identity_dir: &Path, target: &Target) -> Result<(), String> {
    std::fs::create_dir_all(identity_dir).map_err(|err| format!("{}: {err}", identity_dir.display()))?;
    let file = path(identity_dir);
    let temp = file.with_extension("json.tmp");
    let text = serde_json::to_string_pretty(target).map_err(|err| err.to_string())?;
    std::fs::write(&temp, text).map_err(|err| format!("{}: {err}", temp.display()))?;
    std::fs::rename(&temp, &file).map_err(|err| format!("{}: {err}", file.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_no_file_the_server_is_the_local_one() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load(dir.path()), Target::Local);
    }

    #[test]
    fn a_remote_choice_survives_a_restart_and_going_back_is_remembered_too() {
        let dir = tempfile::tempdir().unwrap();
        let remote = Target::Remote { node: "abc".into(), relay: "https://relay.example.com".into() };
        save(dir.path(), &remote).unwrap();
        assert_eq!(load(dir.path()), remote);
        save(dir.path(), &Target::Local).unwrap();
        assert_eq!(load(dir.path()), Target::Local);
        assert!(!dir.path().join("server-target.json.tmp").exists(), "no temporary file is left");
    }

    #[test]
    fn a_damaged_file_means_the_local_server_and_not_a_crash() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(path(dir.path()), "{ not json").unwrap();
        assert_eq!(load(dir.path()), Target::Local);
    }

    #[test]
    fn the_file_names_the_kind_in_words_and_holds_no_secret() {
        let text = serde_json::to_string(&Target::Remote { node: "n".into(), relay: "r".into() }).unwrap();
        assert_eq!(text, r#"{"kind":"remote","node":"n","relay":"r"}"#);
        assert_eq!(serde_json::to_string(&Target::Local).unwrap(), r#"{"kind":"local"}"#);
    }
}
