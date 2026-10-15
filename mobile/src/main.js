import { createApp } from 'vue'
import App from './App.vue'
import { suppressNativeMenus } from '@src/nativeMenu.js'
import '@src/styles/styles.css'

async function start() {
  /* A page with no Tauri under it is a browser: show the screens over the desktop's mock. The
     condition is the bundler's to remove, so the mock never reaches a build. */
  if (import.meta.env.DEV && !window.__TAURI_INTERNALS__) {
    await (await import('./devBackend.js')).installDevBackend()
  }
  suppressNativeMenus(document)
  createApp(App).mount('#app')
}

start()
