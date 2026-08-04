mod agents;
mod files;
mod git;
mod project;
mod runs;
mod settings;
mod terminal;
mod tracker;
mod window;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_window_state::Builder::default().build())
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      // The settings file knows what to open with: the last run's active
      // project lives there. We read it here rather than waiting for the front
      // end — the board gets to load while the webview comes up. No file, or an
      // empty list, means we take the directory the app was launched from if it
      // is tracked; otherwise there is no project, and that is a normal state,
      // not a failure.
      let initial = app
        .path()
        .app_config_dir()
        .ok()
        .map(|dir| dir.join("settings.json"))
        .and_then(|path| settings::file::load(&path).0.last_project)
        .map(std::path::PathBuf::from)
        .or_else(project::default_project);

      let handle = tracker::service::start(app.handle().clone(), initial);
      app.manage(handle);

      // The terminal worker knows no project of its own: a session carries
      // the directory it was created in, and the front end asks for the list
      // by that directory.
      let terminal = terminal::service::start(app.handle().clone());
      app.manage(terminal);

      // The plugin writes the window geometry only on exit; here it starts
      // being written along the way too, so that a run cut short without a
      // clean exit does not open at the size from the run before last.
      window::persist_geometry(app.handle());
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      tracker::commands::tracker_health,
      tracker::commands::tracker_snapshot,
      tracker::commands::tracker_resync,
      tracker::commands::tracker_update,
      tracker::commands::tracker_close,
      tracker::commands::tracker_reopen,
      tracker::commands::tracker_delete,
      tracker::commands::tracker_set_project,
      tracker::commands::tracker_init,
      tracker::commands::tracker_probe,
      tracker::commands::project_root,
      files::commands::files_list,
      files::commands::files_read,
      files::commands::files_write,
      files::commands::files_stat,
      git::git_head,
      runs::commands::project_config,
      settings::commands::settings_load,
      settings::commands::settings_save,
      terminal::commands::terminal_list,
      terminal::commands::terminal_create,
      terminal::commands::terminal_remove,
      terminal::commands::terminal_attach,
      terminal::commands::terminal_detach,
      terminal::commands::terminal_resize,
      terminal::commands::terminal_write,
      terminal::commands::terminal_run_capture,
    ])
    // build + run instead of .run(context): we need the exit event. This is
    // exactly what Builder::run does — build, then run — plus our callback.
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|app_handle, event| {
      // Exit, not ExitRequested: the latter only reports an intention and can
      // be prevented, while agents must be killed once leaving is settled.
      // The callback runs before cleanup_before_exit, so the app is still whole.
      if let tauri::RunEvent::Exit = event {
        terminal::service::shutdown(app_handle);
      }
    });
}
