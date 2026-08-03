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

use tauri::{AppHandle, Manager};

use super::file;
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

fn settings_path(app: &AppHandle) -> Result<PathBuf, SettingsError> {
    app.path()
        .app_config_dir()
        .map(|dir| dir.join("settings.json"))
        .map_err(|err| SettingsError::Dir(err.to_string()))
}

/// `project` means "show me this project's state": that is how the front end
/// gets another project's layout when switching. With no argument we answer
/// about the active project from the file.
#[tauri::command]
pub async fn settings_load(
    app: AppHandle,
    project: Option<String>,
) -> Result<ResolvedSettings, SettingsError> {
    let path = settings_path(&app)?;
    let (settings, problem) = file::load(&path);
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
pub async fn settings_save(app: AppHandle, settings: ResolvedSettings) -> Result<(), SettingsError> {
    let path = settings_path(&app)?;
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
    file::save(&path, &stored).map_err(SettingsError::Write)
}
