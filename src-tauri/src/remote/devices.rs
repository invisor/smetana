//! The list of paired phones: `remote-devices.json` in the app data directory.
//!
//! Its own file rather than a section of `settings.json`, for three reasons. The
//! list is state the app writes, not a preference a person edits, and it changes
//! when a phone arrives rather than when a control moves. Revoking from it has
//! to take effect at once, which a debounced settings save would not. And
//! `settings.json` is copied around, pasted into bug reports and synced between
//! machines by people who would not think of it as a list of keys that may
//! connect.
//!
//! It holds public keys and names, not secrets. The write is atomic the way
//! `settings/file.rs` writes: a neighbouring temp file, flushed, then renamed.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use remote_protocol::{DeviceKind, Platform, PublicKey};
use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "remote-devices.json";
const VERSION: u32 = 1;

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// One paired phone. `key`, `push_key` are lower-case hex.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    pub key: String,
    pub name: String,
    pub platform: Platform,
    pub push_token: Option<String>,
    pub push_key: String,
    /// Seconds since the Unix epoch.
    pub paired_at: u64,
    /// The last time this phone completed a handshake, if it ever has.
    pub last_contact: Option<u64>,
    /// What this device may do. Fixed at pairing by the ticket it spent. An entry written before
    /// kinds existed has none, and is a phone: the safe direction.
    #[serde(default)]
    pub kind: DeviceKind,
}

#[derive(Serialize, Deserialize)]
struct File {
    version: u32,
    devices: Vec<Device>,
}

pub struct Devices {
    path: PathBuf,
    list: Mutex<Vec<Device>>,
}

