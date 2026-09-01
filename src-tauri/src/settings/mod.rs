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

/// What the person wants said in every session they are in.
///
/// Beside `languages` above, read the same way — from the disk on each call —
/// and by the same caller for the same reason: `terminal::service` builds every
/// session in the app, a person's and a run's alike, so reading it there once is
/// what keeps a second road into a session from existing at all. It lives with
/// the 400 ms debounce the languages already live with: a session started in the
/// same fraction of a second as an edit reads the previous text.
///
/// A platform that will not name a config directory answers with the empty
/// string, which is the shipped state and changes nothing.
pub fn agent_prompt(app: &AppHandle) -> String {
    path(app).map(|path| file::agent_prompt(&path)).unwrap_or_default()
}

/// Whether a run may remove each task's worktree after it is merged and closed.
///
/// Beside `agent` above and read the same way, and by the same caller for the
/// same reason: `runs::service` reads it once when a run starts and carries it
/// for the whole of the run, so a night's batches all work to one answer rather
/// than to whatever the file said when each of them happened to spawn.
///
/// A platform that will not name a config directory answers `true`, the shipped
/// state — the same fallback `file::git_remove_worktrees` makes, and for the
/// reason written there.
pub fn git_remove_worktrees(app: &AppHandle) -> bool {
    path(app)
        .map(|path| file::git_remove_worktrees(&path))
        .unwrap_or_else(|| model::Settings::default().git.remove_worktrees)
}

/// Whether the update timer may ask the release feed by itself.
///
/// Beside `agent` above and read the same way, from the disk on each call — and
/// here that is not merely acceptable but the mechanism: `updates::schedule`
/// asks at every tick, which is what lets the switch stop and restart the
/// scheduled check without a restart of the app. The opposite of
/// `git_remove_worktrees`' caller, which reads once and carries the answer for
/// the whole of a run.
///
/// A platform that will not name a config directory answers `true`, the shipped
/// state — the same fallback `file::updates_auto_check` makes, and for the
/// reason written there.
pub fn updates_auto_check(app: &AppHandle) -> bool {
    path(app)
        .map(|path| file::updates_auto_check(&path))
        .unwrap_or_else(|| model::Settings::default().updates.auto_check)
}

/// How big one dialog window was left. `None` — including when there is no
/// settings path at all — asks for the height the content comes to.
pub fn dialog_size(app: &AppHandle, kind: &str) -> Option<model::DialogSize> {
    file::dialog_size(&path(app)?, kind)
}

/// Keeps how big one dialog window was left. A failure is a warning and nothing
/// more: the window on screen is the size the person made it either way, and
/// what is lost is that size at the next opening.
pub fn remember_dialog_size(app: &AppHandle, kind: &str, size: model::DialogSize) {
    let Some(path) = path(app) else {
        return;
    };
    if let Err(err) = file::remember_dialog_size(&path, kind, size) {
        log::warn!("settings: the dialog size was not kept: {err}");
    }
}
