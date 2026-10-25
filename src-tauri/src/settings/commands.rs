//! Settings commands: thin, with no state of their own.
//!
//! Unlike the tracker, no worker is needed here: the truth about settings lives
//! in the front end, nobody changes them from outside, and the file is read and
//! written in milliseconds — a request queue would be guarding something nobody
//! contends for.
//!
//! The active project comes from the front end and sits in the file itself. It
//! used to be fetched from the tracker, and that was a crutch: settings depended
//! on the tracker for a value that belongs to settings.

use std::path::PathBuf;

use tauri::{AppHandle, Manager, State};

use super::file;
use crate::context::Context;
use super::model::{merge, resolve, ResolvedSettings};
use crate::project;

/// Settings have almost no read errors: the file may be missing, it may be
/// broken — that is ordinary life, not a failure. There are two real troubles:
/// nowhere to write, and the write did not go through.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("could not determine the settings directory: {0}")]
    Dir(String),
    #[error("could not save the settings: {0}")]
    Write(String),
}

// Tauri requires a command's error to be serializable.
impl serde::Serialize for SettingsError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub(crate) fn settings_path(ctx: &Context) -> Result<PathBuf, SettingsError> {
    ctx.config_dir()
        .map(|dir| dir.join("settings.json"))
        .map_err(|err| SettingsError::Dir(err.to_owned()))
}

/// Where the file is, for the two readers that hold an `AppHandle` and not a context.
fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("settings.json"))
}

/// Whether the update timer may ask the release feed by itself.
///
/// Beside `role_pair` above and read the same way, from the disk on each call — and
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
        .unwrap_or_else(|| super::model::Settings::default().updates.auto_check)
}

/// How big one dialog window was left. `None` — including when there is no
/// settings path at all — asks for the height the content comes to.
pub fn dialog_size(app: &AppHandle, kind: &str) -> Option<super::model::DialogSize> {
    file::dialog_size(&path(app)?, kind)
}

/// Keeps how big one dialog window was left. A failure is a warning and nothing
/// more: the window on screen is the size the person made it either way, and
/// what is lost is that size at the next opening.
pub fn remember_dialog_size(app: &AppHandle, kind: &str, size: super::model::DialogSize) {
    let Some(path) = path(app) else {
        return;
    };
    if let Err(err) = file::remember_dialog_size(&path, kind, size) {
        log::warn!("settings: the dialog size was not kept: {err}");
    }
}

/// The successful end of `settings_save`, kept as a seam so the wake contract
/// can be exercised without inventing a Tauri application directory in a unit
/// test. Failed writes never reach this function.
fn saved_settings() {
    crate::settings::notify_run_settings_changed();
}

/// `project` means "show me this project's state": that is how the front end
/// gets another project's layout when switching. With no argument we answer
/// about the active project from the file.
#[tauri::command]
pub async fn settings_load(
    ctx: State<'_, Context>,
    project: Option<String>,
) -> Result<ResolvedSettings, SettingsError> {
    let path = settings_path(&ctx)?;
    load_from(&path, project)
}

/// The body of `settings_load` once the file is located, for the phone bridge:
/// the allowlisted commands are plain functions that the Tauri wrapper and
/// `remote::bridge` both call. The path is an argument because finding it takes
/// the context.
pub fn load_from(path: &std::path::Path, project: Option<String>) -> Result<ResolvedSettings, SettingsError> {
    let (settings, problem) = file::load(path);
    match problem {
        Some(file::Problem::Broken) => {
            log::warn!("settings: {} did not read, a copy is next to it in .bak", path.display())
        }
        Some(file::Problem::TooNew) => {
            log::warn!("settings: {} is newer than this build, a copy is next to it in .bak", path.display())
        }
        Some(file::Problem::Unreadable) => {
            log::warn!("settings: {} could not be read, the settings started from defaults", path.display())
        }
        None => {}
    }

    let mut view = resolve(&settings, project.as_deref());
    // The first run: there is no list yet. We open with the directory the app
    // was launched from, if it is tracked; the front end puts it in the list —
    // reading does not change the file.
    if view.open_projects.is_empty() && view.active_project.is_none() {
        view.active_project =
            project::default_project().map(|dir| dir.to_string_lossy().into_owned());
    }
    Ok(view)
}

/// The file is re-read on every write: a person may have edited it between two
/// saves, and other projects' entries in it must not be lost.
#[tauri::command]
pub async fn settings_save(ctx: State<'_, Context>, settings: ResolvedSettings) -> Result<(), SettingsError> {
    let path = settings_path(&ctx)?;
    let (mut stored, problem) = file::load(&path);

    /* The asymmetry is deliberate. A broken or too-new file has already gone
       to .bak — writing over it there is safe, while a refusal would leave a
       person permanently unable to save. With an unreadable file it is the
       other way round: there was nothing to take a copy from, and `rename` asks
       the directory for permissions only, so a write would silently erase
       something we could not even read. */
    if matches!(problem, Some(file::Problem::Unreadable)) {
        return Err(SettingsError::Write(format!(
            "{}: the existing file could not be read, so it was not overwritten",
            path.display()
        )));
    }

    merge(&mut stored, settings, chrono::Utc::now().to_rfc3339());
    file::save(&path, &stored).map_err(SettingsError::Write)?;
    saved_settings();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_successful_settings_save_wakes_a_waiting_run() {
        let mut changed = crate::settings::run_settings_changes();
        let before = *changed.borrow_and_update();

        // This is the exact post-write hook above. A disk failure returns
        // before it, so a waiting run is only woken for a saved policy.
        saved_settings();
        changed.changed().await.expect("settings change sender stays alive");

        assert_ne!(*changed.borrow(), before);
    }
}
