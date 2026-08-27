//! The app's windows: the settings window, the compare window, and the main
//! window's geometry on disk.
//!
//! # The settings window
//!
//! A second `WebviewWindow` rather than a modal, and that is the whole reason it
//! is a window at all: a modal lives inside the main window's bounds and cannot
//! be dragged out of them, so a person cannot put the settings beside what they
//! are changing. It loads the very same bundle with `?view=settings` — the third
//! branch in `src/App.vue`, beside the app and the gallery — so there is one
//! front end, one build, and the settings UI stays reachable in `npm run dev`
//! with no Tauri behind it.
//!
//! The label is what makes "open it again" mean "bring it forward": a second
//! window over the same settings would be two views of one file with no way to
//! tell which one a person is reading.
//!
//! It is built as a **child of the main window**, which is the one thing that
//! keeps it in front of what it is changing. Without it the settings sank
//! behind the app on the first click into the board — a window whose entire
//! purpose is to be looked at beside another one, buried under that one, with
//! only the gear to dig it out again. `parent` says exactly that and no more,
//! in each platform's own words: an owner window on Windows, transient-for on
//! Linux, a child `NSWindow` on macOS. `always_on_top` was the other way to
//! reach it and is a different claim — it would float this window over every
//! other application on the machine, and settings for an app somebody has
//! switched away from have no business on top of their browser.
//!
//! What comes with it on macOS is the child moving when the parent moves. That
//! is the OS's bargain and it is worth taking: it keeps the pairing a person
//! arranged, and the window can still be dragged anywhere, including off the
//! app entirely, which is the whole reason this is a window and not a modal.
//!
//! # The compare window
//!
//! The same shape as the settings window and for the same reasons — one per
//! app, a child of the main window, the one bundle loaded with a `?view=` of
//! its own — with one difference worth naming: what it is looking at travels as
//! two parameters rather than one, and a window already open is re-aimed by an
//! event instead of by a URL it will never load again.
//!
//! # The main window's geometry
//!
//! `tauri-plugin-window-state` *keeps* them, and there is no point rewriting
//! that: multi-monitor setups, minimization and full screen are already handled.
//! **What it no longer does by itself is put them back.** The plugin is built
//! with `skip_initial_state("main")` (`lib.rs`), so the geometry it holds is
//! saved exactly as before and applied to no window on its own, and
//! `open_main_window` below is the one thing that ever applies it — because
//! whether it should be applied is a setting now (`settings.json`'s `window`
//! section, `settings/model.rs`).
//!
//! `skip_initial_state` rather than `with_denylist`, and the difference is the
//! whole design: the denylist takes the window out of the plugin altogether, so
//! the switch would mean "forget where it was" and turning it back on a week
//! later would open the window at the configured size. Skipping only the restore
//! keeps the saving unconditional, in both positions of the switch, which is
//! what makes it reversible the way a person expects.
//!
//! The main window is therefore created **hidden** (`"visible": false` in
//! `tauri.conf.json`) and shown here. Windows declared in the configuration are
//! built before the `setup` hook runs, so by the time this code decides
//! anything the window would already be on screen at the configured size, and
//! restoring from there is a visible jump. `open_main_window` shows it in both
//! branches: a restore that failed must not be able to leave the app with no
//! window at all.
//!
//! **That trailing `show()` is load-bearing rather than a precaution.** `FLAGS`
//! is `StateFlags::all()`, which includes `VISIBLE`, and `restore_state` shows
//! the window only when the state it read says the window was visible. Hiding
//! it in the configuration is what makes a stored `visible: false` reachable in
//! the first place — the plugin records visibility from the window as it finds
//! it — and in that case `restore_state` returns `Ok(())` with nothing on
//! screen, so this call is the only thing putting the app in front of anybody.
//! Do not tidy it away as redundant, and do not narrow `FLAGS` without reading
//! this first.
//!
//! Saving is the other half, and it is untouched by any of the above. The plugin
//! writes to disk in exactly one place, `RunEvent::Exit`, and holds everything in
//! memory until then. So any run that does not reach a clean exit — a crash, a
//! force quit, and in development every rebuild that kills the process — would
//! leave the run-before-last's geometry on disk, and the window would open
//! somewhere other than where it was left.
//!
//! Hence the write also happens along the way. The debounce is mandatory:
//! dragging a window's corner sends hundreds of events, and writing the file on
//! every one of them is hundreds of trips to the disk for a single number. The
//! debounce also settles the question of handler order: by the time the write
//! runs, the plugin's cache has certainly been updated by its own listener of
//! the same event.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::time::Duration;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_window_state::{AppHandleExt, StateFlags, WindowExt};

