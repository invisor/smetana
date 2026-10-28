//! The invoke handler of the shell: what the webview's `invoke` reaches.
//!
//! The webview keeps calling `invoke('tracker_snapshot', …)` under the names it always did. A name the
//! shell answers itself (a window, a dialog, the clipboard, the opener, the client's updater and login
//! item, the connection's own state) goes to the command registered for it; **every other name is
//! forwarded** to the server, whose dispatcher is the one that knows the commands. That is the generic
//! form of forwarding the design leaves the builder to choose: there is no per-command wrapper to keep
//! in step, and a command the server gains reaches the webview with no change here.
//!
//! The arguments are passed as they arrive. The webview names them camelCase, and so does the
//! server's dispatcher (`remote-server/src/dispatch.rs`), which is what lets a phone and a desktop
//! share it. The reply is the server's value; a failure is the server's own error value.

use std::sync::Arc;

use remote_server::bridge::CLIENT_COMMANDS;
use serde_json::Value;
use tauri::ipc::{Invoke, InvokeBody, InvokeError};
use tauri::{Manager, Runtime};

use super::link::Link;

/// The commands this shell registers for itself: the client's own (`CLIENT_COMMANDS`, the server's
/// list of what is not its). `lib.rs` registers exactly these, and a test holds the two lists equal.
pub const LOCAL_COMMANDS: &[&str] = &[
    "sessions_open_log",
    "sessions_open_cwd",
    "sessions_reveal",
    "files_clipboard_write",
    "files_clipboard_read",
    "settings_window_open",
    "window_chrome",
    "drag_drop_space",
    "compare_window_open",
    "image_window_open",
    "dialog_window_open",
    "dialog_window_close",
    "dialog_window_size",
    "dialog_window_closable",
    "window_show_ready",
    "autostart_state",
    "autostart_set",
    "updates_state",
    "updates_check",
    "updates_install",
    "connection_state",
    "connection_retry",
    "device_settings_load",
    "device_settings_save",
    "server_target",
    "server_use_local",
    "server_use_remote",
    "server_local_work",
    "tracker_access_reset",
    "client_file_read",
    "client_open_document",
    "client_open_fetched",
];

/// Where a command goes.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// The shell answers it.
    Local,
    /// A client command this shell does not register: refused rather than sent to a server that
    /// would only answer `Forbidden`. A test keeps the set empty.
    Unavailable,
    /// The server answers it.
    Forward,
}

pub fn route(cmd: &str) -> Route {
    if LOCAL_COMMANDS.contains(&cmd) {
        Route::Local
    } else if CLIENT_COMMANDS.contains(&cmd) {
        Route::Unavailable
    } else {
        Route::Forward
    }
}

/// The commands that only read, and so may wait for the first connection of the run. A call the
/// server would act on (a start, a write, a switch of project) is never held: held, it would be sent
/// after all, and a double press made while the server was starting would start two of whatever it
/// starts. Anything not named here is refused at once while there is no connection.
const READS: &[&str] = &[
    "tracker_health",
    "tracker_snapshot",
    "tracker_resync",
    "tracker_failure",
    "tracker_probe",
    "project_root",
    "home_dir",
    "settings_load",
    "agents_catalog",
    "agents_installed",
    "codex_models",
    "agent_usage",
    "project_config",
    "browser_tools",
    "target_branches",
    "run_state",
    "run_reports",
    "terminal_list",
    "terminal_marks",
    "terminal_restorable",
    "terminal_attach",
    "session_list",
    "session_attach",
    "session_since",
    "crew_list",
    "crew_tree",
    "crew_attach",
    "sessions_list",
    "files_list",
    "files_read",
    "files_stat",
    "git_head",
    "vcs_repos",
    "vcs_status",
    "vcs_in_progress",
    "vcs_branches",
    "vcs_remote_branches",
    "vcs_last_fetch",
    "vcs_tracking",
    "vcs_file_at_head",
    "vcs_file_at_rev",
    "vcs_compare",
    "image_read",
    "attachments_survey",
    "remote_devices",
    "server_work",
    "folders_list",
    "server_file_read",
];

/// Whether `cmd` may wait for the first connection.
pub fn is_read(cmd: &str) -> bool {
    READS.contains(&cmd)
}

