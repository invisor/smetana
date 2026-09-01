//! The app's windows: the settings window, the compare window, the dialog
//! windows, the image window, and the main window's geometry on disk.
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
//! # The dialog windows
//!
//! The same argument as the settings window's, applied to the dialogs a person
//! decides something in: a modal cannot be dragged out of the main window's
//! bounds, so the board it is a question about stays behind a scrim while they
//! answer it. Each of them is a window on the same bundle under
//! `?view=dialog&kind=<name>`, labelled `dialog-<kind>` so opening an open one
//! brings it forward.
//!
//! Two things differ from the two windows above, and both follow from a dialog
//! being the size of what it says. It is built hidden at a provisional height
//! and shown only once the page has measured itself and called
//! `dialog_window_size` — the same order the main window uses, and for the same
//! reason. And it has two phases, because those two things are both true and
//! cannot be true at once. Until somebody drags it, the height is
//! content-driven and re-set on every change to the content, which is what a
//! dialog the size of what it says needs. From the first pixel of a drag the
//! window is the person's: nothing computed moves it again, the size is kept in
//! `settings.json` under `dialogs` so the next one opens at it, and the page
//! fills the window instead of measuring itself — `views/DialogWindow.vue` and
//! `overlays/Modal.vue` hold that half. `resize_is_the_hand` below is what tells
//! the two apart, since a `Resized` says nothing about who caused it — and its
//! header is the one to read before touching it, because the obvious mechanism
//! for that is wrong on two of the three platforms this ships to.
//!
//! These windows are also **outside `tauri-plugin-window-state`** — `lib.rs`
//! filters them out by label prefix. It cannot tell a size somebody chose from
//! one this file computed, and it restores and *shows* a window from its
//! `on_window_ready` hook, which used to defeat the `is_visible` test below and
//! leave a re-opened dialog uncentred.
//!
//! One thing travels the other way: a dialog window destroyed by its own frame
//! says so on the channel the dialog's own answers travel on, because the app
//! window owns every bit of a dialog's state and would otherwise go on serving
//! a window that is not there.
//!
//! The closed list of kinds is the front end's (`src/views/dialogRegistry.js`)
//! and is deliberately not repeated here; what this side checks is the URL, in
//! `kind_query`.
//!
//! # Re-aiming an open window, and the event that used to be lost
//!
//! Three of these windows — settings, compare and image — are focused rather
//! than reloaded when they are already open, so what they are to show next
//! reaches them as an event (`settings:show`, `compare:show`, `image:show`) and
//! never as a URL. That is the decision, not an accident: reloading would throw
//! away the tab somebody is reading, the file they have open in a comparison,
//! or the window they have just dragged onto a second monitor and sized.
//!
//! The hole it left is that a window exists from the moment it is built, long
//! before its webview has loaded and subscribed to anything. Tauri buffers no
//! events, so one sent into that gap is simply gone. The image window showed it
//! plainest, because its frame title is set on the same branch: the title
//! changes synchronously and reliably while the picture arrives only by the
//! event, so clicking a second thumbnail before the window had loaded left a
//! window **naming one picture and showing another** until it was clicked
//! again.
//!
//! So the sender keeps hold of it. `show_now_or_on_ready` emits the event and
//! remembers it under the window's label; the window, once it has loaded and
//! subscribed, calls `window_show_ready`, which emits whatever is being held
//! for it and forgets it. Only the newest is kept — a window re-aimed three
//! times before it finished loading wants the third picture and neither of the
//! other two.
//!
//! Nothing is held for a window one of these commands *builds*: what to show is
//! on the URL it is about to load, and a copy here would be the same picture
//! read a second time the moment the window announced itself. That is also why
//! building forgets what is held — the window before it may have been closed
//! with something still owed to it, and that would reach the new one.
//!
//! Held here rather than in the front end because only this side knows which of
//! the two branches a press took, and because any of the app's webviews can ask
//! for any of these windows: the new-task window is what opens the image window,
//! while the app window opens the other two. One holder answers for all of them;
//! a copy per webview would be several senders answering one announcement with
//! different pictures.
//!
//! Each event name is spelled once per side: `show_event` here, against
//! `SETTINGS_SHOW` and `IMAGE_SHOW` in `src/stores/app.js` and `COMPARE_SHOW`
//! in `src/stores/compare.js`. So are the payloads — `settings_show`,
//! `compare_show` and `image_show` here, against the fields those three
//! watchers read.
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

use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex, MutexGuard,
};
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, Window, WindowEvent};

use crate::settings;
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

/// The image window's label. One per app, the way the two above are one per
/// app, and the consequence is deliberate: clicking a second thumbnail shows
/// that picture in this same window rather than opening a second one, so two
/// screenshots cannot be put side by side. A window per picture would have
/// bought that and paid for it in windows to close one by one.
///
/// It is also the name `capabilities/default.json` has to list. A window not
/// named there reaches no core plugin at all: it would not hear `image:show`,
/// could not read its own picture back and could not close itself on Esc — and
/// nothing in either test suite can see that, since the front end's cannot read
/// a capability file and Rust's cannot reach a webview.
const IMAGE_LABEL: &str = "image";

/// A dialog window's label is `dialog-<kind>`: one window per kind, so opening
/// an open one brings it forward rather than making a second copy, exactly as
/// the settings window's single label does that for it.
///
/// The prefix is also what `capabilities/default.json` matches with the glob
/// `dialog-*`. A window not named there reaches no core plugin at all and comes
/// up unable to talk to anything — which is what to suspect first if a dialog
/// window opens blank.
pub(crate) const DIALOG_PREFIX: &str = "dialog-";

/// The last "show this" sent to a window that may not have been listening for
/// it yet, one entry per window label and only ever the newest.
///
/// A `Vec` rather than a map because it holds at most three things — one
/// settings window, one compare window, one image window — and because
/// `Vec::new` is a `const fn` where `HashMap::new` is not, which is what lets
/// this be a plain `static` with no lock-on-first-use around it.
///
/// The header of this file carries the whole argument for why anything is held
/// at all. What is worth repeating here is what is *not* held: nothing for a
/// dialog window, which already has a handshake of its own — `dialog:hello`
/// answered by `dialog:props`, in `src/stores/app.js` — and nothing for the
/// main window, which is never re-aimed.
static PENDING_SHOW: Mutex<Vec<(String, Value)>> = Mutex::new(Vec::new());

/// The one way in, so that a poisoned lock is answered in one place rather than
/// three. Poisoning is taken rather than propagated: a panic in one of the four
/// tiny functions below could only leave the list itself intact, and refusing
/// every later re-aim over it would cost the feature this whole mechanism is.
fn pending_show() -> MutexGuard<'static, Vec<(String, Value)>> {
    PENDING_SHOW
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Which event carries "show this" to a window, from that window's own label:
/// `settings` becomes `settings:show`, `compare` becomes `compare:show`,
/// `image` becomes `image:show`.
///
/// Derived rather than listed so this side spells the pattern once. The front
/// end spells the three names once each — `SETTINGS_SHOW` and `IMAGE_SHOW` in
/// `src/stores/app.js`, `COMPARE_SHOW` in `src/stores/compare.js` — and nothing
/// mechanical pairs the two sides: a rename on either would not fail, it would
/// leave an open window that is never re-aimed at all.
fn show_event(label: &str) -> String {
    format!("{label}:show")
}

/// What each of the three windows is told, in the words its own watcher reads:
/// `tab`, `path` and `name` in `src/stores/app.js`, `repo` and `branch` in
/// `src/stores/compare.js`. Three functions rather than three `json!` calls at
/// the call sites so that the shape is written once on this side and a test can
/// hold it against those watchers — nothing else pairs the two, and a field
/// renamed on either side costs the feature and fails nothing.
fn settings_show(tab: &str) -> Value {
    json!({ "tab": tab })
}

fn compare_show(repo: &str, branch: &str) -> Value {
    json!({ "repo": repo, "branch": branch })
}

fn image_show(path: &str, name: &str) -> Value {
    json!({ "path": path, "name": name })
}

/// Keeps the newest, and only the newest: a window re-aimed three times before
/// it finished loading wants the third picture and neither of the other two.
fn remember_show(held: &mut Vec<(String, Value)>, label: &str, payload: Value) {
    match held.iter_mut().find(|(name, _)| name == label) {
        Some(slot) => slot.1 = payload,
        None => held.push((label.to_string(), payload)),
    }
}

/// Drops what is held for a label without sending it. Called where a window is
/// about to be built: what it is to show is on the URL it will load, and what
/// was owed to the window before it is owed to nobody.
fn forget_show(held: &mut Vec<(String, Value)>, label: &str) {
    held.retain(|(name, _)| name != label);
}

/// What is owed to a window that has just announced itself, and nothing on a
/// second announcement: a window that has taken its picture is listening from
/// then on, and holding a copy back would re-aim it on its next reload.
fn take_show(held: &mut Vec<(String, Value)>, label: &str) -> Option<Value> {
    let at = held.iter().position(|(name, _)| name == label)?;
    Some(held.remove(at).1)
}

