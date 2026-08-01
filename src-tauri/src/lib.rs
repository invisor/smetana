mod project;
mod settings;
mod tracker;

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
      // Чем открыться, знает файл настроек: там лежит активный проект прошлого
      // запуска. Читаем его здесь, а не ждём фронт, — доска успевает
      // загрузиться, пока поднимается вебвью. Файла нет или список пуст —
      // берём каталог, из которого запустили, если он отслеживается; иначе
      // проекта нет, и это нормальное состояние, а не сбой.
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
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      tracker::commands::tracker_health,
      tracker::commands::tracker_snapshot,
      tracker::commands::tracker_resync,
      tracker::commands::tracker_create,
      tracker::commands::tracker_update,
      tracker::commands::tracker_close,
      tracker::commands::tracker_reopen,
      tracker::commands::tracker_set_project,
      tracker::commands::tracker_init,
      tracker::commands::tracker_probe,
      tracker::commands::project_root,
      settings::commands::settings_load,
      settings::commands::settings_save,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
