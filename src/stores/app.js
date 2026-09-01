/* What the app knows about itself, and what it asks the desktop to do for it:
   open its own windows — the settings window, a dialog in a window of its own,
   and one attached picture in a window of its own — and open a link somewhere
   that is not this webview.

   A store rather than three lines in a component, for the reason the rest of
   `stores/` exists: these are the only files in `src/` that know Tauri is there,
   and a component that imported `@tauri-apps/api` would be the first crack in
   that. It is not `settings.js` because none of it is a setting — the settings
   window is a window, and the version is a fact about the build. */
import { invoke } from '@tauri-apps/api/core'
import { emit, listen } from '@tauri-apps/api/event'
import { getVersion } from '@tauri-apps/api/app'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { usingMockBackend } from './mockBackend.js'
import { CHROME_NONE, chromeFromPlatform } from '../components/shell/windowChrome.js'

/* Which section the settings window should be showing. Not a setting and not
   part of the three-event contract in `settings.js` — nothing about it reaches
   `settings.json`, and the main window is still the only writer of that file.
   It is a message about a window, which is what this store is for.

   The name is spelled here and once more on the far side, as `show_event` in
   `src-tauri/src/window.rs`, which is the side that sends it. */
export const SETTINGS_SHOW = 'settings:show'

/* Opens the settings window on a section, or brings the open one forward — the
   decision is Rust's (`window::settings_window_open`), because the window either
   exists or it does not and only that side can tell.

   The section therefore travels twice, and both halves are needed for one press
   to work in both states. A window being built gets it as a query parameter on
   the URL it already loads (`?view=settings&tab=storage`), the mechanism
   `?view=` and `?theme=` are built on. A window already open is focused rather
   than rebuilt — that is the whole point of the label — so it never sees a new
   URL, and `settings:show` is the only way to reach it.

   **That event is sent by Rust and not from here**, which is the one thing about
   this function worth reading twice. It used to be emitted on the line after
   the `invoke`, and a window built by this very press had not subscribed yet, so
   the event went nowhere and the window stayed on the section its URL named.
   Only Rust knows which of the two branches the press took, so only Rust can
   hold the message back for a window that is still loading — see
   `announceWindowReady` below, and the header of `window.rs` for the whole of
   it.

   In a browser there is no window to make: the mock answers, nothing happens,
   and the gear is a no-op. The settings UI is still reachable there, through
   `?view=settings`, which is what the dev server checks it with. */
export async function openSettingsWindow(tab = null) {
  try {
    await invoke('settings_window_open', { tab })
  } catch (err) {
    console.error('[app] the settings window did not open:', err)
  }
}

/* The settings window's half: which section it has just been asked for. Its own
   `TABS` list decides whether the name means anything — this store carries the
   message and never the vocabulary. */
export async function watchSettingsSection(onShow) {
  return listen(SETTINGS_SHOW, (event) => onShow(event.payload?.tab ?? null))
}

/* Which columns the active project's board has, so the Kanban tab can offer
   them as checkboxes. Two events, the same hello-and-announcement shape
   `settings.js` uses, because a settings window opened at any moment has to
   learn the set it missed.

   Here rather than as a fourth message in the settings contract, and for the
   reason `settings:show` above is here: nothing about it reaches
   `settings.json`, and those three events are about the one file the main
   window is the only writer of.

   Asking Rust — the way the Storage tab asks about the attachment folder —
   would not work: `blocked` is a *computed* column, no such status exists in
   bd (a task in it is `open` with an unclosed blocker), so the real set of
   columns exists only in `boardColumns` on the front end. A list out of Rust
   would be missing exactly the column somebody is most likely to want pinned. */
export const BOARD_COLUMNS = 'board:columns'
export const BOARD_HELLO = 'board:hello'

/* The app window's half: this is the set now. Called on every change to it and
   again whenever a settings window says hello — an announcement nobody is
   listening to costs an event and nothing else. */
