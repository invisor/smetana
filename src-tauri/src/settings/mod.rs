//! What the app remembers between runs: `model.rs` is the schema and the pure
//! rules, `file.rs` is the disk, `commands.rs` is the two thin commands the
//! front end calls.
//!
//! The two functions here are for the rest of the app rather than for the front
//! end: a caller that wants one value out of the file, with no project to
//! resolve against and nobody to report a failure to.

pub mod commands;
pub mod file;
pub mod model;

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

/// Where the file lives. `None` only when the platform will not name a config
/// directory at all, which costs the caller the same as a missing file does.
///
/// `commands::settings_path` builds the same path and keeps its own error type,
/// because a command has somebody to tell and these callers do not.
pub fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("settings.json"))
}

/// Which CLI agent the app is configured to start.
///
/// Read from the disk on each call rather than cached anywhere: there is no
/// settings worker, and one read costs milliseconds — the same reasoning that
/// keeps `files/` and `git.rs` out of a worker. A caller that needs the answer
/// to hold still for a while is the one that keeps a copy of it; `runs::service`
/// does exactly that, for the life of a run.
pub fn agent(app: &AppHandle) -> String {
    path(app).map(|path| file::agent(&path)).unwrap_or_else(|| model::Settings::default().agent)
}

/// What a session started now should speak, and what it should write into bd.
///
/// Beside `agent` above and read the same way, from the disk on each call, and
/// it is deliberately read here rather than taken from `terminal_create`'s
/// arguments: `terminal::service` builds every session in the app, a person's
/// and a run's alike, so reading it once there is what keeps the two from
/// disagreeing. See the `Create` arm for the debounce this lives with.
pub fn languages(app: &AppHandle) -> crate::agents::Languages {
    path(app).map(|path| file::languages(&path)).unwrap_or_default()
}