impl Devices {
    /// Reads the list. A missing file is the first run. A file that is not ours
    /// is set aside rather than overwritten, and the list starts empty — every
    /// phone then has to be paired again, which is the safe direction.
    pub fn load(path: PathBuf) -> Self {
        let list = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<File>(&text) {
                Ok(file) if file.version <= VERSION => file.devices,
                _ => {
                    log::warn!("remote: {} is not readable, starting with no devices", path.display());
                    let _ = fs::copy(&path, path.with_extension("json.bak"));
                    Vec::new()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(err) => {
                log::warn!("remote: could not read {}: {err}", path.display());
                Vec::new()
            }
        };
        Self { path, list: Mutex::new(list) }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Device>> {
        self.list.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn all(&self) -> Vec<Device> {
        self.lock().clone()
    }

    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    pub fn contains(&self, key: &PublicKey) -> bool {
        self.kind_of(key).is_some()
    }

    /// What the device with this key may do, or `None` when it is not on the list.
    pub fn kind_of(&self, key: &PublicKey) -> Option<DeviceKind> {
        let key = key.to_string();
        self.lock().iter().find(|device| device.key == key).map(|device| device.kind)
    }

    /// Adds a device, replacing an earlier entry for the same key (a phone that
    /// pairs again is the same phone). Nothing is kept in memory that was not
    /// written: on a failed write the list is left as it was.
    pub fn add(&self, device: Device) -> Result<(), String> {
        let mut list = self.lock();
        let mut next = list.clone();
        next.retain(|existing| existing.key != device.key);
        next.push(device);
        write(&self.path, &next)?;
        *list = next;
        Ok(())
    }

    /// Removes a device. `false` when there was none by that key.
    pub fn remove(&self, key: &PublicKey) -> Result<bool, String> {
        let key = key.to_string();
        let mut list = self.lock();
        if !list.iter().any(|device| device.key == key) {
            return Ok(false);
        }
        let next: Vec<Device> = list.iter().filter(|device| device.key != key).cloned().collect();
        write(&self.path, &next)?;
        *list = next;
        Ok(true)
    }

    /// Replaces a device's push token (the phone reported a new one). `false` when
    /// there is no such device. A failed write leaves the list as it was.
    pub fn set_push_token(&self, key: &PublicKey, token: String) -> Result<bool, String> {
        let key = key.to_string();
        let mut list = self.lock();
        let Some(index) = list.iter().position(|device| device.key == key) else {
            return Ok(false);
        };
        let mut next = list.clone();
        next[index].push_token = Some(token);
        write(&self.path, &next)?;
        *list = next;
        Ok(true)
    }

    /// Records a completed handshake. A failed write is logged and forgotten:
    /// a timestamp is not worth refusing a phone over.
    pub fn touch(&self, key: &PublicKey, now: u64) {
        let key = key.to_string();
        let mut list = self.lock();
        let Some(device) = list.iter_mut().find(|device| device.key == key) else {
            return;
        };
        device.last_contact = Some(now);
        if let Err(err) = write(&self.path, &list) {
            log::warn!("remote: could not record a contact: {err}");
        }
    }
}

fn write(path: &Path, devices: &[Device]) -> Result<(), String> {
    let text = serde_json::to_string_pretty(&File { version: VERSION, devices: devices.to_vec() })
        .map_err(|err| err.to_string())?;
    write_atomic(path, text.as_bytes())
}

/// A neighbouring file first, flushed to disk, then a rename: a crash leaves the
/// old list or the new one and never half of either.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("remote");
    let temp = path.with_file_name(format!(
        "{stem}.{}.{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(format!("{}: {err}", temp.display()));
    }
    fs::rename(&temp, path).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("{}: {err}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "smetana-devices-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn key(n: u8) -> PublicKey {
        iroh::SecretKey::from_bytes(&[n; 32]).public()
    }

    fn device(n: u8) -> Device {
        Device {
            key: key(n).to_string(),
            name: format!("Phone {n}"),
            platform: Platform::Ios,
            push_token: None,
            push_key: "00".repeat(32),
            paired_at: 5,
            last_contact: None,
            kind: DeviceKind::Phone,
        }
    }

    #[test]
    fn an_entry_written_before_kinds_existed_is_a_phone() {
        let dir = dir();
        let path = dir.join(FILE_NAME);
        let old = format!(
            r#"{{"version":1,"devices":[{{"key":"{}","name":"Old","platform":"ios","push_token":null,"push_key":"00","paired_at":1,"last_contact":null}}]}}"#,
            key(1)
        );
        fs::write(&path, old).unwrap();
        let devices = Devices::load(path);
        assert_eq!(devices.kind_of(&key(1)), Some(DeviceKind::Phone));
        assert_eq!(devices.kind_of(&key(2)), None);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_desktops_kind_survives_a_restart() {
        let dir = dir();
        let path = dir.join(FILE_NAME);
        let devices = Devices::load(path.clone());
        let mut desktop = device(3);
        desktop.kind = DeviceKind::Desktop;
        devices.add(desktop).unwrap();
        assert_eq!(Devices::load(path).kind_of(&key(3)), Some(DeviceKind::Desktop));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_device_survives_a_restart() {
        let dir = dir();
        let path = dir.join(FILE_NAME);
        let devices = Devices::load(path.clone());
        assert!(devices.is_empty());
        devices.add(device(1)).unwrap();
        devices.touch(&key(1), 99);
        let reloaded = Devices::load(path);
        assert!(reloaded.contains(&key(1)));
        assert_eq!(reloaded.all()[0].last_contact, Some(99));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn pairing_the_same_phone_again_replaces_its_entry() {
        let dir = dir();
        let devices = Devices::load(dir.join(FILE_NAME));
        devices.add(device(1)).unwrap();
        let mut again = device(1);
        again.name = "Renamed".into();
        devices.add(again).unwrap();
        assert_eq!(devices.all().len(), 1);
        assert_eq!(devices.all()[0].name, "Renamed");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn removing_a_device_forgets_it_on_disk_too() {
        let dir = dir();
        let path = dir.join(FILE_NAME);
        let devices = Devices::load(path.clone());
        devices.add(device(1)).unwrap();
        devices.add(device(2)).unwrap();
        assert!(devices.remove(&key(1)).unwrap());
        assert!(!devices.remove(&key(1)).unwrap(), "already gone");
        let reloaded = Devices::load(path);
        assert!(!reloaded.contains(&key(1)));
        assert!(reloaded.contains(&key(2)));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_damaged_file_is_kept_aside_and_the_list_starts_empty() {
        let dir = dir();
        let path = dir.join(FILE_NAME);
        fs::write(&path, "{ not json").unwrap();
        let devices = Devices::load(path.clone());
        assert!(devices.is_empty());
        assert!(path.with_extension("json.bak").exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_failed_write_leaves_the_list_as_it_was() {
        let dir = dir();
        // A directory where the file should go makes the rename fail.
        let path = dir.join(FILE_NAME);
        fs::create_dir_all(&path).unwrap();
        let devices = Devices::load(path);
        assert!(devices.add(device(1)).is_err());
        assert!(devices.is_empty());
        let _ = fs::remove_dir_all(dir);
    }
}
