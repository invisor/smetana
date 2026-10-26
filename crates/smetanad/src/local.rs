//! The local pairing credential: how a client on the same machine, run by the same user, is paired
//! without a person.
//!
//! `smetanad run` writes `local-pairing.json` into its data directory, mode 0600, naming the server's
//! key, its loopback address and a secret it accepts as a desktop's pairing code with no confirmation
//! (`Pairing::set_local`). A desktop of the same OS user reads the file and pairs on first connect.
//! **Whoever can read this file can already read the server's key and every repository it works on**,
//! so the file adds no exposure the data directory did not have; the check is the operating system's
//! permission on the directory and the file, and nothing else.
//!
//! The file is rewritten at every start (the secret is new, and the loopback port is whatever the
//! system gave) and removed at a clean stop. A stale file left by a crash names an address nobody
//! listens on and a secret the next start replaces.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use remote_protocol::{PublicKey, SECRET_LEN};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "local-pairing.json";

/// What the file holds, as the client reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credential {
    pub node: PublicKey,
    pub addrs: Vec<SocketAddr>,
    pub secret: [u8; SECRET_LEN],
}

#[derive(Serialize, Deserialize)]
struct Stored {
    version: u32,
    node: String,
    addrs: Vec<String>,
    secret: String,
}

pub fn path(dir: &Path) -> PathBuf {
    dir.join(FILE_NAME)
}

/// A fresh secret.
pub fn new_secret() -> [u8; SECRET_LEN] {
    rand::random()
}

/// Writes the credential for `dir`, readable by the owner alone from the moment it exists: written to a
/// temporary file created with the final mode, then renamed over the old one, so there is no instant at
/// which a reader sees half of it or anybody else can read it.
pub fn write(dir: &Path, credential: &Credential) -> Result<(), String> {
    let stored = Stored {
        version: 1,
        node: credential.node.to_string(),
        addrs: credential.addrs.iter().map(ToString::to_string).collect(),
        secret: data_encoding::HEXLOWER.encode(&credential.secret),
    };
    let text = serde_json::to_vec_pretty(&stored).map_err(|err| err.to_string())?;
    let target = path(dir);
    let temp = dir.join(format!("{FILE_NAME}.{}.tmp", std::process::id()));
    write_private(&temp, &text)?;
    std::fs::rename(&temp, &target).map_err(|err| {
        let _ = std::fs::remove_file(&temp);
        format!("{}: {err}", target.display())
    })
}

#[cfg(unix)]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let _ = std::fs::remove_file(path);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|err| format!("{}: {err}", path.display()))?;
    file.write_all(bytes).map_err(|err| err.to_string())?;
    file.sync_all().map_err(|err| err.to_string())
}

#[cfg(not(unix))]
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|err| format!("{}: {err}", path.display()))
}

/// Reads the credential in `dir`.
pub fn read(dir: &Path) -> Result<Credential, String> {
    let file = path(dir);
    let text = std::fs::read_to_string(&file).map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            format!("{} does not exist: is `smetanad run` running with this --data-dir?", file.display())
        } else {
            format!("{}: {err}", file.display())
        }
    })?;
    let stored: Stored = serde_json::from_str(&text).map_err(|err| format!("{}: {err}", file.display()))?;
    if stored.version != 1 {
        return Err(format!("{}: version {} is not one this build reads", file.display(), stored.version));
    }
    let node = PublicKey::from_str(&stored.node).map_err(|err| format!("{}: node: {err}", file.display()))?;
    let addrs = stored
        .addrs
        .iter()
        .map(|addr| addr.parse().map_err(|err| format!("{}: address {addr}: {err}", file.display())))
        .collect::<Result<Vec<SocketAddr>, _>>()?;
    let secret: [u8; SECRET_LEN] = data_encoding::HEXLOWER
        .decode(stored.secret.as_bytes())
        .ok()
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| format!("{}: the secret is not {SECRET_LEN} bytes of hex", file.display()))?;
    Ok(Credential { node, addrs, secret })
}

/// Removes the file, if there is one. Called at a clean stop.
pub fn remove(dir: &Path) {
    let _ = std::fs::remove_file(path(dir));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Credential {
        Credential {
            node: iroh::SecretKey::from_bytes(&[3; 32]).public(),
            addrs: vec!["127.0.0.1:4242".parse().unwrap()],
            secret: [7; SECRET_LEN],
        }
    }

    #[test]
    fn a_credential_reads_back_as_written() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &sample()).unwrap();
        assert_eq!(read(dir.path()).unwrap(), sample());
        write(dir.path(), &Credential { secret: [8; SECRET_LEN], ..sample() }).unwrap();
        assert_eq!(read(dir.path()).unwrap().secret, [8; SECRET_LEN], "a restart replaces it");
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_by_its_owner_alone() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), &sample()).unwrap();
        let mode = std::fs::metadata(path(dir.path())).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
        let leftovers = std::fs::read_dir(dir.path()).unwrap().count();
        assert_eq!(leftovers, 1, "the temporary file does not stay");
    }

    #[test]
    fn a_missing_file_says_what_to_do() {
        let dir = tempfile::tempdir().unwrap();
        let err = read(dir.path()).unwrap_err();
        assert!(err.contains("smetanad run"), "{err}");
    }

    #[test]
    fn a_damaged_file_is_an_error_and_not_a_guess() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(path(dir.path()), "{ nonsense").unwrap();
        assert!(read(dir.path()).is_err());
    }
}