/// The settings window's label. It is also the name the capability in
/// `capabilities/default.json` lists beside `main`: a window not named there
/// reaches no core plugin at all — no events, no app version — and the settings
/// UI would come up as a page that cannot talk to anything.
const SETTINGS_LABEL: &str = "settings";

/// The compare window's label. One per app, exactly as the settings window is
/// one per app, and for the same reason: two windows over the same question are
/// two views of one thing with no way to tell which is which. It is also the
/// name `capabilities/default.json` has to list — a window not named there
/// reaches no core plugin at all, and the page would come up unable to talk to
/// anything.
const COMPARE_LABEL: &str = "compare";

/// Which section a caller asked for, as a query parameter on the URL the window
/// already loads — the mechanism `?view=` and `?theme=` are already built on.
///
/// The closed list of sections lives in `src/views/SettingsWindow.vue`, which
/// owns the tabs, and is deliberately not repeated here: that window already
/// falls back to General for a name it does not know, and a second copy of the
/// list in Rust would be one more pair to keep in step. What this function is
/// for is the URL rather than the tab — anything but a short plain identifier is
/// dropped, so nothing a caller sends can add a parameter of its own or escape
/// the query string.
fn tab_query(tab: Option<&str>) -> String {
    let plain = tab.filter(|name| {
        !name.is_empty()
            && name.len() <= 32
            && name.chars().all(|c| c.is_ascii_lowercase() || c == '-')
    });
    plain.map(|name| format!("&tab={name}")).unwrap_or_default()
}

/// Which repository and which branch, as parameters on the URL the window
/// already loads — the mechanism `?view=`, `?theme=` and `?tab=` are built on,
/// and what keeps the window checkable in `npm run dev` with no Tauri behind it.
///
/// Both are percent-encoded rather than validated the way `tab_query` validates
/// a section name: a section is a short identifier from a closed list, while a
/// repository is an absolute path and a branch name may hold almost anything a
/// ref allows. Dropping either would leave a window with nothing to compare.
fn compare_query(repo: &str, branch: &str) -> String {
    format!("&repo={}&branch={}", encode(repo), encode(branch))
}

/// Percent-encoding, unreserved characters kept. Written out rather than pulled
/// in: two call sites, and a dependency for eighteen lines is a dependency to
/// keep in step for eighteen lines.
fn encode(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for byte in raw.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// Opens the settings window on a section, or brings the open one forward.
///
/// The section only reaches a window being built: an open one is focused rather
/// than reloaded, exactly as it has always been, so telling *that* window which
/// section to show is a message rather than a URL — `settings:show`, on the
/// event channel the two windows already speak over. Reloading it instead would
/// throw away the tab a person is in the middle of reading to show them one they
/// pressed a button for, and re-ask the app window for everything it holds.
///
/// Deliberately not `async`: a synchronous command runs on the main thread,
/// which is where a window is created on every platform this app targets.
#[tauri::command]
pub fn settings_window_open(app: AppHandle, tab: Option<String>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        // Minimized counts as open, and focusing a minimized window leaves a
        // person pressing the gear with nothing on screen to show for it.
        let _ = window.unminimize();
        return window.set_focus().map_err(|err| err.to_string());
    }

    let mut builder = WebviewWindowBuilder::new(
        &app,
        SETTINGS_LABEL,
        WebviewUrl::App(format!("index.html?view=settings{}", tab_query(tab.as_deref())).into()),
    )
    .title("Settings")
    .inner_size(720.0, 560.0)
    .min_inner_size(520.0, 400.0)
    .resizable(true);

    // No main window is not an error here, any more than it is in
    // `close_children_with_main`: there is simply nothing to stay in front of,
    // and a settings window with no parent is better than none at all.
    if let Some(main) = app.get_webview_window("main") {
        builder = builder.parent(&main).map_err(|err| err.to_string())?;
    }

    builder.build().map(|_| ()).map_err(|err| err.to_string())
}