/// Tells an open window what to show now, and holds on to it in case that
/// window was built a moment ago and has not subscribed yet.
///
/// The event is emitted to every window rather than to the one named, which is
/// what the front end always did from its side: these three names are each
/// listened for in exactly one window, so a target would buy nothing and add a
/// second question about how a target is matched.
///
/// A failed emit is a warning and no second attempt. The window is up on
/// whatever it was showing, which is a smaller failure than not opening at all
/// — and what is held here is unaffected, so a window still loading is still
/// told the moment it announces itself.
fn show_now_or_on_ready(app: &AppHandle, label: &str, payload: Value) {
    remember_show(&mut pending_show(), label, payload.clone());
    if let Err(err) = app.emit(&show_event(label), payload) {
        log::warn!("could not tell the {label} window what to show: {err}");
    }
}

/// A window saying it has loaded, subscribed, and spent what came in on its own
/// URL — so anything held for it may be sent now.
///
/// Which window is asking is the webview's own label rather than an argument,
/// which is one fewer thing a caller can get wrong and one fewer name to keep in
/// step. A label nothing is held for is the ordinary case — every window that
/// was built rather than re-aimed announces itself too — and is answered with
/// silence.
///
/// The announcement has to come **after** the window has subscribed and after it
/// has drawn what its URL named, and both halves are the caller's to get right:
/// announcing before subscribing would lose the very event this exists to
/// deliver, and announcing before the URL is spent would let the URL overwrite
/// the newer picture.
#[tauri::command]
pub fn window_show_ready(app: AppHandle, window: Window) -> Result<(), String> {
    let label = window.label().to_string();
    let Some(payload) = take_show(&mut pending_show(), &label) else {
        return Ok(());
    };
    app.emit(&show_event(&label), payload)
        .map_err(|err| err.to_string())
}

/// Which dialog, as a parameter on the URL the window already loads.
///
/// Validated exactly the way `tab_query` validates a section name, and for the
/// same reason: this reaches a URL, so anything but a short plain identifier is
/// dropped and nothing a caller sends can add a parameter of its own or escape
/// the query string. Answering `None` rather than substituting a default is the
/// difference from `tab_query`, which has a tab to fall back to — there is no
/// sensible default dialog, and a window with no `kind` has nothing to draw.
///
/// The closed list of kinds lives in `src/views/dialogRegistry.js`, which owns
/// them, and is deliberately not repeated here: that window already refuses a
/// kind it does not know, and a second copy of the list in Rust would be one
/// more pair to keep in step.
fn kind_query(kind: &str) -> Option<String> {
    let ok = !kind.is_empty()
        && kind.len() <= 32
        && kind.chars().all(|c| c.is_ascii_lowercase() || c == '-');
    ok.then(|| kind.to_string())
}

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

/// Which picture, as parameters on the URL the window already loads — the same
/// mechanism `?view=`, `?theme=` and `?repo=` are built on, and what keeps the
/// window checkable in `npm run dev` with no Tauri behind it.
///
/// Both are percent-encoded rather than validated the way `kind_query` validates
/// a dialog kind, and for `compare_query`'s reason: a stored path is absolute
/// and a file name is whatever `stored_name` made of it, so dropping either
/// would leave a window with nothing to show.
///
/// The bytes stay where they are. An attachment's `url` in the front end is a
/// `data:` URL of up to 8 MiB of base64, which fits in no URL and would be
/// eleven megabytes over IPC per click; the window reads the file itself with
/// `attachment_reopen`, which is already confined to the store.
fn image_query(path: &str, name: &str) -> String {
    format!("&path={}&name={}", encode(path), encode(name))
}

/// Percent-encoding, unreserved characters kept. Written out rather than pulled
/// in: a handful of call sites, and a dependency for eighteen lines is a
/// dependency to keep in step for eighteen lines.
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
/// That message is sent from here rather than from the front end after this
/// call, and it is held until the window says it is listening: only this side
/// knows which of the two branches below was taken. The header of this file
/// carries the whole of it.
///
/// No section asked for is no message: the gear opens the settings on the tab
/// they were left on, which is what an open window is already showing.
///
/// Deliberately not `async`: a synchronous command runs on the main thread,
/// which is where a window is created on every platform this app targets.
#[tauri::command]
pub fn settings_window_open(app: AppHandle, tab: Option<String>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(SETTINGS_LABEL) {
        // Minimized counts as open, and focusing a minimized window leaves a
        // person pressing the gear with nothing on screen to show for it.
        let _ = window.unminimize();
        if let Some(name) = tab.as_deref() {
            show_now_or_on_ready(&app, SETTINGS_LABEL, settings_show(name));
        }
        return window.set_focus().map_err(|err| err.to_string());
    }

    // The section is on the URL below, so nothing is owed to the window about
    // to be built — including anything the window before it never collected.
    forget_show(&mut pending_show(), SETTINGS_LABEL);

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
        show_now_or_on_ready(&app, COMPARE_LABEL, compare_show(&repo, &branch));
        return window.set_focus().map_err(|err| err.to_string());
    }

    // The pair is on the URL below: nothing is owed to a window being built.
    forget_show(&mut pending_show(), COMPARE_LABEL);

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

/// Opens the image window on one attached picture, or re-aims the open one.
///
/// The picture travels twice for the reason the compare window's pair does: a
/// window being built reads it off the URL, and an open one is focused rather
/// than rebuilt, so the only way to re-aim it is an event — `image:show`, on
/// the channel the app's windows already speak over. Rebuilding it instead
/// would throw away the window somebody has just dragged onto their second
/// monitor and sized.
///
/// The title is known here, unlike a dialog window's: it arrived with the path.
/// It is set again on the focus path, since the open window is now showing a
/// different picture and a frame still naming the previous one would be the
/// window lying about what is in it. **This is the pairing the handshake exists
/// for**: the title is set here, synchronously and reliably, while the picture
/// used to arrive by an event that a window still loading never heard — so a
/// second thumbnail clicked quickly enough left the frame naming one picture
/// over another one. The event is now held until that window says it is
/// listening; the header of this file carries the rest.
///
/// Built hidden and shown once it is placed, the same order the main window and
/// the dialog windows use: a window shown first and moved afterwards is a
/// visible jump. 900x700 is a fixed opening size with a floor under it and the
/// frame draggable from there — there is no measuring loop here at all, which
/// is what lets this window be resizable where a dialog window is not.
///
/// Deliberately not `async`: a synchronous command runs on the main thread,
/// which is where a window is created on every platform this app targets.
#[tauri::command]
pub fn image_window_open(app: AppHandle, path: String, name: String) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(IMAGE_LABEL) {
        let _ = window.unminimize();
        let _ = window.set_title(&window_title(&name));
        show_now_or_on_ready(&app, IMAGE_LABEL, image_show(&path, &name));
        return window.set_focus().map_err(|err| err.to_string());
    }

    // The picture is on the URL below: nothing is owed to a window being built,
    // and a copy of it here would be the same file read a second time the
    // moment that window announced itself.
    forget_show(&mut pending_show(), IMAGE_LABEL);

    let mut builder = WebviewWindowBuilder::new(
        &app,
        IMAGE_LABEL,
        WebviewUrl::App(format!("index.html?view=image{}", image_query(&path, &name)).into()),
    )
    .title(window_title(&name))
    .inner_size(900.0, 700.0)
    .min_inner_size(320.0, 240.0)
    .resizable(true)
    .visible(false);

    // A child of the main window for the reason the three windows above are
    // children: a window whose whole purpose is to be looked at beside the
    // board must not sink behind it on the first click into the board. No main
    // window is not an error, any more than it is there.
    if let Some(main) = app.get_webview_window("main") {
        builder = builder.parent(&main).map_err(|err| err.to_string())?;
    }

    let window = builder.build().map_err(|err| err.to_string())?;
    center_over_main(&app, &window);
    window.show().map_err(|err| err.to_string())
}

/// What the OS frame carries over a picture. The stored name is the whole of it
/// — it is what the strip's tooltip says and what the caption under the picture
/// repeats — and the app's own name is the fall-back for the one case that has
/// no name at all, since an empty title bar says less than a wrong one.
fn window_title(name: &str) -> String {
    if name.is_empty() {
        "Smetana".to_string()
    } else {
        name.to_string()
    }
}

