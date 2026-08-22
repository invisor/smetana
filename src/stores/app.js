/* What the app knows about itself, and the two things it asks the desktop to
   do for it: open its own settings window, and open a link somewhere that is
   not this webview.

   A store rather than three lines in a component, for the reason the rest of
   `stores/` exists: these are the only files in `src/` that know Tauri is there,
   and a component that imported `@tauri-apps/api` would be the first crack in
   that. It is not `settings.js` because none of it is a setting — the settings
   window is a window, and the version is a fact about the build. */
import { invoke } from '@tauri-apps/api/core'
import { emit, listen } from '@tauri-apps/api/event'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl, revealItemInDir } from '@tauri-apps/plugin-opener'
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { usingMockBackend } from './mockBackend.js'

/* Which section the settings window should be showing. Not a setting and not
   part of the three-event contract in `settings.js` — nothing about it reaches
   `settings.json`, and the main window is still the only writer of that file.
   It is a message about a window, which is what this store is for. */
export const SETTINGS_SHOW = 'settings:show'

/* Opens the settings window on a section, or brings the open one forward — the
   decision is Rust's (`window::settings_window_open`), because the window either
   exists or it does not and only that side can tell.

   The section therefore travels twice, and both halves are needed for one press
   to work in both states. A window being built gets it as a query parameter on
   the URL it already loads (`?view=settings&tab=storage`), the mechanism
   `?view=` and `?theme=` are built on. A window already open is focused rather
   than rebuilt — that is the whole point of the label — so it never sees a new
   URL, and the event is the only way to reach it. A fresh window is not
   listening yet and simply misses the event, having already read the parameter.

   In a browser there is no window to make: the mock answers, nothing happens,
   and the gear is a no-op. The settings UI is still reachable there, through
   `?view=settings`, which is what the dev server checks it with. */
export async function openSettingsWindow(tab = null) {
  try {
    await invoke('settings_window_open', { tab })
  } catch (err) {
    console.error('[app] the settings window did not open:', err)
    return
  }
  if (!tab) return
  try {
    await emit(SETTINGS_SHOW, { tab })
  } catch (err) {
    /* The window is open on whatever it was showing, which is a smaller failure
       than not opening at all — hence a warning and no further attempt. */
    console.warn('[app] the settings window was not told which section to show:', err)
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