/// Opens the compare window on a branch, or brings the open one forward.
///
/// The pair travels twice for the reason the settings window's section does: a
/// window being built reads it off the URL, and an open one is focused rather
/// than rebuilt, so it can only be re-aimed by an event — `compare:show`, on
/// the channel the windows already speak over. Rebuilding it instead would
/// throw away the file somebody is in the middle of reading.
///
/// Deliberately not `async`: a synchronous command runs on the main thread,
/// which is where a window is created on every platform this app targets.
#[tauri::command]
pub fn compare_window_open(app: AppHandle, repo: String, branch: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(COMPARE_LABEL) {
        let _ = window.unminimize();
        return window.set_focus().map_err(|err| err.to_string());
    }

    let mut builder = WebviewWindowBuilder::new(
        &app,
        COMPARE_LABEL,
        WebviewUrl::App(format!("index.html?view=compare{}", compare_query(&repo, &branch)).into()),
    )
    .title("Compare branches")
    .inner_size(1040.0, 680.0)
    .min_inner_size(640.0, 400.0)
    .resizable(true);

    // A child of the main window for the reason the settings window is one: a
    // window whose whole purpose is to be looked at beside another one must not
    // sink behind it on the first click into the board.
    if let Some(main) = app.get_webview_window("main") {
        builder = builder.parent(&main).map_err(|err| err.to_string())?;
    }

    builder.build().map(|_| ()).map_err(|err| err.to_string())
}

/// How long we wait after the last movement. Less, and the write happens in the
/// middle of a drag; more, and a window closed right after a resize goes back to
/// relying on `Exit`.
const SETTLE: Duration = Duration::from_millis(500);

/// The same flags as `Builder::default()`: the plugin does not expose them, and
/// saving less than it restores means losing fields on every write of ours.
/// Both directions go through this one constant — `open_main_window` restores
/// with it and `persist_geometry` saves with it — so the pair cannot come apart.
const FLAGS: StateFlags = StateFlags::all();

/// Takes the app's other windows down with the main one.
///
/// The settings window is a viewer, not an owner: every edit it makes travels to
/// the main window, which is the only thing that writes `settings.json`. Left
/// standing after that window is gone it would keep accepting choices and
/// keeping none of them, with nothing on screen to say so — the one failure this
/// codebase refuses everywhere else. The compare window is the same shape one
/// step further: it can only be re-aimed by an event from the app window, so
/// after that window is gone it is a comparison nothing can ever change.
/// Closing them is also what lets the app exit on the last window, the way it
/// did before there was a second one.
///
/// `Destroyed` rather than `CloseRequested`: the front end intercepts the close
/// to flush its last write and destroys the window itself, so the request is
/// preventable and the destruction is not.
pub fn close_children_with_main(app: &AppHandle) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let app = app.clone();
    main.on_window_event(move |event| {
        if !matches!(event, WindowEvent::Destroyed) {
            return;
        }
        for label in [SETTINGS_LABEL, COMPARE_LABEL] {
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.close();
            }
        }
    });
}