/// What each open dialog window is doing about its size.
///
/// One entry per open dialog window: the last size known to be ours, how many
/// events of our own are still owed to us, and the two one-way bits. `latched`
/// is one-way in the strong sense — once a hand has given this window a size,
/// nothing computed ever moves it again, for the window's whole life.
///
/// A `BTreeMap` in a `static` rather than managed state, for one reason: it has
/// to be reachable from a window's event closure, which outlives the call that
/// built it, and `BTreeMap::new` is const where `HashMap::new` is not. The key
/// is the kind, which is already one window's worth of identity — the label is
/// `dialog-<kind>` and there is one window per kind.
///
/// **The entry is about a window and not about a kind**, which is why
/// `dialog_window_open` writes a whole fresh one rather than reaching into the
/// old. The map outlives every window in it — nothing removes an entry — so a
/// latch left standing by a window that has been closed would be inherited by
/// the next window of that kind, which would then never be sized by anything:
/// built at the provisional height, refused a `set_size` by the latch, and shown
/// as a title bar with a scroll bar under it.
///
/// **Every size in here is in whole logical points**, which is the unit the
/// file keeps and the unit `set_size` is given. Physical pixels were the earlier
/// choice and were wrong for one reason: they change when a window is dragged
/// between displays of different scale. tao raises `ScaleFactorChanged` and then
/// a `Resized` carrying a new physical size for an unchanged logical one, and a
/// comparison in pixels reads that as somebody halving the window — latching a
/// dialog nobody had touched, on exactly the multi-monitor setup
/// `remembered_size`'s clamp exists to serve. In points the two sizes are equal
/// and the event says nothing, which is the truth about it.
#[derive(Debug, Default, Clone, Copy)]
struct DialogWindowState {
    /// The last size we know was ours: what the window came to after a
    /// `set_size`, or what a hand left it at. A drag is measured against this
    /// rather than against the previous event, so that a corner nudged a point
    /// at a time accumulates instead of staying inside `hand_moved`'s slack for
    /// ever.
    baseline: (u32, u32),
    /// What our last `set_size` asked for. Kept only to know whether that call
    /// could have produced an event at all: asking for the size the window
    /// already has changes nothing and is answered with nothing, which is the
    /// ordinary case in the fit phase.
    requested: (u32, u32),
    /// How many `Resized` events of our own making are still owed to us. A count
    /// and not a flag, because two `set_size` calls for two different sizes can
    /// both be outstanding: on GTK the event comes back from the compositor's
    /// configure round trip, while the next IPC message from the webview needs
    /// no such round trip and can overtake it. A flag raised twice and spent
    /// once would leave the second event to be judged against the first one's
    /// baseline, and latch. It is bounded by the number of distinct-size
    /// requests in flight, which is what the `requested` gate above buys.
    expecting: u32,
    /// Whether anything computed may still size this window. One-way.
    latched: bool,
    /// Whether a **hand** moved this window, which is a narrower thing than
    /// `latched` and the one the close-write asks about. A window opened at a
    /// remembered size is latched from birth without anybody touching it, so
    /// writing its size back on close would overwrite the person's preference
    /// with whatever `remembered_size` clamped it to on this monitor — a size
    /// dragged out on an external display would be quietly cut down to the
    /// laptop's by one open and close, and not come back.
    moved_by_hand: bool,
}

static DIALOG_WINDOWS: Mutex<BTreeMap<String, DialogWindowState>> = Mutex::new(BTreeMap::new());

/// Records a `set_size` of ours: what we asked for, what the window became, and
/// that an event is now owed to us.
///
/// The count is raised only when the request differs from the last one, and that
/// is what keeps it bounded: a `set_size` to the size the window already has
/// produces no `Resized` at all, and in the fit phase that is most of them — the
/// page reports on every content change and the arithmetic mostly recomputes to
/// the same number. Counting those would leave a total nothing ever spends,
/// swallowing real drag events for the life of the window.
fn note_our_size(kind: &str, requested: (u32, u32), became: (u32, u32)) {
    if let Ok(mut windows) = DIALOG_WINDOWS.lock() {
        let state = windows.entry(kind.to_string()).or_default();
        if state.requested != requested {
            state.requested = requested;
            state.expecting = state.expecting.saturating_add(1);
        }
        state.baseline = became;
    }
}

/// The entry `dialog_window_open` starts a window's life with.
///
/// A whole fresh value rather than a field set on whatever was there, and pure
/// so that it can be tested: the map outlives every window in it, and the
/// previous window of this kind may have left a latch standing — dragged, then
/// closed inside the debounce, so no size ever reached the file. Reaching in to
/// set one field would leave that latch, and the new window, built at the
/// provisional height with no size kept, would be refused its one `set_size` and
/// shown 120 points tall for the rest of the session.
fn open_entry(windows: &mut BTreeMap<String, DialogWindowState>, kind: &str, kept: bool) {
    windows.insert(
        kind.to_string(),
        DialogWindowState { latched: kept, ..DialogWindowState::default() },
    );
}

/// Records what the window came to when nothing of ours asked it to — the
/// window built at a remembered size, which is already where it belongs.
fn note_baseline(kind: &str, size: (u32, u32)) {
    if let Ok(mut windows) = DIALOG_WINDOWS.lock() {
        windows.entry(kind.to_string()).or_default().baseline = size;
    }
}

/// How big a dialog window is right now, in the logical points the file keeps.
/// `None` when the platform will not say, which is not worth a log line: the
/// only thing lost is the size at the next opening.
fn logical_inner(window: &tauri::WebviewWindow) -> Option<settings::model::DialogSize> {
    let scale = window.scale_factor().ok()?;
    let inner = window.inner_size().ok()?.to_logical::<f64>(scale);
    Some(settings::model::DialogSize {
        width: inner.width.round() as u32,
        height: inner.height.round() as u32,
    })
}

/// Whether anything computed may still size this kind's window.
fn is_latched(kind: &str) -> bool {
    DIALOG_WINDOWS
        .lock()
        .ok()
        .and_then(|windows| windows.get(kind).map(|state| state.latched))
        .unwrap_or(false)
}

/// Whether a hand has moved this kind's window during the life of the window
/// that is open now. Narrower than `is_latched`, and `moved_by_hand` records
/// what the difference is for.
fn was_moved_by_hand(kind: &str) -> bool {
    DIALOG_WINDOWS
        .lock()
        .ok()
        .and_then(|windows| windows.get(kind).map(keeps_its_size_on_close))
        .unwrap_or(false)
}

/// Whether this kind's window keeps its size when it closes.
///
/// `moved_by_hand` and not `latched`, and the difference is the whole of this
/// function: a window opened at a remembered size is latched from birth, so
/// asking `latched` here would write its size back on every close — including
/// the size `remembered_size` had clamped it to for whatever monitor it opened
/// on. One open and close on a laptop would cut a size dragged out on an
/// external display down to the laptop's, for good.
fn keeps_its_size_on_close(state: &DialogWindowState) -> bool {
    state.moved_by_hand
}

/// A physical size in whole logical points. A scale factor of zero is not a
/// thing a display has; treating it as 1 keeps a broken answer from becoming an
/// infinity.
fn to_points(physical: (u32, u32), scale: f64) -> (u32, u32) {
    let scale = if scale > 0.0 { scale } else { 1.0 };
    (
        (physical.0 as f64 / scale).round() as u32,
        (physical.1 as f64 / scale).round() as u32,
    )
}

/// Takes one `Resized` — the physical size the platform reported and the scale
/// factor it was measured under — decides whether it was the hand, and leaves
/// the record the next one will be judged against.
///
/// **The whole transition is here rather than in the event closure**, and that
/// is deliberate: a closure needs an `AppHandle`, so no test in this repository
/// can reach one, which is the Rust shape of the problem `CLAUDE.md` describes
/// for `.vue` files. Everything a mutation could quietly revert — the
/// conversion out of pixels, spending one owed event rather than all of them,
/// which events may move the baseline, and which bit a hand sets — lives in
/// this function so that a test can hold it to each.
///
/// The caller keeps only what it cannot hand over: finding the window and
/// asking it for its scale factor.
fn record_resize(state: &mut DialogWindowState, physical: (u32, u32), scale: f64) -> bool {
    let now = to_points(physical, scale);
    let hand = resize_is_the_hand(state, now);
    // A collapsed size is a minimise and says nothing about how big this window
    // is, so it leaves no record at all: neither a baseline, nor a spent count,
    // since the events we are owed may still be due.
    if now.0 != 0 && now.1 != 0 {
        if state.expecting > 0 {
            // One of ours. What the window became is the new baseline whatever
            // we asked for, which is the whole point of counting rather than
            // comparing.
            state.expecting -= 1;
            state.baseline = now;
        } else if hand {
            state.latched = true;
            state.moved_by_hand = true;
            state.baseline = now;
        }
        // Otherwise it is inside the slack of a baseline we already hold, and
        // that baseline is exactly what must not move: a drag of one point per
        // event has to accumulate against the last size known to be ours.
    }
    hand
}

/// Whether a `Resized` is somebody dragging the corner rather than this app
/// sizing its own window.
///
/// **What the design decided is the behaviour: never mistake our own sizing —
/// including the operating system clamping it — for a hand.** The design named
/// reading the size back after each `set_size` as the mechanism for it, and the
/// mechanism is wrong on two of the three platforms this ships to. Do not
/// "restore" it; what follows is the same rule, delivered everywhere.
///
/// On GTK (tao 0.35.3, `platform_impl/linux/window.rs`) `set_inner_size` only
/// posts a request down a channel, while `inner_size` reads an atomic cache
/// updated later from the configure-event. So the read back after a `set_size`
/// is the size from *before* it, the `Resized` that follows carries the new one,
/// and a comparison of the two says "hand" every time — every dialog would latch
/// on its first content sizing, permanently, with a size written to
/// `settings.json` for a window nobody had touched. It is not a race there: it
/// is a deterministic read of a stale cache.
///
/// So the deciding signal is `expecting`, counted up beside every `set_size` of
/// ours and spent by the events that follow, whatever size they carry — which is
/// also what covers the OS clamping a request, on every platform at once, where
/// the read-back only covered it on macOS. The baseline is kept beside it as a
/// second signal, for everything after those events.
///
/// The minimised case is the third guard and belongs to Windows: tao's
/// `WM_SIZE` arm sends `Resized` for every `WM_SIZE`, `SIZE_MINIMIZED` included,
/// where the client area is 0×0. It arrives with no flag standing — nothing of
/// ours asked for it — so `expecting` cannot cover it and it is refused on its
/// own terms. A window with a zero side is not a size anybody dragged to: the
/// hand cannot go below `min_inner_size`.
fn resize_is_the_hand(state: &DialogWindowState, now: (u32, u32)) -> bool {
    // Minimised, or otherwise collapsed to nothing. Never a hand.
    if now.0 == 0 || now.1 == 0 {
        return false;
    }
    // Nothing of ours has sized this window yet, so there is nothing to tell a
    // hand apart from — and there is no hand either: the window is hidden until
    // the first `dialog_window_size`, and a window nobody can see is a window
    // nobody can drag.
    if state.baseline == (0, 0) {
        return false;
    }
    // An event we asked for. Its size is not compared with anything, which is
    // the whole point: what the window became is the platform's answer, and on
    // one of them it is the only correct one available.
    if state.expecting > 0 {
        return false;
    }
    hand_moved(state.baseline, now)
}