export async function announceBoardColumns(columns) {
  try {
    await emit(BOARD_COLUMNS, { columns })
  } catch (err) {
    console.warn('[app] the settings window was not told which columns the board has:', err)
  }
}

export async function watchBoardHello(onHello) {
  return listen(BOARD_HELLO, () => onHello())
}

/* The settings window's half. The hello is sent after the subscription, never
   before — the answer is an event too, and one sent first would be answered
   into a window that is not listening yet. It is not awaited, for the reason
   `watchSharedSettings` records: the subscription already exists and has to
   reach the caller whatever the hello does. A hello that never went costs an
   empty checkbox list until the board next changes, which is the fall-back this
   window already has. */
export async function watchBoardColumns(onColumns) {
  const stop = await listen(BOARD_COLUMNS, (event) => onColumns(event.payload?.columns ?? []))
  emit(BOARD_HELLO, null).catch((err) => {
    console.warn('[app] the app window was not asked which columns the board has:', err)
  })
  return stop
}

/* A dialog in a window of its own: opening one, closing one, sizing one, and
   the two-way traffic that fills it.

   Here for the reason the settings window's message is here: a window is a
   window, not a setting, and this is the store that knows the desktop exists.
   Which dialogs there are, how wide each one is and what each stands on is the
   front end's own list (`views/dialogRegistry.js`) — this file carries the
   messages and never the vocabulary, exactly as `settings:show` does.

   The channels are per kind rather than one channel carrying a name, because
   two dialog windows can be open at once — nothing stops somebody filing a task
   while a run dialog stands open — and one channel would deliver each window
   the other's props. */
const propsChannel = (kind) => `dialog:props:${kind}`
const helloChannel = (kind) => `dialog:hello:${kind}`
const resultChannel = (kind) => `dialog:result:${kind}`

/* Opens a dialog window, or brings the open one forward — the decision is
   Rust's (`window::dialog_window_open`), because the window either exists or it
   does not and only that side can tell. The width comes from the registry; the
   height does not exist yet, and the window stays hidden until the page has
   measured itself and called `sizeDialogWindow` below.

   In a browser there is no window to make: the mock refuses, this says so once
   in the console and the app window carries on. The dialog itself is still
   reachable there, through `?view=dialog&kind=<name>`, which is how it is
   checked by eye. */
export async function openDialogWindow(kind, width) {
  try {
    await invoke('dialog_window_open', { kind, width })
  } catch (err) {
    console.error('[app] the dialog window did not open:', err)
  }
}

/* Closing is a warning rather than an error, and the difference is what the two
   failures cost. A window that did not open leaves a person pressing a menu
   item with nothing to show for it; a window that did not close is on screen
   and can be closed by hand. */
export async function closeDialogWindow(kind) {
  try {
    await invoke('dialog_window_close', { kind })
  } catch (err) {
    console.warn('[app] the dialog window did not close:', err)
  }
}

/* The measured height, how much of the window the page got, and the title the
   OS frame draws. Rust does the sizing rather than this side calling `setSize`
   itself, and that is not ceremony: `core:default` grants neither `set_size`
   nor `show`, so doing it here would mean publishing both to every window in
   the app for the sake of one call.

   `viewport` travels beside the height because neither side can work out the
   overhead alone — Rust knows what it set the window to, the page knows what
   arrived — and the difference is a title bar, or a title bar with borders, or
   nothing at all, depending on the machine. `window::height_to_set` carries the
   whole argument and the measurements behind it. */
export async function sizeDialogWindow(kind, height, viewport, title) {
  try {
    await invoke('dialog_window_size', { kind, height, viewport, title })
  } catch (err) {
    console.warn('[app] the dialog window kept the size it had:', err)
  }
}

/* The app window's half: these are the props now. Sent on every change and
   again whenever a dialog window says hello — which is what makes a window
   opened at any moment learn the state it missed, and what makes a live `busy`
   or a growing `moved` reach a window that is already up. */
