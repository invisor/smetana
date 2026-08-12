mod agents;
mod attachments;
mod files;
mod git;
mod project;
mod runs;
mod settings;
mod shell_env;
mod terminal;
mod tracker;
mod window;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_window_state::Builder::default().build())
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      // Before anything asks for an agent. A bundled app is handed launchd's
      // environment rather than the person's, so the answer takes a login
      // shell to get; starting it here means the first "+ New agent" does not
      // wait for one. See `shell_env`.
      shell_env::warm();
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
      let stored = settings::path(app.handle()).map(|path| settings::file::load(&path).0);
      let initial = stored
        .as_ref()
        .and_then(|settings| settings.last_project.clone())
        .map(std::path::PathBuf::from)
        .or_else(project::default_project);
      // Every project this launch knows about, for the run worker's start-up
      // sweep: a `kill -9` leaves agent processes running and a record of them
      // in each project's `.smetana/runs.json`, and this is the list of files
      // to go and finish. The open list rather than the active project alone —
      // a run in a project somebody has since switched away from left the same
      // orphans — and one that was closed altogether waits until it is opened
      // again, which is the price of the registry living in the project.
      let known: Vec<std::path::PathBuf> = stored
        .map(|settings| settings.open_projects)
        .unwrap_or_default()
        .into_iter()
        .map(std::path::PathBuf::from)
        .chain(initial.clone())
        .collect();

      let tracker = tracker::service::start(app.handle().clone(), initial);
      app.manage(tracker.clone());

      // The terminal worker knows no project of its own: a session carries
      // the directory it was created in, and the front end asks for the list
      // by that directory.
      let terminal = terminal::service::start(app.handle().clone());
      app.manage(terminal.clone());

      // The run worker drives the other two rather than owning anything of its
      // own: it reads the board from the tracker and starts one session per
      // batch through the terminal. Handed clones of both, so it queues behind
      // them like every other caller.
      let runs = runs::service::start(app.handle().clone(), tracker.clone(), terminal, known);
      app.manage(runs);

      // The plugin writes the window geometry only on exit; here it starts
      // being written along the way too, so that a run cut short without a
      // clean exit does not open at the size from the run before last.
      window::persist_geometry(app.handle());
      // The settings window writes nothing itself — see `close_settings_with_main`.
      window::close_settings_with_main(app.handle());
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
      attachments::attachment_import,
      attachments::attachment_write,
      attachments::attachments_survey,
      attachments::attachments_clean,
      git::git_head,
      git::git_branches,
      runs::commands::project_config,
      runs::commands::browser_tools,
      runs::commands::run_start,
      runs::commands::run_stop,
      runs::commands::run_state,
      settings::commands::settings_load,
      settings::commands::settings_save,
      window::settings_window_open,
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