/// Whether two sizes are far enough apart to be different sizes.
///
/// Both are whole logical points, and the slack of one on each axis is what
/// survives the rounding between them and the physical pixels every one of these
/// numbers was carried in: a size is set in points, delivered as pixels and
/// divided back by a scale factor that is not always a whole number.
///
/// It is slack against *rounding* and nothing else, which is why the baseline it
/// is given is the last size known to be ours rather than the previous event's:
/// measured frame to frame, a corner dragged one point at a time would never
/// leave the slack, and a window nudged slowly would be re-fitted back out from
/// under the hand. Nothing but `resize_is_the_hand` above should call this; the
/// rule it is part of lives there.
fn hand_moved(baseline: (u32, u32), now: (u32, u32)) -> bool {
    baseline.0.abs_diff(now.0) > 1 || baseline.1.abs_diff(now.1) > 1
}

/// The size to open a remembered dialog window at, in logical points.
///
/// Two bounds, and each answers a case the other cannot. The monitor's share is
/// for a window dragged out on a large display and opened on a laptop, where
/// the stored size would land off the edge; the floor is the width the dialog's
/// layout was drawn at, which the stored width can only fall below if that
/// width has grown in `src/views/dialogRegistry.js` since the size was kept.
///
/// The share is `HEIGHT_CEILING`, already the bound on a fitted height, applied
/// to the width as well: the argument for it — a window with nowhere to put its
/// footer — reads the same way round for a window with nowhere to put its
/// buttons.
fn remembered_size(stored: (f64, f64), floor_width: f64, monitor: (f64, f64)) -> (f64, f64) {
    let width = stored.0.min(monitor.0 * HEIGHT_CEILING).max(floor_width);
    let height = stored.1.min(monitor.1 * HEIGHT_CEILING);
    (width, height)
}

/// Opens a dialog window on one kind, or brings the open one forward.
///
/// The window is built **hidden and at a provisional height**. Its real height
/// is whatever its content comes to, which nothing on this side can know: the
/// page measures itself and calls `dialog_window_size` below, which sizes the
/// window, centres it and shows it. That is the same order the main window uses
/// — `"visible": false` in `tauri.conf.json`, shown once it is where it belongs
/// — and for the same reason: a window shown first and sized afterwards is a
/// visible jump.
///
/// Two phases, and the window is built for the one it is owed. With no size
/// kept for this kind it is built at the provisional height, and the height
/// stays the content's for as long as nobody drags the corner. With a size kept
/// — somebody dragged this kind of window before — it is built at that size,
/// clamped by `remembered_size`, and carries `&fill=1` so that the page comes
/// up filling the window rather than switching into it a round trip after its
/// first paint.
///
/// Deliberately not `async`: a synchronous command runs on the main thread,
/// which is where a window is created on every platform this app targets.
#[tauri::command]
pub fn dialog_window_open(app: AppHandle, kind: String, width: f64) -> Result<(), String> {
    let kind = kind_query(&kind).ok_or_else(|| format!("not a dialog kind: {kind}"))?;
    let label = format!("{DIALOG_PREFIX}{kind}");

    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        return window.set_focus().map_err(|err| err.to_string());
    }

    // A size somebody dragged this kind of window to, if there is one. It also
    // decides the phase the page comes up in: a window opened at a hand-chosen
    // size must not have its height computed away by the first measurement, so
    // the flag travels on the URL and is right before the first paint rather
    // than one round trip after it.
    let kept = settings::dialog_size(&app, &kind).map(|size| {
        // The monitor the dialog is about to be *centred over*, which is the
        // main window's rather than the machine's first: on a laptop with an
        // external display the primary is often not the one the app is on, and
        // a size clamped against a screen the window is not opening on is not
        // clamped at all. `center_over_main` picks the same window for the same
        // reason. The machine's primary is the fall-back for a run with no main
        // window, where the dialog is centred on the screen anyway.
        let monitor = app
            .get_webview_window("main")
            .and_then(|main| main.current_monitor().ok().flatten())
            .or_else(|| app.primary_monitor().ok().flatten())
            .map(|monitor| {
                let size = monitor.size().to_logical::<f64>(monitor.scale_factor());
                (size.width, size.height)
            })
            .unwrap_or((f64::MAX, f64::MAX));
        remembered_size((size.width as f64, size.height as f64), width, monitor)
    });
    // A whole fresh entry for a whole fresh window — `open_entry` above carries
    // the argument, and is where it is so that a test can reach it.
    if let Ok(mut windows) = DIALOG_WINDOWS.lock() {
        open_entry(&mut windows, &kind, kept.is_some());
    }
    let (built_width, built_height) = kept.unwrap_or((width, PROVISIONAL_HEIGHT));
    let fill = if kept.is_some() { "&fill=1" } else { "" };

    let mut builder = WebviewWindowBuilder::new(
        &app,
        &label,
        WebviewUrl::App(format!("index.html?view=dialog&kind={kind}{fill}").into()),
    )
    // The title the OS frame draws is the dialog's own, and it arrives once the
    // page knows it — the props travel by event, not by URL. Until then the
    // frame carries the app's name rather than a placeholder somebody would see.
    .title("Smetana")
    .inner_size(built_width, built_height)
    // Wider and taller than the layout was drawn at, never narrower: the
    // registry's width is the width every one of this dialog's rows was spaced
    // for, and the floor on the height is below the shortest dialog the app
    // draws — it is there to stop a window being dragged shut.
    .min_inner_size(width, MIN_DIALOG_HEIGHT)
    .resizable(true)
    // A resizable window is offered a zoom button, and a maximized dialog is
    // not a thing this app has a design for — nor one anything here would size
    // afterwards.
    .maximizable(false)
    .visible(false);

    // A child of the main window for the reason the settings and compare windows
    // are children: a window whose whole purpose is to be looked at beside the
    // board must not sink behind it on the first click into the board. No main
    // window is not an error, any more than it is there.
    if let Some(main) = app.get_webview_window("main") {
        builder = builder.parent(&main).map_err(|err| err.to_string())?;
    }

    let window = builder.build().map_err(|err| err.to_string())?;

    let app_handle = app.clone();
    let channel = format!("dialog:result:{kind}");
    let resized_kind = kind.clone();
    // The counter cuts off everything but the last event, the shape
    // `persist_geometry` uses: a drag of the corner sends hundreds, and one
    // trip to the disk per pixel is hundreds of writes for one number.
    let latest = Arc::new(AtomicU64::new(0));
    window.on_window_event(move |event| {
        match event {
            // A dialog window closed by its own frame is the dialog answering
            // "close", and it says so on the channel the guest's own emits
            // travel on. Without this the app window would go on serving a
            // window that is not there: still announcing props to nobody, and —
            // the part somebody would actually see — still counting the dialog
            // as open, so the next project switch produced a toast explaining
            // why a window they had closed themselves had closed.
            //
            // Answered from here rather than from the page, because a page
            // being torn down is not reliably given the chance to say anything.
            WindowEvent::Destroyed => {
                let _ = app_handle.emit(&channel, serde_json::json!({ "name": "close" }));
            }
            // The other end of the 500 ms below, and the reason this arm exists
            // at all: a drag followed straight away by a close is the ordinary
            // way somebody resizes a dialog they then decide against, and the
            // debounced task would wake to find no window and write nothing —
            // so the size they chose would be forgotten and the feature would
            // simply not work for them. `persist_geometry` can afford the same
            // debounce because the plugin's `Exit` handler backstops it; this
            // has no backstop, so the last chance to read the size is taken
            // here, while the window is still there to be asked.
            //
            // Only for a window a **hand** moved, which is not the same as a
            // latched one and is the whole of this guard. A window opened at a
            // remembered size is latched from birth, so asking `is_latched` here
            // would write its size back on every close — including the size
            // `remembered_size` had clamped it to for this monitor. Open a
            // dialog dragged out on an external display once on the laptop,
            // close it, and the preference would be gone for good. An untouched
            // dialog must leave no entry behind at all: a kind with no entry
            // opens fitted, and that is today's behaviour to the letter.
            WindowEvent::CloseRequested { .. } => {
                if !was_moved_by_hand(&resized_kind) {
                    return;
                }
                let Some(window) =
                    app_handle.get_webview_window(&format!("{DIALOG_PREFIX}{resized_kind}"))
                else {
                    return;
                };
                // Read here and written off the thread: the size has to be
                // taken while the window exists, and the file must not be
                // written on the main thread as a window is closing.
                let Some(size) = logical_inner(&window) else {
                    return;
                };
                let app = app_handle.clone();
                let kind = resized_kind.clone();
                tauri::async_runtime::spawn(async move {
                    settings::remember_dialog_size(&app, &kind, size);
                });
            }
            WindowEvent::Resized(size) => {
                // In points, and the scale factor is read now rather than
                // remembered, so that an event arriving straight after a
                // `ScaleFactorChanged` is converted by the factor it was
                // measured under. `DialogWindowState` records why the whole
                // comparison moved out of pixels.
                let Some(window) =
                    app_handle.get_webview_window(&format!("{DIALOG_PREFIX}{resized_kind}"))
                else {
                    return;
                };
                let Ok(scale) = window.scale_factor() else {
                    return;
                };
                // The decision and the record it leaves, under one lock, and
                // all of it in `record_resize` where a test can reach it.
                //
                // The latch is set here and the size is written later. The two
                // are separate on purpose: the phase has to change on the first
                // pixel of the drag, so that nothing computed fights the hand
                // while it is still moving, and the file is worth one write at
                // the end of it.
                let hand = {
                    let Ok(mut windows) = DIALOG_WINDOWS.lock() else {
                        return;
                    };
                    let state = windows.entry(resized_kind.clone()).or_default();
                    record_resize(state, (size.width, size.height), scale)
                };
                if !hand {
                    return;
                }
                let mine = latest.fetch_add(1, Ordering::SeqCst) + 1;
                let latest = latest.clone();
                let app = app_handle.clone();
                let kind = resized_kind.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(SETTLE).await;
                    if latest.load(Ordering::SeqCst) != mine {
                        return;
                    }
                    let Some(window) = app.get_webview_window(&format!("{DIALOG_PREFIX}{kind}"))
                    else {
                        // Closed inside the debounce. Not a loss: the
                        // `CloseRequested` arm above read the size and wrote it
                        // as the window went.
                        return;
                    };
                    let Some(size) = logical_inner(&window) else {
                        return;
                    };
                    settings::remember_dialog_size(&app, &kind, size);
                });
            }
            _ => {}
        }
    });

    Ok(())
}

