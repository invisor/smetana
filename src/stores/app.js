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

/* A link goes to the person's own browser. Inside the webview it would replace
   the app with a web page and leave no way back — there is no address bar and no
   back button in this window.

   The fall-back is what makes the About tab checkable in `npm run dev`, and it
   is gated on there being no Tauri rather than on the call having failed. Those
   are not the same condition and the difference matters: inside the app, a
   failure means the opener ACL refused this URL, which is the one thing the
   plugin exists to do — falling back there would navigate the webview to
   whatever the scope had just declined. So in the app a refusal is reported and
   nothing opens; in a browser, where there was never anything to refuse it, an
   ordinary new tab is the right answer. */
const inTauri = () => Boolean(window.__TAURI_INTERNALS__)

export async function openExternal(url) {
  if (!inTauri()) {
    window.open(url, '_blank', 'noopener')
    return
  }
  try {
    await openUrl(url)
  } catch (err) {
    console.error('[app] the system refused to open the link:', err)
  }
}
