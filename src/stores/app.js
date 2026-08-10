/* What the app knows about itself, and the two things it asks the desktop to
   do for it: open its own settings window, and open a link somewhere that is
   not this webview.

   A store rather than three lines in a component, for the reason the rest of
   `stores/` exists: these are the only files in `src/` that know Tauri is there,
   and a component that imported `@tauri-apps/api` would be the first crack in
   that. It is not `settings.js` because none of it is a setting — the settings
   window is a window, and the version is a fact about the build. */
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import { usingMockBackend } from './mockBackend.js'

/* Opens the settings window, or brings the open one forward — the decision is
   Rust's (`window::settings_window_open`), because the window either exists or
   it does not and only that side can tell.

   In a browser there is no window to make: the mock answers, nothing happens,
   and the gear is a no-op. The settings UI is still reachable there, through
   `?view=settings`, which is what the dev server checks it with. */
export async function openSettingsWindow() {
  try {
    await invoke('settings_window_open')
  } catch (err) {
    console.error('[app] the settings window did not open:', err)
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