/// The height a dialog window is built at, before it has measured itself. It is
/// never seen: the window is hidden until `dialog_window_size` has given it the
/// height its content came to. A window has to be built at *some* size, and this
/// is a plausible one rather than a meaningful one.
const PROVISIONAL_HEIGHT: f64 = 120.0;

/// The floor under a dialog window's height, in logical points. Below the
/// shortest dialog this app draws — it exists so that a window cannot be
/// dragged shut, not to hold any particular dialog open.
const MIN_DIALOG_HEIGHT: f64 = 120.0;

/// How much of a monitor a dialog window may take up in height. A dialog taller
/// than the screen has nowhere to put its footer, and the footer is where the
/// buttons are; past this the page scrolls its own body.
const HEIGHT_CEILING: f64 = 0.9;

/// What `dialog_window_size` answers: whether this window's size is the
/// person's now.
///
/// The answer rides back on a call the page already makes on every change to
/// its viewport — and a hand on the corner is a change to its viewport — which
/// is what saves an event channel and its race: a window latched while it is
/// open learns so on its very next report, and a window opened already latched
/// was told by `&fill=1` on its URL before it painted at all.
#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DialogSized {
    pub latched: bool,
}

/// Gives a dialog window the height its content came to, puts it over the main
/// window, and shows it.
///
/// Called again whenever the content changes height — a validation line
/// appearing under a field, a confirm turning into a progress report — so this
/// is not only a first-paint path. The window is already visible by then and
/// `show` is a no-op, which is why there is no branch for it.
///
/// **Placing it is a first-paint path, and only that.** Whether the window is
/// still hidden is what says which call this is, and it decides the one thing
/// that must not be repeated: a window put back over the main one every time its
/// content changed height would jump out from under the hand of somebody who had
/// dragged it onto their second monitor, on the character that made the branch
/// name invalid. Being draggable anywhere is the whole reason these are windows.
///
/// The title arrives here rather than at build time because the props travel by
/// event: at build time nothing knows what this dialog is called.
///
/// `viewport` is the second number the page sends, and `height_to_set` below is
/// what it is for.
#[tauri::command]
pub fn dialog_window_size(
    app: AppHandle,
    kind: String,
    height: f64,
    viewport: f64,
    title: String,
) -> Result<DialogSized, String> {
    let kind = kind_query(&kind).ok_or_else(|| format!("not a dialog kind: {kind}"))?;
    let Some(window) = app.get_webview_window(&format!("{DIALOG_PREFIX}{kind}")) else {
        // Closed between the measurement and the call. Not an error: the app
        // window closes these on its own when their ground goes, and a race with
        // that is an ordinary outcome. A window that is gone is not a window
        // somebody has given a size to, so the answer is the flag's own floor.
        return Ok(DialogSized { latched: false });
    };

    // Hidden means this is the first measurement, since the window is built
    // hidden and the line at the end of this function is the only thing that
    // ever shows one. A failure to ask is read as "already up", which is the
    // answer that leaves a window where the person put it.
    let first_paint = !window.is_visible().unwrap_or(true);
    let latched = is_latched(&kind);

    // The width is the window's own rather than the registry's: this side is
    // told a height and nothing else, and re-sending a width would be a second
    // opinion about a number only one side holds.
    let scale = window.scale_factor().map_err(|err| err.to_string())?;
    let inner = window
        .inner_size()
        .map_err(|err| err.to_string())?
        .to_logical::<f64>(scale);

    // A window somebody has given a size to is not sized again, ever. The page
    // is filling it rather than measuring itself by then, so the height it
    // sends is the viewport's own and means nothing here; what still arrives
    // with it, and still matters, is the title.
    if !latched {
        let ceiling = window
            .current_monitor()
            .ok()
            .flatten()
            .map(|monitor| {
                monitor.size().to_logical::<f64>(monitor.scale_factor()).height * HEIGHT_CEILING
            })
            .unwrap_or(f64::MAX);

        let want = tauri::LogicalSize::new(
            inner.width,
            height_to_set(height, inner.height, viewport, ceiling),
        );
        window.set_size(want).map_err(|err| err.to_string())?;

        // What we asked for and what the window came to, both recorded, and
        // `resize_is_the_hand` says why it takes two. The request says whether
        // an event is owed to us at all; the read-back is the second signal, and
        // on the one platform where it is a stale cache it is simply the size
        // the next event will be compared against once the flag is spent.
        let became = window
            .inner_size()
            .map(|size| {
                let size = size.to_logical::<f64>(scale);
                (size.width.round() as u32, size.height.round() as u32)
            })
            .unwrap_or_default();
        note_our_size(
            &kind,
            (want.width.round() as u32, want.height.round() as u32),
            became,
        );
    } else if first_paint {
        // Built at the remembered size and never resized since, so there is
        // nothing to set — but the baseline still has to hold what the window
        // actually became, in the points everything in that record is kept in.
        note_baseline(&kind, (inner.width.round() as u32, inner.height.round() as u32));
    }

    let _ = window.set_title(&title);
    if first_paint {
        center_over_main(&app, &window);
    }
    window.show().map_err(|err| err.to_string())?;
    Ok(DialogSized { latched })
}

/// The height to hand `set_size`, given the height the content came to.
///
/// The two numbers before the ceiling are the same measurement taken from the
/// two ends of one window, and the difference between them is the whole of this
/// function. `inner` is what the window answers when it is asked its inner size
/// — the quantity `set_size` also speaks in, which is what makes subtracting
/// one from the other legitimate. `viewport` is how much of that the page says
/// reached it. What is left over is whatever the frame keeps for itself, and
/// adding it back is what makes the window as tall as its content rather than
/// that much shorter.
///
/// **Derived rather than named, because every way of naming it is wrong
/// somewhere.** The overhead is a title bar on macOS and a title bar with
/// borders on Windows and Linux; it moves with the system's appearance and with
/// the OS version. Measured on the machine this bug was found on it came to 32
/// logical points, where the screenshot it was reported from had suggested 26
/// to 28 — a constant written from that screenshot would have been wrong by
/// four points on the very machine that produced it.
///
/// **Not `outer_size - inner_size`, which was the first answer and is zero.**
/// On macOS `inner_size` for a webview window is the webview's own view frame
/// (`tauri-runtime-wry`), `outer_size` is the `NSWindow` frame, and the two
/// answer the same number to the point: 471 logical against 471, with the page
/// getting 439. `inner_position` against `outer_position` is zero for the same
/// reason. Nothing the window says about itself carries the difference — only
/// the page knows what it was given, which is why that number is sent.
///
/// The ceiling is applied last, to the size that is actually set: it is a
/// statement about how much of the screen a window may take, not about how much
/// its content wanted. Past it the page scrolls its own body, which is
/// deliberate.
fn height_to_set(content: f64, inner: f64, viewport: f64, ceiling: f64) -> f64 {
    // Never below zero. A viewport larger than the inner size is not a thing a
    // window should be able to say, and if one ever does, the answer that keeps
    // the footer on screen is no correction rather than a negative one.
    let overhead = (inner - viewport).max(0.0);
    // The floor is the window's own `min_inner_size`, said here as well so that
    // what is asked for is what the OS is going to do anyway. Without it a run
    // of distinct sub-floor requests would each be counted as an event owed to
    // us and none of them answered — the only way in this code to leave
    // `DialogWindowState::expecting` standing. No dialog in this app draws
    // content that short, so this is a wedge rather than a fix.
    (content + overhead).min(ceiling).max(MIN_DIALOG_HEIGHT)
}

