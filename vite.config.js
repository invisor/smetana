import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [vue()],
  // Tauri handles terminal output itself
  clearScreen: false,
  server: {
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] }
  },
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) }
  },
  // Tauri runs in the system webview: WebKitGTK / WKWebView / WebView2.
  //
  // The 500 kB chunk warning is advice about a network, and this bundle never crosses one:
  // it is read off local disk from inside smetana.app, so the cost of its size is parsing,
  // not download. xterm.js and CodeMirror's core are most of the main chunk and both are
  // wanted at start; what is worth splitting already is — a language per chunk
  // (`files/editor/languages.js`) and the gallery, which never reaches the app bundle at all.
  build: { target: ['es2021', 'chrome100', 'safari15'], chunkSizeWarningLimit: 1200 }
})