/// Wraps the handler `generate_handler!` made for the local commands.
pub fn handler<R: Runtime>(
    local: impl Fn(Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke: Invoke<R>| {
        let cmd = invoke.message.command().to_owned();
        match route(&cmd) {
            Route::Local => local(invoke),
            Route::Unavailable => {
                invoke.resolver.reject(format!("`{cmd}` is this app's own command, and this app does not answer it."));
                true
            }
            Route::Forward => {
                let args = match invoke.message.payload() {
                    InvokeBody::Json(value) => value.clone(),
                    InvokeBody::Raw(_) => {
                        invoke.resolver.reject(format!("`{cmd}` was called with a binary body, which is not forwarded."));
                        return true;
                    }
                };
                let app = invoke.message.webview().app_handle().clone();
                let Some(link) = app.try_state::<Arc<Link>>() else {
                    invoke.resolver.reject(format!("The connection to the server has not been set up. {}.", super::state::NOT_SENT));
                    return true;
                };
                let link = link.inner().clone();
                invoke.resolver.respond_async(async move {
                    let result: Result<Value, Value> = link.call(&cmd, args).await;
                    result.map_err(InvokeError)
                });
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_server_commands_are_forwarded_and_the_machines_own_are_not() {
        assert_eq!(route("tracker_snapshot"), Route::Forward);
        assert_eq!(route("terminal_write"), Route::Forward);
        assert_eq!(route("settings_save"), Route::Forward);
        assert_eq!(route("home_dir"), Route::Forward, "the server's home, not this machine's");
        assert_eq!(route("window_chrome"), Route::Local);
        assert_eq!(route("dialog_window_open"), Route::Local);
        assert_eq!(route("updates_check"), Route::Local);
        assert_eq!(route("connection_state"), Route::Local);
    }

    #[test]
    fn a_command_nobody_has_heard_of_goes_to_the_server_which_will_say_so() {
        assert_eq!(route("no_such_command"), Route::Forward);
    }

    #[test]
    fn resetting_a_folder_permission_is_the_shells_own_and_never_sent() {
        assert_eq!(route("tracker_access_reset"), Route::Local);
    }

    /// Every command the server lists as the client's is registered here, so none of them is sent to a
    /// server that would only answer `Forbidden`, and none is refused for want of a handler.
    #[test]
    fn every_client_command_is_answered_here() {
        for cmd in CLIENT_COMMANDS {
            assert_eq!(route(cmd), Route::Local, "`{cmd}` is the client's and this shell does not answer it");
        }
        for cmd in LOCAL_COMMANDS {
            assert!(
                CLIENT_COMMANDS.contains(cmd),
                "`{cmd}` is registered locally but the server does not list it as the client's"
            );
        }
    }

    #[test]
    fn only_reads_wait_for_the_first_connection() {
        for cmd in ["tracker_snapshot", "files_read", "terminal_list", "settings_load", "session_list"] {
            assert!(is_read(cmd), "{cmd}");
        }
        for cmd in [
            "terminal_create", "terminal_shell", "terminal_write", "run_start", "session_start", "tracker_update",
            "tracker_init", "tracker_set_project", "settings_save", "files_write", "vcs_commit", "server_restart",
        ] {
            assert!(!is_read(cmd), "{cmd} must be refused, not held");
        }
        for cmd in READS {
            assert_eq!(route(cmd), Route::Forward, "{cmd}");
        }
    }

    /// `lib.rs` registers exactly `LOCAL_COMMANDS` for the shell.
    #[test]
    fn lib_rs_registers_exactly_the_local_commands() {
        let lib = include_str!("../lib.rs");
        let from = lib.rfind("generate_handler![").expect("lib.rs registers the local commands");
        let body = &lib[from..];
        let body = &body[..body.find("])").unwrap()];
        let mut registered: Vec<&str> = body
            .lines()
            .skip(1)
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("//"))
            .map(|line| line.trim_end_matches(',').rsplit("::").next().unwrap())
            .collect();
        let mut listed: Vec<&str> = LOCAL_COMMANDS.to_vec();
        registered.sort_unstable();
        listed.sort_unstable();
        assert_eq!(listed, registered, "LOCAL_COMMANDS and the generate_handler! in lib.rs have drifted");
    }
}
