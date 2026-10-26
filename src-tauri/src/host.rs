//! The Tauri side of `context.rs`: the one place that makes a `Context` out of an `AppHandle`.
//!
//! Everything the core asks of the application is answered here once, at start-up — the platform's
//! directories, where the bd sidecar sits, and the sink that turns an event into `app.emit`. The
//! shell uses it for the device's own settings file and for the sidecar paths it hands the server.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use crate::context::{Context, Sink};

/// The windows of this app, and nobody else: an event a worker emits reaches the webview exactly as
/// `app.emit` does. The shell starts no worker of its own, so this serves only what the settings
/// helpers it still calls would announce; the server's events reach the windows by the connection
/// (`client/link.rs`), not by here. There are no phones on this side to push to.
struct TauriSink {
    app: AppHandle,
}

impl Sink for TauriSink {
    fn emit(&self, name: &str, payload: serde_json::Value) {
        let _ = self.app.emit(name, &payload);
    }

    fn push(&self, _message: smetana_core::remote::push::Message) {}
}

/// The directory the bundled `bd` sits in — `None` only when the running executable has no
/// discoverable location, in which case an agent is left with whatever `PATH` it inherited.
///
/// `tauri-plugin-shell`'s `sidecar("bd")`, which the tracker used to spawn bd through, is
/// `dirname(current_exe())` joined with the name and nothing else, and deriving the path the same way
/// is what keeps it the same file by construction rather than by a rule copied out of that crate: in a
/// bundle it is `Smetana.app/Contents/MacOS`, beside the app executable, where the bundler drops the
/// external binary with its target triple stripped; under `npm run tauri dev` it is
/// `src-tauri/target/debug`, where the Tauri CLI drops the same file.
///
/// `tauri::utils::platform::current_exe` rather than `std::env::current_exe`, again because it is what
/// the plugin calls: it answers with the path captured at start-up, before anything could have moved,
/// and it resolves an AppImage's mount point back to the image.
pub(crate) fn sidecar_path() -> Option<PathBuf> {
    let exe = tauri::utils::platform::current_exe().ok()?;
    let dir = exe.parent()?;
    // A test binary lives one level below the sidecar, in `target/debug/deps`. The plugin steps up out
    // of that directory, and so does this: the two must agree in precisely the case a test can see.
    let dir = if dir.ends_with("deps") { dir.parent().unwrap_or(dir) } else { dir };
    let mut path = dir.join("bd");
    if cfg!(windows) {
        path.as_mut_os_string().push(".exe");
    }
    Some(path)
}

/// Builds the context of a running application and installs its runtime for the workers' tasks.
pub fn build(app: &AppHandle) -> Context {
    let tauri::async_runtime::RuntimeHandle::Tokio(handle) = tauri::async_runtime::handle();
    crate::context::install_runtime(handle);
    let paths = app.path();
    Context::new(
        crate::dirs::data_dir(app),
        crate::dirs::config_dir(app),
        paths.resource_dir().map_err(|err| err.to_string()),
        sidecar_path(),
        Arc::new(TauriSink { app: app.clone() }),
    )
}