export async function announceDialogProps(kind, props) {
  try {
    await emit(propsChannel(kind), props)
  } catch (err) {
    console.warn('[app] the dialog window was not told what to draw:', err)
  }
}

export async function watchDialogHello(kind, onHello) {
  return listen(helloChannel(kind), () => onHello())
}

/* What the guest emitted, back in the app window where the handlers are. The
   name travels beside the payload because one channel carries every emit a
   dialog has: splitting them per emit would be a channel per button. */
export async function watchDialogResult(kind, onResult) {
  return listen(resultChannel(kind), (event) =>
    onResult(event.payload?.name ?? '', event.payload?.payload)
  )
}

/* The dialog window's half. The hello goes after the subscription, never
   before — the answer is an event too, and one sent first would be answered
   into a window that is not listening yet. Not awaited, for the reason
   `watchBoardColumns` records: the subscription already exists and has to reach
   the caller whatever the hello does. */
export async function watchDialogProps(kind, onProps) {
  const stop = await listen(propsChannel(kind), (event) => onProps(event.payload ?? {}))
  emit(helloChannel(kind), null).catch((err) => {
    console.warn('[app] the app window was not asked what this dialog is about:', err)
  })
  return stop
}

export async function emitDialogResult(kind, name, payload) {
  try {
    await emit(resultChannel(kind), { name, payload })
  } catch (err) {
    console.error('[app] what the dialog answered did not reach the app window:', err)
  }
}

/* One attached picture, shown whole in a window of its own.

   Here beside `openSettingsWindow` and the dialog channels for exactly their
   reason: this is the app asking the desktop to open one of its own windows,
   which is what this store is for, and the alternative is `@tauri-apps/api`
   imported inside `AttachmentStrip.vue` — a component that has to stay
   drawable in `?view=gallery` with nothing behind it.

   The picture travels twice, the way the settings window's section and the
   compare window's pair do, and both halves are needed for one click to work in
   both states. A window being built reads the path and the name off the URL
   Rust wrote (`?view=image&path=…&name=…`); a window already open is focused
   rather than rebuilt, so an event is the only way to re-aim it.

   What travels is the path and never the bytes. The `url` on an attachment
   record is a `data:` URL of up to 8 MiB of base64: it fits in no URL, and
   sending it over the event channel would be eleven megabytes of base64 per
   click. The window reads the file itself, with the command that already exists
   for reading one back out of the store.

   In a browser there is no window to make: the mock refuses, this says so once
   in the console and the dialog carries on. The window itself is still
   reachable there, through `?view=image&path=…&name=…`, which is how it is
   checked by eye. */
export const IMAGE_SHOW = 'image:show'

export async function openImageWindow(path, name) {
  try {
    await invoke('image_window_open', { path, name })
  } catch (err) {
    console.error('[app] the image window did not open:', err)
  }
}

/* The image window's half: which picture it has just been asked for. The names
   of the two fields are the other half of a pair — `image_show` in
   `src-tauri/src/window.rs` writes them, and nothing mechanical holds the two
   sides together. */
export async function watchImageShow(onShow) {
  return listen(IMAGE_SHOW, (event) =>
    onShow(event.payload?.path ?? null, event.payload?.name ?? '')
  )
}

