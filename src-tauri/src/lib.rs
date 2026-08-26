mod agents;
mod attachments;
mod autostart;
mod files;
mod git;
mod project;
mod runs;
mod settings;
mod shell_env;
mod terminal;
mod tracker;
mod updates;
mod vcs;
mod window;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init())
    .plugin(tauri_plugin_opener::init())
    .plugin(tauri_plugin_clipboard_manager::init())
    // `skip_initial_state` and not `with_denylist`: the geometry goes on being
    // saved in both positions of the switch, and putting it back is
    // `window::open_main_window`'s decision — see `window.rs` for why the two
    // halves are split at all.
    .plugin(
      tauri_plugin_window_state::Builder::default()
        .skip_initial_state("main")
        .build(),
    )
    // The login item, and nothing about it in `settings.json`: the operating
    // system's own list is the whole of the truth. `autostart.rs` records why,
    // and why the switch is dead in a development build.
    .plugin(tauri_plugin_autostart::init(
      tauri_plugin_autostart::MacosLauncher::LaunchAgent,
      None,
    ))
    .plugin(tauri_plugin_dialog::init())
    // The two halves of updating in place: `updater` fetches `latest.json`,
    // verifies the signature and replaces the bundle; `process` is the relaunch
    // afterwards. Neither is granted anything in `capabilities/default.json`
    // and neither is called from the webview — `updates.rs` owns the whole
    // state machine and the front end calls its three commands. That module's
    // header records why the grants would be a hole rather than a formality.
    .plugin(tauri_plugin_updater::Builder::new().build())
    .plugin(tauri_plugin_process::init())
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
      // The same file says whether the window goes back where it was left. The
      // main window is created hidden (`tauri.conf.json`), so this call is also
      // what puts it on screen, and it is made here — as early as the answer
      // exists — because everything below only starts worker threads, and a
      // window that waited for them would be a window somebody waits for.
      // Restoring a window already on screen is a jump they watch happen, which
      // is the whole reason it starts hidden. No file at all is the first run,
      // which has nothing to restore either way, so the fallback is only the
      // shipped default agreeing with itself.
      window::open_main_window(
        app.handle(),
        stored.as_ref().map(|settings| settings.window.restore_geometry).unwrap_or(true),
      );
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

      // Whether there is a newer version, and the download of it, are the
      // app's own business rather than a window's: the About tab that shows
      // this lives in the settings window, which is closed as soon as it has
      // been read. Nothing here runs in a development build — `updates.rs`
      // records what an install would do to `target/debug`.
      app.manage(updates::Updates::default());
      updates::schedule(app.handle().clone());

      // The plugin writes the window geometry only on exit; here it starts
      // being written along the way too, so that a run cut short without a
      // clean exit does not open at the size from the run before last.
      window::persist_geometry(app.handle());
      // Neither of the app's other windows owns anything — see
      // `close_children_with_main`.
      window::close_children_with_main(app.handle());
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
      tracker::commands::tracker_repair,
      tracker::commands::tracker_failure,
      tracker::commands::tracker_probe,
      tracker::commands::tracker_search_semantic,
      tracker::commands::project_root,
      files::commands::files_list,
      files::commands::files_read,
      files::commands::files_write,
      files::commands::files_stat,
      files::commands::files_create,
      files::commands::files_mkdir,
      files::commands::files_trash,
      attachments::attachment_import,
      attachments::attachment_write,
      attachments::attachments_survey,
      attachments::attachments_clean,
      git::git_head,
      vcs::commands::vcs_repos,
      vcs::commands::vcs_status,
      vcs::commands::vcs_branches,
      vcs::commands::vcs_tracking,
      vcs::commands::vcs_fetch,
      vcs::commands::vcs_pull,
      vcs::commands::vcs_push,
      vcs::commands::vcs_checkout,
      vcs::commands::vcs_create_branch,
      vcs::commands::vcs_merge,
      vcs::commands::vcs_rebase,
      vcs::commands::vcs_abort,
      vcs::commands::vcs_commit,
      vcs::commands::vcs_suggest_message,
      vcs::commands::vcs_file_at_head,
      vcs::commands::vcs_file_at_rev,
      vcs::commands::vcs_compare,
      runs::commands::project_config,
      runs::commands::browser_tools,
      runs::commands::run_start,
      runs::commands::run_stop,
      runs::commands::run_state,
      runs::commands::target_branches,
      runs::commands::agent_usage,
      settings::commands::settings_load,
      settings::commands::settings_save,
      window::settings_window_open,
      window::compare_window_open,
      autostart::autostart_state,
      autostart::autostart_set,
      updates::updates_state,
      updates::updates_check,
      updates::updates_install,
      terminal::commands::terminal_list,
      terminal::commands::terminal_marks,
      terminal::commands::terminal_create,
      terminal::commands::terminal_shell,
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