/// Puts a dialog window in the middle of the main window, and in the middle of
/// the monitor when there is no main window to be in the middle of.
///
/// Over the main window rather than over the screen, because that is where the
/// person is looking: they pressed something on the board a moment ago, and a
/// dialog centred on a monitor the app happens not to fill would open away from
/// the thing it is a question about.
///
/// All of it in physical pixels, which is what both sides already answer in —
/// converting to logical here would be two conversions to keep in step for an
/// arithmetic that does not need either.
fn center_over_main(app: &AppHandle, window: &tauri::WebviewWindow) {
    let placed = app.get_webview_window("main").and_then(|main| {
        let main_at = main.outer_position().ok()?;
        let main_size = main.outer_size().ok()?;
        let size = window.outer_size().ok()?;
        let x = main_at.x + (main_size.width as i32 - size.width as i32) / 2;
        let y = main_at.y + (main_size.height as i32 - size.height as i32) / 2;
        window
            .set_position(tauri::PhysicalPosition::new(x, y))
            .ok()
    });
    // No main window, or a platform that would not say where it is. The middle
    // of the screen is the ordinary place for a dialog and is where the two
    // other windows of this app land without being asked.
    if placed.is_none() {
        let _ = window.center();
    }
}

/// Closes one dialog window, if it is open.
///
/// The app window calls this when the ground a dialog stood on has gone — the
/// project changed, the task was deleted, the column emptied. A window that is
/// not there is the ordinary case, not a failure: the person may have closed it
/// themselves a moment earlier.
#[tauri::command]
pub fn dialog_window_close(app: AppHandle, kind: String) -> Result<(), String> {
    let kind = kind_query(&kind).ok_or_else(|| format!("not a dialog kind: {kind}"))?;
    if let Some(window) = app.get_webview_window(&format!("{DIALOG_PREFIX}{kind}")) {
        return window.close().map_err(|err| err.to_string());
    }
    Ok(())
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
/// after that window is gone it is a comparison nothing can ever change. The
/// image window is the plainest case of all three: it shows one picture off a
/// draft in a dialog window that is itself about to be closed here, and nothing
/// left standing could ever aim it at another one.
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
        for label in [SETTINGS_LABEL, COMPARE_LABEL, IMAGE_LABEL] {
            if let Some(window) = app.get_webview_window(label) {
                let _ = window.close();
            }
        }
        // Every dialog window too, and for a sharper version of the same
        // reason: a dialog window holds no state of its own and is fed entirely
        // by the app window, so once that window is gone it is a question
        // nothing can answer and a confirm nothing can carry out.
        for (label, window) in app.webview_windows() {
            if label.starts_with(DIALOG_PREFIX) {
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

/// The person's home folder, or `None` where there is no saying.
///
/// A fact about the machine rather than a choice about the app, which is what
/// puts it beside `window_chrome` above and in `stores/app.js` on the other
/// side: that store is what the app asks the desktop, and without a command
/// here a component wanting this would have to import `@tauri-apps/api` itself
/// — the one import that would put it out of reach of every test the front end
/// has.
///
/// The one reader today is the review window's Repository column, where a
/// repository outside the project draws `~/work/smetana-infra` instead of an
/// absolute path (`src/components/git/repoLabel.js`). `None` is an ordinary
/// answer and not a failure: that rule then draws the path unchanged, which is
/// true rather than merely shorter.
///
/// `access::home()` and not a fifth reading of the environment. That function
/// is the one that carries a name, its rule about an empty `HOME` is written
/// down there, and this command is exactly the missing half `repoLabel.js`
/// names — a way of handing that answer to the front end.
#[tauri::command]
pub fn home_dir() -> Option<String> {
    crate::tracker::access::home().map(|home| home.to_string_lossy().into_owned())
}

/// Which units and origin a drag-drop event's `position` arrives in, as one of
/// the two names `src/components/terminal/dropPoint.js` holds.
///
/// Tauri types that field `PhysicalPosition` on every platform, and on two of
/// the three it is not physical at all. wry reads the point out of the toolkit
/// and `tauri-runtime-wry` passes it through unscaled, so what arrives is
/// whatever the toolkit measures in: on macOS `draggingLocation()` against the
/// webview's `frame()`, both of which AppKit states in points — the same
/// `frame()` wry itself reads back as a `LogicalSize` in `WebView::bounds` — and
/// on Linux GTK's `drag-motion` widget coordinates, which are logical too. Only
/// Windows reports device pixels, from `ScreenToClient` on the client area.
///
/// It matters because the front end divides by `devicePixelRatio` to reach the
/// CSS pixels `document.elementFromPoint` reads: on a Retina Mac that division
/// halved a point that was already CSS pixels, and the panel took a drop only
/// where the halved point happened to land back inside it.
///
/// A compile-time fact rather than a runtime one, for the same reason
/// `window_chrome` above is one: the front end cannot ask what it was built
/// for, and a user-agent string is a guess.
#[tauri::command]
pub fn drag_drop_space() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "physical"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "logical"
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
    use super::{
        compare_query, compare_show, drag_drop_space, forget_show, hand_moved, height_to_set,
        image_query, image_show, keeps_its_size_on_close, kind_query, open_entry, record_resize,
        remember_show, remembered_size, resize_is_the_hand, settings_show, show_event, tab_query,
        take_show, window_chrome, window_title, COMPARE_LABEL, DialogWindowState, IMAGE_LABEL,
        SETTINGS_LABEL,
    };
    use serde_json::{json, Value};
    use std::collections::BTreeMap;

    /// The two words this command may answer with, named here in full because
    /// they are a contract with `src/components/shell/windowChrome.js` and
    /// nothing else pins them. That module answers `none` for a word it has not
    /// heard of, deliberately — a browser is the ordinary way to reach it — so a
    /// rename on either side does not fail, it silently costs the feature:
    /// macOS draws the project name under the traffic lights, and Windows and
    /// Linux lose all three buttons on a window that has no system ones.
    #[test]
    fn the_front_end_is_told_one_of_the_two_words_it_knows() {
        assert!(
            matches!(window_chrome(), "traffic-lights" | "buttons"),
            "window_chrome answered {:?}, which components/shell/windowChrome.js reads as no chrome at all",
            window_chrome()
        );
        #[cfg(target_os = "macos")]
        assert_eq!(window_chrome(), "traffic-lights");
        #[cfg(not(target_os = "macos"))]
        assert_eq!(window_chrome(), "buttons");
    }

    /// The two words this command may answer with, named here in full for the
    /// reason the pair above are: they are a contract with
    /// `src/components/terminal/dropPoint.js`, and that module falls back to
    /// dividing by the device pixel ratio for a word it has not heard of. A
    /// rename on either side therefore costs the fix rather than failing —
    /// on a Retina Mac the agent panel goes back to taking a drop only over
    /// part of itself.
    #[test]
    fn the_front_end_is_told_which_units_a_drop_arrives_in() {
        assert!(
            matches!(drag_drop_space(), "logical" | "physical"),
            "drag_drop_space answered {:?}, which components/terminal/dropPoint.js reads as physical",
            drag_drop_space()
        );
        #[cfg(target_os = "windows")]
        assert_eq!(drag_drop_space(), "physical");
        #[cfg(not(target_os = "windows"))]
        assert_eq!(drag_drop_space(), "logical");
    }

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

    /// A stored path is absolute and holds whatever the person's home folder is
    /// called, and a stored name is `stored_name`'s own making — so both are
    /// percent-encoded into the query string rather than validated away.
    #[test]
    fn the_picture_rides_as_percent_encoded_parameters() {
        assert_eq!(
            image_query("/Users/me/App Support/attachments/a b.png", "a&b.png"),
            "&path=%2FUsers%2Fme%2FApp%20Support%2Fattachments%2Fa%20b.png&name=a%26b.png"
        );
    }

    /// Nothing a caller sends may add a parameter of its own or leave the query
    /// string — the same rule `compare_query` is held to, on the same grounds.
    #[test]
    fn nothing_in_a_picture_name_can_escape_the_query_string() {
        let query = image_query("/tmp/a b.png", "x&view=gallery#y=z?q");
        assert_eq!(query.matches('&').count(), 2);
        assert!(!query.contains("view=gallery"));
        assert!(!query.contains('#'));
        assert!(!query.contains('?'));
    }

    /// The frame says which picture is in the window, and the one case with no
    /// name to say it with falls back to the app's own rather than to nothing.
    #[test]
    fn the_frame_carries_the_file_name() {
        assert_eq!(window_title("20260806-121314-mock.png"), "20260806-121314-mock.png");
        assert_eq!(window_title(""), "Smetana");
    }

    #[test]
    fn kind_query_keeps_a_plain_identifier() {
        assert_eq!(kind_query("new-branch"), Some("new-branch".to_string()));
    }

    /// The same rule `tab_query` is held to, and for the same reason: this
    /// reaches a URL. The difference is the answer to a name that fails it —
    /// there is no sensible default dialog, so the whole call is refused rather
    /// than falling back to one.
    #[test]
    fn kind_query_drops_anything_that_could_reach_the_url() {
        assert_eq!(kind_query("new branch"), None);
        assert_eq!(kind_query("new&theme=light"), None);
        assert_eq!(kind_query("New-Branch"), None);
        assert_eq!(kind_query(""), None);
        assert_eq!(kind_query(&"a".repeat(33)), None);
    }

    /// The measurement this fix was written from, in the numbers it was taken
    /// in: a run dialog whose content came to 471 logical points, in a window
    /// answering 471 for its inner size while the page it holds reported 439.
    /// Without the correction the window is set to 471 and the page is 32
    /// points short of its own content — a scroll bar down the right edge and a
    /// footer cut off by the bottom of the window, which is the bug.
    #[test]
    fn the_frame_keeps_a_share_and_it_is_added_back() {
        assert_eq!(height_to_set(471.0, 471.0, 439.0, 1276.2), 503.0);
    }

    /// A window that hands the page all of its inner size is corrected by
    /// nothing at all. This is the arithmetic's other end and the reason it
    /// names no platform: where the two numbers agree there is no overhead to
    /// add, and the same line does the right thing.
    #[test]
    fn a_window_that_keeps_nothing_is_left_alone() {
        assert_eq!(height_to_set(471.0, 471.0, 471.0, 1276.2), 471.0);
    }

    /// The correction is the same whatever size the window happens to be when
    /// the page speaks, which is what lets the first measurement — taken while
    /// the window is still at its provisional height — land the window right in
    /// one step rather than converging on it.
    #[test]
    fn the_overhead_does_not_depend_on_the_size_it_is_measured_at() {
        assert_eq!(
            height_to_set(471.0, 120.0, 88.0, 1276.2),
            height_to_set(471.0, 471.0, 439.0, 1276.2)
        );
    }

    /// The floor is the same number `min_inner_size` is given, and is here so
    /// that a request the OS would clamp is not made in the first place: a
    /// clamped request is answered with no `Resized`, and an event owed and
    /// never delivered is the one thing that leaves the count standing.
    #[test]
    fn nothing_is_asked_for_below_the_floor_the_window_has() {
        assert_eq!(height_to_set(40.0, 471.0, 439.0, 1276.2), 120.0);
    }

    /// The ceiling is about the window and is applied to the size that is set,
    /// after the overhead rather than before it. Content taller than the screen
    /// is an ordinary outcome: the page scrolls its own body and the footer is
    /// reachable by scrolling.
    #[test]
    fn the_ceiling_holds_the_size_that_is_set() {
        assert_eq!(height_to_set(2000.0, 471.0, 439.0, 900.0), 900.0);
    }

    /// The size the window came to after our own `set_size` is the baseline, so
    /// a `Resized` carrying it is our own doing and must not latch the window.
    #[test]
    fn our_own_resize_is_not_the_hand() {
        assert!(!hand_moved((880, 632), (880, 632)));
    }

    /// A point of slack on each axis. Both sides are whole logical points, and
    /// the point they can differ by is what survives the rounding between them
    /// and the physical pixels the size was carried in.
    #[test]
    fn a_point_of_rounding_is_not_the_hand() {
        assert!(!hand_moved((880, 632), (881, 631)));
    }

    #[test]
    fn a_drag_of_the_corner_is_the_hand() {
        assert!(hand_moved((880, 632), (1180, 632)), "wider by 300 is a hand");
        assert!(hand_moved((880, 632), (880, 900)), "taller by 268 is a hand");
    }

    /// A size dragged out on a large display must open on a small one. The
    /// share is `HEIGHT_CEILING`, which is what the fitted height is already
    /// bounded by.
    #[test]
    fn a_remembered_size_is_clamped_to_the_monitor() {
        let (width, height) = remembered_size((3000.0, 2000.0), 440.0, (1440.0, 900.0));
        assert_eq!(width, 1296.0, "0.9 of a 1440-point display");
        assert_eq!(height, 810.0, "0.9 of a 900-point display");
    }

    /// Never narrower than the width the dialog's layout was drawn at. The
    /// width can only have been dragged wider, so this catches the other case:
    /// a registry width that has grown since the size was kept.
    #[test]
    fn a_remembered_size_is_never_narrower_than_the_registry_width() {
        let (width, _) = remembered_size((500.0, 400.0), 720.0, (1440.0, 900.0));
        assert_eq!(width, 720.0);
    }

    /// A size that fits is handed back as it is.
    #[test]
    fn a_remembered_size_that_fits_is_left_alone() {
        assert_eq!(remembered_size((980.0, 720.0), 720.0, (1920.0, 1200.0)), (980.0, 720.0));
    }

    /// A window that has been sized by us and is sitting still.
    fn settled(baseline: (u32, u32)) -> DialogWindowState {
        DialogWindowState { baseline, ..DialogWindowState::default() }
    }

    /// The event our own `set_size` is owed, and the whole reason the count
    /// decides rather than the baseline: on GTK the size read back after a
    /// `set_size` is the one from *before* it, so the event that follows
    /// carries a size the baseline has never held. Compared, it says "hand" —
    /// on every dialog, on its first content sizing, for ever. The count says
    /// "ours" whatever size it carries, which is also what covers the OS
    /// clamping a request on any platform.
    #[test]
    fn the_event_our_own_set_size_is_owed_is_never_the_hand() {
        let state = DialogWindowState {
            baseline: (880, 632),
            expecting: 1,
            ..DialogWindowState::default()
        };
        assert!(
            !resize_is_the_hand(&state, (880, 1006)),
            "a stale read-back makes our own sizing look like a 374-point drag"
        );
    }

    /// Two `set_size` calls can be outstanding at once, and on GTK the second
    /// can be *asked for* before the first is answered: the event comes back
    /// from the compositor's configure round trip while the next IPC message
    /// from the webview needs no round trip at all. A count raised twice and
    /// spent all at once would leave the second event judged against the first
    /// one's baseline, and latch a window nobody touched. Both are owed, so
    /// both are ours — and this goes through `record_resize`, so the spending
    /// is the code's rather than the test's.
    #[test]
    fn two_sizings_in_flight_are_both_ours() {
        let mut state = DialogWindowState {
            baseline: (440, 300),
            expecting: 2,
            ..DialogWindowState::default()
        };
        assert!(!record_resize(&mut state, (440, 380), 1.0), "the first is owed");
        assert!(!record_resize(&mut state, (440, 460), 1.0), "and so is the second");
        assert!(
            record_resize(&mut state, (700, 460), 1.0),
            "nothing is owed now, so the next one is judged on its size"
        );
    }

    /// A size in points does not change when a window is dragged between
    /// displays of different scale, and that is why the comparison is in
    /// points: tao raises `ScaleFactorChanged` and then a `Resized` carrying a
    /// new physical size for an unchanged logical one, which compared in pixels
    /// reads as somebody doubling the window.
    ///
    /// The scale factor is the argument rather than the caller's business, so
    /// that this holds the conversion to account and not just the comparison.
    #[test]
    fn crossing_between_displays_of_different_scale_is_not_the_hand() {
        // A window 440x316 points. On a 1x display that is 440x316 physical; the
        // same window moved to a 2x display reports 880x632 for the same size.
        let mut state = settled((440, 316));
        assert!(
            !record_resize(&mut state, (880, 632), 2.0),
            "the same window under a different scale factor is the same size"
        );
        assert_eq!(state.baseline, (440, 316), "and nothing about the record changed");
        assert!(!state.latched);
    }

    /// A corner dragged a point at a time. No single event leaves the slack, so
    /// the baseline must not creep along behind it: measured frame to frame,
    /// a slow drag would never register at all and the window would be fitted
    /// back out from under the hand.
    #[test]
    fn a_slow_drag_accumulates_against_the_last_size_that_was_ours() {
        let mut state = settled((440, 316));
        assert!(
            !record_resize(&mut state, (441, 316), 1.0),
            "one point is inside the rounding slack"
        );
        assert_eq!(state.baseline, (440, 316), "so the baseline stays where it was");
        assert!(
            record_resize(&mut state, (442, 316), 1.0),
            "two points from the last size known to be ours is a hand"
        );
        assert_eq!(state.baseline, (442, 316), "and now the hand's size is the record");
    }

    /// What a hand leaves behind, which is what the close-write asks about.
    #[test]
    fn a_hand_is_what_makes_a_size_worth_keeping() {
        let mut state = settled((440, 316));
        assert!(!keeps_its_size_on_close(&state), "nothing has moved it yet");
        assert!(record_resize(&mut state, (700, 316), 1.0));
        assert!(state.latched, "nothing computed may size it now");
        assert!(keeps_its_size_on_close(&state), "and its size is worth keeping");
    }

    /// The case the two bits exist to tell apart. A window opened at a
    /// remembered size is latched before anybody has touched it, so a close
    /// that asked `latched` would write back whatever `remembered_size` clamped
    /// it to on this monitor — quietly cutting a size dragged out on an
    /// external display down to the laptop's, with no way back.
    #[test]
    fn a_window_opened_at_a_remembered_size_writes_nothing_back_on_close() {
        let mut windows = BTreeMap::new();
        open_entry(&mut windows, "run", true);
        let state = windows.get("run").expect("written");
        assert!(state.latched, "nothing computed may size it");
        assert!(!keeps_its_size_on_close(state), "but no hand has moved it, so its size is not news");
    }

    /// A minimise reports a collapsed size and must leave no trace at all —
    /// not a baseline the next event would be measured against, and not a spent
    /// count, since the events we are owed may still be due.
    #[test]
    fn minimising_leaves_the_record_untouched() {
        let mut state = DialogWindowState {
            baseline: (440, 316),
            expecting: 1,
            ..DialogWindowState::default()
        };
        assert!(!record_resize(&mut state, (0, 0), 1.0));
        assert_eq!(state.baseline, (440, 316));
        assert_eq!(state.expecting, 1, "the event we are owed is still owed");
        assert!(!state.latched);
    }

    /// And once it is spent, the next one is judged on its size again.
    #[test]
    fn the_event_after_that_one_is_judged_on_its_size() {
        assert!(resize_is_the_hand(&settled((880, 632)), (1180, 632)));
        assert!(!resize_is_the_hand(&settled((880, 632)), (881, 631)));
    }

    /// Windows sends `Resized` for `SIZE_MINIMIZED` too, where the client area
    /// is 0×0, and it arrives with no flag standing because nothing of ours
    /// asked for it. Minimising a dialog must not latch it: the hand cannot go
    /// below `min_inner_size`, so a zero side is not a size anybody dragged to.
    #[test]
    fn minimising_a_window_is_not_the_hand() {
        assert!(!resize_is_the_hand(&settled((880, 632)), (0, 0)));
        assert!(!resize_is_the_hand(&settled((880, 632)), (880, 0)));
    }

    /// Before anything of ours has sized it, a window is still being brought
    /// into existence — and it is hidden until the first `dialog_window_size`,
    /// so there is no hand on it either.
    #[test]
    fn a_window_nothing_has_sized_yet_is_not_being_dragged() {
        assert!(!resize_is_the_hand(&settled((0, 0)), (440, 120)));
    }

    /// What a dead window left behind must not reach the next one. The map
    /// outlives its windows, and the case this is about is reachable: drag a
    /// dialog, close it inside the 500 ms debounce, and the latch stands with
    /// nothing written to the file — so the next window of that kind opens with
    /// no size kept, and a latch inherited from a window that no longer exists
    /// would refuse it the one `set_size` that ever fits it to its content.
    #[test]
    fn opening_a_window_clears_what_the_last_one_left() {
        let mut windows = BTreeMap::new();
        windows.insert(
            "run".to_string(),
            DialogWindowState {
                baseline: (880, 632),
                requested: (880, 632),
                expecting: 1,
                latched: true,
                moved_by_hand: true,
            },
        );

        open_entry(&mut windows, "run", false);

        let state = windows.get("run").expect("the entry is written, not removed");
        assert!(!state.latched, "a latch from a window that is gone must not reach this one");
        assert!(!state.moved_by_hand);
        assert_eq!(state.expecting, 0, "no event is owed to a window that has not been sized");
        assert_eq!(state.baseline, (0, 0), "and nothing of this window's size is known yet");
    }

    /// The other half: a window that *is* opening at a remembered size starts
    /// latched, because nothing computed may size it — and still without a
    /// hand against it, which is what keeps its size off the disk on a close
    /// nobody dragged anything during.
    #[test]
    fn opening_at_a_remembered_size_starts_latched_but_untouched() {
        let mut windows = BTreeMap::new();
        open_entry(&mut windows, "review-changes", true);
        let state = windows.get("review-changes").expect("written");
        assert!(state.latched);
        assert!(!state.moved_by_hand, "nobody has touched this window yet");
    }

    /// A viewport larger than the window's inner size is not something a window
    /// should be able to say. If one ever does, the window is left at its
    /// content height rather than pulled below it — a short window is the bug
    /// being fixed, and no arithmetic here may reintroduce it.
    #[test]
    fn an_impossible_viewport_cannot_shorten_the_window() {
        assert_eq!(height_to_set(471.0, 439.0, 471.0, 1276.2), 471.0);
    }

    /* The handshake that keeps a `*:show` from being lost. What is tested is the
       list of what is being held — the whole of the mechanism that does not need
       a webview to exist. The emit itself, the command's one line and the order
       the front end announces in are the parts no test in this repository can
       reach. */

    /// The three names the front end listens for, and the reason the derivation
    /// is written out here rather than assumed: nothing pairs the two sides, so
    /// a rename does not fail — it leaves an open window that is never re-aimed.
    #[test]
    fn each_window_is_told_on_a_channel_named_after_it() {
        assert_eq!(show_event(SETTINGS_LABEL), "settings:show");
        assert_eq!(show_event(COMPARE_LABEL), "compare:show");
        assert_eq!(show_event(IMAGE_LABEL), "image:show");
    }

    /// The bug this exists for: a window built a moment ago has not subscribed,
    /// the event is gone, and what it was told is waiting for it when it says it
    /// is there.
    #[test]
    fn a_window_that_was_not_listening_yet_is_told_when_it_announces_itself() {
        let mut held: Vec<(String, Value)> = Vec::new();
        remember_show(&mut held, IMAGE_LABEL, json!({ "path": "/a.png", "name": "a.png" }));

        assert_eq!(
            take_show(&mut held, IMAGE_LABEL),
            Some(json!({ "path": "/a.png", "name": "a.png" }))
        );
    }

    /// A window re-aimed several times before it finished loading wants the last
    /// picture and none of the ones before it.
    #[test]
    fn only_the_newest_of_several_is_kept() {
        let mut held: Vec<(String, Value)> = Vec::new();
        for name in ["a.png", "b.png", "c.png"] {
            remember_show(&mut held, IMAGE_LABEL, json!({ "path": name, "name": name }));
        }

        assert_eq!(held.len(), 1, "one window is owed one picture, not three");
        assert_eq!(
            take_show(&mut held, IMAGE_LABEL),
            Some(json!({ "path": "c.png", "name": "c.png" }))
        );
    }

    /// Nothing is owed twice. A window that has taken its picture is listening
    /// from then on, and a copy held back would re-aim it on its next reload.
    #[test]
    fn a_second_announcement_is_owed_nothing() {
        let mut held: Vec<(String, Value)> = Vec::new();
        remember_show(&mut held, IMAGE_LABEL, json!({ "path": "/a.png", "name": "a.png" }));

        take_show(&mut held, IMAGE_LABEL);

        assert_eq!(take_show(&mut held, IMAGE_LABEL), None);
    }

    /// The ordinary case, and the one that has to stay silent: every window that
    /// was *built* rather than re-aimed announces itself too, having already
    /// read what it is to show off its own URL.
    #[test]
    fn a_window_nobody_re_aimed_is_owed_nothing() {
        let mut held: Vec<(String, Value)> = Vec::new();

        assert_eq!(take_show(&mut held, IMAGE_LABEL), None);
    }

    /// Building forgets. Otherwise a window closed with something still owed to
    /// it would hand that on to the next window under the same label, which for
    /// the image window means opening on one picture and jumping to another.
    #[test]
    fn a_window_that_is_built_collects_nothing_the_last_one_left() {
        let mut held: Vec<(String, Value)> = Vec::new();
        remember_show(&mut held, IMAGE_LABEL, json!({ "path": "/old.png", "name": "old.png" }));

        forget_show(&mut held, IMAGE_LABEL);

        assert_eq!(take_show(&mut held, IMAGE_LABEL), None);
    }

    /// One holder for three windows, so each label answers for itself: a
    /// comparison re-aimed while a picture is waiting must reach the compare
    /// window and leave the picture where it is.
    #[test]
    fn the_three_windows_are_owed_separately() {
        let mut held: Vec<(String, Value)> = Vec::new();
        remember_show(&mut held, IMAGE_LABEL, json!({ "path": "/a.png", "name": "a.png" }));
        remember_show(&mut held, SETTINGS_LABEL, json!({ "tab": "storage" }));
        remember_show(
            &mut held,
            COMPARE_LABEL,
            json!({ "repo": "/tmp/r", "branch": "feature" }),
        );

        forget_show(&mut held, SETTINGS_LABEL);

        assert_eq!(take_show(&mut held, SETTINGS_LABEL), None);
        assert_eq!(
            take_show(&mut held, COMPARE_LABEL),
            Some(json!({ "repo": "/tmp/r", "branch": "feature" }))
        );
        assert_eq!(
            take_show(&mut held, IMAGE_LABEL),
            Some(json!({ "path": "/a.png", "name": "a.png" }))
        );
    }

    /// The payload fields, named here because they are the other half of the
    /// pair `show_event` is one half of: `src/stores/app.js` reads `tab`, `path`
    /// and `name`, and `src/stores/compare.js` reads `repo` and `branch`. A
    /// field renamed on one side costs the feature and fails nothing.
    #[test]
    fn each_window_is_told_in_the_words_its_watcher_reads() {
        assert_eq!(settings_show("storage"), json!({ "tab": "storage" }));
        assert_eq!(
            compare_show("/tmp/r", "feature"),
            json!({ "repo": "/tmp/r", "branch": "feature" })
        );
        assert_eq!(
            image_show("/a.png", "a.png"),
            json!({ "path": "/a.png", "name": "a.png" })
        );
    }
}
