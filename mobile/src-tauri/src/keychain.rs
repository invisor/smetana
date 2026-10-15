//! The iOS Keychain as a `SecretStore`.
//!
//! Generic-password items under one service name. Every item is
//! `kSecAttrAccessibleAfterFirstUnlock`, and that is not a detail: the push secret
//! is read by the notification service extension, which runs when a push arrives
//! — often with the phone locked — and an item readable only while unlocked would
//! make every notification arrive undecrypted.
//!
//! No access group is named here. An item with none goes into the first group in
//! the process's `keychain-access-groups` entitlement, and the app and the
//! extension carry the same single entry (`gen/apple/*.entitlements`), which is
//! what lets the extension see what the app wrote. Naming a group in code would
//! need the team's prefix at run time.
//!
//! Compiled for every Apple target so that a host build checks it; it is only
//! *used* on iOS (`lib.rs`).

use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::data::CFData;
use core_foundation::dictionary::CFMutableDictionary;
use core_foundation::string::CFString;
use security_framework_sys::access_control::kSecAttrAccessibleAfterFirstUnlock;
use security_framework_sys::base::{errSecItemNotFound, errSecSuccess};
use security_framework_sys::item::{
    kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword, kSecMatchLimit, kSecReturnData,
    kSecValueData,
};
use security_framework_sys::keychain_item::{SecItemAdd, SecItemCopyMatching, SecItemDelete, SecItemUpdate};

use crate::secrets::SecretStore;

// Two constants the `-sys` crate leaves out.
#[link(name = "Security", kind = "framework")]
extern "C" {
    static kSecAttrAccessible: core_foundation::string::CFStringRef;
    static kSecMatchLimitOne: core_foundation::string::CFStringRef;
}

/// Also spelled in `gen/apple/NotificationService/NotificationService.swift`.
pub const SERVICE: &str = "com.invisor.smetana.companion.vault";

pub struct KeychainStore;

type Dict = CFMutableDictionary<CFType, CFType>;

unsafe fn key(raw: *const std::ffi::c_void) -> CFType {
    CFType::wrap_under_get_rule(raw as _)
}

fn base(name: &str) -> Dict {
    let mut query = Dict::new();
    unsafe {
        query.set(key(kSecClass as _), key(kSecClassGenericPassword as _));
    }
    query.set(unsafe { key(kSecAttrService as _) }, CFString::new(SERVICE).as_CFType());
    query.set(unsafe { key(kSecAttrAccount as _) }, CFString::new(name).as_CFType());
    query
}

impl SecretStore for KeychainStore {
    fn get(&self, name: &str) -> Option<String> {
        let mut query = base(name);
        query.set(unsafe { key(kSecReturnData as _) }, CFBoolean::true_value().as_CFType());
        query.set(unsafe { key(kSecMatchLimit as _) }, unsafe { key(kSecMatchLimitOne as _) });
        let mut found: core_foundation::base::CFTypeRef = std::ptr::null();
        let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut found) };
        if status != errSecSuccess || found.is_null() {
            return None;
        }
        let data = unsafe { CFData::wrap_under_create_rule(found as _) };
        String::from_utf8(data.bytes().to_vec()).ok()
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        let data = CFData::from_buffer(value.as_bytes());
        let mut attributes = Dict::new();
        attributes.set(unsafe { key(kSecValueData as _) }, data.as_CFType());
        attributes.set(unsafe { key(kSecAttrAccessible as _) }, unsafe { key(kSecAttrAccessibleAfterFirstUnlock as _) });
        let status = unsafe { SecItemUpdate(base(name).as_concrete_TypeRef(), attributes.as_concrete_TypeRef()) };
        if status == errSecSuccess {
            return Ok(());
        }
        if status != errSecItemNotFound {
            return Err(format!("the Keychain refused an update (status {status})"));
        }
        let mut add = base(name);
        add.set(unsafe { key(kSecValueData as _) }, data.as_CFType());
        add.set(unsafe { key(kSecAttrAccessible as _) }, unsafe { key(kSecAttrAccessibleAfterFirstUnlock as _) });
        let status = unsafe { SecItemAdd(add.as_concrete_TypeRef(), std::ptr::null_mut()) };
        if status == errSecSuccess {
            Ok(())
        } else {
            Err(format!("the Keychain refused an item (status {status})"))
        }
    }

    fn delete(&self, name: &str) {
        unsafe {
            SecItemDelete(base(name).as_concrete_TypeRef());
        }
    }
}
