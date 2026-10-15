//! Where the phone's secrets live.
//!
//! The iroh key, the push key, the desktop's key and the relay URL are kept in the
//! platform's own vault — the iOS Keychain, the Android Keystore — and never in a
//! file of the app's. `SecretStore` is the seam: the platforms implement it
//! (`keychain.rs`, `android.rs`), the tests use `MemoryStore`.
//!
//! The values are short strings. Keys are stored as lower-case hex.

use std::collections::HashMap;
use std::sync::Mutex;

pub trait SecretStore: Send + Sync {
    fn get(&self, name: &str) -> Option<String>;
    fn set(&self, name: &str, value: &str) -> Result<(), String>;
    fn delete(&self, name: &str);
}

/// Names of the entries. The push secret's name is also read by the iOS
/// notification extension (`gen/apple/NotificationService/`) and by the Android
/// messaging service (`SecretVault.kt`), so it is spelled out once, here, and
/// those two repeat it.
pub mod names {
    pub const IROH_KEY: &str = "iroh_key";
    pub const PUSH_SECRET: &str = "push_secret";
    pub const PAIRING: &str = "pairing";
    pub const PUSH_TOKEN: &str = "push_token";
    pub const PUSH_TOKEN_REPORTED: &str = "push_token_reported";
    pub const LAST_SEEN: &str = "last_seen";
}

#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, name: &str) -> Option<String> {
        self.0.lock().unwrap().get(name).cloned()
    }
    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        self.0.lock().unwrap().insert(name.to_owned(), value.to_owned());
        Ok(())
    }
    fn delete(&self, name: &str) {
        self.0.lock().unwrap().remove(name);
    }
}

/// The store for a build that is neither iOS nor Android (`cargo tauri dev` on a
/// desktop, to look at the shell): a file in the data directory. It is **not** a
/// vault and is not compiled into a phone build.
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub struct FileStore(pub std::path::PathBuf);

#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl FileStore {
    fn read(&self) -> HashMap<String, String> {
        std::fs::read_to_string(&self.0).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
impl SecretStore for FileStore {
    fn get(&self, name: &str) -> Option<String> {
        self.read().get(name).cloned()
    }
    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        let mut all = self.read();
        all.insert(name.to_owned(), value.to_owned());
        if let Some(dir) = self.0.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&self.0, serde_json::to_string(&all).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
    }
    fn delete(&self, name: &str) {
        let mut all = self.read();
        if all.remove(name).is_some() {
            let _ = std::fs::write(&self.0, serde_json::to_string(&all).unwrap_or_default());
        }
    }
}

/// A 32-byte key as the string the vault holds.
pub fn encode_key(bytes: &[u8; 32]) -> String {
    data_encoding::HEXLOWER.encode(bytes)
}

pub fn decode_key(text: &str) -> Option<[u8; 32]> {
    data_encoding::HEXLOWER.decode(text.trim().as_bytes()).ok()?.try_into().ok()
}

/// The phone's two private keys, made on first use.
#[derive(Clone)]
pub struct Identity {
    pub iroh: iroh::SecretKey,
    pub push_secret: [u8; 32],
}

impl Identity {
    pub fn load_or_create(store: &dyn SecretStore) -> Result<Self, String> {
        let iroh = match store.get(names::IROH_KEY).and_then(|t| decode_key(&t)) {
            Some(bytes) => iroh::SecretKey::from_bytes(&bytes),
            None => {
                let key = iroh::SecretKey::generate();
                store.set(names::IROH_KEY, &encode_key(&key.to_bytes()))?;
                key
            }
        };
        let push_secret = match store.get(names::PUSH_SECRET).and_then(|t| decode_key(&t)) {
            Some(bytes) => bytes,
            None => {
                let secret = iroh::SecretKey::generate().to_bytes();
                store.set(names::PUSH_SECRET, &encode_key(&secret))?;
                secret
            }
        };
        Ok(Self { iroh, push_secret })
    }

    pub fn push_key(&self) -> [u8; 32] {
        remote_protocol::push::public_key(&self.push_secret)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_identity_is_made_once_and_read_back() {
        let store = MemoryStore::default();
        let first = Identity::load_or_create(&store).unwrap();
        let second = Identity::load_or_create(&store).unwrap();
        assert_eq!(first.iroh.public(), second.iroh.public());
        assert_eq!(first.push_secret, second.push_secret);
        assert_ne!(first.iroh.to_bytes(), first.push_secret, "two independent keys");
    }

    #[test]
    fn the_push_key_is_the_public_half_of_the_push_secret() {
        let identity = Identity::load_or_create(&MemoryStore::default()).unwrap();
        let sealed = remote_protocol::push::seal(&identity.push_key(), b"x");
        assert_eq!(remote_protocol::push::open(&identity.push_secret, &sealed).unwrap(), b"x");
    }

    #[test]
    fn a_damaged_entry_is_replaced_not_trusted() {
        let store = MemoryStore::default();
        store.set(names::IROH_KEY, "nonsense").unwrap();
        let identity = Identity::load_or_create(&store).unwrap();
        assert_eq!(decode_key(&store.get(names::IROH_KEY).unwrap()), Some(identity.iroh.to_bytes()));
    }
}
