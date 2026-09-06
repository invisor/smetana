import { createApp } from 'vue'
import App from './App.vue'
import { installMockBackend } from './stores/mockBackend.js'
import { initAgents } from './stores/agents.js'
import { suppressNativeMenus } from './nativeMenu.js'
import './styles/styles.css'

// In a browser we swap the IPC for fixtures; under Tauri it does nothing.
installMockBackend()

/* No native right-click menu anywhere, in any of the five views — see
   `nativeMenu.js` for what the platform offers and why none of it belongs here.
   This is the one place all five pass through, and all four OS windows with
   them.
 *
 * Unconditional, and not "only under Tauri": `npm run dev` and `?view=gallery`
 * are how a component is checked, and a check is worth having only if what it
 * shows is what the app does. The browser's own inspector is still a keystroke
 * away. */
suppressNativeMenus(document)

/* The harness catalogue, read before anything is drawn.
 *
 * Here rather than in a view, and awaited rather than fired off, for the reason
 * `stores/agents.js` records: what a harness can do decides whether a menu row
 * is greyed, and a row greyed a round trip later is a row somebody has already
 * pressed. This is the one place all five views pass through, and the settings
 * window needs it as much as the app does — its agent picker is built from the
 * same rows.
 *
 * The wait costs nothing a person can see: every OS window in this app stays
 * hidden until its webview announces itself (`window_show_ready`), so this
 * round trip happens behind a window nobody is looking at yet.
 *
 * `.finally` and not `.then`, so that "there is no branch in which the app does
 * not mount" is a property of this line rather than of `initAgents`'s current
 * body. That function swallows its own failures today — a read that fails leaves
 * the list empty and greys the rows that depend on it — but a window that never
 * mounts is a blank rectangle with nothing anywhere to say why, and that outcome
 * must not be one edit away. */
initAgents().finally(() => {
  createApp(App).mount('#app')
})
