//! The window's size and position on disk.
//!
//! `tauri-plugin-window-state` keeps them, and there is no point rewriting it
//! here: multi-monitor setups, minimization and full screen are already handled.
//! There is one problem — it writes to disk in exactly one place,
//! `RunEvent::Exit`, and holds everything in memory until then. So any run that
//! does not reach a clean exit — a crash, a force quit, and in development every
//! rebuild that kills the process — leaves the run-before-last's geometry on
//! disk, and the window opens somewhere other than where it was left.
//!
//! Hence the write also happens along the way. The debounce is mandatory:
//! dragging a window's corner sends hundreds of events, and writing the file on
//! every one of them is hundreds of trips to the disk for a single number. The
//! debounce also settles the question of handler order: by the time the write
//! runs, the plugin's cache has certainly been updated by its own listener of
//! the same event.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_window_state::{AppHandleExt, StateFlags};

/// How long we wait after the last movement. Less, and the write happens in the
/// middle of a drag; more, and a window closed right after a resize goes back to
/// relying on `Exit`.
const SETTLE: Duration = Duration::from_millis(500);

/// The same flags as `Builder::default()`: the plugin does not expose them, and
/// saving less than it restores means losing fields on every write of ours.
const FLAGS: StateFlags = StateFlags::all();

/// Subscribes the main window to writing its own geometry.
///
/// There may be no window — a configuration without `main`, a headless run;
/// that is not an error, there is simply nothing to write.
pub fn persist_geometry(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    // The counter cuts off everything but the last event: a task that wakes up
    // writes only if nobody arrived after it.
    let latest = Arc::new(AtomicU64::new(0));
    let app = app.clone();

    window.on_window_event(move |event| {
        if !matches!(event, WindowEvent::Resized(_) | WindowEvent::Moved(_)) {
            return;
        }
        let mine = latest.fetch_add(1, Ordering::SeqCst) + 1;
        let latest = latest.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SETTLE).await;
            if latest.load(Ordering::SeqCst) != mine {
                return;
            }
            if let Err(err) = app.save_window_state(FLAGS) {
                // No reason to crash: the window on screen does not change
                // because of it, and lost geometry costs one inconvenience at
                // the next launch.
                log::warn!("could not save the window geometry: {err}");
            }
        });
    });
}
