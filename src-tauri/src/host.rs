//! The Tauri side of `context.rs`: the one place that makes a `Context` out of an `AppHandle`.
//!
//! Everything a worker used to ask the application for is answered here once, at start-up — the
//! platform's directories, where the bd sidecar sits, and the sink that turns an event into
//! `app.emit` plus the phones' bridge. The workers get the result and never see `tauri` again.

use std::path::PathBuf;
use std::sync::Arc;

use tauri::{AppHandle, Emitter, Manager};

use crate::context::{Context, Sink};
use crate::remote::{push, Remote};

/// Windows and remote devices, from one place: an event reaches the windows exactly as `app.emit`
/// does and every device that has listened for it, so that a device and the window cannot be told
/// different things. An event that is not on a device kind's list reaches the windows and no device
/// of that kind — the hub refuses it — which is why one sink serves the events that used to be
/// emitted to windows alone. Terminal output and log lines take the two extra roads below: they are
/// delivered to the devices that follow a session, not to every listener.
struct TauriSink {
    app: AppHandle,
}

impl Sink for TauriSink {
    fn emit(&self, name: &str, payload: serde_json::Value) {
        let _ = self.app.emit(name, &payload);
        if let Some(remote) = self.app.try_state::<Arc<Remote>>() {
            remote.hub.publish(name, &payload);
        }
    }

    fn terminal_chunk(&self, session: u64, seq: u64, bytes: &[u8]) {
        if let Some(remote) = self.app.try_state::<Arc<Remote>>() {
            remote.hub.publish_terminal_chunk(session, seq, bytes);
        }
    }

    fn terminal_log(&self, session: u64, entries: &serde_json::Value) {
        if let Some(remote) = self.app.try_state::<Arc<Remote>>() {
            remote.hub.publish_terminal_log(session, entries);
        }
    }

    fn push(&self, message: smetana_core::remote::push::Message) {
        if let Some(remote) = self.app.try_state::<Arc<Remote>>() {
            push::dispatch(&remote, message);
        }
    }
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
        paths.app_data_dir().map_err(|err| err.to_string()),
        paths.app_config_dir().map_err(|err| err.to_string()),
        paths.resource_dir().map_err(|err| err.to_string()),
        sidecar_path(),
        Arc::new(TauriSink { app: app.clone() }),
    )
}

/// The context `setup` put into the managed state, for a command wrapper that was handed an
/// `AppHandle` rather than the context itself.
pub fn context(app: &AppHandle) -> Context {
    app.state::<Context>().inner().clone()
}
