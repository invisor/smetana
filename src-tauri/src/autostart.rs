//! The login item: whether the app opens itself when the person signs in.
//!
//! # The truth is in the operating system, not in `settings.json`
//!
//! Nothing about this is stored in the settings file, and that is the whole
//! design rather than an omission. A login item can be removed from outside the
//! app — macOS System Settings → General → Login Items, the Startup tab of the
//! Windows task manager, whatever the desktop environment on Linux calls its
//! own list — and a copy in a file of ours would then disagree with the machine.
//! From there only two things can be done and both are worse than having no
//! copy. Bringing the system into line with the file at start-up means the app
//! silently putting back what somebody removed by hand a minute earlier, in
//! their own system settings. Not reconciling at all means the switch stating a
//! position the machine does not hold until the next time it is pressed — a
//! control claiming a state the app does not have, which is the one failure
//! this codebase refuses everywhere else.
//!
//! So the plugin's own answer is read every time the tab is opened, and both
//! commands below answer with what was read back afterwards rather than with
//! what the caller asked for. A registration that failed therefore puts the
//! switch back by itself, and there is no error branch to design.
//!
//! # Not from a development build
//!
//! `supported` is false under `debug_assertions`, and the row is drawn disabled
//! rather than hidden. Enabling it from `npm run tauri dev` would write the path
//! of `target/debug/smetana` into the machine's own list of things to start,
//! where it outlives any `cargo clean` that removes the binary: the person then
//! gets a silent failure every morning, on a machine where nobody will look for
//! it, from an app they never installed. Disabled rather than hidden because a
//! row that is not there reads as "not built yet" to the next person, and
//! `?view=gallery` exists so that nothing has to be guessed at.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_autostart::ManagerExt;

/// What the switch is drawn from. `supported` is whether this build may touch
/// the machine's list at all; `enabled` is whether there is an entry in it
/// right now, as the plugin reports it and never as anybody asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutostartState {
    pub supported: bool,
    pub enabled: bool,
}

/// The answer a build that may not register anything gives to both commands.
/// `enabled: false` beside `supported: false` is not a claim about the machine:
/// the row is disabled, so nothing is being stated about a list this build is
/// not allowed to read.
const UNSUPPORTED: AutostartState = AutostartState { supported: false, enabled: false };

/// Reads the machine's list. A read that fails is answered as "no entry" rather
/// than as a refusal: the switch stays live, so pressing it is the way out, and
/// that is a better offer than a control disabled by something nobody can see.
fn read(app: &AppHandle) -> AutostartState {
    if cfg!(debug_assertions) {
        return UNSUPPORTED;
    }
    let enabled = app.autolaunch().is_enabled().unwrap_or_else(|err| {
        log::warn!("could not read whether the app starts at login: {err}");
        false
    });
    AutostartState { supported: true, enabled }
}

/// What the General tab asks when it is opened. Deliberately not asked when the
/// settings window mounts: the other five tabs have no use for the answer, and
/// this one reaches the operating system.
#[tauri::command]
pub fn autostart_state(app: AppHandle) -> AutostartState {
    read(&app)
}

/// The press. Never fails and never reports what was asked for — the answer is
/// a fresh read, so a registration the system declined comes back as the switch
/// returning to where it was.
#[tauri::command]
pub fn autostart_set(app: AppHandle, enabled: bool) -> AutostartState {
    if cfg!(debug_assertions) {
        return UNSUPPORTED;
    }
    let outcome =
        if enabled { app.autolaunch().enable() } else { app.autolaunch().disable() };
    if let Err(err) = outcome {
        // Not an error to the caller: the read below is what the switch draws,
        // and it already says the change did not happen.
        log::warn!("could not change whether the app starts at login: {err}");
    }
    read(&app)
}

#[cfg(test)]
mod tests {
    use super::UNSUPPORTED;

    /// A build that may not register anything says so, and says nothing about
    /// the machine's list beside it. Both commands take an `AppHandle` and so
    /// need a running app; what is reachable from here is the answer they hand
    /// back in that case, which is the one the settings window draws its
    /// disabled row from.
    #[test]
    fn a_build_that_may_not_register_claims_nothing_about_the_machine() {
        assert!(!UNSUPPORTED.supported);
        assert!(!UNSUPPORTED.enabled);
    }

    /// The two field names cross the IPC boundary and the settings window reads
    /// them by name, so the serialization is the contract rather than the
    /// struct.
    #[test]
    fn the_state_travels_as_supported_and_enabled() {
        let json = serde_json::to_string(&UNSUPPORTED).expect("the state must serialize");
        assert_eq!(json, r#"{"supported":false,"enabled":false}"#);
    }
}