/// Puts the main window where it was left, if that is what the person wants,
/// and shows it either way.
///
/// The window is hidden until this runs — see the header — so this is also the
/// only thing that ever puts it on screen. Hence the `show()` outside the
/// branch, which answers two different cases rather than one. A restore that
/// failed, or was never asked for, costs a window in the wrong place, and
/// showing it only on the way out of the restoring branch would cost the app
/// its window altogether. The other is `restore_state` **succeeding and
/// declining to show**: it carries `StateFlags::VISIBLE` and shows the window
/// only if the state it read was visible, so a stored `visible: false` —
/// reachable now that the window starts hidden — comes back `Ok(())` with
/// nothing on screen. This line is what answers that, and it is not redundant.
///
/// Neither failure is worth crashing over and neither is worth a dialog: the
/// worst of them is a window at the size the configuration names, which is
/// exactly what the app did before there was a setting.
///
/// There may be no window — a configuration without `main`, a headless run;
/// that is not an error, there is simply nothing to show.
pub fn open_main_window(app: &AppHandle, restore: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if restore {
        if let Err(err) = window.restore_state(FLAGS) {
            log::warn!("could not restore the window geometry: {err}");
        }
    }
    // macOS is served by `titleBarStyle: "Overlay"` in the configuration, which
    // keeps its real traffic lights over the bar the front end draws. No other
    // platform has such a style, so there the decorations come off outright and
    // `shell/WindowControls.vue` draws the three buttons instead.
    //
    // Here rather than in the configuration because `decorations` has no
    // per-platform form there and would take macOS's with it. It is free of a
    // flash for the reason the whole of this function exists: the window is
    // created hidden and is shown below.
    #[cfg(not(target_os = "macos"))]
    if let Err(err) = window.set_decorations(false) {
        log::warn!("could not drop the window decorations: {err}");
    }
    if let Err(err) = window.show() {
        log::warn!("could not show the main window: {err}");
    }
}

/// Which chrome the app window has, as one of the three names
/// `src/components/shell/windowChrome.js` holds.
///
/// A compile-time fact rather than a runtime one, which is why it is decided
/// here: the front end has no way to ask what it was built for, and a
/// user-agent string is a guess. The third name, `none`, is never returned —
/// it is what the store answers when this command cannot be reached at all,
/// which is a browser.
#[tauri::command]
pub fn window_chrome() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        "traffic-lights"
    }
    #[cfg(not(target_os = "macos"))]
    {
        "buttons"
    }
}

/// Subscribes the main window to writing its own geometry.
///
/// There may be no window — a configuration without `main`, a headless run;
/// that is not an error, there is simply nothing to write.
pub fn persist_geometry(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    // The counter cuts off everything but the last event: a task that wakes up
    // writes only if nobody arrived after it.
    let latest = Arc::new(AtomicU64::new(0));
    let app = app.clone();

    window.on_window_event(move |event| {
        if !matches!(event, WindowEvent::Resized(_) | WindowEvent::Moved(_)) {
            return;
        }
        let mine = latest.fetch_add(1, Ordering::SeqCst) + 1;
        let latest = latest.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SETTLE).await;
            if latest.load(Ordering::SeqCst) != mine {
                return;
            }
            if let Err(err) = app.save_window_state(FLAGS) {
                // No reason to crash: the window on screen does not change
                // because of it, and lost geometry costs one inconvenience at
                // the next launch.
                log::warn!("could not save the window geometry: {err}");
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::{compare_query, tab_query};

    #[test]
    fn a_section_rides_as_a_query_parameter() {
        assert_eq!(tab_query(Some("storage")), "&tab=storage");
        assert_eq!(tab_query(None), "");
    }

    /// Nothing a caller sends may add a parameter of its own or leave the query
    /// string. The window falls back to General for a name it does not know, so
    /// dropping the whole parameter costs one press landing on the first tab.
    #[test]
    fn anything_that_is_not_a_plain_name_is_dropped() {
        for odd in ["", "Storage", "storage&view=gallery", "sto rage", "../etc", "a".repeat(33).as_str()] {
            assert_eq!(tab_query(Some(odd)), "", "{odd:?}");
        }
    }

    /// The repository is an absolute path and a branch name may hold a slash, a
    /// space or anything else a ref allows, so both are percent-encoded into
    /// the query string. Nothing a caller sends may add a parameter of its own.
    #[test]
    fn the_pair_rides_as_percent_encoded_parameters() {
        assert_eq!(
            compare_query("/Users/me/my repo", "feat/a&b"),
            "&repo=%2FUsers%2Fme%2Fmy%20repo&branch=feat%2Fa%26b"
        );
    }

    #[test]
    fn nothing_in_a_name_can_escape_the_query_string() {
        let query = compare_query("/tmp/r", "x&view=gallery#y=z?q");
        assert_eq!(query.matches('&').count(), 2);
        assert!(!query.contains("view=gallery"));
    }
}
