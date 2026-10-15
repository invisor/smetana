//! The Android side of the Rust core: the Keystore-backed vault, the device's
//! model name, and the entry points the Kotlin classes call into.
//!
//! The vault itself is Kotlin (`SecretVault.kt`): an AES-GCM key that lives in the
//! Android Keystore and never leaves it, and values stored encrypted under it.
//! Rust reaches it through JNI. The class is loaded through the activity's class
//! loader because a native thread's `FindClass` sees only the system's classes.
//!
//! `Java_..._PushCrypto_nativeOpen` is the Android path of the push test vector:
//! the messaging service passes the push secret it read from the vault and the
//! blob it was sent, and gets the plaintext back from `push::open_wire` — the same
//! function the Rust test opens the committed vector with.

use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::sys::jbyteArray;
use jni::{JNIEnv, JavaVM};

use crate::secrets::SecretStore;

const CLASS: &str = "com.invisor.smetana.companion.SecretVault";

fn with_env<T>(f: impl FnOnce(&mut JNIEnv, &JObject) -> jni::errors::Result<T>) -> Option<T> {
    let context = ndk_context::android_context();
    let vm = unsafe { JavaVM::from_raw(context.vm().cast()) }.ok()?;
    let activity = unsafe { JObject::from_raw(context.context().cast()) };
    let mut env = vm.attach_current_thread().ok()?;
    let result = f(&mut env, &activity);
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
        return None;
    }
    result.ok()
}

fn vault_class<'a>(env: &mut JNIEnv<'a>, activity: &JObject) -> jni::errors::Result<JClass<'a>> {
    let loader = env.call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
    let name = env.new_string(CLASS)?;
    let class = env
        .call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&name)])?
        .l()?;
    Ok(JClass::from(class))
}

pub struct VaultStore;

impl SecretStore for VaultStore {
    fn get(&self, name: &str) -> Option<String> {
        with_env(|env, activity| {
            let class = vault_class(env, activity)?;
            let key = env.new_string(name)?;
            let value = env
                .call_static_method(
                    &class,
                    "get",
                    "(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;",
                    &[JValue::Object(activity), JValue::Object(&key)],
                )?
                .l()?;
            if value.is_null() {
                return Ok(None);
            }
            let text: String = env.get_string(&JString::from(value))?.into();
            Ok(Some(text))
        })
        .flatten()
    }

    fn set(&self, name: &str, value: &str) -> Result<(), String> {
        with_env(|env, activity| {
            let class = vault_class(env, activity)?;
            let key = env.new_string(name)?;
            let value = env.new_string(value)?;
            let ok = env
                .call_static_method(
                    &class,
                    "put",
                    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Z",
                    &[JValue::Object(activity), JValue::Object(&key), JValue::Object(&value)],
                )?
                .z()?;
            Ok(ok)
        })
        .filter(|ok| *ok)
        .map(|_| ())
        .ok_or_else(|| "the Keystore refused a value".to_owned())
    }

    fn delete(&self, name: &str) {
        let _ = with_env(|env, activity| {
            let class = vault_class(env, activity)?;
            let key = env.new_string(name)?;
            env.call_static_method(
                &class,
                "delete",
                "(Landroid/content/Context;Ljava/lang/String;)V",
                &[JValue::Object(activity), JValue::Object(&key)],
            )?;
            Ok(())
        });
    }
}

/// `Build.MODEL`, for "Pixel 8 wants to connect".
pub fn device_name() -> Option<String> {
    with_env(|env, _| {
        let class = env.find_class("android/os/Build")?;
        let model = env.get_static_field(class, "MODEL", "Ljava/lang/String;")?.l()?;
        let text: String = env.get_string(&JString::from(model))?.into();
        Ok(text)
    })
    .filter(|name| !name.trim().is_empty())
}

/// `PushCrypto.nativeOpen(secret: ByteArray, blob: String): ByteArray?`
#[no_mangle]
pub extern "system" fn Java_com_invisor_smetana_companion_PushCrypto_nativeOpen<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    secret: JByteArray<'a>,
    blob: JString<'a>,
) -> jbyteArray {
    let opened = (|| {
        let secret: [u8; 32] = env.convert_byte_array(&secret).ok()?.try_into().ok()?;
        let wire: String = env.get_string(&blob).ok()?.into();
        crate::push::open_wire(&secret, &wire)
    })();
    match opened.and_then(|bytes| env.byte_array_from_slice(&bytes).ok()) {
        Some(array) => array.into_raw(),
        None => std::ptr::null_mut(),
    }
}

/// `PushCrypto.nativeTokenChanged(token: String)`
#[no_mangle]
pub extern "system" fn Java_com_invisor_smetana_companion_PushCrypto_nativeTokenChanged<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    token: JString<'a>,
) {
    if let Ok(token) = env.get_string(&token) {
        crate::token_changed(&String::from(token));
    }
}

/// `PushCrypto.nativeOpenedFromPush(target: String)`
#[no_mangle]
pub extern "system" fn Java_com_invisor_smetana_companion_PushCrypto_nativeOpenedFromPush<'a>(
    mut env: JNIEnv<'a>,
    _class: JClass<'a>,
    target: JString<'a>,
) {
    if let Ok(target) = env.get_string(&target) {
        crate::push_target(&String::from(target));
    }
}
