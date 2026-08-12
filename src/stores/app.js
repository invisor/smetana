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
import { openUrl } from '@tauri-apps/plugin-opener'
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
