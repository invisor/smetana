// The `split` build leaves part of the in-process half unreferenced (the commands it forwards instead,
// the phone endpoint's start, the run gate of an install). It is kept compiling rather than cut out so
// the two builds cannot drift apart, and the allowance sits on exactly the modules that need it.
mod autostart;
mod clipboard;
#[cfg_attr(feature = "split", allow(dead_code, unused_imports))]
mod commands;
mod connection;
mod host;
#[cfg_attr(feature = "split", allow(dead_code, unused_imports))]
mod remote;
#[cfg(feature = "split")]
mod split;
#[cfg_attr(feature = "split", allow(dead_code, unused_imports))]
mod updates;
mod window;

// Everything that does the work lives in `crates/core`, which has no `tauri`. These bring its modules
// in under the names the shell has always used, so `crate::tracker::...` still reads the same.
// The `split` build starts none of the workers these name.
#[cfg_attr(feature = "split", allow(unused_imports))]
use smetana_core::{
  attachments, context, project, rlimit, runs, session, settings, shell_env, terminal,
  tracker,
};

use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// How much of one log file is written before it is rolled aside, and how many
/// rolled-aside files are kept. The numbers are here rather than left to the
/// plugin's defaults because both of the plugin's are wrong for this app: 40 KB
/// a file, and `KeepOne`, which deletes the previous file every time it rolls.
/// A run works unattended for a night, and a night's log is the only account of
/// what it did — a strategy that can delete the beginning of that account
/// deletes exactly the part somebody reads first.
///
/// So: 2 MiB apiece and ten rolled-aside files beside the one being written,
/// which is at most eleven files and 22 MiB in the person's log directory, and
/// it is a ceiling rather than an estimate. `KeepAll` would also never delete a
/// run's early lines, and it was not taken for the other half of the same
/// sentence: how much of somebody's disk this is allowed to occupy has to be a
/// number, and under `KeepAll` there is none. A night would have to write
/// 22 MiB — around 180 000 lines — before the oldest of the eleven is dropped.
///
/// One thing about that file to know before reading an incident by it: its
/// stamps are **UTC**, the plugin's default, while the run report beside it is
/// stamped local (`chrono::Local::now()`, `runs/service.rs`), so the two are an
/// offset apart and the offset is the reader's to hold.
/// `TimezoneStrategy::UseLocal` is not the fix and was not taken: the `time`
/// crate refuses `now_local()` in a process that has threads unless it is built
/// with `unsound_local_offset`, and the plugin quietly falls back to UTC — the
/// call would change the format and read as though the question had been
/// settled.
const LOG_FILE_SIZE: u128 = 2 * 1024 * 1024;
const LOG_FILES_KEPT: usize = 10;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  // First, and before the builder rather than inside the setup hook: launchd
  // hands a bundled app a soft descriptor limit of 256, every agent
  // `terminal::pty` starts inherits it, and a limit raised after a child exists
  // is of no use to that child. `rlimit` records what claude prints when it
  // gets the inherited number, and why the hard limit is not the answer.
  rlimit::raise();
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
        // The dialog windows are not the plugin's, and it is not enough to skip
        // restoring them. `restore_state` sets the size, sets the position and
        // — with `StateFlags::VISIBLE` and a stored `visible: true`, which is
        // what these entries hold — calls `show()`, from `on_window_ready`,
        // long before the page has measured anything. So a dialog kind opened
        // once before came up at its old geometry for a moment; worse,
        // `dialog_window_size` decides first paint by `!is_visible()`, which
        // that `show` made false, so such a window was never centred over the
        // main window at all. Their size is kept in `settings.json` now, by the
        // one rule that can tell a size somebody chose from one this app
        // computed. `with_filter` and not `with_denylist`, because the denylist
        // takes literal labels and the closed list of dialog kinds is the front
        // end's (`src/views/dialogRegistry.js`).
        //
        // The prefix is named rather than repeated. It is `pub(crate)` for this
        // one caller and reaches nothing outside the crate, which is the whole
        // cost; the alternative was a third copy of the literal that builds the
        // label and reads it back, and a copy that drifted would put these
        // windows back under the plugin silently — which is the bug this filter
        // is here to fix.
        .with_filter(|label| !label.starts_with(crate::window::DIALOG_PREFIX))
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
      // First in this hook, because everything below it writes to `log` and a
      // `log::warn!` made before a logger exists goes nowhere at all.
      //
      // Not under `cfg!(debug_assertions)`, which is where the Tauri template
      // put this line on the first day and where it stayed for a month: the
      // build that works unattended overnight is the release one, so the only
      // account of what a run did was written by the one build that never runs
      // a run. `TargetKind::LogDir` is `~/Library/Logs/<bundle identifier>/` on
      // macOS, `$XDG_DATA_HOME/<identifier>/logs` on Linux; `file_name: None`
      // takes the product name, so the file is `Smetana.log`. On a machine
      // that installed an earlier build the directory entry still reads
      // `smetana.log`, and goes on doing so until the file first passes
      // `LOG_FILE_SIZE`: APFS is case-insensitive and case-preserving by
      // default, so the creating open finds the file already there rather than
      // making a second one beside it — the same consequence the bundle
      // directory has under an in-place update. The first rotation ends it:
      // `rename_file_to_dated` moves the entry to `Smetana_<date>.log`, and the
      // `open_file` after it creates a `Smetana.log` that does not exist yet,
      // so the new spelling takes over and stays. One cosmetic cost is left
      // behind: `remove_old_files` matches archives with
      // `starts_with(&self.file_name)`, a case-sensitive comparison, so an
      // archive rotated under the old spelling stops counting toward
      // `KeepSome` and is never pruned. The set of those is bounded — after the
      // switch every new archive carries the new spelling and is pruned as
      // usual — and nothing here deletes a file it should not. Stdout stays
      // beside it, and that is the line `npm run tauri dev` prints.
      //
      // `TargetKind::Webview` is deliberately not a third target: the front end
      // does not read this log, nothing in the tree calls `attachConsole`, and
      // a receiver nobody reads is one more place a path can reach.
      //
      // The one plugin in this file whose failure is swallowed rather than
      // carried out on a `?`. Everything it does at registration is disk —
      // `create_dir_all` on the log directory, the creating open of
      // `Smetana.log`, the `read_dir` the rotation counts old files with — and
      // a `?` here leaves the setup hook, comes back out of `build()` as
      // `Error::Setup` and lands on the `expect` at the bottom of this file. In
      // a bundled app that panic has nowhere to print, so what a person gets is
      // the icon bouncing and no window: no dialog, and no log either, because
      // the log is the thing that failed. A full disk is the ordinary way in,
      // and this is an app that fills disks overnight with worktrees and
      // reports; a read-only home, a file where the directory should be and a
      // directory owned by somebody else are the others. Only registration
      // needs the guard — a write that fails afterwards falls back to stderr
      // inside fern and takes nothing down with it.
      if let Err(err) = app.handle().plugin(
        tauri_plugin_log::Builder::default()
          .level(log::LevelFilter::Info)
          .max_file_size(LOG_FILE_SIZE)
          .rotation_strategy(RotationStrategy::KeepSome(LOG_FILES_KEPT))
          .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
          ])
          .build(),
      ) {
        // There is nowhere to say this: the thing that says things is what
        // just failed to start.
        let _ = err;
      }
      // Whatever `raise` above had to say, said now that there is something to
      // say it to. See `rlimit`: it runs before the builder, so it cannot log.
      rlimit::report();
      // Before anything asks for an agent. A bundled app is handed launchd's
      // environment rather than the person's, so the answer takes a login
      // shell to get; starting it here means the first "+ New agent" does not
      // wait for one. See `shell_env`.
      #[cfg(not(feature = "split"))]
      shell_env::warm();
      // The settings file knows what to open with: the last run's active
      // project lives there. We read it here rather than waiting for the front
      // end — the board gets to load while the webview comes up. No file, or an
      // empty list, means we take the directory the app was launched from if it
      // is tracked; otherwise there is no project, and that is a normal state,
      // not a failure.
      // What the workers are given instead of this handle: the data directory, a sink for
      // events and the path of the bd sidecar. Built once, here, and managed so that a command
      // wrapper holding an `AppHandle` can hand the same one on (`host::context`).
      let ctx = host::build(app.handle());
      app.manage(ctx.clone());
      let stored = ctx.settings_path().map(|path| settings::file::load(&path).0);
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
      // The workers, the in-process half. The `split` build has none: it holds a connection to
      // a server that has them (`split/`), and what this block starts is that server's.
      #[cfg(not(feature = "split"))]
      {
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

        let tracker = tracker::service::start(ctx.clone(), initial);
        app.manage(tracker.clone());

        // The terminal worker knows no project of its own: a session carries
        // the directory it was created in, and the front end asks for the list
        // by that directory. `known` is passed for its own start-up sweep too
        // (smetana-kkz2, point 4 — macOS reads each project's own run registry
        // for a dead instance's coalition id) — the identical list
        // `runs::service::start` below sweeps its registries against, computed
        // once above rather than read a second time here.
        let terminal = terminal::service::start(ctx.clone(), known.clone());
        app.manage(terminal.clone());

        // The session worker: the driven half of the same idea, one tokio task
        // beside the terminal's and sharing nothing with it. It knows no project
        // of its own either — a session carries the directory it was started in.
        // What it owns that the terminal does not is the in-app permission
        // listener, which it binds once when it starts.
              let session = session::service::start(ctx.clone());
              app.manage(session.clone());

        // The run worker drives the other two rather than owning anything of its
        // own: it reads the board from the tracker and starts one session per
        // batch through the terminal. Handed clones of both, so it queues behind
        // them like every other caller.
              let runs = runs::service::start(
                  ctx.clone(),
                  tracker.clone(),
                  terminal,
                  session,
                  known,
              );
        app.manage(runs);

      }

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
      // The phone companion: the device key, the device list, and the endpoint
      // if a phone is already paired. Last, because it needs nothing from the
      // workers above and nothing waits for it.
      #[cfg(not(feature = "split"))]
      remote::start(app.handle().clone());
      #[cfg(feature = "split")]
      split::start(app.handle());
      // Neither of the app's other windows owns anything — see
      // `close_children_with_main`.
      window::close_children_with_main(app.handle());
      Ok(())
    })
    // Which commands the webview reaches in-process is the first list, and it is the one the
    // phone bridge's test holds equal to `DESKTOP_COMMANDS`. The `split` build registers only what
    // the machine itself answers, and `split::handler` sends every other name to the server; the
    // second list is held equal to `split::forward::LOCAL_COMMANDS` by a test.
    .invoke_handler({
      #[cfg(not(feature = "split"))]
      {
    tauri::generate_handler![
          commands::tracker::tracker_health,
          commands::tracker::tracker_snapshot,
          commands::tracker::tracker_resync,
          commands::tracker::tracker_update,
          commands::tracker::tracker_close,
          commands::tracker::tracker_reopen,
          commands::tracker::tracker_delete,
          commands::tracker::tracker_set_project,
          commands::tracker::tracker_init,
          commands::tracker::tracker_repair,
          commands::tracker::tracker_failure,
          commands::tracker::tracker_access_repair,
          commands::tracker::tracker_access_reset,
          commands::tracker::tracker_probe,
          commands::tracker::tracker_search_semantic,
          commands::tracker::project_root,
          commands::files::files_list,
          commands::files::files_read,
          commands::files::files_write,
          commands::files::files_stat,
          commands::files::files_create,
          commands::files::files_mkdir,
          commands::files::files_trash,
          commands::files::files_copy,
          commands::files::files_copy_external,
          commands::files::files_move,
          commands::files::files_rename,
          commands::files::files_clipboard_write,
          commands::files::files_clipboard_read,
          commands::attachments::attachment_import,
          commands::attachments::attachment_reopen,
          commands::attachments::attachment_write,
          commands::attachments::attachments_survey,
          commands::attachments::attachments_clean,
          commands::attachments::image_read,
          commands::git::git_head,
          commands::vcs::vcs_repos,
          commands::vcs::vcs_status,
          commands::vcs::vcs_in_progress,
          commands::vcs::vcs_branches,
          commands::vcs::vcs_remote_branches,
          commands::vcs::vcs_last_fetch,
          commands::vcs::vcs_tracking,
          commands::vcs::vcs_fetch,
          commands::vcs::vcs_pull,
          commands::vcs::vcs_push,
          commands::vcs::vcs_checkout,
          commands::vcs::vcs_checkout_remote,
          commands::vcs::vcs_create_branch,
          commands::vcs::vcs_rename_branch,
          commands::vcs::vcs_delete_branch,
          commands::vcs::vcs_merge,
          commands::vcs::vcs_rebase,
          commands::vcs::vcs_abort,
          commands::vcs::vcs_commit,
          commands::vcs::vcs_discard,
          commands::vcs::vcs_suggest_message,
          commands::vcs::vcs_file_at_head,
          commands::vcs::vcs_file_at_rev,
          commands::vcs::vcs_compare,
          commands::runs::project_config,
          commands::runs::project_config_save_defaults,
          commands::runs::browser_tools,
          commands::runs::run_start,
          commands::runs::run_stop,
          commands::runs::run_release,
          commands::runs::run_state,
          commands::runs::run_reports,
          commands::runs::target_branches,
          commands::runs::agent_usage,
          commands::agents::agents_catalog,
          commands::agents::codex_models,
          commands::agents::agents_installed,
          commands::sessions::sessions_list,
          commands::sessions::sessions_open_log,
          commands::sessions::sessions_open_cwd,
          commands::sessions::sessions_reveal,
          commands::sessions::sessions_delete,
          commands::settings::settings_load,
          commands::settings::settings_save,
          window::settings_window_open,
          window::window_chrome,
          window::home_dir,
          window::drag_drop_space,
          window::compare_window_open,
          window::image_window_open,
          window::dialog_window_open,
          window::dialog_window_close,
          window::dialog_window_size,
          window::dialog_window_closable,
          window::window_show_ready,
          autostart::autostart_state,
          autostart::autostart_set,
          updates::updates_state,
          updates::updates_check,
          updates::updates_install,
          commands::terminal::terminal_list,
          commands::terminal::terminal_marks,
          commands::terminal::terminal_create,
          commands::terminal::terminal_shell,
          commands::terminal::terminal_remove,
          commands::terminal::terminal_attach,
          commands::terminal::terminal_detach,
          commands::terminal::terminal_resize,
          commands::terminal::terminal_write,
          commands::terminal::terminal_clear,
          commands::terminal::terminal_run_capture,
          commands::terminal::terminal_restorable,
          commands::terminal::terminal_forget,
          commands::session::session_start,
                commands::session::crew_tree,
                commands::session::crew_attach,
                commands::session::crew_send,
                commands::session::crew_stop,
                commands::session::crew_clear,
          commands::session::session_attach,
          commands::session::session_since,
          commands::session::session_send,
          commands::session::session_answer,
          commands::session::session_stop,
          commands::session::session_close,
          remote::commands::remote_pair_begin,
          remote::commands::remote_pair_cancel,
          remote::commands::remote_pair_decide,
          remote::commands::remote_devices,
          remote::commands::remote_device_revoke,
          connection::connection_state,
          connection::connection_retry,
        ]
      }
      #[cfg(feature = "split")]
      {
        split::handler(tauri::generate_handler![
        commands::sessions::sessions_open_log,
        commands::sessions::sessions_open_cwd,
        commands::sessions::sessions_reveal,
        commands::files::files_clipboard_write,
        commands::files::files_clipboard_read,
        window::settings_window_open,
        window::window_chrome,
        window::drag_drop_space,
        window::compare_window_open,
        window::image_window_open,
        window::dialog_window_open,
        window::dialog_window_close,
        window::dialog_window_size,
        window::dialog_window_closable,
        window::window_show_ready,
        autostart::autostart_state,
        autostart::autostart_set,
        updates::updates_state,
        updates::updates_check,
        updates::updates_install,
        connection::connection_state,
        connection::connection_retry,
      ])
      }
    })
    // build + run instead of .run(context): we need the exit event. This is
    // exactly what Builder::run does — build, then run — plus our callback.
    .build(tauri::generate_context!())
    .expect("error while running tauri application")
    .run(|app_handle, event| {
      // Exit, not ExitRequested: the latter only reports an intention and can
      // be prevented, while agents must be killed once leaving is settled.
      // The callback runs before cleanup_before_exit, so the app is still whole.
      if let tauri::RunEvent::Exit = event {
        if let Some(terminal) = app_handle.try_state::<terminal::service::TerminalHandle>() {
          terminal::service::shutdown(&terminal);
        }
        // The driven sessions' children are agents too, and an app that left
        // them running would leave a `claude` in the process list every time it
        // closed. Beside the terminal's rather than inside it: the two workers
        // share no state, and each kills what it started.
        if let Some(session) = app_handle.try_state::<session::service::SessionHandle>() {
          session::service::shutdown(&session);
        }
        // And the phone endpoint, so connected phones are told rather than left
        // to time out.
        remote::shutdown(app_handle);
      }
    });
}