/* ---- a window saying it is ready to be re-aimed ------------------------- */
/* The second half of the three `*:show` families above, and of `compare:show`
   in `stores/compare.js`. It is one function for all three because it is one
   fact — this window has loaded — and Rust reads which window is speaking off
   the webview's own label rather than from an argument.

   **Why it is needed at all.** An open window is focused rather than reloaded,
   so what to show next reaches it as an event; but a window exists from the
   moment it is built, long before its webview has subscribed to anything, and
   Tauri buffers nothing. A second thumbnail clicked before the image window had
   loaded therefore set the frame's title and never reached the picture — a
   window naming one picture and showing another. Rust now holds the last
   "show this" per window and hands it over on this call.

   **The order the caller has to keep**, and the whole of what a window owes
   this: announce *after* subscribing, since an event answered into a window
   that is not listening is the very thing this exists to prevent, and *after*
   drawing what came in on the URL, since the URL is the older of the two and
   would otherwise be painted over the newer picture.

   A window that was built rather than re-aimed announces itself too and is
   answered with silence — Rust holds nothing for it, because what it is showing
   arrived on its URL. Nothing keeps state here: this store has none, and the
   holder is one per app rather than one per webview, which is what stops two
   windows answering the same announcement with different pictures. */
export async function announceWindowReady() {
  try {
    await invoke('window_show_ready')
  } catch (err) {
    /* A window on whatever its URL named, which is a smaller failure than not
       opening at all — hence a warning and no second attempt. */
    console.warn('[app] the desktop was not told this window is ready:', err)
  }
}

/* Whether the app opens itself when the person signs in, and the press that
   changes it.

   Here rather than in `settings.js` because none of it is a setting: nothing
   about the login item reaches `settings.json`, and the machine's own list is
   the whole of the truth — `src-tauri/src/autostart.rs` records why a copy of
   it in a file of ours would be worse than none. Both answer with the state
   Rust read back *after* doing anything, so a registration the system declined
   puts the switch back by itself and there is no error branch here to design.

   Neither ever rejects. Nobody to ask — a browser, an ACL — is answered with
   "this build may not register anything", which is the same shape a development
   build gets from Rust and draws the same disabled row. A thrown error would
   instead leave the tab with no answer at all, on a screen where every other
   row draws something. */
export async function autostartState() {
  try {
    return await invoke('autostart_state')
  } catch (err) {
    console.debug('[app] nobody to ask about the login item (a browser, there is no Tauri):', err)
    return { supported: false, enabled: false }
  }
}

export async function setAutostart(enabled) {
  try {
    return await invoke('autostart_set', { enabled })
  } catch (err) {
    console.error('[app] the login item did not change:', err)
    return { supported: false, enabled: false }
  }
}

/* What is left of the subscription of the agent this machine would actually
   run, for the Agents tab in the settings window.

   This store rather than `runs.js`, and the reason is what `app.js` is for: it
   is a one-off question put to the desktop with no state behind it and nothing
   to keep — the answer is minutes old the moment it is given, it belongs to no
   project and no run, and it does not outlive the window that asked. Keeping it
   in `runs.js` was the rejected alternative: nothing about it is a run, and it
   would have acquired a place in a store that survives the settings window
   closing.

   The answer travels whole, in Rust's own shape, and is handed to the pure
   module that turns it into sentences (`components/settings/usage.js`) — the
   same division `surveyStorage` keeps with `storage.js`.

   It rejects rather than answering with a shape of its own, unlike
   `autostartState` above: the command is infallible in Rust, so a failure here
   is the channel rather than the answer, and there is a line on that tab for
   saying so. An invented "unreadable" would put a sentence about somebody's
   login under a fault that has nothing to do with it.

   The agent is named by the caller rather than left to Rust to read out of
   `settings.json`, and that is not an optimisation. The front end owns that
   field and the file is up to a debounce behind it, so a window that has just
   changed the agent and asks in the same breath would be answered about the one
   it left — for as long as the probe takes, under a heading honest enough about
   who replied to look like an ordinary substitution. `null` is a caller with no
   opinion, which is what the file is still for. */
export async function readAgentUsage(agent = null) {
  try {
    return await invoke('agent_usage', { agent })
  } catch (err) {
    console.error('[app] the subscription allowance could not be read:', err)
    throw new Error(err && typeof err === 'object' && typeof err.message === 'string' ? err.message : String(err))
  }
}

