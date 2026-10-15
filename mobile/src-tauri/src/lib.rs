//! The Smetana companion's Rust core: pairing with a workstation over iroh, the
//! bridge that carries the desktop's commands and events to the front end, and
//! the push plumbing. The design is
//! `docs/superpowers/specs/2026-10-15-mobile-companion-design.md`.
//!
//! The front end sees this as five commands (`pair`, `connect`, `unpair`,
//! `bridge_state`, and the three `bridge_*` calls), two events (`bridge:event`
//! and `bridge:status`) and nothing else; `core.rs` is the logic, and this file
//! only moves it across the Tauri boundary.

mod core;
mod push;
mod secrets;
mod version;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_vendor = "apple")]
mod keychain;

use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, State};

pub use crate::core::{PROTOCOL_MAJOR, PROTOCOL_MINOR};
pub use crate::version::older_than;
use crate::core::{Config, Core, Out, Status};
use crate::secrets::SecretStore;

const STORE_URL_ANDROID: &str = "https://play.google.com/store/apps/details?id=com.invisor.smetana.companion";
/// A placeholder until the app has a store listing: the id is not real.
const STORE_URL_IOS: &str = "https://apps.apple.com/app/smetana-companion/id0000000000";

static CORE: OnceLock<Arc<Core>> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();
static PUSH_TARGET: Mutex<Option<String>> = Mutex::new(None);

#[derive(Serialize)]
struct BridgeState {
    paired: bool,
    version: &'static str,
    status: Status,
}

#[tauri::command]
async fn pair(core: State<'_, Arc<Core>>, ticket: String) -> Result<(), String> {
    core.pair(&ticket).await
}

#[tauri::command]
fn connect(core: State<'_, Arc<Core>>) -> Result<(), String> {
    core.connect()
}

#[tauri::command]
fn unpair(core: State<'_, Arc<Core>>) {
    core.unpair()
}

#[tauri::command]
fn bridge_state(core: State<'_, Arc<Core>>) -> BridgeState {
    BridgeState { paired: core.pairing().is_some(), version: env!("CARGO_PKG_VERSION"), status: core.status() }
}

#[tauri::command]
async fn bridge_invoke(core: State<'_, Arc<Core>>, cmd: String, args: Option<Value>) -> Result<Value, Value> {
    let args = args.unwrap_or_else(|| Value::Object(Default::default()));
    core.invoke(&cmd, args).await.map_err(|err| err.to_value())
}

#[tauri::command]
fn bridge_listen(core: State<'_, Arc<Core>>, event: String) -> Result<(), Value> {
    core.listen(&event).map_err(|err| err.to_value())
}

#[tauri::command]
fn bridge_unlisten(core: State<'_, Arc<Core>>, event: String) {
    core.unlisten(&event)
}

/// The target of the notification the app was opened from, once. Routing by it is
/// a later stage's; this only hands it over.
#[tauri::command]
fn take_push_target() -> Option<String> {
    PUSH_TARGET.lock().unwrap().take()
}

/// A push token arrived from the platform (APNs or FCM).
pub fn token_changed(token: &str) {
    if let Some(core) = CORE.get() {
        core.set_push_token(token);
    }
}

/// The app was opened by tapping a notification.
pub fn push_target(target: &str) {
    *PUSH_TARGET.lock().unwrap() = Some(target.to_owned());
    if let Some(app) = APP.get() {
        let _ = app.emit("push:target", target);
    }
}

/// The iOS hooks call these two (`gen/apple/Sources/PushRegistration.m`).
///
/// # Safety
/// `token` must be null or a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn smetana_push_token_changed(token: *const std::ffi::c_char) {
    if !token.is_null() {
        token_changed(&std::ffi::CStr::from_ptr(token).to_string_lossy());
    }
}

/// # Safety
/// `target` must be null or a NUL-terminated string.
#[no_mangle]
pub unsafe extern "C" fn smetana_push_target(target: *const std::ffi::c_char) {
    if !target.is_null() {
        push_target(&std::ffi::CStr::from_ptr(target).to_string_lossy());
    }
}

fn platform() -> (Arc<dyn SecretStore>, remote_protocol::Platform, String, &'static str) {
    #[cfg(target_os = "android")]
    {
        let name = android::device_name().unwrap_or_else(|| "Android phone".into());
        return (Arc::new(android::VaultStore), remote_protocol::Platform::Android, name, STORE_URL_ANDROID);
    }
    #[cfg(target_os = "ios")]
    {
        return (Arc::new(keychain::KeychainStore), remote_protocol::Platform::Ios, "iPhone".into(), STORE_URL_IOS);
    }
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        // A desktop run, to look at the shell: no vault, a file in the temp
        // directory. Not a configuration anyone ships.
        let path = std::env::temp_dir().join("smetana-companion-dev.json");
        (
            Arc::new(secrets::FileStore(path)),
            remote_protocol::Platform::Android,
            "Companion (development)".into(),
            STORE_URL_ANDROID,
        )
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(mobile)]
    {
        builder = builder.plugin(tauri_plugin_barcode_scanner::init());
    }
    builder
        .setup(|app| {
            let handle = app.handle().clone();
            let (store, platform, name, store_url) = platform();
            let _ = STORE_URL_IOS.len();
            let sink_app = handle.clone();
            let sink: core::Sink = Arc::new(move |out| match out {
                Out::Event { name, payload } => {
                    let _ = sink_app.emit("bridge:event", serde_json::json!({ "name": name, "payload": payload }));
                }
                Out::Status(status) => {
                    let _ = sink_app.emit("bridge:status", status);
                }
            });
            let config = Config::new(env!("CARGO_PKG_VERSION"), platform, &name, store_url);
            let core = Core::new(store, sink, config);
            let _ = CORE.set(core.clone());
            let _ = APP.set(handle);
            app.manage(core.clone());
            if core.pairing().is_some() {
                tauri::async_runtime::spawn(async move {
                    let _ = core.connect();
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            pair,
            connect,
            unpair,
            bridge_state,
            bridge_invoke,
            bridge_listen,
            bridge_unlisten,
            take_push_target
        ])
        .run(tauri::generate_context!())
        .expect("error while running the Smetana companion");
}
