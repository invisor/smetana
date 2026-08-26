import { createApp } from 'vue'
import App from './App.vue'
import { installMockBackend } from './stores/mockBackend.js'
import { suppressNativeMenus } from './nativeMenu.js'
import './styles/styles.css'

// In a browser we swap the IPC for fixtures; under Tauri it does nothing.
installMockBackend()

/* No native right-click menu anywhere, in any of the four views — see
   `nativeMenu.js` for what the platform offers and why none of it belongs here.
   This is the one place all four pass through, and all three OS windows with
   them.
 *
 * Unconditional, and not "only under Tauri": `npm run dev` and `?view=gallery`
 * are how a component is checked, and a check is worth having only if what it
 * shows is what the app does. The browser's own inspector is still a keystroke
 * away. */
suppressNativeMenus(document)

createApp(App).mount('#app')