/* ---- the window this app is drawn in ------------------------------------ */
/* The app window has no title bar of its own any more: the scope bar is it.
   What is left over here is the small amount of that arrangement which needs a
   window to ask, and it is here rather than in a component for the reason the
   rest of this file exists — `@tauri-apps/api` is a store's import and nobody
   else's. `components/shell/windowChrome.js` holds the rules; none of them are
   repeated here. */

/* Which chrome the window around us has. `none` on any failure, and the
   commonest failure is the ordinary one: a browser, where the command does not
   exist, there is no window and there is no title bar to move a bar into. The
   name that comes back is one of `components/shell/windowChrome.js`'s three,
   and that module decides what an unrecognised one means. */
export async function readWindowChrome() {
  try {
    return chromeFromPlatform(await invoke('window_chrome'))
  } catch (err) {
    console.debug('[app] no window chrome to ask about (a browser):', err)
    return CHROME_NONE
  }
}

/* The person's home folder, or null where there is nobody to ask.
   `window::home_dir` in Rust; `null` in a browser, and `null` too on a machine
   with no `HOME` at all.

   Here for the reason the chrome above is here: it is a fact about the desktop
   rather than a setting, and the alternative is `@tauri-apps/api` imported
   inside a component. Nothing keeps it — a home folder does not change while
   the app runs, and the one window that wants it asks when it opens.

   Silent on failure and answering null, like `isWindowMaximized` and unlike the
   three writes above it: nobody asked for this, and the one reader
   (`components/git/repoLabel.js`) treats an absent home as an ordinary state
   and draws an absolute path. */
export async function homeDir() {
  try {
    return (await invoke('home_dir')) ?? null
  } catch (err) {
    console.debug('[app] no home folder to read (a browser, there is no Tauri):', err)
    return null
  }
}

/* Whether the window is fullscreen, now and whenever it changes. There is no
   fullscreen event of its own, so the resize is the signal and the window is
   asked outright — the answer is a boolean and the question is cheap.

   Returns the unsubscribe, or a function that does nothing when there is no
   window at all: a caller must be able to clean up without knowing which. */
export async function watchFullscreen(onChange) {
  try {
    const appWindow = getCurrentWindow()
    onChange(await appWindow.isFullscreen())
    return await appWindow.onResized(async () => {
      try {
        onChange(await appWindow.isFullscreen())
      } catch (err) {
        console.warn('[app] could not read the fullscreen state:', err)
      }
    })
  } catch (err) {
    console.debug('[app] no window to watch for fullscreen (a browser):', err)
    return () => {}
  }
}

export async function minimizeWindow() {
  try {
    await getCurrentWindow().minimize()
  } catch (err) {
    console.error('[app] minimizing the window failed:', err)
  }
}

export async function toggleMaximizeWindow() {
  try {
    await getCurrentWindow().toggleMaximize()
  } catch (err) {
    console.error('[app] maximizing the window failed:', err)
  }
}

/* `close()`, never `destroy()`. `stores/settings.js` intercepts
   `onCloseRequested` to flush the pending write of `settings.json`, and a
   button that destroyed the window instead would drop somebody's last change
   with nothing on screen to say so.

   It needs `core:window:allow-close` granted explicitly in
   `capabilities/default.json`, beside the `allow-destroy` that same flush needs
   on the far side of the interception. `core:default` does **not** carry it:
   that is the nine plugin defaults, of which `core:window:default` is 28
   entries holding neither. Nothing in either suite can catch the omission — the
   front end's tests cannot read a capability file, Rust's cannot reach this
   call, and the only two platforms that draw this button are the two CI never
   builds — so the button would simply log the line below and leave a window
   with no system close button of its own uncloseable. */
export async function closeWindow() {
  try {
    await getCurrentWindow().close()
  } catch (err) {
    console.error('[app] closing the window failed:', err)
  }
}

/* Which of the two the middle button is. Silent on failure and answering
   `false`, unlike the three above: there is nothing a person asked for here to
   report the failure of, and a browser reaches it on every check. */
