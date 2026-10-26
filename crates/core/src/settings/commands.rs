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
    #[error("the settings patch was refused: {0}")]
    Patch(String),
}

// A command's error has to be serializable for the shell to hand it to the webview.
impl serde::Serialize for SettingsError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub fn settings_path(ctx: &Context) -> Result<PathBuf, SettingsError> {
    ctx.config_dir()
        .map(|dir| dir.join("settings.json"))
        .map_err(|err| SettingsError::Dir(err.to_owned()))
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
pub async fn settings_load(
    ctx: &Context,
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
pub async fn settings_save(ctx: &Context, settings: ResolvedSettings) -> Result<(), SettingsError> {
    let path = settings_path(&ctx)?;
    // The same lock a patch takes (`split::apply_patch`): a whole-document write that landed between a
    // patch's read and its rename would be undone by it, or undo it. Nothing below awaits.
    let guard = super::split::WRITE.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
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
    drop(guard);
    saved_settings();
    Ok(())
}

/// `settings_device_half`, for a client whose earlier build kept the whole document here: the device's
/// half of the server's file, or `null` when the server has none. Read once by the migration.
pub async fn settings_device_half(ctx: &Context) -> Result<serde_json::Value, SettingsError> {
    let path = settings_path(ctx)?;
    Ok(super::split::device_half_of_file(&path)?.unwrap_or(serde_json::Value::Null))
}

/// `settings_patch`, the server's only road for a client to change its half of the settings: the
/// fields that changed, applied to the file under one lock and announced to every client as
/// `settings:changed`. Answers the server half now in force. See `split.rs` for what the half is.
pub async fn settings_patch(ctx: &Context, patch: serde_json::Value) -> Result<serde_json::Value, SettingsError> {
    let path = settings_path(ctx)?;
    let now = chrono::Utc::now().to_rfc3339();
    let announce = |half: &serde_json::Value| ctx.emit_json(super::split::SETTINGS_CHANGED, half.clone());
    let half = super::split::apply_patch(&path, &patch, &now, false, &announce)?;
    Ok(half.unwrap_or(serde_json::Value::Null))
}

/// `settings_adopt`, the migration's way in: the server half of a device's old settings file, taken
/// only by a server that has no settings of its own yet. Answers whether it was taken.
pub async fn settings_adopt(ctx: &Context, patch: serde_json::Value) -> Result<bool, SettingsError> {
    let path = settings_path(ctx)?;
    let now = chrono::Utc::now().to_rfc3339();
    let announce = |half: &serde_json::Value| ctx.emit_json(super::split::SETTINGS_CHANGED, half.clone());
    Ok(super::split::apply_patch(&path, &patch, &now, true, &announce)?.is_some())
}

/// `device_settings_save`, the client build's own half: the same whole save the single-file build
/// makes, but of the device's fields only (see `split::merge_device`). Never reaches a server.
pub async fn device_settings_save(ctx: &Context, settings: ResolvedSettings) -> Result<(), SettingsError> {
    let path = settings_path(ctx)?;
    let (mut stored, problem) = file::load(&path);
    if matches!(problem, Some(file::Problem::Unreadable)) {
        return Err(SettingsError::Write(format!(
            "{}: the existing file could not be read, so it was not overwritten",
            path.display()
        )));
    }
    super::split::merge_device(&mut stored, settings, chrono::Utc::now().to_rfc3339());
    file::save(&path, &stored).map_err(SettingsError::Write)
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

    struct Recorder(std::sync::Mutex<Vec<(String, serde_json::Value)>>);
    impl crate::context::Sink for Recorder {
        fn emit(&self, name: &str, payload: serde_json::Value) {
            self.0.lock().unwrap().push((name.to_owned(), payload));
        }
        fn push(&self, _: crate::remote::push::Message) {}
    }

    #[tokio::test]
    async fn a_patch_is_broadcast_as_settings_changed_with_the_half_now_in_force() {
        let dir = tempfile::tempdir().unwrap();
        let sink = std::sync::Arc::new(Recorder(Default::default()));
        let ctx = Context::new(Err("x".into()), Ok(dir.path().to_path_buf()), Err("x".into()), None, sink.clone());
        let answer = settings_patch(&ctx, serde_json::json!({"agent": "codex"})).await.unwrap();
        settings_patch(&ctx, serde_json::json!({"agentPrompt": "brief", "appearance": {"theme": "light"}})).await.unwrap();
        let heard = sink.0.lock().unwrap();
        assert_eq!(heard.len(), 2);
        assert!(heard.iter().all(|(name, _)| name == "settings:changed"));
        assert_eq!(heard[0].1, answer);
        assert_eq!(heard[1].1["agent"], "codex", "the second announcement still holds the first change");
        assert_eq!(heard[1].1["agentPrompt"], "brief");
        assert!(heard[1].1.get("appearance").is_none(), "the device's fields are never broadcast");
    }

    #[tokio::test]
    async fn adopting_is_announced_only_when_it_was_taken() {
        let dir = tempfile::tempdir().unwrap();
        let sink = std::sync::Arc::new(Recorder(Default::default()));
        let ctx = Context::new(Err("x".into()), Ok(dir.path().to_path_buf()), Err("x".into()), None, sink.clone());
        assert!(settings_adopt(&ctx, serde_json::json!({"agent": "codex"})).await.unwrap());
        assert!(!settings_adopt(&ctx, serde_json::json!({"agent": "claude"})).await.unwrap());
        assert_eq!(sink.0.lock().unwrap().len(), 1);
    }
}
