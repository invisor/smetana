use std::path::PathBuf;

use smetana_core::context::Context;
use smetana_core::settings::commands as logic;
use smetana_core::settings::commands::SettingsError;
use smetana_core::settings::model::ResolvedSettings;
use tauri::{AppHandle, Manager, State};

/// The device's own half of the settings, read from the file on this machine: the same read
/// `settings_load` makes, of a file whose server fields are not this device's to use. See
/// `settings::split` in the core crate. The server's half is the server's and goes over the connection.
#[tauri::command]
pub async fn device_settings_load(ctx: State<'_, Context>, project: Option<String>) -> Result<ResolvedSettings, SettingsError> {
    logic::settings_load(&ctx, project).await
}

/// Saves the device's fields of the settings into the file on this machine and leaves the server's
/// as that file has them.
#[tauri::command]
pub async fn device_settings_save(ctx: State<'_, Context>, settings: ResolvedSettings) -> Result<(), SettingsError> {
    logic::device_settings_save(&ctx, settings).await
}

/// Where the file is, for the two readers that hold an `AppHandle` and not a context.
fn path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|dir| dir.join("settings.json"))
}

/// Whether the update timer may ask the release feed by itself.
///
/// Beside `settings::role_pair` in the core and read the same way, from the disk on each call — and
/// here that is not merely acceptable but the mechanism: `updates::schedule`
/// asks at every tick, which is what lets the switch stop and restart the
/// scheduled check without a restart of the app. The opposite of
/// `git_remove_worktrees`' caller, which reads once and carries the answer for
/// the whole of a run.
///
/// A platform that will not name a config directory answers `true`, the shipped
/// state — the same fallback `smetana_core::settings::file::updates_auto_check` makes, and for the
/// reason written there.
pub fn updates_auto_check(app: &AppHandle) -> bool {
    path(app)
        .map(|path| smetana_core::settings::file::updates_auto_check(&path))
        .unwrap_or_else(|| smetana_core::settings::model::Settings::default().updates.auto_check)
}

/// How big one dialog window was left. `None` — including when there is no
/// settings path at all — asks for the height the content comes to.
pub fn dialog_size(app: &AppHandle, kind: &str) -> Option<smetana_core::settings::model::DialogSize> {
    smetana_core::settings::file::dialog_size(&path(app)?, kind)
}

/// Keeps how big one dialog window was left. A failure is a warning and nothing
/// more: the window on screen is the size the person made it either way, and
/// what is lost is that size at the next opening.
pub fn remember_dialog_size(app: &AppHandle, kind: &str, size: smetana_core::settings::model::DialogSize) {
    let Some(path) = path(app) else {
        return;
    };
    if let Err(err) = smetana_core::settings::file::remember_dialog_size(&path, kind, size) {
        log::warn!("settings: the dialog size was not kept: {err}");
    }
}