export async function isWindowMaximized() {
  try {
    return await getCurrentWindow().isMaximized()
  } catch {
    return false
  }
}

/* The version this build carries — `tauri.conf.json`'s `version`, which is the
   one a person would quote in a bug report. `null` when there is nobody to ask
   (a browser): the About tab draws a dash rather than inventing a number, since
   a wrong version in a report is worse than none. */
export async function appVersion() {
  try {
    return await getVersion()
  } catch (err) {
    console.debug('[app] no version to read (a browser, there is no Tauri):', err)
    return null
  }
}

/* Whether there is a real back end behind `invoke`, which is **not** the same
   question as whether `window.__TAURI_INTERNALS__ ` is defined: `mockIPC` sets
   that property itself, so it is true in the dev server as well. The store that
   installs the fixtures publishes what it decided, and that is the only honest
   answer here. */
const hasBackEnd = () => Boolean(window.__TAURI_INTERNALS__) && !usingMockBackend()

/* A link goes to the person's own browser. Inside the webview it would replace
   the app with a web page and leave no way back — there is no address bar and no
   back button in this window.

   Which branch is taken is decided *before* the call rather than by catching its
   failure, because the two failures mean opposite things. In the app, `openUrl`
   failing means the opener ACL refused this URL — the one thing the plugin
   exists to do — and falling back to `window.open` would navigate the webview to
   exactly what the scope had just declined. In a browser there is no ACL and no
   system to ask: an ordinary new tab is the whole of what "open this link" can
   mean, and it is what makes the About tab checkable in `npm run dev`. */
export async function openExternal(url) {
  if (!hasBackEnd()) {
    window.open(url, '_blank', 'noopener')
    return
  }
  try {
    await openUrl(url)
  } catch (err) {
    console.error('[app] the system refused to open the link:', err)
  }
}

/* A path onto the system clipboard, for the file tree's two copy verbs.

   The branch is chosen the way `openExternal` above chooses its own — before
   the call, not by catching its failure — and here the reason is sharper than a
   scope refusal. `mockBackend.js` rejects loudly on every command it has never
   heard of, by design, so a plugin call in the dev server does not fail in a way
   worth falling back from: it fails always. Without the second half these two
   items could not be checked in `npm run dev` at all.

   `navigator.clipboard` is the browser half rather than the whole of it because
   it wants a secure context and a gesture the webview does not always agree it
   had, which is a failure with no visible cause; the plugin has neither
   condition.

   Answered with whether it worked, and not left to a thrown error: both callers
   put a toast on the screen either way, and a copy is the one action whose
   success has nothing on screen to show for it — an empty clipboard and a
   clipboard holding the path look exactly alike until somebody pastes. */
export async function copyText(text) {
  try {
    if (hasBackEnd()) await writeText(text)
    else await navigator.clipboard.writeText(text)
    return true
  } catch (err) {
    console.error('[app] the text did not reach the clipboard:', err)
    return false
  }
}

/* Show a file or a folder in the platform's own file manager — Finder, Explorer
   or whatever the desktop runs. The path is absolute: `revealItemInDir` is given
   to the operating system as it stands, and a relative one would name whatever
   happens to sit under the process's working directory.

   There is no browser half, and inventing one would be a lie: a web page cannot
   open a file manager, and `window.open` on a `file:` URL is refused by every
   engine this could run in. It answers `false` there, which the caller turns
   into the same toast a refusal in the app produces — a person in the dev server
   learns that this verb is the app's, which is true. */
export async function revealInFileManager(path) {
  if (!hasBackEnd()) {
    console.debug('[app] nothing to reveal in (a browser, there is no file manager to ask)')
    return false
  }
  try {
    await revealItemInDir(path)
    return true
  } catch (err) {
    console.error('[app] the file manager did not open:', err)
    return false
  }
}
