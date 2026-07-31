mod tracker;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      // Проект один — тот, в котором лежит .beads. Выбор каталога появится позже.
      let project_dir = tracker::service::find_project_dir()?;
      let handle = tracker::service::start(app.handle().clone(), project_dir);
      app.manage(handle);
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      tracker::commands::tracker_snapshot,
      tracker::commands::tracker_resync,
      tracker::commands::tracker_create,
      tracker::commands::tracker_update,
      tracker::commands::tracker_close,
      tracker::commands::tracker_reopen,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
