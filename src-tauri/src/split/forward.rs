//! The invoke handler of the split build: what the webview's `invoke` reaches.
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

/// The commands this shell registers for itself in the split build: the client's own
/// (`CLIENT_COMMANDS`, the server's list of what is not its) less the one that cannot be served, plus
/// the connection's. `lib.rs` registers exactly these, and a test holds the two lists equal.
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
];

/// Client commands the split build does not have, and says so: `tracker_access_reset` resets the
/// system's stored refusal of a folder for the process that holds it and restarts the app, and the
/// process that reads the folder is now the server's.
const UNAVAILABLE: &[&str] = &["tracker_access_reset"];

/// Where a command goes.
#[derive(Debug, PartialEq, Eq)]
pub enum Route {
    /// The shell answers it.
    Local,
    /// A client command this build cannot serve.
    Unavailable,
    /// The server answers it.
    Forward,
}

pub fn route(cmd: &str) -> Route {
    if LOCAL_COMMANDS.contains(&cmd) {
        Route::Local
    } else if UNAVAILABLE.contains(&cmd) || CLIENT_COMMANDS.contains(&cmd) {
        Route::Unavailable
    } else {
        Route::Forward
    }
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
                invoke.resolver.reject(format!("`{cmd}` is not available in this build: the server is a separate process."));
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
                    invoke.resolver.reject("The connection to the server has not been set up.".to_owned());
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
    fn a_client_command_the_split_build_lacks_is_refused_here_and_never_sent() {
        assert_eq!(route("tracker_access_reset"), Route::Unavailable);
    }

    /// Every command the server lists as the client's is either registered here or declared
    /// unavailable, so none of them is sent to a server that would only answer `Forbidden`.
    #[test]
    fn every_client_command_is_local_or_declared_unavailable() {
        for cmd in CLIENT_COMMANDS {
            assert_ne!(route(cmd), Route::Forward, "`{cmd}` is the client's and would be forwarded");
        }
        for cmd in LOCAL_COMMANDS {
            assert!(
                CLIENT_COMMANDS.contains(cmd),
                "`{cmd}` is registered locally but the server does not list it as the client's"
            );
        }
    }

    /// `lib.rs` registers exactly `LOCAL_COMMANDS` for the split build.
    #[test]
    fn lib_rs_registers_the_local_commands_for_the_split_build() {
        let lib = include_str!("../lib.rs");
        let from = lib.rfind("generate_handler![").expect("lib.rs registers the split commands");
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
        assert_eq!(listed, registered, "LOCAL_COMMANDS and the split generate_handler! in lib.rs have drifted");
    }
}
